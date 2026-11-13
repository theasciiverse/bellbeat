//! One song being played: judging presses against the chart, the combo,
//! the score and the groove meter.
//!
//! Every time here is song time in seconds, already moved by the player's
//! latency offset, so a note is on time when its `t` is.

use crate::chart::{self, Chart, Difficulty, Note, Sound, LANES};
use crate::song::{Score, SongDef};

/// How far off a press may be and still count.
pub const PERFECT: f64 = 0.045;
pub const GOOD: f64 = 0.095;
/// A press this early takes the note as a miss (pressed far too soon).
pub const EARLY_MISS: f64 = 0.14;
/// A hold let go this close to its end still counts as held through.
const HOLD_GRACE: f64 = 0.15;
/// Seconds before the first note.
pub const LEAD_IN: f64 = 2.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Judge {
    Perfect,
    Good,
    Miss,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Hold {
    #[default]
    None,
    /// Held down now.
    Holding,
    Done,
    Dropped,
}

/// What the synth should do, collected each update.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cue {
    Hit(Sound, bool),
    Release(Sound),
    Thud,
}

#[derive(Clone, Copy, Debug)]
pub struct Spark {
    pub lane: usize,
    pub x: f32,
    pub vy: f32,
    pub born: f64,
}

pub struct Play {
    pub song: usize,
    pub def: &'static SongDef,
    pub diff: Difficulty,
    pub score_sheet: Score,
    pub chart: Chart,
    pub judged: Vec<Option<(Judge, f64)>>,
    pub hold: Vec<Hold>,
    /// Notes before this are all judged.
    cursor: usize,
    pub combo: u32,
    pub max_combo: u32,
    pub points: f64,
    pub counts: [u32; 3],
    /// 0..1; at 0 the song is lost.
    pub groove: f64,
    pub failed: bool,
    pub finished: bool,
    /// When each lane was last pressed, and whether it is down.
    pub pressed_at: [f64; LANES],
    pub down: [bool; LANES],
    /// The last judgement and when, for the big word over the lanes.
    pub last: Option<(Judge, f64, f64)>,
    pub sparks: Vec<Spark>,
    /// Lanes glitching after a miss, until when.
    pub glitch: [f64; LANES],
    pub cues: Vec<Cue>,
    pub demo: bool,
    pub now: f64,
    seed: u32,
    /// The song's length (last note's end, or the music's end).
    pub length: f64,
}

impl Play {
    pub fn new(song: usize, def: &'static SongDef, diff: Difficulty, music_end: f64) -> Play {
        let score_sheet = def.score();
        let chart = chart::build(def, &score_sheet, diff);
        let n = chart.notes.len();
        let last = chart.notes.iter().map(|n| n.t + n.hold).fold(0.0, f64::max);
        Play {
            song,
            def,
            diff,
            score_sheet,
            chart,
            judged: vec![None; n],
            hold: vec![Hold::None; n],
            cursor: 0,
            combo: 0,
            max_combo: 0,
            points: 0.0,
            counts: [0; 3],
            groove: 0.6,
            failed: false,
            finished: false,
            pressed_at: [-99.0; LANES],
            down: [false; LANES],
            last: None,
            sparks: Vec::new(),
            glitch: [-99.0; LANES],
            cues: Vec::new(),
            demo: false,
            now: -LEAD_IN,
            seed: 0x9e37_79b9,
            length: last.max(music_end),
        }
    }

    pub fn multiplier(&self) -> u32 {
        1 + (self.combo / 10).min(3)
    }

    pub fn notes(&self) -> &[Note] {
        &self.chart.notes
    }

    /// Share of the notes so far hit, perfect counting full and good half.
    pub fn accuracy(&self) -> f64 {
        let n = self.counts.iter().sum::<u32>();
        if n == 0 {
            return 1.0;
        }
        (self.counts[0] as f64 + 0.5 * self.counts[1] as f64) / n as f64
    }

    pub fn grade(&self) -> &'static str {
        if self.failed {
            return "F";
        }
        let a = self.accuracy();
        if a >= 0.95 {
            "S"
        } else if a >= 0.9 {
            "A"
        } else if a >= 0.8 {
            "B"
        } else if a >= 0.7 {
            "C"
        } else {
            "D"
        }
    }

    pub fn full_combo(&self) -> bool {
        !self.failed && self.counts[2] == 0 && self.counts[0] + self.counts[1] > 0
    }

    pub fn score(&self) -> u64 {
        self.points.round() as u64
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32
    }

    fn judge(&mut self, i: usize, j: Judge, at: f64) {
        self.judged[i] = Some((j, at));
        self.counts[j as usize] += 1;
        let lane = self.chart.notes[i].lane;
        let (loss, gain) = match self.diff {
            Difficulty::Easy => (0.05, 0.04),
            Difficulty::Normal => (0.07, 0.03),
            Difficulty::Hard => (0.09, 0.025),
        };
        match j {
            Judge::Miss => {
                self.combo = 0;
                self.groove = (self.groove - loss).max(0.0);
                self.glitch[lane] = at + 0.3;
                self.cues.push(Cue::Thud);
                log::debug!("miss note {i} lane {lane} at {at:.3}");
            }
            Judge::Perfect | Judge::Good => {
                self.combo += 1;
                self.max_combo = self.max_combo.max(self.combo);
                let base = if j == Judge::Perfect { 1000.0 } else { 500.0 };
                self.points += base * self.multiplier() as f64;
                self.groove = (self.groove + if j == Judge::Perfect { gain } else { gain / 2.0 }).min(1.0);
                for _ in 0..if j == Judge::Perfect { 6 } else { 3 } {
                    let s = Spark { lane, x: self.rand(), vy: 8.0 + self.rand() * 14.0, born: at };
                    self.sparks.push(s);
                }
            }
        }
        self.last = Some((j, at, at - self.chart.notes[i].t));
        if self.groove <= 0.0 && !self.failed {
            self.failed = true;
            log::info!("groove ran out at {at:.2}");
        }
    }

    /// A lane went down at song time `t`. `exact` says whether its release
    /// will be reported (a terminal with key releases, or a gamepad).
    pub fn press(&mut self, lane: usize, t: f64, exact: bool) {
        if lane >= LANES || self.failed || self.finished {
            return;
        }
        self.down[lane] = true;
        self.pressed_at[lane] = t;
        // the nearest unjudged note in this lane within reach
        let mut best: Option<(usize, f64)> = None;
        for i in self.cursor..self.chart.notes.len() {
            let n = self.chart.notes[i];
            if n.t - t > EARLY_MISS {
                break;
            }
            if n.lane != lane || self.judged[i].is_some() {
                continue;
            }
            let d = t - n.t;
            if d.abs() <= EARLY_MISS && best.is_none_or(|(_, bd)| d.abs() < bd.abs()) {
                best = Some((i, d));
            }
        }
        let Some((i, d)) = best else {
            log::debug!("press lane {lane} at {t:.3}: nothing there");
            return;
        };
        let j = if d.abs() <= PERFECT {
            Judge::Perfect
        } else if d.abs() <= GOOD {
            Judge::Good
        } else {
            Judge::Miss
        };
        log::debug!("press lane {lane} at {t:.3}: note {i} off by {:+.0} ms, {j:?}", d * 1000.0);
        self.judge(i, j, t);
        if j != Judge::Miss {
            let n = self.chart.notes[i];
            if n.hold > 0.0 {
                // without key releases a hold cannot be let go of, so it holds itself
                self.hold[i] = Hold::Holding;
                self.cues.push(Cue::Hit(n.sound, exact));
                if !exact {
                    self.hold[i] = Hold::Done;
                    self.points += n.hold * 400.0 * self.multiplier() as f64;
                }
            } else {
                self.cues.push(Cue::Hit(n.sound, false));
            }
        }
    }

    /// A lane came up at song time `t`.
    pub fn release(&mut self, lane: usize, t: f64) {
        if lane >= LANES {
            return;
        }
        self.down[lane] = false;
        for i in 0..self.chart.notes.len() {
            if self.hold[i] != Hold::Holding || self.chart.notes[i].lane != lane {
                continue;
            }
            let n = self.chart.notes[i];
            let end = n.t + n.hold;
            self.hold[i] = if t >= end - HOLD_GRACE { Hold::Done } else { Hold::Dropped };
            log::debug!("release lane {lane} at {t:.3}: hold {i} {:?}", self.hold[i]);
            self.cues.push(Cue::Release(n.sound));
        }
    }

    /// Move the song on to `t`.
    pub fn update(&mut self, t: f64) {
        let dt = (t - self.now).clamp(0.0, 0.25);
        self.now = t;
        if self.demo {
            self.autoplay(t);
        }
        // notes gone past without a press
        while self.cursor < self.chart.notes.len() {
            let i = self.cursor;
            let n = self.chart.notes[i];
            if self.judged[i].is_some() {
                self.cursor += 1;
                continue;
            }
            if t - n.t > GOOD {
                self.judge(i, Judge::Miss, t);
                self.cursor += 1;
            } else {
                break;
            }
        }
        // holds: points while held, done at the end
        let mult = self.multiplier() as f64;
        for i in 0..self.chart.notes.len() {
            if self.hold[i] != Hold::Holding {
                continue;
            }
            let n = self.chart.notes[i];
            let end = n.t + n.hold;
            self.points += dt.min((end - (t - dt)).max(0.0)) * 400.0 * mult;
            if t >= end {
                self.hold[i] = Hold::Done;
                self.cues.push(Cue::Release(n.sound));
            }
        }
        self.sparks.retain(|s| t - s.born < 0.6);
        if !self.finished && (self.failed || t > self.length + 1.5) {
            self.finished = true;
            log::info!("song over: score {} grade {} combo {} counts {:?}", self.score(), self.grade(), self.max_combo, self.counts);
        }
    }

    /// The demo plays by itself: right on time, now and then a touch late.
    fn autoplay(&mut self, t: f64) {
        for i in self.cursor..self.chart.notes.len() {
            let n = self.chart.notes[i];
            if n.t > t {
                break;
            }
            if self.judged[i].is_none() {
                let late = if i % 9 == 4 { 0.06 } else { 0.0 };
                if t >= n.t + late {
                    self.press(n.lane, n.t + late, true);
                }
            }
        }
        for i in 0..self.chart.notes.len() {
            let n = self.chart.notes[i];
            if self.hold[i] == Hold::Holding && t >= n.t + n.hold {
                self.release(n.lane, t);
            }
        }
        for lane in 0..LANES {
            if self.down[lane] && t - self.pressed_at[lane] > 0.08 && !self.hold.iter().zip(&self.chart.notes).any(|(h, n)| *h == Hold::Holding && n.lane == lane) {
                self.down[lane] = false;
            }
        }
    }

    /// 0..1 through the song.
    pub fn progress(&self) -> f64 {
        (self.now / self.length).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::SONGS;

    fn play(song: usize, d: Difficulty) -> Play {
        let def = SONGS[song];
        let end = def.secs();
        Play::new(song, def, d, end)
    }

    #[test]
    fn perfect_good_and_miss() {
        let mut p = play(1, Difficulty::Normal);
        let n0 = p.notes()[0];
        p.update(n0.t - 0.5);
        p.press(n0.lane, n0.t + 0.02, true);
        assert_eq!(p.judged[0].map(|j| j.0), Some(Judge::Perfect));
        assert_eq!(p.combo, 1);
        let n1 = p.notes()[1];
        p.press(n1.lane, n1.t - 0.07, true);
        assert_eq!(p.judged[1].map(|j| j.0), Some(Judge::Good));
        // the third goes by untouched
        let n2 = p.notes()[2];
        p.update(n2.t + 0.2);
        assert_eq!(p.judged[2].map(|j| j.0), Some(Judge::Miss));
        assert_eq!(p.combo, 0);
        assert_eq!(p.counts, [1, 1, 1]);
    }

    #[test]
    fn a_press_in_the_wrong_lane_or_too_soon_does_nothing() {
        let mut p = play(1, Difficulty::Normal);
        let n0 = p.notes()[0];
        p.press((n0.lane + 1) % LANES, n0.t, true);
        p.press(n0.lane, n0.t - 0.5, true);
        assert!(p.judged[0].is_none());
        // too early but close: taken as a miss
        p.press(n0.lane, n0.t - 0.12, true);
        assert_eq!(p.judged[0].map(|j| j.0), Some(Judge::Miss));
    }

    #[test]
    fn holds_count_only_when_held_through() {
        for d in [Difficulty::Easy, Difficulty::Normal] {
            let mut p = play(0, d);
            let Some(i) = p.notes().iter().position(|n| n.hold > 0.5) else { panic!("no hold") };
            let n = p.notes()[i];
            p.update(n.t - 0.01);
            p.press(n.lane, n.t, true);
            assert_eq!(p.hold[i], Hold::Holding);
            p.update(n.t + n.hold * 0.3);
            p.release(n.lane, n.t + n.hold * 0.3);
            assert_eq!(p.hold[i], Hold::Dropped);
        }
        // without key releases a hold holds itself
        let mut p = play(0, Difficulty::Easy);
        let i = p.notes().iter().position(|n| n.hold > 0.5).unwrap();
        let n = p.notes()[i];
        p.press(n.lane, n.t, false);
        assert_eq!(p.hold[i], Hold::Done);
    }

    #[test]
    fn the_demo_plays_every_song_through() {
        for (s, def) in SONGS.iter().enumerate() {
            for d in Difficulty::ALL {
                let mut p = play(s, d);
                p.demo = true;
                let mut t = -LEAD_IN;
                while !p.finished && t < def.secs() + 10.0 {
                    t += 1.0 / 60.0;
                    p.update(t);
                }
                assert!(p.finished && !p.failed, "{} {d:?}", def.id);
                assert_eq!(p.counts[2], 0, "{} {d:?} missed {:?}", def.id, p.counts);
                assert!(p.full_combo());
                assert!(matches!(p.grade(), "S" | "A"), "{} {d:?} grade {}", def.id, p.grade());
                assert!(p.hold.iter().zip(p.notes()).all(|(h, n)| n.hold == 0.0 || *h == Hold::Done), "{} {d:?}", def.id);
            }
        }
    }

    #[test]
    fn missing_everything_loses_the_song() {
        let mut p = play(4, Difficulty::Hard);
        let mut t = -LEAD_IN;
        while !p.finished {
            t += 0.05;
            p.update(t);
        }
        assert!(p.failed);
        assert_eq!(p.grade(), "F");
    }
}
