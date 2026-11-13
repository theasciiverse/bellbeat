//! From a song to the notes you play.
//!
//! The chart is the song's own melody: every note you hit sounds that note
//! of the tune, and a note you miss is a hole in the song. The difficulty
//! chooses how much of the melody is yours; the rest plays itself.
//!
//! - **Easy:** the melody notes that start on a beat.
//! - **Normal:** those that start on an eighth.
//! - **Hard:** all of it, and the snare's backbeat with your other hand.
//!
//! A note's lane follows the tune's shape: higher notes to the right, and a
//! step up or down never stays in the same lane, so the hands move the way
//! the melody does. Melody notes of half a bar or more are held.

use crate::song::{Score, SongDef, Tone};

pub const LANES: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_name(s: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.name().eq_ignore_ascii_case(s))
    }
}

/// What hitting a note plays.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sound {
    /// A melody note, by its index in `Score::lead`.
    Lead(usize),
    Snare,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Note {
    /// Seconds from the song's start.
    pub t: f64,
    pub lane: usize,
    /// For a hold, how long it lasts in seconds; 0 for a tap.
    pub hold: f64,
    pub sound: Sound,
}

/// Steps a melody note must last to be held.
const HOLD_STEPS: usize = 8;

pub struct Chart {
    pub notes: Vec<Note>,
    /// Which melody notes are the player's (the others play themselves).
    pub played: Vec<bool>,
    /// Snare hits the player plays on Hard, by step.
    pub snare_played: Vec<usize>,
}

fn wanted(t: &Tone, d: Difficulty) -> bool {
    match d {
        Difficulty::Easy => t.step % 4 == 0,
        Difficulty::Normal => t.step % 2 == 0,
        Difficulty::Hard => true,
    }
}

/// The lane for each melody note, from the tune's shape.
pub fn lanes(lead: &[Tone]) -> Vec<usize> {
    let (lo, hi) = lead.iter().fold((i32::MAX, i32::MIN), |(lo, hi), t| (lo.min(t.midi), hi.max(t.midi)));
    let span = (hi - lo + 1).max(1) as f32;
    let mut out = Vec::with_capacity(lead.len());
    let mut prev: Option<(i32, usize)> = None;
    for t in lead {
        let mut lane = (((t.midi - lo) as f32 / span) * LANES as f32) as usize;
        lane = lane.min(LANES - 1);
        if let Some((pm, pl)) = prev {
            // a different note never lands in the same lane as the one before,
            // and it moves the way the tune does
            if t.midi > pm && lane <= pl {
                lane = if pl + 1 < LANES { pl + 1 } else { pl.saturating_sub(1) };
            } else if t.midi < pm && lane >= pl {
                lane = if pl > 0 { pl - 1 } else { (pl + 1).min(LANES - 1) };
            } else if t.midi == pm {
                lane = pl;
            }
        }
        prev = Some((t.midi, lane));
        out.push(lane);
    }
    out
}

pub fn build(song: &SongDef, score: &Score, d: Difficulty) -> Chart {
    let step = song.step_secs();
    let lane_of = lanes(&score.lead);
    let mut notes = Vec::new();
    let mut played = vec![false; score.lead.len()];
    for (i, t) in score.lead.iter().enumerate() {
        if !wanted(t, d) {
            continue;
        }
        played[i] = true;
        let hold = if t.len >= HOLD_STEPS { (t.len as f64 - 1.0) * step } else { 0.0 };
        notes.push(Note { t: t.step as f64 * step, lane: lane_of[i], hold, sound: Sound::Lead(i) });
    }
    let mut snare_played = Vec::new();
    if d == Difficulty::Hard {
        for &s in &score.snare {
            // the other hand from the melody's last lane
            let last = score.lead.iter().rposition(|t| t.step <= s).map(|i| lane_of[i]).unwrap_or(0);
            let lane = if last <= 1 { 3 } else { 0 };
            let t = s as f64 * step;
            let busy = notes.iter().any(|n| n.lane == lane && (n.t - t).abs() < step * 1.5 || (n.lane == lane && n.hold > 0.0 && t >= n.t && t <= n.t + n.hold));
            if busy {
                continue;
            }
            snare_played.push(s);
            notes.push(Note { t, lane, hold: 0.0, sound: Sound::Snare });
        }
    }
    notes.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.lane.cmp(&b.lane)));
    Chart { notes, played, snare_played }
}

/// Notes a second, at the busiest eight seconds, for the song list's stars.
pub fn density(chart: &Chart) -> f64 {
    let mut best = 0usize;
    let mut j = 0;
    for i in 0..chart.notes.len() {
        while chart.notes[i].t - chart.notes[j].t > 8.0 {
            j += 1;
        }
        best = best.max(i - j + 1);
    }
    best as f64 / 8.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::SONGS;

    #[test]
    fn charts_grow_with_difficulty() {
        for s in SONGS {
            let sc = s.score();
            let n: Vec<usize> = Difficulty::ALL.iter().map(|&d| build(s, &sc, d).notes.len()).collect();
            assert!(n[0] >= 20, "{} easy has {}", s.id, n[0]);
            assert!(n[0] <= n[1] && n[1] <= n[2], "{} {n:?}", s.id);
            assert!(density(&build(s, &sc, Difficulty::Easy)) < density(&build(s, &sc, Difficulty::Hard)) + 0.01);
        }
    }

    #[test]
    fn notes_are_playable() {
        for s in SONGS {
            let sc = s.score();
            for d in Difficulty::ALL {
                let c = build(s, &sc, d);
                assert!(c.notes.windows(2).all(|w| w[0].t <= w[1].t), "{} {d:?} unsorted", s.id);
                for (i, a) in c.notes.iter().enumerate() {
                    assert!(a.lane < LANES);
                    // nothing lands in a lane while it is still held, and two
                    // taps in one lane are a sixteenth apart at least
                    for b in &c.notes[i + 1..] {
                        if b.t > a.t + a.hold + 1.0 {
                            break;
                        }
                        if b.lane == a.lane {
                            assert!(b.t >= a.t + a.hold + s.step_secs() * 0.99, "{} {d:?}: lane {} at {:.2} and {:.2}", s.id, a.lane, a.t, b.t);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn lanes_follow_the_tune() {
        let tone = |step, midi| Tone { step, len: 1, midi };
        let lead = [tone(0, 60), tone(1, 62), tone(2, 64), tone(3, 72), tone(4, 72), tone(5, 60)];
        let l = lanes(&lead);
        assert_eq!(l[0], 0);
        assert!(l[1] > l[0] && l[2] > l[1]);
        assert_eq!(l[3], l[4]);
        assert!(l[5] < l[4]);
    }
}
