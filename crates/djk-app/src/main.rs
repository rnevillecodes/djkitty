//! djkitty: the DJ app that grows with you.

mod devtools;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list-audio-devices") {
        return devtools::list_audio_devices();
    }
    if let Some(i) = args.iter().position(|a| a == "--midi-monitor") {
        let filter = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned();
        return devtools::midi_monitor(filter.as_deref().unwrap_or("DDJ-FLX2"));
    }
    println!("djkitty");
    Ok(())
}
