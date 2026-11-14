# Bellbeat

Asciiverse rhythm game, our own (original IP). Rust + Ratatui. Named after
BEL (^G, 0x07), the one sound a terminal was built to make; working name,
David may rename it. Built 2026-11-13 to fill the "keyboard dancer" gap
(nothing licensed existed in the terminal; see the catalog check that day).

**The idea that makes it ours:** the chart *is* the song's melody. Every note
you hit plays that note of the tune the moment you hit it; a note you miss is
a hole in the song. The difficulty only chooses how much of the melody is
yours (Easy: notes on the beat; Normal: on the eighths; Hard: all of it,
its chords where the song has a harmony line, plus the snare backbeat with
the other hand); the rest plays itself. No audio files: nine songs of our
own, written as text in `songs/*.song` (built in with `include_str!`) and
synthesised live. **All music is our own, written for this game**: no
existing melodies, folk or public-domain ones included.

**Status** (2026-11-14, version 0.6.0): playable end to end. Song list with a
live preview, three difficulties, play, pause menu, results with grade and
best score, calibration, practice (a section, slowed down, on repeat). Not
yet played by David. `ROADMAP.md` has what each release up to 1.0.0 brings.
Licence MIT. Repo github.com/theasciiverse/bellbeat (public), branch `main`,
tags `vX.Y.Z`; the release workflow builds every tag (`--locked`, so commit
Cargo.lock before tagging).

```bash
cargo run --release                          # play
cargo run --release -- --demo --song kernel-panic --difficulty hard
cargo run --release -- --shot 14 --size 100x30 --song kernel-panic   # headless frame
cargo run --release -- --bench 600 --size 100x30
asciiverse/record-preview.sh                 # re-record the 100x30 catalog preview
RUST_LOG=debug cargo run --release 2>bellbeat.log
cargo test
```

## How it works

- `src/song.rs`: the song file format and its parser (the module comment
  documents it). Settings (`title blurb bpm meter swing voice order`), then
  `[name] N bars` sections with `chords lead harmony bass arp kick snare
  hat`. One token per sixteenth: `E5`, `-` hold, `.` rest; bass and arpeggio
  in chord degrees (`1 3 5 7 8`) over a chord a bar; drums as `x...`; a
  line is one bar (repeated) or the whole section. `Song::time` places a
  step with swing. Parsing checks everything with line numbers and never
  panics (a fuzz test mutates the songs), so a parsed song always plays.
- `songs/*.song`: the built-in songs, gentlest first in `BUILTIN`.
- `src/chart.rs`: the melody (and on Hard the chords and snare) to lanes. A
  note's lane follows the tune's shape: higher to the right, and a step up
  or down never stays in the lane before. Melody notes of half a bar or
  longer are holds. On Hard a harmony note under a melody note of yours goes
  in the lane beside it, on its side, when the lane is free.
- `src/synth.rs`: the band. FM bell, pulse, plucked saw and breathy flute
  leads, triangle bass, detuned saw pad, square arpeggio, kick, snare, hat,
  an echo on the melody. Plays what is not the player's on schedule and the
  player's notes on `hit`/`release`. Counts samples: `mark` is the clock's
  anchor. Without a device (or `--mute`) `Audio::pump` makes the samples at
  the device's pace, so clock and scope behave the same.
- `src/app.rs`: screens (list, play and pause, results, calibrate, practice)
  and the `Clock`: song seconds, leaning 8% a frame towards the synth's
  count while a song plays (not after it stops), snapping past 80 ms, never
  backwards. Practice plays the song at a tempo (`Song::at_tempo`) between
  two section starts (`Play::new` with a window, `Synth::start_window` with
  a count-in) and starts over at the end. Presses are stamped when read and converted
  with `Clock::at`, so judging does not wait for the frame. The player's
  offset (calibrated) moves judging and drawing alike.
- `src/play.rs`: judging (perfect 45 ms, good 95 ms, a press up to 140 ms
  early takes the note as a miss), combo and multiplier (x1 to x4 every ten),
  holds (held through within 150 ms of the end, else dropped; without key
  releases a hold completes on its hit), groove meter (a miss costs 0.05 to
  0.09 by difficulty; empty loses the song), grade S/A/B/C/D/F, demo autoplay.
- `src/ui.rs`: lanes drawn as upper half blocks with foreground and
  background as the two halves, so notes fall by half rows and two things can
  share a cell; beat lines; sparks; mojibake glitch on a miss; Braille
  oscilloscope of the real output steadied on a zero crossing; `$ tail -f
  notes` log of judgements with note names and offsets; menus.
- `src/input.rs`: D F J K or arrows; pad d-pad, X A Y B, LB LT RT RB, all in
  lane order. Without the kitty protocol a key's repeats arrive as presses:
  a key pressed again within 60 ms of its last event is ignored.
- `src/save.rs`: `scores` and `settings` as text in the state folder
  (`BELLBEAT_STATE` overrides it, for tests).
- `src/font.rs`: Braillestorm's 5x7 font in half blocks, for the title and
  the grade.

Session logging per the Asciiverse convention: every key and pad event,
presses with their offsets, state changes (play, pause, results, calibrate),
and a snapshot every 500 ms.

Layout: drawn for 100x30, plays at 80x24 (lanes 7 wide) and down to about
60x20 (lanes 5 wide, the panels drop to one line).

## Preview

`asciiverse/record-preview.sh`: the demo on Kernel Panic, Hard, muted, with
hub's `castrec.py`; the demo skips the kitty keyboard query, which a recorder
never answers.

## Not done yet

- Not played by a person yet (only the demo, tmux and the tests).
- No real-device latency check beyond calibration: rodio's buffer is absorbed
  by the offset.
- macOS and Windows built by the workflow only.
