//! Songs, written as text: every built-in song in Bellbeat is our own,
//! composed for the game, kept in `songs/*.song` and synthesised live by
//! `synth.rs`. Players write their own in the same notation.
//!
//! A song file has settings at the top, then its sections:
//!
//! ```text
//! title  Hello, World
//! blurb  The first thing every program says.
//! bpm    104
//! meter  4          beats a bar (2 to 7); 3 for a waltz
//! swing  50         where the off-beat eighth falls, 50 (straight) to 75
//! voice  bell       the melody's sound: bell, pulse, pluck or flute
//! order  intro verse chorus verse
//!
//! [verse] 8 bars
//! chords  C G Am F
//! lead    E5 - - - G5 - - - C6 - - - - - - -
//!         B5 - - - ...
//! ```
//!
//! - `chords`: one chord a bar (`Am`, `G7`, `Bbmaj7`, `Dsus4`), cycled to
//!   fill the section; the pad holds it, and the bass and arpeggio take
//!   their notes from it.
//! - `lead` and `harmony`: one token per sixteenth: a note (`E5`, `F#4`,
//!   `Bb5`), `-` to hold the note before, `.` for silence. The lead is the
//!   melody you play; the harmony is a second voice, yours on Hard as the
//!   other half of a chord.
//! - `bass` and `arp`: one token per sixteenth in chord degrees (`1` root,
//!   `3` third, `5` fifth, `7` seventh, `8` octave), `-` to hold, `.` rest.
//! - `kick`, `snare`, `hat`: `x` a hit, `.` nothing, one character per
//!   sixteenth; spaces between them are only for reading.
//!
//! Every line is either one bar long, and repeats to fill the section, or
//! as long as the whole section. A line that starts with a space carries
//! on the line before it; a line that starts with `#` is a comment.
//!
//! Song files come from players, so `parse` checks everything and says
//! what is wrong and on which line; a song that parses can always be
//! scored, charted and played.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lead {
    /// FM bell: the BEL character's song, bright and ringing.
    Bell,
    /// A narrow pulse, the classic chip lead.
    Pulse,
    /// A plucked saw that closes quickly.
    Pluck,
    /// A soft breathy sine with vibrato.
    Flute,
}

impl Lead {
    pub const NAMES: [&'static str; 4] = ["bell", "pulse", "pluck", "flute"];

    pub fn from_name(s: &str) -> Option<Lead> {
        Some(match s.to_ascii_lowercase().as_str() {
            "bell" => Lead::Bell,
            "pulse" => Lead::Pulse,
            "pluck" => Lead::Pluck,
            "flute" => Lead::Flute,
            _ => return None,
        })
    }
}

/// One sixteenth of a melodic line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok {
    /// A note (MIDI number) or, in bass and arpeggio, a chord degree in
    /// semitones above the root.
    On(i32),
    Hold,
    Rest,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    pub bars: usize,
    /// One chord a bar, cycled: root (MIDI, octave 3) and intervals.
    pub chords: Vec<(i32, Vec<i32>)>,
    pub lead: Vec<Tok>,
    pub harmony: Vec<Tok>,
    /// Degrees as tokens: `On(d)` with d the degree's number (1, 3, 5, 7, 8).
    pub bass: Vec<Tok>,
    pub arp: Vec<Tok>,
    pub kick: Vec<bool>,
    pub snare: Vec<bool>,
    pub hat: Vec<bool>,
}

#[derive(Clone, Debug)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub blurb: String,
    pub bpm: f32,
    /// Beats a bar.
    pub meter: usize,
    /// Where the second eighth of a beat falls, as a share of the beat:
    /// 0.5 straight, 0.67 a triplet swing.
    pub swing: f32,
    pub lead: Lead,
    pub sections: Vec<Section>,
    pub order: Vec<usize>,
    /// One of the songs that come with the game.
    pub builtin: bool,
}

/// What is wrong with a song file, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct SongError {
    /// The line, from 1; 0 for the file as a whole.
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for SongError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.msg)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, SongError> {
    Err(SongError { line, msg: msg.into() })
}

// ------------------------------------------------------------------ built in

/// The songs that come with the game, gentlest first.
const BUILTIN: [(&str, &str); 9] = [
    ("lullaby-for-a-daemon", include_str!("../songs/lullaby-for-a-daemon.song")),
    ("hello-world", include_str!("../songs/hello-world.song")),
    ("waltz-for-a-spinning-disk", include_str!("../songs/waltz-for-a-spinning-disk.song")),
    ("cron-at-midnight", include_str!("../songs/cron-at-midnight.song")),
    ("carriage-return", include_str!("../songs/carriage-return.song")),
    ("tab-complete-shuffle", include_str!("../songs/tab-complete-shuffle.song")),
    ("null-pointer-samba", include_str!("../songs/null-pointer-samba.song")),
    ("escape-sequence", include_str!("../songs/escape-sequence.song")),
    ("kernel-panic", include_str!("../songs/kernel-panic.song")),
];

pub fn builtins() -> Vec<Song> {
    BUILTIN
        .iter()
        .filter_map(|(id, text)| match parse(text, id) {
            Ok(mut s) => {
                s.builtin = true;
                Some(s)
            }
            Err(e) => {
                // the tests keep this from happening
                log::error!("built-in song {id}: {e}");
                None
            }
        })
        .collect()
}

// ------------------------------------------------------------------ notes

/// A note name like `C5`, `F#4` or `Bb3` as a MIDI note number, for
/// octaves -1 to 9.
pub fn parse_note(s: &str) -> Option<i32> {
    let mut chars = s.chars();
    let letter = chars.next()?;
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (shift, octave) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest)
    };
    if octave.is_empty() || octave.len() > 2 {
        return None;
    }
    let octave: i32 = octave.parse().ok()?;
    if !(-1..=9).contains(&octave) {
        return None;
    }
    Some(12 * (octave + 1) + base + shift)
}

/// The chord qualities a song may use, as intervals from the root.
const QUALITIES: [(&str, &[i32]); 9] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("7", &[0, 4, 7, 10]),
    ("m7", &[0, 3, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("dim", &[0, 3, 6]),
    ("6", &[0, 4, 7, 9]),
];

/// A chord name like `Am`, `G7`, `Bbmaj7` or `C#m7`: its root (MIDI, in
/// octave 3) and its notes as intervals from the root.
pub fn parse_chord(s: &str) -> Option<(i32, Vec<i32>)> {
    let mut chars = s.chars();
    let letter = chars.next()?;
    let rest = chars.as_str();
    let (acc, quality) = if let Some(q) = rest.strip_prefix('#') {
        ("#", q)
    } else if let Some(q) = rest.strip_prefix('b') {
        ("b", q)
    } else {
        ("", rest)
    };
    let root = parse_note(&format!("{letter}{acc}3"))?;
    let iv = QUALITIES.iter().find(|(q, _)| *q == quality)?.1;
    Some((root, iv.to_vec()))
}

/// What a degree means in a chord: semitones above the root.
fn degree(d: i32, iv: &[i32]) -> i32 {
    match d {
        3 => iv[1],
        5 => iv[2],
        7 => *iv.get(3).unwrap_or(&10),
        8 => 12,
        _ => 0,
    }
}

// ------------------------------------------------------------------ parsing

const SONG_KEYS: [&str; 7] = ["title", "blurb", "bpm", "meter", "swing", "voice", "order"];
const SECTION_KEYS: [&str; 8] = ["chords", "lead", "harmony", "bass", "arp", "kick", "snare", "hat"];

/// The largest song file read, in bytes.
pub const MAX_FILE: u64 = 256 * 1024;
const MAX_SECTIONS: usize = 64;
const MAX_BARS: usize = 64;
const MAX_ORDER: usize = 128;
const MAX_SECS: f64 = 600.0;
const MIN_NOTES: usize = 8;

/// A setting or a section's line, with continuations joined.
struct Entry {
    key: String,
    line: usize,
    value: String,
}

struct Block {
    name: String,
    line: usize,
    bars: usize,
    entries: Vec<Entry>,
}

/// Read a song file. `id` names the song (its file name, for a player's).
pub fn parse(text: &str, id: &str) -> Result<Song, SongError> {
    if text.len() as u64 > MAX_FILE {
        return err(0, format!("the file is too long (more than {} KB)", MAX_FILE / 1024));
    }
    let mut head: Vec<Entry> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let ln = n + 1;
        if raw.chars().any(|c| c.is_control() && c != '\t') {
            return err(ln, "a control character; only plain text can be in a song");
        }
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let in_section = !blocks.is_empty();
        let entries = match blocks.last_mut() {
            Some(b) => &mut b.entries,
            None => &mut head,
        };
        if raw.starts_with([' ', '\t']) {
            match entries.last_mut() {
                Some(e) => {
                    e.value.push(' ');
                    e.value.push_str(t);
                }
                None => return err(ln, "this line starts with a space, so it carries on the line before, but there is none"),
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix('[') {
            let Some((name, after)) = rest.split_once(']') else {
                return err(ln, "a section starts like `[verse] 8 bars`; the `]` is missing");
            };
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return err(ln, "a section's name is one word, like `[verse]`");
            }
            if blocks.iter().any(|b| b.name == name) {
                return err(ln, format!("there is already a section named `{name}`"));
            }
            let mut w = after.split_whitespace();
            let bars = match w.next().map(|b| b.parse::<usize>()) {
                Some(Ok(b)) if (1..=MAX_BARS).contains(&b) => b,
                Some(Ok(_)) => return err(ln, format!("a section has 1 to {MAX_BARS} bars")),
                _ => return err(ln, format!("how many bars is `{name}`? Write it like `[{name}] 8 bars`")),
            };
            if !matches!(w.next(), None | Some("bar" | "bars")) || w.next().is_some() {
                return err(ln, format!("after the bars, nothing more: `[{name}] {bars} bars`"));
            }
            if blocks.len() >= MAX_SECTIONS {
                return err(ln, format!("at most {MAX_SECTIONS} sections"));
            }
            blocks.push(Block { name: name.to_string(), line: ln, bars, entries: Vec::new() });
            continue;
        }
        let (key, value) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
        let key = key.to_ascii_lowercase();
        let allowed: &[&str] = if in_section { &SECTION_KEYS } else { &SONG_KEYS };
        if !allowed.contains(&key.as_str()) {
            let other = if in_section { SONG_KEYS.contains(&key.as_str()) } else { SECTION_KEYS.contains(&key.as_str()) };
            return if other && in_section {
                err(ln, format!("`{key}` belongs at the top of the file, before the first section"))
            } else if other {
                err(ln, format!("`{key}` belongs in a section; start one with a line like `[verse] 8 bars`"))
            } else {
                err(ln, format!("unknown line `{key}`; it can be one of: {}", allowed.join(", ")))
            };
        }
        if entries.iter().any(|e| e.key == key) {
            return err(ln, format!("`{key}` is here twice"));
        }
        entries.push(Entry { key, line: ln, value: value.trim().to_string() });
    }

    let get = |k: &str| head.iter().find(|e| e.key == k);
    let title = match get("title") {
        Some(e) if !e.value.is_empty() => {
            if e.value.chars().count() > 48 {
                return err(e.line, "the title is longer than 48 characters");
            }
            e.value.clone()
        }
        Some(e) => return err(e.line, "the title is empty"),
        None => return err(1, "the song needs a title: a line like `title  My Song`"),
    };
    let blurb = match get("blurb") {
        Some(e) if e.value.chars().count() > 90 => return err(e.line, "the blurb is longer than 90 characters"),
        Some(e) => e.value.clone(),
        None => String::new(),
    };
    let number = |k: &str, lo: f32, hi: f32, default: Option<f32>| -> Result<f32, SongError> {
        match get(k) {
            Some(e) => match e.value.parse::<f32>() {
                Ok(v) if (lo..=hi).contains(&v) => Ok(v),
                _ => err(e.line, format!("`{k}` is a number from {lo} to {hi}")),
            },
            None => match default {
                Some(d) => Ok(d),
                None => err(1, format!("the song needs `{k}`, a number from {lo} to {hi}")),
            },
        }
    };
    let bpm = number("bpm", 40.0, 240.0, None)?;
    let meter = {
        let m = number("meter", 2.0, 7.0, Some(4.0))?;
        if m.fract() != 0.0 {
            return err(get("meter").map_or(1, |e| e.line), "`meter` is a whole number of beats a bar, 2 to 7");
        }
        m as usize
    };
    let swing = number("swing", 50.0, 75.0, Some(50.0))? / 100.0;
    let lead = match get("voice") {
        Some(e) => Lead::from_name(&e.value).ok_or(SongError { line: e.line, msg: format!("the voice is one of: {}", Lead::NAMES.join(", ")) })?,
        None => Lead::Bell,
    };
    if blocks.is_empty() {
        return err(text.lines().count().max(1), "the song has no sections yet; start one with a line like `[verse] 8 bars`");
    }
    let order = match get("order") {
        Some(e) => {
            let mut v = Vec::new();
            for name in e.value.split_whitespace() {
                match blocks.iter().position(|b| b.name == name) {
                    Some(i) => v.push(i),
                    None => return err(e.line, format!("there is no section `{name}` to play")),
                }
            }
            if v.is_empty() {
                return err(e.line, "`order` names the sections to play, like `order  intro verse chorus`");
            }
            if v.len() > MAX_ORDER {
                return err(e.line, format!("at most {MAX_ORDER} sections in the order"));
            }
            v
        }
        None => (0..blocks.len()).collect(),
    };

    let bar = meter * 4;
    let mut sections = Vec::new();
    for b in &blocks {
        let len = b.bars * bar;
        let what = |e: &Entry| format!("`{}` in [{}]", e.key, b.name);
        let fits = |e: &Entry, n: usize| -> Result<(), SongError> {
            if n == bar || n == len {
                Ok(())
            } else if b.bars == 1 {
                err(e.line, format!("{} has {n} steps; one bar of {meter}/4 is {bar} sixteenths", what(e)))
            } else {
                err(e.line, format!("{} has {n} steps; it should be one bar ({bar}) to repeat, or all {} bars ({len})", what(e), b.bars))
            }
        };
        let find = |k: &str| b.entries.iter().find(|e| e.key == k);
        let chords = match find("chords") {
            Some(e) => {
                let mut v = Vec::new();
                for c in e.value.split_whitespace() {
                    match parse_chord(c) {
                        Some(ch) => v.push(ch),
                        None => return err(e.line, format!("`{c}` is not a chord here: a root (C, F#, Bb) and one of m, 7, m7, maj7, sus2, sus4, dim, 6, or nothing")),
                    }
                }
                if v.is_empty() {
                    return err(e.line, format!("[{}] needs its chords, one a bar, like `chords  C G Am F`", b.name));
                }
                v
            }
            None => return err(b.line, format!("[{}] needs its chords, one a bar, like `chords  C G Am F`", b.name)),
        };
        let melodic = |k: &str| -> Result<Vec<Tok>, SongError> {
            let Some(e) = find(k) else { return Ok(Vec::new()) };
            let toks: Vec<&str> = e.value.split_whitespace().collect();
            if toks.is_empty() || toks == ["."] {
                return Ok(Vec::new());
            }
            let mut out = Vec::with_capacity(toks.len());
            for t in &toks {
                out.push(match *t {
                    "-" => Tok::Hold,
                    "." => Tok::Rest,
                    _ => match parse_note(t) {
                        Some(m) if (24..=108).contains(&m) => Tok::On(m),
                        Some(_) => return err(e.line, format!("{}: `{t}` is out of range; notes go from C1 to C8", what(e))),
                        None => return err(e.line, format!("{}: `{t}` is not a note; notes look like E5, F#4 or Bb3, `-` holds and `.` rests", what(e))),
                    },
                });
            }
            fits(e, out.len())?;
            Ok(out)
        };
        let degrees = |k: &str| -> Result<Vec<Tok>, SongError> {
            let Some(e) = find(k) else { return Ok(Vec::new()) };
            let toks: Vec<&str> = e.value.split_whitespace().collect();
            if toks.is_empty() || toks == ["."] {
                return Ok(Vec::new());
            }
            let mut out = Vec::with_capacity(toks.len());
            for t in &toks {
                out.push(match *t {
                    "-" => Tok::Hold,
                    "." => Tok::Rest,
                    "1" | "3" | "5" | "7" | "8" => Tok::On(t.parse().unwrap_or(1)),
                    _ => return err(e.line, format!("{}: `{t}` is not a degree; use 1 3 5 7 8 of the chord, `-` and `.`", what(e))),
                });
            }
            fits(e, out.len())?;
            Ok(out)
        };
        let drums = |k: &str| -> Result<Vec<bool>, SongError> {
            let Some(e) = find(k) else { return Ok(Vec::new()) };
            let mut out = Vec::new();
            for c in e.value.chars().filter(|c| !c.is_whitespace()) {
                match c {
                    'x' | 'X' => out.push(true),
                    '.' => out.push(false),
                    _ => return err(e.line, format!("{}: `{c}` is neither `x` (a hit) nor `.` (nothing)", what(e))),
                }
            }
            if out.is_empty() {
                return Ok(out);
            }
            fits(e, out.len())?;
            Ok(out)
        };
        sections.push(Section {
            name: b.name.clone(),
            bars: b.bars,
            chords,
            lead: melodic("lead")?,
            harmony: melodic("harmony")?,
            bass: degrees("bass")?,
            arp: degrees("arp")?,
            kick: drums("kick")?,
            snare: drums("snare")?,
            hat: drums("hat")?,
        });
    }
    let song = Song { id: id.to_string(), title, blurb, bpm, meter, swing, lead, sections, order, builtin: false };
    let secs = song.secs();
    if secs > MAX_SECS {
        return err(get("order").map_or(1, |e| e.line), format!("the song lasts {:.0} s; ten minutes at most", secs));
    }
    let notes = song.score().lead.len();
    if notes < MIN_NOTES {
        let line = blocks.iter().flat_map(|b| &b.entries).find(|e| e.key == "lead").map_or(1, |e| e.line);
        return err(line, format!("the melody has {notes} notes in the order played; a song needs {MIN_NOTES} at least"));
    }
    Ok(song)
}

// ------------------------------------------------------------------ the score

/// A sounding note: when, how long (both in steps) and what.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub step: usize,
    pub len: usize,
    pub midi: i32,
}

/// Everything a song plays, laid out in steps from its start.
#[derive(Clone, Debug, Default)]
pub struct Score {
    pub steps: usize,
    pub lead: Vec<Tone>,
    pub harmony: Vec<Tone>,
    pub bass: Vec<Tone>,
    pub arp: Vec<Tone>,
    /// The chord each bar: (first step, root, intervals).
    pub chords: Vec<(usize, i32, Vec<i32>)>,
    pub kick: Vec<usize>,
    pub snare: Vec<usize>,
    pub hat: Vec<usize>,
    /// Where each section begins, in steps, in play order.
    pub sections: Vec<usize>,
}

/// A line of tokens, cycled over `len` steps from `at`.
fn line(toks: &[Tok], at: usize, len: usize, mut pitch: impl FnMut(usize, i32) -> i32, out: &mut Vec<Tone>) {
    if toks.is_empty() {
        return;
    }
    let mut open: Option<Tone> = None;
    for i in 0..len {
        match toks[i % toks.len()] {
            Tok::Hold => {
                if let Some(t) = &mut open {
                    t.len += 1;
                }
            }
            Tok::Rest => {
                if let Some(t) = open.take() {
                    out.push(t);
                }
            }
            Tok::On(v) => {
                if let Some(t) = open.take() {
                    out.push(t);
                }
                open = Some(Tone { step: at + i, len: 1, midi: pitch(at + i, v) });
            }
        }
    }
    if let Some(t) = open {
        out.push(t);
    }
}

impl Song {
    /// Steps (sixteenths) a bar.
    pub fn bar(&self) -> usize {
        self.meter * 4
    }

    pub fn score(&self) -> Score {
        let bar = self.bar();
        let mut sc = Score::default();
        let mut at = 0;
        for &si in &self.order {
            let s = &self.sections[si];
            let len = s.bars * bar;
            sc.sections.push(at);
            let chord_at = |step: usize| &s.chords[((step - at) / bar) % s.chords.len()];
            for b in 0..s.bars {
                let (r, iv) = chord_at(at + b * bar);
                sc.chords.push((at + b * bar, *r, iv.clone()));
            }
            line(&s.lead, at, len, |_, m| m, &mut sc.lead);
            line(&s.harmony, at, len, |_, m| m, &mut sc.harmony);
            line(&s.bass, at, len, |st, d| {
                let (r, iv) = chord_at(st);
                r - 12 + degree(d, iv)
            }, &mut sc.bass);
            line(&s.arp, at, len, |st, d| {
                let (r, iv) = chord_at(st);
                r + 24 + degree(d, iv)
            }, &mut sc.arp);
            for (pat, out) in [(&s.kick, &mut sc.kick), (&s.snare, &mut sc.snare), (&s.hat, &mut sc.hat)] {
                if pat.is_empty() {
                    continue;
                }
                for i in 0..len {
                    if pat[i % pat.len()] {
                        out.push(at + i);
                    }
                }
            }
            at += len;
        }
        sc.steps = at;
        sc
    }

    /// Seconds a straight sixteenth lasts.
    pub fn step_secs(&self) -> f64 {
        60.0 / self.bpm as f64 / 4.0
    }

    /// When step `s` sounds, in seconds from the start, swing and all: in
    /// each beat the second eighth falls at `swing` of the beat, and the
    /// sixteenths halfway either side of it.
    pub fn time(&self, s: usize) -> f64 {
        let r = self.swing as f64;
        let within = [0.0, r / 2.0, r, r + (1.0 - r) / 2.0][s % 4];
        ((s / 4) as f64 + within) * 4.0 * self.step_secs()
    }

    pub fn secs(&self) -> f64 {
        let steps: usize = self.order.iter().map(|&i| self.sections[i].bars * self.bar()).sum();
        self.time(steps)
    }

    /// The same song played at `k` times the tempo (practice).
    pub fn at_tempo(&self, k: f32) -> Song {
        let mut s = self.clone();
        s.bpm *= k;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_chords_parse() {
        assert_eq!(parse_note("A4"), Some(69));
        assert_eq!(parse_note("C4"), Some(60));
        assert_eq!(parse_note("F#5"), Some(78));
        assert_eq!(parse_note("Bb3"), Some(58));
        assert_eq!(parse_note("H2"), None);
        assert_eq!(parse_note("C99999999999"), None);
        assert_eq!(parse_note("C"), None);
        assert_eq!(parse_chord("Am"), Some((57, vec![0, 3, 7])));
        assert_eq!(parse_chord("Bbmaj7"), Some((58, vec![0, 4, 7, 11])));
        assert_eq!(parse_chord("C#m7"), Some((49, vec![0, 3, 7, 10])));
        assert_eq!(parse_chord("A\u{20ac}"), None);
        assert_eq!(parse_chord("\u{e9}"), None);
        assert_eq!(parse_chord(""), None);
    }

    #[test]
    fn every_builtin_song_is_well_formed() {
        let all = builtins();
        assert_eq!(all.len(), BUILTIN.len(), "a built-in song failed to parse");
        for s in &all {
            let sc = s.score();
            assert!(sc.lead.len() > 40, "{} has only {} melody notes", s.id, sc.lead.len());
            assert!((40.0..170.0).contains(&s.secs()), "{} lasts {:.0} s", s.id, s.secs());
            // the melody sits where the lanes and the bell sound good
            assert!(sc.lead.iter().all(|t| (55..=96).contains(&t.midi)), "{}", s.id);
            assert!(sc.harmony.iter().all(|t| (50..=93).contains(&t.midi)), "{}", s.id);
            assert!(!s.blurb.is_empty() && s.title.chars().count() <= 30, "{}", s.id);
        }
        let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        assert!(all.iter().any(|s| s.meter == 3), "a song in 3/4");
        assert!(all.iter().any(|s| s.swing > 0.55), "a song with swing");
        assert!(all.iter().filter(|s| s.score().harmony.len() > 8).count() >= 4, "harmony for chords on Hard");
        for (i, a) in ids.iter().enumerate() {
            assert!(!ids[i + 1..].contains(a));
        }
    }

    #[test]
    fn holds_and_rests_make_lengths() {
        let mut out = Vec::new();
        let toks = [Tok::On(72), Tok::Hold, Tok::Hold, Tok::Rest, Tok::On(74), Tok::Rest, Tok::On(76), Tok::Hold];
        line(&toks, 0, 8, |_, m| m, &mut out);
        assert_eq!(out, vec![Tone { step: 0, len: 3, midi: 72 }, Tone { step: 4, len: 1, midi: 74 }, Tone { step: 6, len: 2, midi: 76 }]);
    }

    #[test]
    fn swing_moves_the_off_beats_only() {
        let mut s = builtins().remove(0);
        s.bpm = 60.0;
        s.swing = 0.5;
        assert!((s.time(2) - 0.5).abs() < 1e-9 && (s.time(5) - 1.25).abs() < 1e-9);
        s.swing = 2.0 / 3.0;
        assert!((s.time(4) - 1.0).abs() < 1e-9);
        assert!((s.time(2) - 2.0 / 3.0).abs() < 1e-6);
        assert!(s.time(1) < s.time(2) && s.time(2) < s.time(3) && s.time(3) < s.time(4));
    }

    const SMALL: &str = "title  Small\nbpm 120\n\n[a] 2 bars\nchords C G\nlead   C5 - D5 - E5 - F5 - G5 - A5 - B5 - C6 -\nkick x... x... x... x...\n";

    #[test]
    fn a_small_song_parses() {
        let s = parse(SMALL, "small").unwrap();
        assert_eq!(s.title, "Small");
        assert_eq!(s.meter, 4);
        assert_eq!(s.order, vec![0]);
        assert_eq!(s.score().lead.len(), 16);
        assert_eq!(s.score().kick.len(), 8);
    }

    #[test]
    fn mistakes_are_named_with_their_line() {
        let bad = |text: &str| parse(text, "t").unwrap_err();
        assert_eq!(bad("bpm 100\n[a] 1\nchords C\nlead C5\n").line, 1);
        let e = bad(&SMALL.replace("C5 - D5", "C5 - H5"));
        assert_eq!(e.line, 6);
        assert!(e.msg.contains("H5"), "{e}");
        let e = bad(&SMALL.replace("chords C G", "chords C Gx"));
        assert_eq!(e.line, 5);
        let e = bad(&SMALL.replace(" B5 - C6 -", ""));
        assert!(e.msg.contains("12 steps"), "{e}");
        let e = bad(&SMALL.replace("bpm 120", "bpm 9000"));
        assert_eq!(e.line, 2);
        let e = bad(&SMALL.replace("[a] 2 bars", "[a] lots"));
        assert_eq!(e.line, 4);
        let e = bad(&format!("{SMALL}order a b\n"));
        assert!(e.msg.contains("top of the file"), "{e}");
        let e = bad(&SMALL.replace("bpm 120", "bpm 120\norder a b"));
        assert!(e.msg.contains("no section `b`"), "{e}");
        let e = bad(&SMALL.replace("kick x... x... x... x...", "kick x..o"));
        assert!(e.msg.contains("`o`"), "{e}");
        let e = bad("title A\nbpm 100\n");
        assert!(e.msg.contains("no sections"), "{e}");
        let e = bad("title A\nbpm 100\n[a] 1\nchords C\nlead C5\n");
        assert!(e.msg.contains("16"), "{e}");
        let e = bad("  carries on\n");
        assert_eq!(e.line, 1);
        let e = bad(&SMALL.replace("title  Small", "title  Sm\u{1b}[2Jall"));
        assert!(e.msg.contains("control"), "{e}");
    }

    #[test]
    fn three_four_has_twelve_steps_a_bar() {
        let t = "title W\nbpm 150\nmeter 3\n[a] 3 bars\nchords C\nlead C5 - - E5 - - G5 - - E5 - - D5 - - F5 - - A5 - - G5 - - C5 - - - - - E5 - - G5 - -\n";
        let s = parse(t, "w").unwrap();
        assert_eq!(s.bar(), 12);
        assert_eq!(s.score().steps, 36);
        assert!(parse(&t.replace("meter 3", "meter 4"), "w").is_err());
    }

    /// Song files come from players: no input, however broken, may panic.
    #[test]
    fn garbage_never_panics() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let pieces = ["[", "]", "#", "-", ".", "x", " ", "\n", "\t", "C#", "b", "9", "0", "bars", "lead", "chords", "\u{e9}", "\u{20ac}", "meter 7", "swing 75", "order", "Bb", "m7", "-1", "999999999999", "\r"];
        for (_, text) in BUILTIN.iter().chain([("small", SMALL)].iter()) {
            for _ in 0..300 {
                let mut s: Vec<char> = text.chars().collect();
                for _ in 0..1 + rnd() % 6 {
                    let at = rnd() as usize % (s.len() + 1);
                    match rnd() % 3 {
                        0 if at < s.len() => {
                            s.remove(at);
                        }
                        1 => {
                            let p = pieces[rnd() as usize % pieces.len()];
                            for (k, c) in p.chars().enumerate() {
                                s.insert(at + k, c);
                            }
                        }
                        _ => {
                            let len = (rnd() % 40) as usize;
                            let end = (at + len).min(s.len());
                            s.drain(at..end);
                        }
                    }
                }
                let t: String = s.into_iter().collect();
                if let Ok(song) = parse(&t, "fuzz") {
                    let sc = song.score();
                    let _ = song.secs();
                    let _ = crate::chart::build(&song, &sc, crate::chart::Difficulty::Hard);
                }
            }
        }
        for t in ["", "\n\n", "[", "[]", "[a]", "title", "title x\nbpm 1e40", "title x\nbpm NaN\n", "title x\nbpm 100\nmeter 3.5\n"] {
            assert!(parse(t, "x").is_err(), "{t:?}");
        }
    }
}
