//! The songs, written out here as text: every song in Bellbeat is our own,
//! composed for the game, and the synthesiser in `synth.rs` plays it.
//!
//! A song is a list of sections played in an order. Each section has:
//!
//! - `chords`: one chord a bar ("Am F C G"), cycled to fill the section;
//!   the pad holds it, and the bass and arpeggio take their notes from it.
//! - `lead`: the melody, one token per sixteenth: a note (`E5`, `F#4`,
//!   `Bb5`), `-` to hold the note before, `.` for silence.
//! - `bass` and `arp`: one token per sixteenth in chord degrees (`1` root,
//!   `3` third, `5` fifth, `7` seventh, `8` octave), `-` to hold, `.` rest.
//! - `kick`, `snare`, `hat`: `x` a hit, `.` nothing, one character per
//!   sixteenth.
//!
//! Every pattern repeats until it fills the section, so a one-bar drum beat
//! covers eight bars. `charts.rs` turns the melody (and on Hard the snare)
//! into the notes you play.

/// One sixteenth note is a step; sixteen make a bar.
pub const BAR: usize = 16;

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

pub struct Section {
    pub bars: usize,
    pub chords: &'static str,
    pub lead: &'static str,
    pub bass: &'static str,
    pub arp: &'static str,
    pub kick: &'static str,
    pub snare: &'static str,
    pub hat: &'static str,
}

pub struct SongDef {
    pub id: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub bpm: f32,
    pub lead: Lead,
    pub sections: &'static [Section],
    pub order: &'static [usize],
}

// ------------------------------------------------------------------ songs

const HELLO: SongDef = SongDef {
    id: "hello-world",
    title: "Hello, World",
    blurb: "The first thing every program says, rung out on the bell.",
    bpm: 104.0,
    lead: Lead::Bell,
    sections: &[
        // 0: intro, the band alone
        Section {
            bars: 2,
            chords: "C G",
            lead: ".",
            bass: "1 - - - . . . . 5 - - - . . . .",
            arp: ".",
            kick: "x.......x.......",
            snare: "................",
            hat: "..x...x...x...x.",
        },
        // 1: verse
        Section {
            bars: 8,
            chords: "C G Am F",
            lead: "E5 - - - G5 - - - C6 - - - - - - - \
                   B5 - - - G5 - - - D5 - - - - - - - \
                   C6 - - - B5 - - - A5 - - - E5 - - - \
                   F5 - - - E5 - - - D5 - - - - - - - \
                   E5 - G5 - C6 - - - D6 - C6 - B5 - - - \
                   D6 - - - B5 - - - G5 - - - - - - - \
                   A5 - B5 - C6 - - - E6 - - - D6 - C6 - \
                   C6 - - - - - - - . . . . . . . .",
            bass: "1 - - - . . 1 - 5 - - - . . 5 -",
            arp: ".",
            kick: "x.......x.......",
            snare: "....x.......x...",
            hat: "..x...x...x...x.",
        },
        // 2: chorus
        Section {
            bars: 8,
            chords: "F G Em Am F G C C",
            lead: "A5 - - - A5 - G5 - F5 - - - A5 - - - \
                   G5 - - - - - - - D5 - - - G5 - - - \
                   G5 - - - G5 - F5 - E5 - - - G5 - - - \
                   A5 - - - - - - - E5 - - - - - - - \
                   F5 - A5 - C6 - - - A5 - C6 - F6 - - - \
                   D6 - - - - - - - B5 - - - G5 - - - \
                   C6 - - - G5 - - - E5 - - - G5 - - - \
                   C6 - - - - - - - - - - - . . . .",
            bass: "1 - . 1 5 - . 5 1 - . 1 5 - 8 -",
            arp: "1 . 3 . 5 . 8 . 5 . 3 . 5 . 8 .",
            kick: "x.......x.......",
            snare: "....x.......x...",
            hat: "x.x.x.x.x.x.x.x.",
        },
        // 3: outro
        Section {
            bars: 2,
            chords: "F C",
            lead: "A5 - - - G5 - - - F5 - - - - - - - \
                   E5 - - - - - - - - - - - - - - -",
            bass: "1 - - - - - - - 5 - - - - - - -",
            arp: ".",
            kick: "x...............",
            snare: "................",
            hat: "................",
        },
    ],
    order: &[0, 1, 2, 1, 3],
};

const CARRIAGE: SongDef = SongDef {
    id: "carriage-return",
    title: "Carriage Return",
    blurb: "Back to the start of the line, and swing on down it again.",
    bpm: 116.0,
    lead: Lead::Pulse,
    sections: &[
        Section {
            bars: 2,
            chords: "Am G",
            lead: ".",
            bass: "1 - . 1 . . 8 - 1 - . 1 . . 5 -",
            arp: ".",
            kick: "x.....x...x.....",
            snare: "....x.......x...",
            hat: "x.x.x.x.x.x.x.x.",
        },
        Section {
            bars: 8,
            chords: "Am Am F G",
            lead: "A4 - . C5 E5 - . A5 - - G5 - E5 - . . \
                   . . E5 - G5 - A5 - C6 - - - A5 - . . \
                   F5 - . A5 C6 - . A5 G5 - F5 - E5 - . . \
                   D5 - - - G5 - B5 - D6 - - - B5 - G5 - \
                   A4 - . C5 E5 - . A5 - - G5 - E5 - . . \
                   . . E5 - G5 - A5 - E6 - - - C6 - . . \
                   A5 - . C6 F6 - . C6 A5 - F5 - C5 - . . \
                   B4 - D5 - G5 - - - B5 - - - - - . .",
            bass: "1 - . 1 . . 8 - 1 - . 1 . . 5 -",
            arp: ".",
            kick: "x.....x...x.....",
            snare: "....x.......x...",
            hat: "x.x.x.x.x.x.x.x.",
        },
        Section {
            bars: 8,
            chords: "Dm Am E Am",
            lead: "D5 - F5 - A5 - - - F5 - D5 - A4 - - - \
                   C5 - E5 - A5 - - - E5 - C5 - A4 - - - \
                   B4 - . E5 G#5 - . B5 - - G#5 - E5 - . . \
                   A5 - - - - - - - . . E5 - A5 - . . \
                   F5 - A5 - D6 - - - C6 - A5 - F5 - - - \
                   E5 - A5 - C6 - - - B5 - A5 - E5 - - - \
                   G#5 - B5 - E6 - D6 - B5 - G#5 - E5 - D5 - \
                   C5 - - - A4 - - - - - - - . . . .",
            bass: "1 - . 5 8 - . 5 1 - . 5 8 - 5 -",
            arp: "1 3 5 3 1 3 5 3 1 3 5 3 1 3 5 3",
            kick: "x.....x...x.....",
            snare: "....x.......x...",
            hat: "x.xxx.x.x.xxx.x.",
        },
        Section {
            bars: 2,
            chords: "Am Am",
            lead: "E5 - . C5 A4 - - - . . . . . . . . \
                   A5 - - - - - - - - - - - . . . .",
            bass: "1 - - - - - - - . . . . . . . .",
            arp: ".",
            kick: "x...........x...",
            snare: "................",
            hat: "x...x...x...x...",
        },
    ],
    order: &[0, 1, 2, 1, 2, 3],
};

const SAMBA: SongDef = SongDef {
    id: "null-pointer-samba",
    title: "Null Pointer Samba",
    blurb: "Points at nothing, dances anyway.",
    bpm: 126.0,
    lead: Lead::Pluck,
    sections: &[
        Section {
            bars: 2,
            chords: "Dm7 G7",
            lead: ".",
            bass: "1 - . 5 . . 1 - 1 - . 5 . . 1 .",
            arp: ".",
            kick: "x..x..x.x..x..x.",
            snare: "...x..x....x..x.",
            hat: "xxxxxxxxxxxxxxxx",
        },
        Section {
            bars: 8,
            chords: "Dm7 G7 Dm7 G7 Bbmaj7 A7 Dm7 Dm7",
            lead: "D5 . F5 . A5 - . C6 - . A5 . G5 - . . \
                   . . F5 - G5 . B5 - . D6 - - B5 . . . \
                   A5 . C6 . D6 - . C6 - . A5 . F5 - . . \
                   G5 - . F5 - . D5 - . . F5 . G5 - - - \
                   D6 - . C6 - . A5 - . F5 - . D5 - . . \
                   C#6 - . A5 - . E5 - . G5 - . A5 - - - \
                   F5 . A5 . D6 - . A5 . F5 . D5 - - . . \
                   D5 - - - . . A4 . D5 . . . . . . .",
            bass: "1 - . 5 . . 1 - 1 - . 5 . . 1 .",
            arp: ".",
            kick: "x..x..x.x..x..x.",
            snare: "...x..x....x..x.",
            hat: "x.xxx.xxx.xxx.xx",
        },
        Section {
            bars: 8,
            chords: "Gm7 C7 F F Bb A7 Dm Dm",
            lead: "G5 - . A#5 D6 - . F6 - - D6 . A#5 - . . \
                   C6 - . E5 G5 - . A#5 - - G5 . E5 - . . \
                   F5 . A5 . C6 . F6 - - - C6 . A5 - . . \
                   . . C6 . A5 . F5 . C5 - - - . . . . \
                   D6 - . D6 - . C6 - A#5 - . D6 - - . . \
                   C#6 - . C#6 - . E6 - D6 - . C#6 - - . . \
                   D6 . A5 . F5 . A5 . D6 . A5 . F5 . D5 . \
                   D5 - - - - - - - . . . . . . . .",
            bass: "1 - . 5 . . 1 - 1 - . 5 . . 1 .",
            arp: "1 . 3 5 . 7 . 5 3 . 1 . 5 . 7 .",
            kick: "x..x..x.x..x..x.",
            snare: "...x..x....x..x.",
            hat: "xxxxxxxxxxxxxxxx",
        },
        Section {
            bars: 2,
            chords: "Dm7 Dm7",
            lead: "A5 . F5 . D5 - - - . . . . . . . . \
                   D6 - - - - - - - - - - - . . . .",
            bass: "1 - . 5 . . 1 - 1 - - - - - - -",
            arp: ".",
            kick: "x..x..x.x.......",
            snare: "...x..x.........",
            hat: "x.x.x.x.x.x.....",
        },
    ],
    order: &[0, 1, 2, 1, 3],
};

const PANIC: SongDef = SongDef {
    id: "kernel-panic",
    title: "Kernel Panic",
    blurb: "Everything stops at once. Except you.",
    bpm: 148.0,
    lead: Lead::Pulse,
    sections: &[
        Section {
            bars: 4,
            chords: "Em C D B",
            lead: ".",
            bass: "1 1 8 1 1 1 8 1 1 1 8 1 1 1 8 1",
            arp: "1 5 8 5 1 5 8 5 1 5 8 5 1 5 8 5",
            kick: "x...x...x...x...",
            snare: "................",
            hat: "..x...x...x...x.",
        },
        Section {
            bars: 8,
            chords: "Em C D B",
            lead: "E5 G5 B5 E6 . B5 G5 E5 B5 . G5 . E6 - D6 - \
                   C6 . E6 . G5 . C6 . E6 - D6 - C6 - B5 - \
                   D6 . A5 . F#5 . A5 . D6 - E6 - F#6 - D6 - \
                   D#6 - - - B5 - - - F#5 . B5 . D#6 . F#6 . \
                   E6 . D6 . B5 . G5 . E5 G5 B5 . E6 - - - \
                   E6 . D6 . C6 . G5 . E5 . G5 . C6 - - - \
                   F#6 . E6 . D6 . A5 . F#5 . A5 . D6 . E6 . \
                   F#6 - - - D#6 - - - B5 - - - . . . .",
            bass: "1 1 8 1 1 1 8 1 1 1 8 1 1 1 8 1",
            arp: ".",
            kick: "x...x...x...x...",
            snare: "....x.......x...",
            hat: "x.xxx.xxx.xxx.xx",
        },
        Section {
            bars: 8,
            chords: "Am F C G Am F C B",
            lead: "A5 - - - C6 - - - E6 - - - D6 - C6 - \
                   C6 - - - A5 - - - F5 - - - A5 - - - \
                   G5 - - - C6 - - - E6 - - - G6 - - - \
                   D6 - - - B5 - - - G5 - A5 - B5 - D6 - \
                   E6 . E6 . D6 . C6 . A5 . C6 . E6 - - - \
                   F6 . E6 . C6 . A5 . F5 . A5 . C6 - - - \
                   G6 . E6 . C6 . G5 . E6 . C6 . G5 . E5 . \
                   D#6 - - - F#6 - - - B5 - - - - - - -",
            bass: "1 - - - 1 - - - 1 - - - 5 - 8 -",
            arp: "1 5 8 5 3 5 8 5 1 5 8 5 3 5 8 5",
            kick: "x.......x.......",
            snare: "....x.......x...",
            hat: "x.x.x.x.x.x.x.x.",
        },
        Section {
            bars: 2,
            chords: "Em Em",
            lead: "E6 . B5 . G5 . E5 . B4 . E5 . G5 . B5 . \
                   E6 - - - - - - - - - - - . . . .",
            bass: "1 1 8 1 1 1 8 1 1 - - - - - - -",
            arp: ".",
            kick: "x...x...x...x...",
            snare: "....x.......x...",
            hat: "x.x.x.x.........",
        },
    ],
    order: &[0, 1, 2, 1, 2, 3],
};

const LULLABY: SongDef = SongDef {
    id: "lullaby-for-a-daemon",
    title: "Lullaby for a Daemon",
    blurb: "It never sleeps, so sing it something soft while it works.",
    bpm: 84.0,
    lead: Lead::Flute,
    sections: &[
        Section {
            bars: 2,
            chords: "F C",
            lead: ".",
            bass: "1 - - - - - - - 5 - - - - - - -",
            arp: "1 . 3 . 5 . 8 . 5 . 3 . 5 . 3 .",
            kick: "................",
            snare: "................",
            hat: "................",
        },
        Section {
            bars: 8,
            chords: "F Dm Bb C",
            lead: "A5 - - - - - - - C6 - - - A5 - - - \
                   F5 - - - - - - - D5 - - - - - - - \
                   D5 - - - F5 - - - A#5 - - - A5 - G5 - \
                   G5 - - - - - - - - - - - . . . . \
                   A5 - - - - - - - C6 - - - F6 - - - \
                   E6 - - - D6 - - - A5 - - - - - - - \
                   A#5 - - - A5 - - - G5 - - - F5 - - - \
                   E5 - - - - - - - G5 - - - - - - -",
            bass: "1 - - - - - - - 5 - - - - - - -",
            arp: "1 . 3 . 5 . 8 . 5 . 3 . 5 . 3 .",
            kick: "x...............",
            snare: "................",
            hat: "....x.......x...",
        },
        Section {
            bars: 8,
            chords: "Gm C F Dm Bb C F F",
            lead: "A#5 - - - A5 - - - G5 - - - D5 - - - \
                   E5 - - - G5 - - - C6 - - - - - - - \
                   C6 - - - A5 - - - F5 - - - A5 - - - \
                   D6 - - - - - - - A5 - - - - - - - \
                   D6 - - - C6 - - - A#5 - - - D6 - - - \
                   C6 - - - A#5 - - - G5 - - - E5 - - - \
                   F5 - - - - - - - - - - - - - - - \
                   . . . . . . . . . . . . . . . .",
            bass: "1 - - - - - - - 5 - - - 3 - - -",
            arp: "1 . 3 . 5 . 8 . 5 . 3 . 5 . 3 .",
            kick: "x.......x.......",
            snare: "................",
            hat: "....x.......x...",
        },
        Section {
            bars: 2,
            chords: "Bb F",
            lead: "D6 - - - C6 - - - A5 - - - - - - - \
                   F5 - - - - - - - - - - - - - - -",
            bass: "1 - - - - - - - 1 - - - - - - -",
            arp: "1 . 3 . 5 . 8 . . . . . . . . .",
            kick: "................",
            snare: "................",
            hat: "................",
        },
    ],
    order: &[0, 1, 2, 1, 3],
};

/// In order of difficulty, gentlest first.
pub const SONGS: [&SongDef; 5] = [&LULLABY, &HELLO, &CARRIAGE, &SAMBA, &PANIC];

// ------------------------------------------------------------------ parsing

/// A note name like `C5`, `F#4` or `Bb3` as a MIDI note number.
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
    let rest: String = chars.collect();
    let (shift, octave) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let octave: i32 = octave.parse().ok()?;
    Some(12 * (octave + 1) + base + shift)
}

/// A chord name like `Am`, `G7`, `Bbmaj7` or `C#m7`: its root (MIDI, in
/// octave 3) and its notes as intervals from the root.
pub fn parse_chord(s: &str) -> Option<(i32, Vec<i32>)> {
    let split = if s.len() > 1 && matches!(&s[1..2], "#" | "b") { 2 } else { 1 };
    let root = parse_note(&format!("{}3", &s[..split]))?;
    let quality = &s[split..];
    let iv = match quality {
        "" => vec![0, 4, 7],
        "m" => vec![0, 3, 7],
        "7" => vec![0, 4, 7, 10],
        "m7" => vec![0, 3, 7, 10],
        "maj7" => vec![0, 4, 7, 11],
        "sus2" => vec![0, 2, 7],
        "sus4" => vec![0, 5, 7],
        _ => return None,
    };
    Some((root, iv))
}

/// What a degree token means in a chord: semitones above the root.
fn degree(tok: &str, iv: &[i32]) -> Option<i32> {
    Some(match tok {
        "1" => 0,
        "3" => iv[1],
        "5" => iv[2],
        "7" => *iv.get(3).unwrap_or(&10),
        "8" => 12,
        _ => return None,
    })
}

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

fn tokens(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

/// A melodic line: tokens per step, cycled over `len` steps from `at`.
fn line(toks: &[&str], at: usize, len: usize, mut pitch: impl FnMut(usize, &str) -> Option<i32>, out: &mut Vec<Tone>) {
    if toks.is_empty() || toks == ["."] {
        return;
    }
    let mut open: Option<Tone> = None;
    for i in 0..len {
        let tok = toks[i % toks.len()];
        match tok {
            "-" => {
                if let Some(t) = &mut open {
                    t.len += 1;
                }
            }
            "." => {
                if let Some(t) = open.take() {
                    out.push(t);
                }
            }
            _ => {
                if let Some(t) = open.take() {
                    out.push(t);
                }
                if let Some(m) = pitch(at + i, tok) {
                    open = Some(Tone { step: at + i, len: 1, midi: m });
                }
            }
        }
    }
    if let Some(t) = open {
        out.push(t);
    }
}

impl SongDef {
    pub fn score(&self) -> Score {
        let mut sc = Score::default();
        let mut at = 0;
        for &si in self.order {
            let s = &self.sections[si];
            let len = s.bars * BAR;
            sc.sections.push(at);
            let chords: Vec<(i32, Vec<i32>)> = tokens(s.chords).iter().map(|c| parse_chord(c).unwrap_or((48, vec![0, 4, 7]))).collect();
            let chord_at = |step: usize| &chords[((step - at) / BAR) % chords.len()];
            for b in 0..s.bars {
                let (r, iv) = chord_at(at + b * BAR);
                sc.chords.push((at + b * BAR, *r, iv.clone()));
            }
            line(&tokens(s.lead), at, len, |_, t| parse_note(t), &mut sc.lead);
            line(&tokens(s.bass), at, len, |st, t| {
                let (r, iv) = chord_at(st);
                degree(t, iv).map(|d| r - 12 + d)
            }, &mut sc.bass);
            line(&tokens(s.arp), at, len, |st, t| {
                let (r, iv) = chord_at(st);
                degree(t, iv).map(|d| r + 24 + d)
            }, &mut sc.arp);
            for (pat, out) in [(s.kick, &mut sc.kick), (s.snare, &mut sc.snare), (s.hat, &mut sc.hat)] {
                let p: Vec<char> = pat.chars().filter(|c| !c.is_whitespace()).collect();
                if p.is_empty() {
                    continue;
                }
                for i in 0..len {
                    if p[i % p.len()] == 'x' {
                        out.push(at + i);
                    }
                }
            }
            at += len;
        }
        sc.steps = at;
        sc
    }

    /// Seconds per step.
    pub fn step_secs(&self) -> f64 {
        60.0 / self.bpm as f64 / 4.0
    }

    pub fn secs(&self) -> f64 {
        self.score().steps as f64 * self.step_secs()
    }
}

pub fn find(id: &str) -> Option<usize> {
    SONGS.iter().position(|s| s.id == id)
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
        assert_eq!(parse_chord("Am"), Some((57, vec![0, 3, 7])));
        assert_eq!(parse_chord("Bbmaj7"), Some((58, vec![0, 4, 7, 11])));
        assert_eq!(parse_chord("C#m7"), Some((49, vec![0, 3, 7, 10])));
    }

    #[test]
    fn every_song_is_well_formed() {
        for s in SONGS {
            for (i, sec) in s.sections.iter().enumerate() {
                let len = sec.bars * BAR;
                for c in tokens(sec.chords) {
                    assert!(parse_chord(c).is_some(), "{} section {i}: chord {c}", s.id);
                }
                // a written-out line must fill its section exactly, or be one
                // bar that repeats
                for (name, l) in [("lead", sec.lead), ("bass", sec.bass), ("arp", sec.arp)] {
                    let t = tokens(l);
                    if t == ["."] {
                        continue;
                    }
                    assert!(t.len() == len || t.len() == BAR, "{} section {i} {name}: {} tokens for {len} steps", s.id, t.len());
                    for tok in t {
                        let ok = matches!(tok, "-" | "." | "1" | "3" | "5" | "7" | "8") || parse_note(tok).is_some();
                        assert!(ok, "{} section {i} {name}: token {tok}", s.id);
                    }
                }
                for (name, d) in [("kick", sec.kick), ("snare", sec.snare), ("hat", sec.hat)] {
                    assert_eq!(d.len(), BAR, "{} section {i} {name}", s.id);
                }
            }
            for &o in s.order {
                assert!(o < s.sections.len());
            }
            let sc = s.score();
            assert!(sc.lead.len() > 40, "{} has only {} melody notes", s.id, sc.lead.len());
            assert!((40.0..150.0).contains(&s.secs()), "{} lasts {:.0} s", s.id, s.secs());
            // the melody sits where the lanes and the bell sound good
            assert!(sc.lead.iter().all(|t| (55..=96).contains(&t.midi)), "{}", s.id);
        }
    }

    #[test]
    fn holds_and_rests_make_lengths() {
        let mut out = Vec::new();
        line(&tokens("C5 - - . D5 . E5 -"), 0, 8, |_, t| parse_note(t), &mut out);
        assert_eq!(out, vec![Tone { step: 0, len: 3, midi: 72 }, Tone { step: 4, len: 1, midi: 74 }, Tone { step: 6, len: 2, midi: 76 }]);
    }
}
