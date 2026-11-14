//! The band: every sound in Bellbeat is synthesised here, sample by
//! sample, from the song's score (no audio files, as in Braillestorm and
//! Kaiku).
//!
//! - The backing (bass, pad, arpeggio, drums) and every melody note that is
//!   not yours play on their own, on time.
//! - Your notes play when you hit them (`hit`), and a held note sounds for
//!   as long as you hold it (`release`). Miss one and that note is simply
//!   not in the song.
//! - The synth counts every sample it makes, and that count is the game's
//!   clock (`mark`): the notes fall in step with what you hear.
//! - The last samples are kept for the oscilloscope beside the lanes.
//!
//! With a sound device the mixer pulls samples through `Out`; without one
//! (or muted) the game pulls them itself at the same pace (`pump`), so the
//! clock and the scope work the same either way.

use crate::chart::{Chart, Sound};
use crate::song::{Lead, Score, Song};
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const SR: u32 = 44_100;
const SRF: f32 = SR as f32;
pub const SCOPE: usize = 2048;
/// Samples rendered at a time.
pub const CHUNK: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Inst {
    Lead(Lead),
    Bass,
    Pad,
    Arp,
    Kick,
    Snare,
    Hat,
    /// The metronome; the flag is the bar's first beat.
    Click(bool),
    /// A note gone missing: a dull knock.
    Thud,
    /// Menus: a tick when moving, a chime when choosing.
    Tick,
    Chime,
}

#[derive(Clone, Copy, Debug)]
struct Ev {
    at: i64,
    inst: Inst,
    midi: f32,
    len: u32,
    vol: f32,
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    inst: Inst,
    freq: f32,
    ph: f32,
    ph2: f32,
    age: u32,
    /// Samples the note is held for; the release follows.
    gate: u32,
    vol: f32,
    lp: f32,
    /// The note this voice plays for the player, to release it.
    tag: Option<Sound>,
}

/// The harmony plays this much softer than the melody.
const HARMONY: f32 = 0.6;

fn lead_volume(lead: Lead) -> f32 {
    match lead {
        Lead::Bell => 0.30,
        Lead::Pulse => 0.16,
        Lead::Pluck => 0.20,
        Lead::Flute => 0.24,
    }
}

fn hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// Attack, then the level while held, then the release.
fn adsr(age: u32, gate: u32, attack: f32, sustain: f32, decay: f32, release: f32) -> f32 {
    let t = age as f32 / SRF;
    let g = gate as f32 / SRF;
    let held = |t: f32| {
        if t < attack {
            t / attack
        } else {
            sustain + (1.0 - sustain) * (-(t - attack) / decay).exp()
        }
    };
    if t < g {
        held(t)
    } else {
        held(g) * (1.0 - (t - g) / release).max(0.0)
    }
}

impl Voice {
    fn new(inst: Inst, midi: f32, len: u32, vol: f32) -> Voice {
        Voice { inst, freq: hz(midi), ph: 0.0, ph2: 0.0, age: 0, gate: len, vol, lp: 0.0, tag: None }
    }

    /// Samples after which the voice is silent.
    fn life(&self) -> u32 {
        let tail = match self.inst {
            Inst::Lead(Lead::Bell) => 1.6,
            Inst::Lead(Lead::Flute) | Inst::Pad => 0.4,
            Inst::Lead(_) => 0.12,
            Inst::Bass => 0.06,
            Inst::Arp => 0.0,
            Inst::Kick => 0.35,
            Inst::Snare | Inst::Thud => 0.25,
            Inst::Hat => 0.08,
            Inst::Click(_) | Inst::Tick => 0.06,
            Inst::Chime => 0.5,
        };
        match self.inst {
            Inst::Kick | Inst::Snare | Inst::Hat | Inst::Click(_) | Inst::Thud | Inst::Tick | Inst::Chime => (tail * SRF) as u32,
            _ => self.gate + (tail * SRF) as u32,
        }
    }

    fn sample(&mut self, noise: &mut u32) -> f32 {
        let mut rnd = || {
            *noise ^= *noise << 13;
            *noise ^= *noise >> 17;
            *noise ^= *noise << 5;
            *noise as f32 / u32::MAX as f32 * 2.0 - 1.0
        };
        let t = self.age as f32 / SRF;
        let step = |ph: &mut f32, f: f32| {
            *ph = (*ph + f / SRF).fract();
            *ph
        };
        let x = match self.inst {
            Inst::Lead(Lead::Bell) => {
                // two-operator FM, the modulator at 3.5 times the pitch and
                // fading, so it strikes bright and rings out pure
                let m = step(&mut self.ph2, self.freq * 3.5);
                let index = 2.2 * (-t * 7.0).exp() + 0.25;
                let p = step(&mut self.ph, self.freq);
                let ring = (-t * 2.2).exp();
                let rel = if self.age > self.gate { (1.0 - (self.age - self.gate) as f32 / (0.25 * SRF)).max(0.0) } else { 1.0 };
                ((p + index * (m * TAU).sin() / TAU) * TAU).sin() * ring * rel * (t / 0.002).min(1.0)
            }
            Inst::Lead(Lead::Pulse) => {
                let vib = if t > 0.18 { 1.0 + 0.004 * (t * TAU * 5.5).sin() } else { 1.0 };
                let p = step(&mut self.ph, self.freq * vib);
                let sq = if p < 0.25 { 1.0 } else { -0.33 };
                self.lp += (sq - self.lp) * 0.35;
                self.lp * adsr(self.age, self.gate, 0.004, 0.7, 0.15, 0.08)
            }
            Inst::Lead(Lead::Pluck) => {
                let p = step(&mut self.ph, self.freq);
                let saw = p * 2.0 - 1.0;
                let c = 0.04 + 0.6 * (-t * 14.0).exp();
                self.lp += (saw - self.lp) * c;
                self.lp * adsr(self.age, self.gate, 0.002, 0.35, 0.12, 0.08) * 1.3
            }
            Inst::Lead(Lead::Flute) => {
                let vib = if t > 0.2 { 1.0 + 0.005 * (t * TAU * 5.0).sin() } else { 1.0 };
                let p = step(&mut self.ph, self.freq * vib);
                let tone = (p * TAU).sin() + 0.12 * (2.0 * p * TAU).sin();
                let breath = rnd() * 0.06 * (-t * 8.0).exp() + rnd() * 0.015;
                (tone + breath) * adsr(self.age, self.gate, 0.04, 0.85, 0.3, 0.25)
            }
            Inst::Bass => {
                let p = step(&mut self.ph, self.freq);
                let tri = 1.0 - 4.0 * (p - 0.5).abs();
                (tri * 0.8 + (p * TAU).sin() * 0.5) * adsr(self.age, self.gate, 0.003, 0.8, 0.2, 0.05)
            }
            Inst::Pad => {
                let a = step(&mut self.ph, self.freq * 1.004) * 2.0 - 1.0;
                let b = step(&mut self.ph2, self.freq * 0.996) * 2.0 - 1.0;
                self.lp += (a + b - self.lp) * 0.04;
                self.lp * adsr(self.age, self.gate, 0.25, 1.0, 1.0, 0.35)
            }
            Inst::Arp => {
                let p = step(&mut self.ph, self.freq);
                let sq = if p < 0.5 { 1.0 } else { -1.0 };
                self.lp += (sq - self.lp) * 0.2;
                self.lp * (-t * 16.0).exp() * (t / 0.002).min(1.0)
            }
            Inst::Kick => {
                let f = 45.0 + 110.0 * (-t * 30.0).exp();
                let p = step(&mut self.ph, f);
                (p * TAU).sin() * (-t * 9.0).exp() + rnd() * 0.15 * (-t * 200.0).exp()
            }
            Inst::Snare => {
                let p = step(&mut self.ph, 185.0);
                let n = rnd();
                self.lp += (n - self.lp) * 0.5;
                (n - self.lp * 0.5) * 0.7 * (-t * 16.0).exp() + (p * TAU).sin() * 0.5 * (-t * 28.0).exp()
            }
            Inst::Hat => {
                let n = rnd();
                self.lp += (n - self.lp) * 0.6;
                (n - self.lp) * (-t * 50.0).exp()
            }
            Inst::Click(accent) => {
                let f = if accent { 1760.0 } else { 1320.0 };
                (TAU * f * t).sin() * (-t * 70.0).exp()
            }
            Inst::Thud => {
                let n = rnd();
                self.lp += (n - self.lp) * 0.05;
                ((TAU * 80.0 * t).sin() * 0.6 + self.lp * 3.0) * (-t * 25.0).exp()
            }
            Inst::Tick => (TAU * 2200.0 * t).sin() * (-t * 90.0).exp(),
            Inst::Chime => {
                let m = step(&mut self.ph2, self.freq * 3.5);
                let p = step(&mut self.ph, self.freq);
                ((p + 1.5 * (-t * 9.0).exp() * (m * TAU).sin() / TAU) * TAU).sin() * (-t * 6.0).exp()
            }
        };
        self.age += 1;
        x * self.vol
    }

    fn lead(&self) -> bool {
        matches!(self.inst, Inst::Lead(_) | Inst::Chime)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Only the menu sounds.
    Idle,
    /// A song, played once from the lead-in.
    Song,
    /// A song looping quietly from `from`, behind the song list.
    Preview { from: i64 },
    /// Clicks every `period` samples, for calibration.
    Metronome { period: i64 },
}

pub struct Synth {
    pub mode: Mode,
    pub paused: bool,
    /// Samples from the song's start; negative in the lead-in.
    pub pos: i64,
    events: Vec<Ev>,
    next: usize,
    end: i64,
    lead: Lead,
    /// What each melody and harmony note is, for the player's hits.
    tones: Vec<(f32, u32)>,
    harmony: Vec<(f32, u32)>,
    voices: Vec<Voice>,
    noise: u32,
    echo: Vec<f32>,
    echo_at: usize,
    pub scope: Vec<f32>,
    pub scope_at: usize,
    /// (pos, when) at the start of the last chunk: the clock's anchor.
    pub mark: Option<(i64, Instant)>,
    pub rendered: u64,
    pub volume: f32,
    /// Leave sound out (the device is there but muted): still runs, silent.
    pub silent: bool,
}

impl Default for Synth {
    fn default() -> Self {
        Synth::new()
    }
}

impl Synth {
    pub fn new() -> Synth {
        Synth {
            mode: Mode::Idle,
            paused: false,
            pos: 0,
            events: Vec::new(),
            next: 0,
            end: 0,
            lead: Lead::Bell,
            tones: Vec::new(),
            harmony: Vec::new(),
            voices: Vec::with_capacity(64),
            noise: 0x2468_ace1,
            echo: vec![0.0; SR as usize],
            echo_at: 0,
            scope: vec![0.0; SCOPE],
            scope_at: 0,
            mark: None,
            rendered: 0,
            volume: 1.0,
            silent: false,
        }
    }

    /// Lay out the song: everything that plays itself as events, and the
    /// player's melody notes for `hit`. `chart` None plays all of it (the
    /// preview).
    pub fn load(&mut self, song: &Song, score: &Score, chart: Option<&Chart>) {
        let secs = |s: usize| song.time(s) * SRF as f64;
        let at = |s: usize| secs(s) as i64;
        let len = |s: usize, n: usize| (secs(s + n) - secs(s)) as u32;
        let sixteenth = (song.step_secs() * SRF as f64) as u32;
        let mut ev = Vec::new();
        self.lead = song.lead;
        self.tones = score.lead.iter().map(|t| (t.midi as f32, len(t.step, t.len))).collect();
        self.harmony = score.harmony.iter().map(|t| (t.midi as f32, len(t.step, t.len))).collect();
        let lead_vol = lead_volume(song.lead);
        for (i, t) in score.lead.iter().enumerate() {
            if chart.is_some_and(|c| c.played[i]) {
                continue;
            }
            ev.push(Ev { at: at(t.step), inst: Inst::Lead(song.lead), midi: t.midi as f32, len: len(t.step, t.len).saturating_sub(sixteenth / 4), vol: lead_vol });
        }
        for (i, t) in score.harmony.iter().enumerate() {
            if chart.is_some_and(|c| c.harmony_played[i]) {
                continue;
            }
            ev.push(Ev { at: at(t.step), inst: Inst::Lead(song.lead), midi: t.midi as f32, len: len(t.step, t.len).saturating_sub(sixteenth / 4), vol: lead_vol * HARMONY });
        }
        for t in &score.bass {
            ev.push(Ev { at: at(t.step), inst: Inst::Bass, midi: t.midi as f32, len: len(t.step, t.len).saturating_sub(sixteenth / 3), vol: 0.20 });
        }
        for t in &score.arp {
            ev.push(Ev { at: at(t.step), inst: Inst::Arp, midi: t.midi as f32, len: len(t.step, 1), vol: 0.035 });
        }
        for (s, root, iv) in &score.chords {
            for &i in iv.iter().take(3) {
                ev.push(Ev { at: at(*s), inst: Inst::Pad, midi: (root + 12 + i) as f32, len: len(*s, song.bar()).saturating_sub(sixteenth), vol: 0.035 });
            }
        }
        let mine: &[usize] = chart.map_or(&[], |c| &c.snare_played);
        for &s in &score.kick {
            ev.push(Ev { at: at(s), inst: Inst::Kick, midi: 0.0, len: 0, vol: 0.42 });
        }
        for &s in &score.snare {
            if !mine.contains(&s) {
                ev.push(Ev { at: at(s), inst: Inst::Snare, midi: 0.0, len: 0, vol: 0.22 });
            }
        }
        for &s in &score.hat {
            ev.push(Ev { at: at(s), inst: Inst::Hat, midi: 0.0, len: 0, vol: if s % 4 == 0 { 0.06 } else { 0.04 } });
        }
        ev.sort_by_key(|e| e.at);
        self.events = ev;
        self.end = at(score.steps);
        self.next = 0;
        self.voices.retain(|v| matches!(v.inst, Inst::Tick | Inst::Chime));
    }

    /// Practice: only what lies between `from` and `to` (seconds) plays,
    /// after a count-in of clicks every `beat` seconds for `lead_in`.
    pub fn start_window(&mut self, from: f64, to: f64, lead_in: f64, beat: f64) {
        let (a, b) = ((from * SRF as f64) as i64, (to * SRF as f64) as i64);
        self.events.retain(|e| e.at >= a && e.at < b);
        let mut k = 1;
        while k as f64 * beat <= lead_in + 1e-6 && k <= 8 {
            let at = a - (k as f64 * beat * SRF as f64) as i64;
            self.events.push(Ev { at, inst: Inst::Click(k % 4 == 0), midi: 0.0, len: 0, vol: 0.4 });
            k += 1;
        }
        self.events.sort_by_key(|e| e.at);
        self.end = b;
        self.mode = Mode::Song;
        self.paused = false;
        self.volume = 1.0;
        self.seek(a - (lead_in * SRF as f64) as i64);
    }

    /// Start the loaded song after `lead_in` seconds.
    pub fn start(&mut self, lead_in: f64) {
        self.mode = Mode::Song;
        self.paused = false;
        self.volume = 1.0;
        self.seek((-lead_in * SRF as f64) as i64);
    }

    pub fn preview(&mut self, from_secs: f64) {
        let from = (from_secs * SRF as f64) as i64;
        self.mode = Mode::Preview { from };
        self.paused = false;
        self.volume = 0.45;
        self.seek(from - (0.3 * SRF) as i64);
    }

    pub fn metronome(&mut self, bpm: f64, lead_in: f64) {
        self.events.clear();
        self.voices.clear();
        self.mode = Mode::Metronome { period: (60.0 / bpm * SRF as f64) as i64 };
        self.paused = false;
        self.volume = 1.0;
        self.pos = (-lead_in * SRF as f64) as i64;
        self.mark = None;
    }

    pub fn stop(&mut self) {
        self.mode = Mode::Idle;
        self.paused = false;
        self.voices.retain(|v| matches!(v.inst, Inst::Tick | Inst::Chime | Inst::Thud));
    }

    fn seek(&mut self, pos: i64) {
        self.pos = pos;
        // the old anchor belongs to what played before
        self.mark = None;
        self.next = self.events.partition_point(|e| e.at < pos);
        self.voices.clear();
    }

    /// The player hit a note: play it now.
    pub fn hit(&mut self, sound: Sound, held: bool) {
        match sound {
            Sound::Lead(i) => {
                let Some(&(midi, len)) = self.tones.get(i) else { return };
                self.play_tagged(sound, midi, len, held, lead_volume(self.lead));
            }
            Sound::Harmony(i) => {
                let Some(&(midi, len)) = self.harmony.get(i) else { return };
                self.play_tagged(sound, midi, len, held, lead_volume(self.lead) * HARMONY);
            }
            Sound::Snare => self.add(Voice::new(Inst::Snare, 0.0, 0, 0.26)),
        }
    }

    fn play_tagged(&mut self, sound: Sound, midi: f32, len: u32, held: bool, vol: f32) {
        // a held note sounds until `release`; a tap for its length
        let gate = if held { u32::MAX / 4 } else { len.saturating_sub(SR / 40) };
        let mut v = Voice::new(Inst::Lead(self.lead), midi, gate, vol);
        v.tag = Some(sound);
        self.add(v);
    }

    /// The player let go of a held note.
    pub fn release(&mut self, sound: Sound) {
        if matches!(sound, Sound::Lead(_) | Sound::Harmony(_)) {
            for v in self.voices.iter_mut().filter(|v| v.tag == Some(sound)) {
                v.gate = v.gate.min(v.age);
            }
        }
    }

    pub fn sfx(&mut self, inst: Inst, midi: f32, vol: f32) {
        self.add(Voice::new(inst, midi, 0, vol));
    }

    fn add(&mut self, v: Voice) {
        if self.voices.len() >= 64 {
            // the oldest goes
            if let Some(i) = self.voices.iter().enumerate().max_by_key(|(_, v)| v.age).map(|(i, _)| i) {
                self.voices.swap_remove(i);
            }
        }
        self.voices.push(v);
    }

    /// Fill `out` with the next samples.
    pub fn render(&mut self, out: &mut [f32]) {
        self.mark = Some((self.pos, Instant::now()));
        for o in out.iter_mut() {
            *o = self.sample();
        }
        self.rendered += out.len() as u64;
    }

    fn sample(&mut self) -> f32 {
        let running = !self.paused && self.mode != Mode::Idle;
        if running {
            match self.mode {
                Mode::Song | Mode::Preview { .. } => {
                    while self.next < self.events.len() && self.events[self.next].at <= self.pos {
                        let e = self.events[self.next];
                        self.next += 1;
                        let v = Voice::new(e.inst, e.midi, e.len, e.vol);
                        self.add(v);
                    }
                    if let Mode::Preview { from } = self.mode {
                        if self.pos >= self.end + (SRF * 0.5) as i64 {
                            self.seek(from);
                        }
                    }
                }
                Mode::Metronome { period } => {
                    if self.pos >= 0 && self.pos % period == 0 {
                        let beat = self.pos / period;
                        self.add(Voice::new(Inst::Click(beat % 4 == 0), 0.0, 0, 0.5));
                    }
                }
                Mode::Idle => {}
            }
        }
        let (mut dry, mut send) = (0.0f32, 0.0f32);
        let noise = &mut self.noise;
        if !self.paused {
            for v in self.voices.iter_mut() {
                let x = v.sample(noise);
                dry += x;
                if v.lead() {
                    send += x;
                }
            }
            self.voices.retain(|v| v.age < v.life());
        }
        // a dotted-eighth-ish echo on the melody
        let delay = (SR as usize * 3) / 10;
        let back = self.echo[(self.echo_at + self.echo.len() - delay) % self.echo.len()];
        self.echo[self.echo_at] = send + back * 0.35;
        self.echo_at = (self.echo_at + 1) % self.echo.len();
        let mixed = (dry + back * 0.3) * self.volume;
        let y = mixed.tanh() * 0.9;
        self.scope[self.scope_at] = y;
        self.scope_at = (self.scope_at + 1) % SCOPE;
        if running {
            self.pos += 1;
        }
        if self.silent {
            0.0
        } else {
            y
        }
    }

    /// The song position at `now`, from the last chunk's anchor, in seconds.
    pub fn estimate(&self, now: Instant) -> Option<f64> {
        let (pos, when) = self.mark?;
        Some(pos as f64 / SR as f64 + now.saturating_duration_since(when).as_secs_f64())
    }
}

/// The mixer's view of the synth: a chunk at a time, so the lock is taken
/// rarely.
pub struct Out {
    synth: Arc<Mutex<Synth>>,
    buf: [f32; CHUNK],
    at: usize,
}

impl Out {
    pub fn new(synth: Arc<Mutex<Synth>>) -> Out {
        Out { synth, buf: [0.0; CHUNK], at: CHUNK }
    }
}

impl Iterator for Out {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.at == CHUNK {
            if let Ok(mut s) = self.synth.lock() {
                s.render(&mut self.buf);
            } else {
                self.buf = [0.0; CHUNK];
            }
            self.at = 0;
        }
        let x = self.buf[self.at];
        self.at += 1;
        Some(x)
    }
}

impl rodio::Source for Out {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        std::num::NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        std::num::NonZero::new(SR).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The sound device, if there is one, with the synth on its mixer.
pub struct Audio {
    _sink: Option<rodio::MixerDeviceSink>,
    pub synth: Arc<Mutex<Synth>>,
    /// When the game pulls samples itself: since when, and how many.
    pumped_from: Option<Instant>,
    pub muted: bool,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let synth = Arc::new(Mutex::new(Synth::new()));
        let sink = if muted {
            None
        } else {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    s.mixer().add(Out::new(synth.clone()));
                    Some(s)
                }
                Err(e) => {
                    log::warn!("no audio: {e}");
                    None
                }
            }
        };
        log::info!("audio {}", if sink.is_some() { "on" } else { "off, the game keeps time itself" });
        let pumped_from = if sink.is_none() { Some(Instant::now()) } else { None };
        Audio { _sink: sink, synth, pumped_from, muted }
    }

    /// No device and no pumping: the caller renders the synth itself
    /// (`--shot`, `--bench`, tests).
    pub fn headless() -> Audio {
        Audio { _sink: None, synth: Arc::new(Mutex::new(Synth::new())), pumped_from: None, muted: true }
    }

    pub fn device(&self) -> bool {
        self._sink.is_some()
    }

    /// Without a device, make the samples the device would have asked for
    /// by now, so the song moves on and the scope has something to show.
    pub fn pump(&mut self) {
        let Some(from) = self.pumped_from else { return };
        let Ok(mut s) = self.synth.lock() else { return };
        let due = (from.elapsed().as_secs_f64() * SR as f64) as u64;
        let mut buf = [0.0f32; CHUNK];
        // never more than a quarter second at once (after a stall)
        if due > s.rendered + SR as u64 / 4 {
            s.rendered = due - SR as u64 / 4;
        }
        while s.rendered + CHUNK as u64 <= due {
            s.render(&mut buf);
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut Synth) -> R) -> Option<R> {
        self.synth.lock().ok().map(|mut s| f(&mut s))
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        let m = self.muted;
        self.with(|s| s.silent = m);
        log::info!("muted {m}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{build, Difficulty};
    use crate::song::builtins;

    #[test]
    fn every_song_renders_cleanly() {
        for song in &builtins() {
            let sc = song.score();
            let chart = build(song, &sc, Difficulty::Hard);
            let mut s = Synth::new();
            s.load(song, &sc, Some(&chart));
            s.start(0.0);
            let mut buf = vec![0.0f32; SR as usize * 12];
            s.render(&mut buf);
            assert!(buf.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{}", song.id);
            let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
            assert!(rms > 0.01, "{} is too quiet: {rms}", song.id);
            // every melody note a player can hit sounds
            for i in 0..sc.lead.len().min(8) {
                s.hit(Sound::Lead(i), false);
            }
            for i in 0..sc.harmony.len().min(4) {
                s.hit(Sound::Harmony(i), true);
                s.release(Sound::Harmony(i));
            }
            let mut buf = vec![0.0f32; SR as usize];
            s.render(&mut buf);
            assert!(buf.iter().all(|x| x.is_finite()));
        }
    }

    #[test]
    fn the_clock_counts_samples_and_stops_in_pause() {
        let mut s = Synth::new();
        let song = &builtins()[0];
        s.load(song, &song.score(), None);
        s.start(1.0);
        assert_eq!(s.pos, -(SR as i64));
        let mut buf = vec![0.0; 1000];
        s.render(&mut buf);
        assert_eq!(s.pos, -(SR as i64) + 1000);
        s.paused = true;
        s.render(&mut buf);
        assert_eq!(s.pos, -(SR as i64) + 1000);
    }

    #[test]
    fn a_held_note_rings_until_released() {
        let mut s = Synth::new();
        let song = &builtins()[0];
        let sc = song.score();
        s.load(song, &sc, Some(&build(song, &sc, Difficulty::Easy)));
        s.mode = Mode::Idle;
        s.hit(Sound::Lead(0), true);
        let mut buf = vec![0.0; SR as usize];
        s.render(&mut buf);
        assert_eq!(s.voices.iter().filter(|v| v.tag == Some(Sound::Lead(0))).count(), 1);
        s.release(Sound::Lead(0));
        let mut buf = vec![0.0; SR as usize * 2];
        s.render(&mut buf);
        assert_eq!(s.voices.iter().filter(|v| v.tag == Some(Sound::Lead(0))).count(), 0);
    }
}
