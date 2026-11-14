# Bellbeat roadmap

Versions follow the games rule (games/CLAUDE.md): 0.x.0 while playable but
unfinished, 1.0.0 when everything planned is in.

## 0.5.0 (2026-11-13): playable

- Four lanes, half-row smooth notes, holds, beat lines, sparks, a glitch on a miss
- The melody is yours: hit notes sound, missed notes leave a hole
- Five songs of our own (84 to 148 bpm), three difficulties from one score
- Perfect and good windows, combo and multiplier, groove meter and losing a song
- Calibration by tapping to clicks; offset and fall speed saved
- Best score, grade and full combo per song and difficulty, in the state folder
- Song list with a live preview and stars; pause menu; results
- Keyboard (DFJK or arrows), kitty key releases for holds, gamepad
- Oscilloscope of the actual output and a running log of judgements
- Demo autoplay, `--shot`, `--bench`, session logging

## 0.6.0 (2026-11-14): more to play

- Four more songs: Waltz for a Spinning Disk (3/4), Cron at Midnight, Tab
  Complete Shuffle (swung) and Escape Sequence; nine in all
- Songs can have any meter from 2/4 to 7/4 and a swing
- Chords on Hard: two lanes at once where the score has a harmony note
  (five songs have harmonies)
- A practice mode: any section, 50% to 100% tempo, a count-in, round and
  round without the groove meter
- The songs live in `songs/*.song`, built in, in the notation 0.7 opens to
  players
- A lost song goes on to its results again (the clock stopped with the music)

## 0.7.0 (2026-11-14): the song file

- Players' songs as `.song` text files, in the built-in songs' notation:
  read from `songs` in the state folder and `songs` beside the program,
  checked with line-numbered errors; a broken file shows on the list with
  its error and opens in the editor to be mended
- A song editor in the terminal (N new, E edit; a built-in song opens as
  the player's own copy): the song as coloured text, every note sounding as
  it is typed, the song checked after every key, listen from the cursor's
  section, play from there, undo, save

## 1.0.0

- Played through by David on all songs and difficulties, and tuned
- The windows, groove drain and stars tuned from real play
- Verified on macOS and Windows terminals, and on the Steam Deck with its pad
