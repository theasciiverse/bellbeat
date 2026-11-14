//! Keys and gamepad buttons, each press stamped with the moment it was read.
//!
//! The four lanes are D F J K, or the arrow keys (← ↓ ↑ →), or on a pad the
//! d-pad, the face buttons laid out the same way (X A Y B), or the
//! shoulders and triggers (LB LT RT RB).
//!
//! With the kitty keyboard protocol (or on a pad) key releases arrive, so
//! held notes are held for real. Without it a terminal sends a key's repeats
//! as fresh presses; a lane ignores presses that come quicker than a
//! repeat could be told from a real tap, and held notes hold themselves.
//!
//! Gamepads: only pads in SDL's controller database count (gilrs also
//! reports 3D mice and tablet key pads as pads); `BELLBEAT_ANY_PAD=1`
//! accepts everything.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

/// Presses of one key quicker than this after the last are its repeats.
const REPEAT: Duration = Duration::from_millis(60);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Char(char),
    Left,
    Right,
    Up,
    Down,
    Enter,
    Esc,
    Back,
    Tab,
    CtrlC,
    /// Ctrl and a letter (lower case), other than Ctrl-C.
    Ctrl(char),
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    F(u8),
    PadLeft,
    PadRight,
    PadUp,
    PadDown,
    PadSouth,
    PadEast,
    PadWest,
    PadNorth,
    PadLB,
    PadRB,
    PadLT,
    PadRT,
    PadStart,
    PadSelect,
}

#[derive(Clone, Copy, Debug)]
pub struct Press {
    pub key: K,
    pub down: bool,
    pub at: Instant,
    /// Its release will be reported.
    pub exact: bool,
}

impl Press {
    pub fn lane(&self) -> Option<usize> {
        Some(match self.key {
            K::Char('d' | 'D') | K::Left | K::PadLeft | K::PadWest | K::PadLB => 0,
            K::Char('f' | 'F') | K::Down | K::PadDown | K::PadSouth | K::PadLT => 1,
            K::Char('j' | 'J') | K::Up | K::PadUp | K::PadNorth | K::PadRT => 2,
            K::Char('k' | 'K') | K::Right | K::PadRight | K::PadEast | K::PadRB => 3,
            _ => return None,
        })
    }
}

/// What a press means outside the lanes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    Quit,
}

pub fn nav(k: K) -> Option<Nav> {
    Some(match k {
        K::Up | K::PadUp | K::Char('w' | 'W') => Nav::Up,
        K::Down | K::PadDown | K::Char('s' | 'S') => Nav::Down,
        K::Left | K::PadLeft | K::Char('a' | 'A') => Nav::Left,
        K::Right | K::PadRight | K::Char('d' | 'D') => Nav::Right,
        K::Enter | K::Char(' ') | K::PadSouth | K::PadStart => Nav::Ok,
        K::Esc | K::Back | K::PadEast | K::PadSelect => Nav::Back,
        K::CtrlC => Nav::Quit,
        _ => return None,
    })
}

pub struct Input {
    /// Key releases are reported (kitty keyboard protocol).
    pub exact: bool,
    /// Typing text (the song editor): a held key repeats, as in any editor.
    pub typing: bool,
    pub presses: Vec<Press>,
    last: Vec<(K, Instant)>,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    pub pad_name: Option<String>,
}

#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("BELLBEAT_ANY_PAD").is_ok_and(|v| v == "1")
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input {
            exact,
            typing: false,
            presses: Vec::new(),
            last: Vec::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad_name: None,
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, at: Instant) {
        let key = match k.code {
            KeyCode::Char('c' | 'C') if k.modifiers.contains(KeyModifiers::CONTROL) => K::CtrlC,
            KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::CONTROL) => K::Ctrl(c.to_ascii_lowercase()),
            // with every key reported as a code, a shifted letter can come
            // as the plain letter and Shift
            KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() => K::Char(c.to_ascii_uppercase()),
            KeyCode::Char(c) => K::Char(c),
            KeyCode::Home => K::Home,
            KeyCode::End => K::End,
            KeyCode::PageUp => K::PageUp,
            KeyCode::PageDown => K::PageDown,
            KeyCode::Delete => K::Delete,
            KeyCode::F(n) => K::F(n),
            KeyCode::Left => K::Left,
            KeyCode::Right => K::Right,
            KeyCode::Up => K::Up,
            KeyCode::Down => K::Down,
            KeyCode::Enter => K::Enter,
            KeyCode::Esc => K::Esc,
            KeyCode::Backspace => K::Back,
            KeyCode::Tab => K::Tab,
            _ => return,
        };
        match k.kind {
            KeyEventKind::Release => {
                self.presses.push(Press { key, down: false, at, exact: true });
            }
            KeyEventKind::Repeat => {
                if self.typing {
                    self.presses.push(Press { key, down: true, at, exact: true });
                }
            }
            KeyEventKind::Press => {
                if !self.exact && !self.typing {
                    // a repeat looks like a press here: keep the time of the
                    // latest, and let it through only after a gap
                    let prev = self.last.iter_mut().find(|(k, _)| *k == key);
                    let repeat = prev.as_ref().is_some_and(|(_, t)| at.duration_since(*t) < REPEAT);
                    match prev {
                        Some(p) => p.1 = at,
                        None => self.last.push((key, at)),
                    }
                    if repeat {
                        return;
                    }
                }
                self.presses.push(Press { key, down: true, at, exact: self.exact });
            }
        }
    }

    pub fn open_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(_, p)| (p.name().to_string(), usable_pad(&p))).collect();
                log::info!("gamepads (name, used): {all:?}");
                self.pad_name = all.into_iter().find(|(_, used)| *used).map(|(n, _)| n);
                self.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
    }

    /// Drain gamepad events into `presses`.
    pub fn poll_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            let mut events = Vec::new();
            while let Some(ev) = g.next_event() {
                events.push(ev);
            }
            for ev in events {
                let Some(g) = self.gilrs.as_ref() else { return };
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                let (b, down) = match ev.event {
                    EventType::Connected => {
                        self.pad_name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.pad_name);
                        continue;
                    }
                    EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        self.pad_name = None;
                        continue;
                    }
                    EventType::ButtonPressed(b, _) => (b, true),
                    EventType::ButtonReleased(b, _) => (b, false),
                    _ => continue,
                };
                let key = match b {
                    Button::DPadLeft => K::PadLeft,
                    Button::DPadRight => K::PadRight,
                    Button::DPadUp => K::PadUp,
                    Button::DPadDown => K::PadDown,
                    Button::South => K::PadSouth,
                    Button::East => K::PadEast,
                    Button::West => K::PadWest,
                    Button::North => K::PadNorth,
                    Button::LeftTrigger => K::PadLB,
                    Button::RightTrigger => K::PadRB,
                    Button::LeftTrigger2 => K::PadLT,
                    Button::RightTrigger2 => K::PadRT,
                    Button::Start => K::PadStart,
                    Button::Select => K::PadSelect,
                    _ => continue,
                };
                log::debug!("pad {key:?} {}", if down { "down" } else { "up" });
                self.presses.push(Press { key, down, at: Instant::now(), exact: true });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventState;

    fn key(c: char, kind: KeyEventKind) -> KeyEvent {
        KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::NONE, kind, state: KeyEventState::NONE }
    }

    #[test]
    fn repeats_are_dropped_without_releases() {
        let mut i = Input::new(false);
        let t = Instant::now();
        i.on_key(key('d', KeyEventKind::Press), t);
        // the terminal's repeats, every 30 ms
        for n in 1..10 {
            i.on_key(key('d', KeyEventKind::Press), t + Duration::from_millis(400 + n * 30));
        }
        // a real second tap after letting go
        i.on_key(key('d', KeyEventKind::Press), t + Duration::from_millis(1000));
        let downs: Vec<_> = i.presses.iter().filter(|p| p.down).collect();
        // the first repeat after the delay gets through once; the stream after it does not
        assert!(downs.len() <= 3, "{}", downs.len());
        assert_eq!(downs[0].lane(), Some(0));
        assert!(!downs[0].exact);
    }

    #[test]
    fn typing_keeps_repeats_and_reads_ctrl_and_shift() {
        let mut i = Input::new(false);
        i.typing = true;
        let t = Instant::now();
        for n in 0..5 {
            i.on_key(key('x', KeyEventKind::Press), t + Duration::from_millis(n * 30));
        }
        assert_eq!(i.presses.len(), 5);
        i.presses.clear();
        let mut k = key('s', KeyEventKind::Press);
        k.modifiers = KeyModifiers::CONTROL;
        i.on_key(k, t);
        let mut k = key('e', KeyEventKind::Press);
        k.modifiers = KeyModifiers::SHIFT;
        i.on_key(k, t);
        assert_eq!(i.presses.iter().map(|p| p.key).collect::<Vec<_>>(), vec![K::Ctrl('s'), K::Char('E')]);
    }

    #[test]
    fn lanes_and_navigation() {
        let p = |key| Press { key, down: true, at: Instant::now(), exact: true };
        assert_eq!(p(K::Char('f')).lane(), Some(1));
        assert_eq!(p(K::Up).lane(), Some(2));
        assert_eq!(p(K::PadEast).lane(), Some(3));
        assert_eq!(p(K::PadLB).lane(), Some(0));
        assert_eq!(p(K::Enter).lane(), None);
        assert_eq!(nav(K::PadSouth), Some(Nav::Ok));
        assert_eq!(nav(K::Esc), Some(Nav::Back));
    }
}
