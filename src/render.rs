//! `--render SONG DIR`: a song written out as sound files, made by the same
//! synth that plays it in the game, for other games to play our songs
//! (Cascade builds its charts from the full mix; tapline plays the backing
//! and sounds each melody note as the player hits it).
//!
//! Writes into DIR, 16-bit mono WAV at 44.1 kHz:
//!
//! - `mix.wav`: the whole song, every part, as the song list's preview
//!   plays it, with its last notes left to ring out.
//! - `backing.wav`: everything but the melody (harmony, bass, pad,
//!   arpeggio, drums).
//! - `tones/lead-<midi>-<ms>.wav`: each melody note the song has, once per
//!   pitch and length, alone with its echo.
//! - `notes.tsv`: the melody, one note a line: seconds from the start,
//!   MIDI note, length in seconds, step (sixteenth), length in steps, and
//!   its file in `tones/`.
//! - `song.txt`: `key value` lines: id, title, blurb, bpm, meter, swing,
//!   voice, seconds, and `section` lines with each section's start in
//!   seconds.

use crate::chart::Chart;
use crate::song::Song;
use crate::synth::{Synth, SR};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

/// Seconds after the last note for the echo and the bell to die away.
const TAIL: f64 = 2.0;

pub fn run(song: &Song, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir.join("tones"))?;
    let score = song.score();
    let secs = song.secs();

    // the whole song
    let mut synth = Synth::new();
    synth.load(song, &score, None);
    synth.start(0.0);
    let mix = take(&mut synth, secs + TAIL);
    write_wav(&dir.join("mix.wav"), &trim(mix))?;

    // the backing: the melody marked as the player's, so it is left out
    let backing = Chart { notes: Vec::new(), played: vec![true; score.lead.len()], harmony_played: vec![false; score.harmony.len()], snare_played: Vec::new() };
    let mut synth = Synth::new();
    synth.load(song, &score, Some(&backing));
    synth.start(0.0);
    let back = take(&mut synth, secs + TAIL);
    write_wav(&dir.join("backing.wav"), &trim(back))?;

    // each melody note, as the synth sounds it when the player hits it
    let step = song.step_secs();
    let mut tones: BTreeMap<(i32, u32), String> = BTreeMap::new();
    let mut notes = String::from("# secs\tmidi\tlen_secs\tstep\tlen_steps\tfile\n");
    for t in &score.lead {
        let at = song.time(t.step);
        let len = song.time(t.step + t.len) - at;
        // the game shortens a note by a quarter step, as `Synth::load` does
        let gate = (len - step / 4.0).max(0.01);
        let ms = (gate * 1000.0).round() as u32;
        let name = format!("lead-{}-{}.wav", t.midi, ms);
        if !tones.contains_key(&(t.midi, ms)) {
            let mut s = Synth::new();
            s.set_voice(song.lead);
            s.note(t.midi, ms as f32 / 1000.0);
            let samples = take(&mut s, ms as f64 / 1000.0 + TAIL);
            write_wav(&dir.join("tones").join(&name), &trim(samples))?;
            tones.insert((t.midi, ms), name.clone());
        }
        notes.push_str(&format!("{at:.6}\t{}\t{len:.6}\t{}\t{}\t{name}\n", t.midi, t.step, t.len));
    }
    std::fs::write(dir.join("notes.tsv"), notes)?;

    let mut info = String::new();
    let voice = format!("{:?}", song.lead).to_lowercase();
    for (k, v) in [("id", song.id.clone()), ("title", song.title.clone()), ("blurb", song.blurb.clone()), ("bpm", song.bpm.to_string()), ("meter", song.meter.to_string()), ("swing", song.swing.to_string()), ("voice", voice), ("secs", format!("{secs:.6}"))] {
        info.push_str(&format!("{k} {v}\n"));
    }
    for &s in &score.sections {
        info.push_str(&format!("section {:.6}\n", song.time(s)));
    }
    std::fs::write(dir.join("song.txt"), info)?;
    eprintln!("{}: {:.1} s, {} melody notes, {} tones, in {}", song.id, secs, score.lead.len(), tones.len(), dir.display());
    Ok(())
}

/// `secs` of the synth's output.
fn take(synth: &mut Synth, secs: f64) -> Vec<f32> {
    let mut out = vec![0.0f32; (secs * SR as f64) as usize];
    for chunk in out.chunks_mut(crate::synth::CHUNK) {
        synth.render(chunk);
    }
    out
}

/// Cut the silence at the end, keeping a few milliseconds, with a short
/// fade so nothing clicks.
fn trim(mut v: Vec<f32>) -> Vec<f32> {
    let last = v.iter().rposition(|x| x.abs() > 1.0 / 4096.0).unwrap_or(0);
    v.truncate((last + SR as usize / 100).min(v.len()));
    let fade = (SR as usize / 100).min(v.len());
    let n = v.len();
    for (i, x) in v[n - fade..].iter_mut().enumerate() {
        *x *= 1.0 - i as f32 / fade as f32;
    }
    v
}

fn write_wav(path: &Path, samples: &[f32]) -> io::Result<()> {
    let data = (samples.len() * 2) as u32;
    let mut f = io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&SR.to_le_bytes())?;
    f.write_all(&(SR * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data.to_le_bytes())?;
    for &x in samples {
        f.write_all(&((x.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())?;
    }
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::builtins;

    #[test]
    fn a_song_renders_to_files() {
        let song = builtins().into_iter().find(|s| s.id == "hello-world").unwrap();
        let dir = std::env::temp_dir().join(format!("bellbeat-render-{}", std::process::id()));
        run(&song, &dir).unwrap();
        let mix = std::fs::metadata(dir.join("mix.wav")).unwrap().len();
        // the song is about a minute and a half: well over a second of sound
        assert!(mix > 44 + SR as u64 * 2 * 30, "{mix}");
        assert!(std::fs::metadata(dir.join("backing.wav")).unwrap().len() > 44 + SR as u64 * 2 * 30);
        let notes = std::fs::read_to_string(dir.join("notes.tsv")).unwrap();
        let lines: Vec<&str> = notes.lines().skip(1).collect();
        assert_eq!(lines.len(), song.score().lead.len());
        for l in lines {
            let file = l.split('\t').nth(5).unwrap();
            assert!(dir.join("tones").join(file).is_file(), "{file}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
