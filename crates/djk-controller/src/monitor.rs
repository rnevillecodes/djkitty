//! MIDI monitor dev mode: list ports, log raw incoming messages and send test messages.
//!
//! Used to discover a controller's MIDI messages (see `docs/controllers/ddj-flx2.md`).

use std::io::{BufRead, Write};

use midir::{MidiInput, MidiOutput};

/// Decode a raw MIDI message into a readable description.
pub fn describe(bytes: &[u8]) -> String {
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let hex = hex.join(" ");
    let Some(&status) = bytes.first() else {
        return hex;
    };
    let ch = (status & 0x0F) + 1;
    let d1 = bytes.get(1).copied().unwrap_or(0);
    let d2 = bytes.get(2).copied().unwrap_or(0);
    let kind = match status & 0xF0 {
        0x80 => format!("note-off ch{ch:<2} note {d1:3} (0x{d1:02X}) vel {d2:3}"),
        0x90 => format!("note-on  ch{ch:<2} note {d1:3} (0x{d1:02X}) vel {d2:3}"),
        0xA0 => format!("poly-at  ch{ch:<2} note {d1:3} val {d2:3}"),
        0xB0 => format!("cc       ch{ch:<2} cc   {d1:3} (0x{d1:02X}) val {d2:3}"),
        0xC0 => format!("program  ch{ch:<2} {d1:3}"),
        0xD0 => format!("chan-at  ch{ch:<2} {d1:3}"),
        0xE0 => {
            let v = (u16::from(d2) << 7) | u16::from(d1);
            format!("pitchbnd ch{ch:<2} {v:5}")
        }
        _ => "system".to_owned(),
    };
    format!("{hex:<10} {kind}")
}

/// Parse a line of hex bytes like `90 0B 7F` into raw bytes.
pub fn parse_hex(line: &str) -> Result<Vec<u8>, String> {
    line.split_whitespace()
        .map(|t| {
            let t = t.trim_start_matches("0x").trim_start_matches("0X");
            u8::from_str_radix(t, 16).map_err(|_| format!("not a hex byte: {t}"))
        })
        .collect()
}

/// Run the monitor until stdin closes. Opens every input port whose name contains
/// `filter` (or all ports when `filter` is empty) and the first matching output port.
pub fn run(filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();
    let input = MidiInput::new("djkitty-monitor-in")?;
    let output = MidiOutput::new("djkitty-monitor-out")?;

    writeln!(stdout, "MIDI input ports:")?;
    for p in input.ports() {
        writeln!(stdout, "  {}", input.port_name(&p).unwrap_or_default())?;
    }
    writeln!(stdout, "MIDI output ports:")?;
    for p in output.ports() {
        writeln!(stdout, "  {}", output.port_name(&p).unwrap_or_default())?;
    }

    let matches = |name: &str| filter.is_empty() || name.contains(filter);

    let mut connections = Vec::new();
    for p in input.ports() {
        let name = input.port_name(&p).unwrap_or_default();
        if !matches(&name) {
            continue;
        }
        let in_port = MidiInput::new("djkitty-monitor-in")?;
        let label = name.clone();
        let conn = in_port.connect(
            &p,
            "djkitty-monitor",
            move |stamp_us, msg, _| {
                println!(
                    "[{:>10.3} ms] {label}: {}",
                    stamp_us as f64 / 1000.0,
                    describe(msg)
                );
            },
            (),
        )?;
        writeln!(stdout, "Listening on: {name}")?;
        connections.push(conn);
    }
    if connections.is_empty() {
        writeln!(stdout, "No input port matches {filter:?}.")?;
    }

    let mut out_conn = None;
    if let Some(p) = output
        .ports()
        .into_iter()
        .find(|p| matches(&output.port_name(p).unwrap_or_default()))
    {
        let name = output.port_name(&p).unwrap_or_default();
        out_conn = Some(output.connect(&p, "djkitty-monitor-out")?);
        writeln!(stdout, "Sending to: {name}")?;
    }

    writeln!(
        stdout,
        "\nMove controls to see their messages. Type hex bytes (e.g. `90 0B 7F`) and press \
         Enter to send them to the controller. Ctrl+C or close stdin to quit.\n"
    )?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match (parse_hex(&line), out_conn.as_mut()) {
            (Ok(bytes), Some(conn)) => {
                conn.send(&bytes)?;
                println!("sent: {}", describe(&bytes));
            }
            (Ok(_), None) => println!("no output port to send to"),
            (Err(e), _) => println!("{e}"),
        }
    }
    drop(connections);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_note_and_cc() {
        assert!(describe(&[0x90, 0x0B, 0x7F]).contains("note-on  ch1  note  11"));
        assert!(describe(&[0xB1, 0x13, 0x40]).contains("cc       ch2  cc    19"));
    }

    #[test]
    fn parses_hex_lines() {
        assert_eq!(parse_hex("90 0b 0x7F").unwrap(), vec![0x90, 0x0B, 0x7F]);
        assert!(parse_hex("zz").is_err());
    }
}
