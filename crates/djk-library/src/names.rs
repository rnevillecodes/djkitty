//! Turning tags, file names and set names into names Windows accepts, and picking a
//! free name when one is already taken.

/// Longest name (in characters) given to a folder or file inside a vault.
pub const MAX_NAME_CHARS: usize = 100;

/// Characters Windows does not allow in a file or folder name.
const FORBIDDEN: &str = "<>:\"/\\|?*";

/// Names Windows reserves for devices, with or without an extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Make `s` safe to use as one file or folder name on Windows (and so everywhere):
/// forbidden and control characters become `_`, leading spaces and trailing dots and
/// spaces are trimmed, reserved device names get a `_` after their stem, and the result
/// is capped at [`MAX_NAME_CHARS`] characters, keeping a short extension. May return an
/// empty string; callers pick their own fallback.
pub fn sanitize_component(s: &str) -> String {
    let replaced: String = s
        .chars()
        .map(|c| {
            if c.is_control() || FORBIDDEN.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name = trim(&replaced).to_owned();

    // Reserved device names: "CON", "con.mp3", "Nul .txt" are all refused by Windows.
    let (stem, ext) = split_ext(&name);
    let stem_trimmed = stem.trim_end_matches([' ', '.']);
    if RESERVED
        .iter()
        .any(|r| r.eq_ignore_ascii_case(stem_trimmed))
    {
        name = format!("{stem_trimmed}_{ext}");
    }

    if name.chars().count() > MAX_NAME_CHARS {
        let (stem, ext) = split_ext(&name);
        let keep = MAX_NAME_CHARS.saturating_sub(ext.chars().count());
        let stem: String = stem.chars().take(keep).collect();
        name = format!("{}{ext}", trim(&stem));
    }
    trim(&name).to_owned()
}

fn trim(s: &str) -> &str {
    s.trim_start_matches(' ').trim_end_matches(['.', ' '])
}

/// Split into stem and extension (with its dot). Only a short, dot-free-stem-preserving
/// extension counts, so "Mr. Oizo" keeps its whole name as the stem.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 6 && !name[i + 1..].contains(' ') => {
            (&name[..i], &name[i..])
        }
        _ => (name, ""),
    }
}

/// Folder name for an artist tag, `Unknown Artist` when it has none (or nothing usable).
pub fn artist_folder(artist: &str) -> String {
    let name = sanitize_component(artist.trim());
    if name.is_empty() {
        "Unknown Artist".to_owned()
    } else {
        name
    }
}

/// A file name safe for the vault, falling back to `Untitled<ext>`.
pub fn file_name(name: &str) -> String {
    let clean = sanitize_component(name);
    // Prefix a letter so a bare extension (".mp3") still splits as one.
    let probe = format!("x{clean}");
    let (stem, ext) = split_ext(&probe);
    if stem[1..].trim().is_empty() {
        format!("Untitled{ext}")
    } else {
        clean
    }
}

/// `name` if `taken` says it is free, otherwise `stem (2).ext`, `stem (3).ext`...
/// The suffix goes before the extension and the result stays within [`MAX_NAME_CHARS`].
pub fn unique_name(name: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    let (stem, ext) = split_ext(name);
    (2u32..)
        .map(|n| {
            let suffix = format!(" ({n}){ext}");
            let keep = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
            let stem: String = stem.chars().take(keep).collect();
            format!("{}{suffix}", stem.trim_end())
        })
        .find(|candidate| !taken(candidate))
        .expect("some suffix is free")
}

/// `name` with any ` (n)` suffix added by [`unique_name`] removed, so a name can be
/// recognised as a numbered variant of another.
pub fn base_name(name: &str) -> String {
    let (stem, ext) = split_ext(name);
    if let Some(open) = stem.rfind(" (")
        && stem.ends_with(')')
        && stem[open + 2..stem.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit())
        && stem.len() > open + 3
    {
        return format!("{}{ext}", &stem[..open]);
    }
    name.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_and_control_characters_are_replaced() {
        assert_eq!(sanitize_component("AC/DC: Live"), "AC_DC_ Live");
        assert_eq!(sanitize_component(r#"a<b>c"d\e|f?g*h"#), "a_b_c_d_e_f_g_h");
        assert_eq!(sanitize_component("tab\there"), "tab_here");
    }

    #[test]
    fn trailing_dots_and_spaces_are_trimmed() {
        assert_eq!(sanitize_component("Mr. Oizo. . "), "Mr. Oizo");
        assert_eq!(sanitize_component("  lead"), "lead");
        assert_eq!(sanitize_component("..."), "");
        assert_eq!(sanitize_component("   "), "");
    }

    #[test]
    fn reserved_names_get_an_underscore() {
        assert_eq!(sanitize_component("CON"), "CON_");
        assert_eq!(sanitize_component("nul.mp3"), "nul_.mp3");
        assert_eq!(sanitize_component("COM1"), "COM1_");
        assert_eq!(sanitize_component("lpt9.flac"), "lpt9_.flac");
        // Only the exact device names.
        assert_eq!(sanitize_component("Console"), "Console");
        assert_eq!(sanitize_component("COM10"), "COM10");
    }

    #[test]
    fn long_names_are_capped_keeping_the_extension() {
        let long = format!("{}.flac", "x".repeat(300));
        let s = sanitize_component(&long);
        assert_eq!(s.chars().count(), MAX_NAME_CHARS);
        assert!(s.ends_with("x.flac"));
        let folder = sanitize_component(&"é".repeat(150));
        assert_eq!(folder.chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn fallbacks() {
        assert_eq!(artist_folder(""), "Unknown Artist");
        assert_eq!(artist_folder("  ?? "), "__");
        assert_eq!(artist_folder("..."), "Unknown Artist");
        assert_eq!(artist_folder("Bicep"), "Bicep");
        assert_eq!(file_name(".mp3"), "Untitled.mp3");
        assert_eq!(file_name("02 Glue.mp3"), "02 Glue.mp3");
        assert_eq!(file_name("a:b.mp3"), "a_b.mp3");
    }

    #[test]
    fn unique_names_add_a_numbered_suffix_before_the_extension() {
        let taken = ["01 Intro.mp3", "01 intro (2).MP3"];
        let is_taken = |n: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(n));
        assert_eq!(unique_name("01 Intro.mp3", is_taken), "01 Intro (3).mp3");
        assert_eq!(unique_name("Other.mp3", is_taken), "Other.mp3");
        // Case-insensitive: Windows treats these as the same name.
        assert_eq!(unique_name("01 INTRO.mp3", is_taken), "01 INTRO (3).mp3");
        assert_eq!(unique_name("Sets", |n| n == "Sets"), "Sets (2)");
        let long = format!("{}.mp3", "y".repeat(MAX_NAME_CHARS - 4));
        let u = unique_name(&long, |n| n == long);
        assert!(u.ends_with(" (2).mp3") && u.chars().count() <= MAX_NAME_CHARS);
    }

    #[test]
    fn base_names_strip_the_suffix() {
        assert_eq!(base_name("Friday (2).m3u8"), "Friday.m3u8");
        assert_eq!(base_name("Friday.m3u8"), "Friday.m3u8");
        assert_eq!(base_name("Mix (live).m3u8"), "Mix (live).m3u8");
        assert_eq!(base_name("Mix ().m3u8"), "Mix ().m3u8");
    }
}
