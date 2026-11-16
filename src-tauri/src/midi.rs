use crate::actions::Control;
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

/// A message from a MIDI input: a note (with its 0-based channel, so a pad can be told apart), or a
/// knob/button/program change with its value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEvent {
    Note { channel: u8, event: NoteEvent },
    Control { control: Control, value: u8 },
}

fn parse(msg: &[u8]) -> Option<MidiEvent> {
    let channel = *msg.first()? & 0x0F;
    let note = |event| Some(MidiEvent::Note { channel, event });
    match *msg {
        [status, n, vel, ..] if status & 0xF0 == 0x90 && vel > 0 => note(NoteEvent::On(n)),
        [status, n, _, ..] if matches!(status & 0xF0, 0x80 | 0x90) => note(NoteEvent::Off(n)),
        [status, number, value, ..] if status & 0xF0 == 0xB0 => {
            Some(MidiEvent::Control { control: Control::Cc { channel, number }, value })
        }
        [status, number, ..] if status & 0xF0 == 0xC0 => {
            Some(MidiEvent::Control { control: Control::Program { channel, number }, value: 127 })
        }
        _ => None,
    }
}

pub fn list_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new("MusicBar-scan") else { return vec![] };
    input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect()
}

fn connect(name: &str, on_event: Arc<dyn Fn(MidiEvent) + Send + Sync>) -> Option<MidiInputConnection<()>> {
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
    on_event: impl Fn(MidiEvent) + Send + Sync + 'static,
) {
    let on_event: Arc<dyn Fn(MidiEvent) + Send + Sync> = Arc::new(on_event);
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
        let note = |channel, event| Some(MidiEvent::Note { channel, event });
        assert_eq!(parse(&[0x90, 60, 100]), note(0, NoteEvent::On(60)));
        assert_eq!(parse(&[0x93, 60, 0]), note(3, NoteEvent::Off(60)));
        assert_eq!(parse(&[0x89, 36, 64]), note(9, NoteEvent::Off(36)));
        assert_eq!(parse(&[0xE0, 0, 64]), None, "pitch bend");
    }

    #[test]
    fn parses_controls() {
        assert_eq!(
            parse(&[0xB1, 21, 127]),
            Some(MidiEvent::Control { control: Control::Cc { channel: 1, number: 21 }, value: 127 })
        );
        assert_eq!(
            parse(&[0xC0, 5]),
            Some(MidiEvent::Control { control: Control::Program { channel: 0, number: 5 }, value: 127 })
        );
    }
}
