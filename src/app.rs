//! The screens and what moves between them: the song list, playing a
//! song (and its pause menu), the results, and calibration.

use crate::chart::{self, Difficulty, LANES};
use crate::input::{nav, Nav, Press, K};
use crate::play::{Cue, Play, LEAD_IN};
use crate::save::{self, Best, Settings};
use crate::song::SONGS;
use crate::synth::{Audio, Inst};
use std::time::Instant;

/// The song clock: seconds from the song's start, kept in step with the
/// samples the synth has made.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub t: f64,
    at: Instant,
    pub running: bool,
}

impl Clock {
    pub fn new() -> Clock {
        Clock { t: 0.0, at: Instant::now(), running: false }
    }

    pub fn start(&mut self, t: f64, now: Instant) {
        *self = Clock { t, at: now, running: true };
    }

    /// Move on to `now`, leaning towards the synth's count when there is one.
    pub fn tick(&mut self, now: Instant, estimate: Option<f64>) {
        if !self.running {
            self.at = now;
            return;
        }
        let mut t = self.t + now.saturating_duration_since(self.at).as_secs_f64();
        if let Some(e) = estimate {
            let d = e - t;
            if d.abs() > 0.08 {
                t = e;
            } else {
                t += d * 0.08;
            }
        }
        // never backwards
        self.t = t.max(self.t);
        self.at = now;
    }

    /// The clock's reading at `when` (a press read a moment ago).
    pub fn at(&self, when: Instant) -> f64 {
        if when >= self.at {
            self.t + (when - self.at).as_secs_f64()
        } else {
            self.t - (self.at - when).as_secs_f64()
        }
    }
}

/// Calibration: tap along to the clicks.
pub struct Cal {
    pub bpm: f64,
    /// How far each tap was from its click, in seconds.
    pub taps: Vec<f64>,
    pub flash: f64,
    pub done: bool,
}

pub const CAL_TAPS: usize = 16;
const CAL_LEAD_IN: f64 = 2.4;

impl Cal {
    pub fn period(&self) -> f64 {
        60.0 / self.bpm
    }

    /// The middle tap's offset, in whole milliseconds.
    pub fn median_ms(&self) -> Option<i32> {
        if self.taps.is_empty() {
            return None;
        }
        let mut v = self.taps.clone();
        v.sort_by(f64::total_cmp);
        let m = if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
        Some((m * 1000.0).round() as i32)
    }

    pub fn tap(&mut self, t: f64) {
        if self.done || t < -0.3 {
            return;
        }
        let p = self.period();
        let beat = (t / p).round();
        let off = t - beat * p;
        if off.abs() < 0.3 {
            self.taps.push(off);
            log::debug!("calibration tap {:+.0} ms", off * 1000.0);
        }
        if self.taps.len() >= CAL_TAPS {
            self.done = true;
            log::info!("calibration done: median {:?} ms", self.median_ms());
        }
    }
}

pub enum Screen {
    Select,
    Play { play: Box<Play>, paused: bool, item: usize, over_at: Option<f64> },
    Results { play: Box<Play>, new_best: bool, shown: Instant },
    Calibrate(Cal),
}

pub const PAUSE_ITEMS: [&str; 3] = ["Carry on", "Start again", "Back to the songs"];

pub struct App {
    pub screen: Screen,
    pub sel: usize,
    pub diff: Difficulty,
    pub settings: Settings,
    pub bests: Vec<Best>,
    /// Notes a second at the busiest, per song and difficulty.
    pub density: Vec<[f64; 3]>,
    pub clock: Clock,
    pub demo: bool,
    pub quit: bool,
    pub audio: Audio,
    pub exact: bool,
    /// A short message on the song list (a saved setting).
    pub note: Option<(String, Instant)>,
    /// Last judgements, newest last, for the panel's log.
    pub log: Vec<String>,
}

impl App {
    pub fn new(audio: Audio, settings: Settings, bests: Vec<Best>, exact: bool) -> App {
        let density = SONGS
            .iter()
            .map(|s| {
                let sc = s.score();
                let mut d = [0.0; 3];
                for diff in Difficulty::ALL {
                    d[diff.index()] = chart::density(&chart::build(s, &sc, diff));
                }
                d
            })
            .collect();
        App {
            screen: Screen::Select,
            sel: 1,
            diff: Difficulty::Normal,
            settings,
            bests,
            density,
            clock: Clock::new(),
            demo: false,
            quit: false,
            audio,
            exact,
            note: None,
            log: Vec::new(),
        }
    }

    pub fn offset(&self) -> f64 {
        self.settings.offset_ms as f64 / 1000.0
    }

    pub fn best(&self, song: usize, d: Difficulty) -> Option<&Best> {
        self.bests.iter().find(|b| b.song == SONGS[song].id && b.diff == d.name())
    }

    /// Back to the list, with the chosen song playing softly behind it.
    pub fn to_select(&mut self) {
        self.screen = Screen::Select;
        self.clock.running = false;
        self.preview();
    }

    pub fn preview(&mut self) {
        let def = SONGS[self.sel];
        let sc = def.score();
        // from the second section, where the tune comes in
        let from = sc.sections.get(1).copied().unwrap_or(0) as f64 * def.step_secs();
        self.audio.with(|s| {
            s.load(def, &sc, None);
            s.preview(from);
        });
    }

    pub fn start(&mut self, song: usize, diff: Difficulty) {
        let def = SONGS[song];
        let mut play = Play::new(song, def, diff, def.secs());
        play.demo = self.demo;
        self.audio.with(|s| {
            s.load(def, &play.score_sheet, Some(&play.chart));
            s.start(LEAD_IN);
        });
        self.clock.start(-LEAD_IN, Instant::now());
        play.update(-LEAD_IN - self.offset());
        log::info!("play {} {} ({} notes) offset {} ms speed {}", def.id, diff.name(), play.chart.notes.len(), self.settings.offset_ms, self.settings.speed);
        self.log.clear();
        self.screen = Screen::Play { play: Box::new(play), paused: false, item: 0, over_at: None };
    }

    pub fn calibrate(&mut self) {
        let cal = Cal { bpm: 100.0, taps: Vec::new(), flash: -9.0, done: false };
        self.audio.with(|s| s.metronome(cal.bpm, CAL_LEAD_IN));
        self.clock.start(-CAL_LEAD_IN, Instant::now());
        log::info!("calibration start");
        self.screen = Screen::Calibrate(cal);
    }

    fn sfx(&mut self, inst: Inst, midi: f32) {
        self.audio.with(|s| s.sfx(inst, midi, 0.25));
    }

    fn say(&mut self, s: String) {
        self.note = Some((s, Instant::now()));
    }

    /// Every frame: the clock, the song, and the presses since the last.
    pub fn frame(&mut self, presses: Vec<Press>, now: Instant) {
        self.audio.pump();
        let est = self.audio.with(|s| s.estimate(now)).flatten();
        self.clock.tick(now, est);
        for p in presses {
            if p.down {
                log::debug!("press {:?}", p.key);
            }
            if p.key == K::CtrlC {
                self.quit = true;
                return;
            }
            self.on_press(p);
            if self.quit {
                return;
            }
        }
        self.advance(now);
    }

    fn on_press(&mut self, p: Press) {
        let offset = self.offset();
        let t = self.clock.at(p.at) - offset;
        match &mut self.screen {
            Screen::Select => {
                if !p.down {
                    return;
                }
                match p.key {
                    K::Char('c' | 'C') => return self.calibrate(),
                    K::Char('[') => {
                        self.settings.speed = self.settings.speed.saturating_sub(1).max(1);
                        save::save_settings(&self.settings);
                        self.say(format!("speed {}", self.settings.speed));
                    }
                    K::Char(']') => {
                        self.settings.speed = (self.settings.speed + 1).min(10);
                        save::save_settings(&self.settings);
                        self.say(format!("speed {}", self.settings.speed));
                    }
                    K::Char('-' | '_') => {
                        self.settings.offset_ms = (self.settings.offset_ms - 5).max(-300);
                        save::save_settings(&self.settings);
                        self.say(format!("offset {:+} ms", self.settings.offset_ms));
                    }
                    K::Char('+' | '=') => {
                        self.settings.offset_ms = (self.settings.offset_ms + 5).min(300);
                        save::save_settings(&self.settings);
                        self.say(format!("offset {:+} ms", self.settings.offset_ms));
                    }
                    K::Char('m' | 'M') => self.audio.toggle_mute(),
                    K::Char('q' | 'Q') => self.quit = true,
                    k => match nav(k) {
                        Some(Nav::Up) => {
                            self.sel = (self.sel + SONGS.len() - 1) % SONGS.len();
                            self.preview();
                            self.sfx(Inst::Tick, 0.0);
                        }
                        Some(Nav::Down) => {
                            self.sel = (self.sel + 1) % SONGS.len();
                            self.preview();
                            self.sfx(Inst::Tick, 0.0);
                        }
                        Some(Nav::Left) => {
                            self.diff = Difficulty::ALL[(self.diff.index() + 2) % 3];
                            self.sfx(Inst::Tick, 0.0);
                        }
                        Some(Nav::Right) => {
                            self.diff = Difficulty::ALL[(self.diff.index() + 1) % 3];
                            self.sfx(Inst::Tick, 0.0);
                        }
                        Some(Nav::Ok) => {
                            self.sfx(Inst::Chime, 84.0);
                            let (s, d) = (self.sel, self.diff);
                            self.start(s, d);
                        }
                        Some(Nav::Back) | Some(Nav::Quit) => self.quit = true,
                        None => {}
                    },
                }
            }
            Screen::Play { play, paused, item, .. } => {
                if *paused {
                    if !p.down {
                        return;
                    }
                    match nav(p.key) {
                        Some(Nav::Up) => *item = (*item + PAUSE_ITEMS.len() - 1) % PAUSE_ITEMS.len(),
                        Some(Nav::Down) => *item = (*item + 1) % PAUSE_ITEMS.len(),
                        Some(Nav::Back) => self.resume(),
                        Some(Nav::Ok) => match *item {
                            0 => self.resume(),
                            1 => {
                                let (s, d) = (play.song, play.diff);
                                self.audio.with(|s| s.paused = false);
                                self.start(s, d);
                            }
                            _ => {
                                self.audio.with(|s| s.paused = false);
                                self.to_select();
                            }
                        },
                        _ => {}
                    }
                    return;
                }
                let pause = matches!(p.key, K::Esc | K::PadStart | K::PadSelect | K::Char('p' | 'P') | K::Back | K::Tab);
                if pause && p.down {
                    if play.demo {
                        // the demo ends on any of these
                        self.demo = false;
                        self.to_select();
                        return;
                    }
                    *paused = true;
                    *item = 0;
                    self.clock.running = false;
                    self.audio.with(|s| s.paused = true);
                    // let go of everything: releases will come in pause
                    let tt = play.now;
                    for lane in 0..LANES {
                        if play.down[lane] {
                            play.release(lane, tt);
                        }
                    }
                    log::info!("paused at {tt:.2}");
                    return;
                }
                if play.demo {
                    return;
                }
                if p.key == K::Char('m') || p.key == K::Char('M') {
                    if p.down {
                        self.audio.toggle_mute();
                    }
                    return;
                }
                if let Some(lane) = p.lane() {
                    if p.down {
                        play.press(lane, t, p.exact);
                    } else {
                        play.release(lane, t);
                    }
                }
            }
            Screen::Results { play, shown, .. } => {
                if !p.down || shown.elapsed().as_secs_f64() < 0.6 {
                    return;
                }
                match p.key {
                    K::Char('r' | 'R') => {
                        let (s, d) = (play.song, play.diff);
                        self.start(s, d);
                    }
                    k if matches!(nav(k), Some(Nav::Ok) | Some(Nav::Back)) => self.to_select(),
                    _ => {}
                }
            }
            Screen::Calibrate(cal) => {
                if !p.down {
                    return;
                }
                let raw = self.clock.at(p.at);
                match p.key {
                    K::Enter | K::PadStart if cal.done => {
                        if let Some(ms) = cal.median_ms() {
                            self.settings.offset_ms = ms.clamp(-300, 300);
                            save::save_settings(&self.settings);
                            self.say(format!("offset set to {:+} ms", self.settings.offset_ms));
                        }
                        self.to_select();
                    }
                    K::Char('r' | 'R') => self.calibrate(),
                    // not the pad's B: that is a lane, and taps
                    K::Esc | K::Back | K::PadSelect => self.to_select(),
                    _ => {
                        if p.lane().is_some() || p.key == K::Char(' ') || p.key == K::Enter {
                            cal.tap(raw);
                        }
                    }
                }
            }
        }
    }

    fn resume(&mut self) {
        if let Screen::Play { paused, .. } = &mut self.screen {
            *paused = false;
        }
        self.audio.with(|s| s.paused = false);
        self.clock.running = true;
        self.clock.tick(Instant::now(), None);
        log::info!("resumed");
    }

    fn advance(&mut self, now: Instant) {
        let t = self.clock.t - self.offset();
        let mut cues = Vec::new();
        let mut finished = None;
        match &mut self.screen {
            Screen::Play { play, paused, over_at, .. } => {
                if !*paused {
                    play.update(t);
                    cues = std::mem::take(&mut play.cues);
                    if play.finished && over_at.is_none() {
                        *over_at = Some(t);
                        if play.failed {
                            self.audio.with(|s| s.stop());
                        }
                    }
                    if over_at.is_some_and(|o| t - o > 1.2) {
                        finished = Some(());
                    }
                }
            }
            Screen::Results { shown, .. } => {
                if self.demo && shown.elapsed().as_secs_f64() > 4.0 {
                    let next = (self.sel + 1) % SONGS.len();
                    self.sel = next;
                    let d = self.diff;
                    self.start(next, d);
                }
            }
            Screen::Calibrate(cal) => {
                let raw = self.clock.t;
                let p = cal.period();
                if raw >= 0.0 {
                    cal.flash = (raw / p).floor() * p;
                }
            }
            Screen::Select => {}
        }
        for c in &cues {
            match *c {
                Cue::Hit(sound, held) => self.audio.with(|s| s.hit(sound, held)),
                Cue::Release(sound) => self.audio.with(|s| s.release(sound)),
                Cue::Thud => self.audio.with(|s| s.sfx(Inst::Thud, 0.0, 0.18)),
            };
        }
        if let Screen::Play { play, .. } = &self.screen {
            if let Some((j, at, off)) = play.last {
                let line = crate::ui::log_line(play, j, at, off);
                if self.log.last() != Some(&line) {
                    self.log.push(line);
                    if self.log.len() > 40 {
                        self.log.remove(0);
                    }
                }
            }
        }
        if finished.is_some() {
            let Screen::Play { play, .. } = std::mem::replace(&mut self.screen, Screen::Select) else { unreachable!() };
            let mut new_best = false;
            if !play.demo && !play.failed {
                let b = Best {
                    song: play.def.id.into(),
                    diff: play.diff.name().into(),
                    score: play.score(),
                    accuracy: (play.accuracy() * 1000.0).round() as u32,
                    combo: play.max_combo,
                    full_combo: play.full_combo(),
                    grade: play.grade().into(),
                };
                new_best = save::record(&mut self.bests, b);
            }
            log::info!("results: {} {} score {} grade {} new best {new_best}", play.def.id, play.diff.name(), play.score(), play.grade());
            self.audio.with(|s| s.sfx(Inst::Chime, if play.failed { 60.0 } else { 88.0 }, 0.3));
            self.screen = Screen::Results { play, new_best, shown: now };
        }
    }

    /// A line for the session log every half second.
    pub fn snapshot(&self) {
        match &self.screen {
            Screen::Select => log::debug!("snapshot: songs, {} {} speed {} offset {}", SONGS[self.sel].id, self.diff.name(), self.settings.speed, self.settings.offset_ms),
            Screen::Play { play, paused, .. } => log::debug!(
                "snapshot: play {} {} t {:.2} paused {paused} score {} combo {} groove {:.2} counts {:?} down {:?}",
                play.def.id,
                play.diff.name(),
                play.now,
                play.score(),
                play.combo,
                play.groove,
                play.counts,
                play.down
            ),
            Screen::Results { play, .. } => log::debug!("snapshot: results {} {}", play.def.id, play.grade()),
            Screen::Calibrate(c) => log::debug!("snapshot: calibrate {} taps, median {:?}", c.taps.len(), c.median_ms()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_takes_the_middle_tap() {
        let mut c = Cal { bpm: 100.0, taps: Vec::new(), flash: 0.0, done: false };
        let p = c.period();
        for (i, off) in [0.03, 0.05, 0.04, 0.2, 0.045, 0.04, 0.035, 0.05, 0.04, 0.042, 0.038, 0.041, 0.044, 0.039, 0.04, 0.043].iter().enumerate() {
            c.tap(i as f64 * p + off);
        }
        assert!(c.done);
        assert_eq!(c.median_ms(), Some(41));
    }

    #[test]
    fn the_clock_follows_the_synth_without_jumping_back() {
        let now = Instant::now();
        let mut c = Clock::new();
        c.start(0.0, now);
        let later = now + std::time::Duration::from_millis(100);
        c.tick(later, Some(0.13));
        assert!(c.t > 0.1 && c.t < 0.13);
        c.tick(later + std::time::Duration::from_millis(10), Some(0.5));
        assert_eq!(c.t, 0.5);
        c.tick(later + std::time::Duration::from_millis(20), Some(0.2));
        assert!(c.t >= 0.5);
    }
}
