//! Vault screens: the welcome screen shown while no vault is open, the "Create vault"
//! dialog, the cloud-sync warning and the vault switcher in the library toolbar.

use std::path::{Path, PathBuf};

use egui::{Align, Layout, RichText, Ui, Vec2};

use crate::app::DjApp;
use crate::{theme, widgets};

/// State of the vault screens and dialogs.
#[derive(Default)]
pub struct VaultUi {
    /// Why no vault is open, or why the last attempt failed (shown on the welcome screen
    /// and in the switcher).
    pub problem: Option<String>,
    /// The "Create vault" dialog, while open.
    pub create: Option<CreateForm>,
    /// A vault waiting for the user to accept the cloud-sync warning, and the service.
    pub confirm_cloud: Option<(PendingVault, &'static str)>,
}

#[derive(Default)]
pub struct CreateForm {
    pub name: String,
    pub parent: Option<PathBuf>,
    pub error: Option<String>,
}

/// A vault to create or open once the user has seen the cloud-sync warning.
#[derive(Clone, Debug, PartialEq)]
pub enum PendingVault {
    Create { parent: PathBuf, name: String },
    Open(PathBuf),
}

impl PendingVault {
    fn target(&self) -> PathBuf {
        match self {
            PendingVault::Create { parent, name } => parent.join(name.trim()),
            PendingVault::Open(root) => root.clone(),
        }
    }
}

// ---------------------------------------------------------------- cloud folders

/// The cloud service syncing `path`, if it is inside a OneDrive, Dropbox or iCloud Drive
/// folder. A heuristic, only used to warn.
pub fn synced_by(path: &Path) -> Option<&'static str> {
    let env = |k: &str| std::env::var_os(k).map(|v| v.to_string_lossy().into_owned());
    let dropbox_info = ["LOCALAPPDATA", "APPDATA"].iter().find_map(|k| {
        let dir = std::env::var_os(k)?;
        std::fs::read_to_string(Path::new(&dir).join("Dropbox").join("info.json")).ok()
    });
    synced_by_with(path, &env, dropbox_info.as_deref())
}

fn synced_by_with(
    path: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    dropbox_info: Option<&str>,
) -> Option<&'static str> {
    for key in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(base) = env(key).filter(|b| !b.trim().is_empty())
            && is_under(path, Path::new(&base))
        {
            return Some("OneDrive");
        }
    }
    if let Some(info) = dropbox_info
        && dropbox_paths(info)
            .iter()
            .any(|base| is_under(path, Path::new(base)))
    {
        return Some("Dropbox");
    }
    for part in path.components() {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        match name.as_str() {
            "dropbox" => return Some("Dropbox"),
            "iclouddrive" | "icloud drive" => return Some("iCloud Drive"),
            _ => {}
        }
    }
    None
}

/// The folder paths in Dropbox's `info.json` (`"path": "C:\\Users\\me\\Dropbox"`).
fn dropbox_paths(info: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = info;
    while let Some(i) = rest.find("\"path\"") {
        rest = &rest[i + 6..];
        let Some(open) = rest.find('"') else { break };
        let after = &rest[open + 1..];
        let mut value = String::new();
        let mut chars = after.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        value.push(n);
                    }
                }
                '"' => break,
                c => value.push(c),
            }
        }
        if !value.is_empty() {
            out.push(value);
        }
        rest = after;
    }
    out
}

/// Whether `path` is `base` or inside it, compared the way Windows compares paths.
fn is_under(path: &Path, base: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    let (p, b) = (norm(path), norm(base));
    !b.is_empty() && (p == b || p.starts_with(&format!("{b}\\")))
}

// ---------------------------------------------------------------- actions

/// Create or open a vault, after the cloud-sync warning if it applies.
pub fn request(app: &mut DjApp, pending: PendingVault) {
    if let Some(service) = synced_by(&pending.target()) {
        app.vault_ui.confirm_cloud = Some((pending, service));
        return;
    }
    carry_out(app, pending);
}

fn carry_out(app: &mut DjApp, pending: PendingVault) {
    match pending {
        PendingVault::Create { parent, name } => match app.create_vault(&parent, &name) {
            Ok(()) => app.vault_ui.create = None,
            Err(e) => {
                let form = app.vault_ui.create.get_or_insert_with(Default::default);
                form.error = Some(e);
            }
        },
        PendingVault::Open(root) => {
            if let Err(e) = app.open_vault(&root) {
                app.vault_ui.problem = Some(e.clone());
                if app.vault.is_some() {
                    app.notify(e);
                }
            }
        }
    }
}

/// Pick a folder and open it as a vault.
fn pick_and_open(app: &mut DjApp) {
    if let Some(root) = rfd::FileDialog::new()
        .set_title("Open a djkitty vault")
        .pick_folder()
    {
        if !djk_library::Vault::is_vault(&root) {
            let msg = format!("{} is not a djkitty vault", root.display());
            app.vault_ui.problem = Some(msg.clone());
            if app.vault.is_some() {
                app.notify(msg);
            }
            return;
        }
        request(app, PendingVault::Open(root));
    }
}

fn start_create(app: &mut DjApp) {
    let parent = app
        .vault
        .as_ref()
        .and_then(|v| v.root().parent().map(Path::to_path_buf))
        .or_else(|| {
            directories::UserDirs::new()
                .map(|u| u.audio_dir().unwrap_or_else(|| u.home_dir()).to_path_buf())
        });
    app.vault_ui.create = Some(CreateForm {
        name: "My DJ Vault".to_owned(),
        parent,
        error: None,
    });
}

// ---------------------------------------------------------------- welcome screen

/// Shown in place of the library while no vault is open.
pub fn welcome(app: &mut DjApp, ui: &mut Ui) {
    ui.add_space((ui.available_height() * 0.12).min(60.0));
    ui.vertical_centered(|ui| {
        ui.set_max_width(460.0);
        ui.label(RichText::new("Welcome to djkitty").size(24.0).strong());
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Your library lives in a vault: a folder holding a copy of every track you \
                 import, with its hot cues, beatgrids and sets. Put it on any drive, or a USB \
                 stick to take to a gig.",
            )
            .color(theme::TEXT_DIM),
        );
        if let Some(problem) = &app.vault_ui.problem {
            ui.add_space(10.0);
            ui.label(RichText::new(problem).color(theme::WARN));
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let w = 2.0 * 180.0 + ui.spacing().item_spacing.x;
            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
            let create = egui::Button::new(
                RichText::new("Create new vault…")
                    .strong()
                    .color(theme::LIT_INK),
            )
            .min_size(Vec2::new(180.0, 34.0));
            if widgets::lit_button(ui, true, Some(theme::ACCENT), create).clicked() {
                start_create(app);
            }
            let open = egui::Button::new("Open existing vault…").min_size(Vec2::new(180.0, 34.0));
            if widgets::lit_button(ui, true, None, open).clicked() {
                pick_and_open(app);
            }
        });
        let recent = app.settings.recent_vaults.clone();
        if !recent.is_empty() {
            ui.add_space(20.0);
            ui.label(RichText::new("RECENT VAULTS").small().color(theme::HEADING));
            ui.add_space(4.0);
            for root in recent {
                recent_vault_button(app, ui, &root);
            }
        }
    });
}

fn recent_vault_button(app: &mut DjApp, ui: &mut Ui, root: &Path) {
    let available = djk_library::Vault::is_vault(root);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let label = if available {
        RichText::new(format!("{name}   ·   {}", root.display()))
    } else {
        RichText::new(format!("{name}   ·   not available")).color(theme::TEXT_DIM)
    };
    let resp = ui
        .add_enabled(available, egui::Button::new(label).frame(false))
        .on_hover_text(root.display().to_string())
        .on_disabled_hover_text(format!("{} is not available", root.display()));
    if resp.clicked() {
        request(app, PendingVault::Open(root.to_path_buf()));
    }
}

// ---------------------------------------------------------------- dialogs

/// The "Create vault" dialog: a name, a location and a preview of the folder.
pub fn create_dialog(app: &mut DjApp, ctx: &egui::Context) {
    let Some(form) = app.vault_ui.create.as_mut() else {
        return;
    };
    let mut submit = false;
    let mut close = false;
    let mut choose = false;
    let modal = egui::Modal::new(egui::Id::unique("create_vault_dialog")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new("Create a vault").size(18.0).strong());
        ui.label(
            RichText::new(
                "djkitty makes a new folder with this name. Every track you import is \
                 copied into it; your original files are never changed.",
            )
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.add_space(10.0);
        ui.label(RichText::new("Name").small().color(theme::TEXT_DIM));
        let field = ui.add(
            widgets::text_field(&mut form.name)
                .hint_text("Vault name")
                .desired_width(f32::INFINITY),
        );
        if field.changed() {
            form.error = None;
        }
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Location").small().color(theme::TEXT_DIM));
        ui.horizontal(|ui| {
            let shown = form
                .parent
                .as_ref()
                .map_or_else(|| "Choose a folder".to_owned(), |p| p.display().to_string());
            ui.add(egui::Label::new(shown).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::lit_button(ui, true, None, egui::Button::new("Choose location…"))
                    .clicked()
                {
                    choose = true;
                }
            });
        });
        ui.add_space(8.0);
        if let Some(parent) = &form.parent
            && !form.name.trim().is_empty()
        {
            ui.label(
                RichText::new(format!(
                    "Creates  {}",
                    parent.join(form.name.trim()).display()
                ))
                .small()
                .color(theme::TEXT_DIM),
            );
        }
        if let Some(err) = &form.error {
            ui.label(RichText::new(err).small().color(theme::DANGER));
        }
        ui.add_space(12.0);
        let valid = form.parent.is_some() && !form.name.trim().is_empty();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut label = RichText::new("Create").strong();
            if valid {
                label = label.color(theme::LIT_INK);
            }
            let create = egui::Button::new(label).min_size(Vec2::new(80.0, 28.0));
            if widgets::lit_button(ui, valid, Some(theme::ACCENT), create).clicked() {
                submit = true;
            }
            let cancel = egui::Button::new("Cancel").min_size(Vec2::new(80.0, 28.0));
            if widgets::lit_button(ui, true, None, cancel).clicked() {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    if choose {
        let mut dialog = rfd::FileDialog::new().set_title("Where should the vault go?");
        if let Some(p) = form.parent.as_ref().filter(|p| p.is_dir()) {
            dialog = dialog.set_directory(p);
        }
        if let Some(p) = dialog.pick_folder() {
            form.parent = Some(p);
            form.error = None;
        }
    }
    if close {
        app.vault_ui.create = None;
        return;
    }
    if submit
        && let Some(parent) = form.parent.clone()
        && !form.name.trim().is_empty()
    {
        let name = form.name.clone();
        request(app, PendingVault::Create { parent, name });
    }
}

/// "This folder is synced by …" warning before creating or opening a vault there.
pub fn cloud_dialog(app: &mut DjApp, ctx: &egui::Context) {
    let Some((pending, service)) = app.vault_ui.confirm_cloud.clone() else {
        return;
    };
    let mut proceed = false;
    let mut back = false;
    let modal = egui::Modal::new(egui::Id::unique("cloud_vault_dialog")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.label(
            RichText::new(format!("This folder is synced by {service}"))
                .size(18.0)
                .strong(),
        );
        ui.add_space(6.0);
        ui.label(format!("{}", pending.target().display()));
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "{service} can damage the vault's library database while djkitty is using \
                 it, for example by syncing it halfway through a change. A folder that \
                 isn't synced is safer."
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let other = egui::Button::new(RichText::new("Choose another folder").strong())
                .min_size(Vec2::new(0.0, 28.0));
            if widgets::lit_button(ui, true, Some(theme::ACCENT), other).clicked() {
                back = true;
            }
            let anyway = egui::Button::new("Continue anyway").min_size(Vec2::new(0.0, 28.0));
            if widgets::lit_button(ui, true, None, anyway).clicked() {
                proceed = true;
            }
        });
    });
    if modal.should_close() {
        back = true;
    }
    if proceed {
        app.vault_ui.confirm_cloud = None;
        carry_out(app, pending);
    } else if back {
        app.vault_ui.confirm_cloud = None;
        if matches!(pending, PendingVault::Open(_)) {
            pick_and_open(app);
        }
        // For a new vault, the create dialog is still open to pick another location.
    }
}

// ---------------------------------------------------------------- switcher

/// The current vault's name, opening a menu to switch to or create another vault.
pub fn switcher(app: &mut DjApp, ui: &mut Ui) {
    let Some(vault) = &app.vault else { return };
    let name = vault.name();
    let root = vault.root().to_path_buf();
    let button = widgets::lit_button(
        ui,
        true,
        None,
        egui::Button::image_and_text(
            widgets::icon(widgets::icons::FOLDER, 14.0, theme::TEXT_DIM),
            RichText::new(&name).strong(),
        )
        .min_size(Vec2::new(0.0, widgets::FIELD_HEIGHT)),
    )
    .on_hover_text(format!("Vault: {}\nClick to switch vaults", root.display()));
    let mut action = None;
    egui::Popup::menu(&button)
        .align(egui::RectAlign::BOTTOM_START)
        .show(|ui| {
            ui.set_min_width(240.0);
            let blocked = app.vault_switch_blocked();
            if let Some(why) = &blocked {
                ui.label(RichText::new(why).small().color(theme::WARN));
                ui.separator();
            }
            let enabled = blocked.is_none();
            if ui
                .add_enabled(enabled, egui::Button::new("Open another vault…"))
                .clicked()
            {
                action = Some(SwitchAction::Open);
            }
            if ui
                .add_enabled(enabled, egui::Button::new("Create new vault…"))
                .clicked()
            {
                action = Some(SwitchAction::Create);
            }
            let others: Vec<PathBuf> = app
                .settings
                .recent_vaults
                .iter()
                .filter(|p| p.as_path() != root.as_path())
                .cloned()
                .collect();
            if !others.is_empty() {
                ui.separator();
                ui.label(RichText::new("Recent").small().color(theme::TEXT_DIM));
                for p in others {
                    let label = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    let available = djk_library::Vault::is_vault(&p);
                    if ui
                        .add_enabled(enabled && available, egui::Button::new(label))
                        .on_hover_text(p.display().to_string())
                        .on_disabled_hover_text(if available {
                            String::new()
                        } else {
                            format!("{} is not available", p.display())
                        })
                        .clicked()
                    {
                        action = Some(SwitchAction::Recent(p));
                    }
                }
            }
        });
    match action {
        Some(SwitchAction::Open) => pick_and_open(app),
        Some(SwitchAction::Create) => start_create(app),
        Some(SwitchAction::Recent(p)) => request(app, PendingVault::Open(p)),
        None => {}
    }
}

enum SwitchAction {
    Open,
    Create,
    Recent(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn onedrive_folders_are_detected() {
        let env = env_with(&[("OneDrive", r"C:\Users\dj\OneDrive")]);
        assert_eq!(
            synced_by_with(Path::new(r"C:\Users\dj\OneDrive\Music\Vault"), &env, None),
            Some("OneDrive")
        );
        assert_eq!(
            synced_by_with(Path::new(r"c:/users/DJ/onedrive/Vault"), &env, None),
            Some("OneDrive")
        );
        // A sibling whose name only starts the same is not inside it.
        assert_eq!(
            synced_by_with(Path::new(r"C:\Users\dj\OneDrive Backup\Vault"), &env, None),
            None
        );
        let business = env_with(&[("OneDriveCommercial", r"C:\Users\dj\OneDrive - Club")]);
        assert_eq!(
            synced_by_with(Path::new(r"C:\Users\dj\OneDrive - Club\V"), &business, None),
            Some("OneDrive")
        );
    }

    #[test]
    fn dropbox_is_detected_from_its_info_file() {
        let info = r#"{"personal": {"path": "D:\\Sync\\Box", "host": 1}}"#;
        let env = env_with(&[]);
        assert_eq!(
            synced_by_with(Path::new(r"D:\Sync\Box\Vault"), &env, Some(info)),
            Some("Dropbox")
        );
        assert_eq!(
            synced_by_with(Path::new(r"D:\Sync\Other"), &env, Some(info)),
            None
        );
        assert_eq!(dropbox_paths(info), vec![r"D:\Sync\Box".to_owned()]);
    }

    #[test]
    fn well_known_folder_names_are_detected() {
        let env = env_with(&[]);
        assert_eq!(
            synced_by_with(Path::new(r"C:\Users\dj\iCloudDrive\Vault"), &env, None),
            Some("iCloud Drive")
        );
        assert_eq!(
            synced_by_with(Path::new(r"E:\Dropbox\Vault"), &env, None),
            Some("Dropbox")
        );
        assert_eq!(synced_by_with(Path::new(r"D:\DJ\Vault"), &env, None), None);
    }
}
