use midir::{Ignore, MidiInput, MidiInputConnection};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteEvent {
    On(u8),
    Off(u8),
}

fn parse(msg: &[u8]) -> Option<NoteEvent> {
    match *msg {
        [status, note, vel, ..] if status & 0xF0 == 0x90 && vel > 0 => Some(NoteEvent::On(note)),
        [status, note, _, ..] if matches!(status & 0xF0, 0x80 | 0x90) => Some(NoteEvent::Off(note)),
        _ => None,
    }
}

pub fn list_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new("MusicBar-scan") else { return vec![] };
    input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect()
}

fn connect(name: &str, on_event: Arc<dyn Fn(NoteEvent) + Send + Sync>) -> Option<MidiInputConnection<()>> {
    let mut input = MidiInput::new("MusicBar").ok()?;
    input.ignore(Ignore::All);
    let port = input.ports().into_iter().find(|p| input.port_name(p).ok().as_deref() == Some(name))?;
    input
        .connect(&port, "MusicBar-in", move |_, msg, _| {
            if let Some(ev) = parse(msg) {
                on_event(ev);
            }
        }, ())
        .ok()
}

/// Keeps connections in sync with the available ports (handles hot-plugging).
/// `wanted` returns the configured port name, or `None` for all ports.
pub fn spawn(
    wanted: impl Fn() -> Option<String> + Send + 'static,
    on_event: impl Fn(NoteEvent) + Send + Sync + 'static,
) {
    let on_event: Arc<dyn Fn(NoteEvent) + Send + Sync> = Arc::new(on_event);
    thread::spawn(move || {
        let mut conns: HashMap<String, MidiInputConnection<()>> = HashMap::new();
        loop {
            let wanted = wanted();
            let targets: Vec<String> = list_ports()
                .into_iter()
                .filter(|name| wanted.as_ref().is_none_or(|w| w == name))
                .collect();
            conns.retain(|name, _| targets.contains(name));
            for name in targets {
                if !conns.contains_key(&name) {
                    if let Some(conn) = connect(&name, on_event.clone()) {
                        conns.insert(name, conn);
                    }
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_messages() {
        assert!(matches!(parse(&[0x90, 60, 100]), Some(NoteEvent::On(60))));
        assert!(matches!(parse(&[0x93, 60, 0]), Some(NoteEvent::Off(60))));
        assert!(matches!(parse(&[0x80, 60, 64]), Some(NoteEvent::Off(60))));
        assert!(parse(&[0xB0, 64, 127]).is_none());
    }
}
