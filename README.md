# Bellbeat

A rhythm game for the terminal, named after BEL (^G, 0x07), the one sound a
terminal was ever built to make.

Notes fall down four lanes; hit each one as it reaches the line. **Every
note you hit is a note of the song:** it plays the moment you hit it, and a
note you miss is a hole in the tune. Play well and you are playing the
melody. The nine songs are Asciiverse's own, synthesised as they play:
a lullaby, a waltz in 3/4, a swung shuffle, a samba and a kernel panic
among them.

## How to play

Install it with [av](https://asciiverse.sh): `av install bellbeat`, then
start it from av's list. Or build it: `cargo run --release`.

- **D F J K**, or the arrow keys **← ↓ ↑ →**: the four lanes
- **Gamepad**: the d-pad, the face buttons laid out the same way (X A Y B),
  or LB LT RT RB
- Long notes are held until their tail has passed the line
- **Esc** or **P** (a pad's Start): pause
- **M**: sound on or off

On the song list: **↑ ↓** pick a song, **← →** the difficulty, **Enter**
play, **[ ]** how fast the notes fall, **- +** the sound offset, **C**
calibrate, **P** practise, **E** edit a song, **N** write a new one,
**Esc** quit.

- **Easy:** the melody's notes on the beat; the rest of the tune plays itself.
- **Normal:** the melody's notes on every eighth.
- **Hard:** the whole melody, its chords (two lanes at once where the song
  has a harmony), and the snare's backbeat with your other hand.

Hits come as PERFECT (within 45 ms) or GOOD (within 95 ms); every ten
notes in a row raise the multiplier, up to x4. Misses drain the groove
meter, and when it runs dry the song is lost.

**Practice:** press P on the song list, pick a section with **↑ ↓** and a
tempo from 50% to 100% with **← →** (**Tab** changes the difficulty), and
**Enter** plays that section round and round, with a count-in, no groove
meter and no losing. Your accuracy on the last round shows beside the lanes.

## Your own songs

Every song in Bellbeat is a plain text file, and you can write your own.
Press **N** on the song list for a new one, or **E** on a song to edit it
(a built-in song opens as your own copy). The editor shows the song file:

```
title  Night Tune
bpm    100
meter  4          beats a bar, 2 to 7 (3 for a waltz)
swing  50         50 is straight, 66 swings the eighths
voice  bell       bell, pulse, pluck or flute
order  intro tune tune

[tune] 4 bars
chords C Am F G
lead   C5 - - - E5 - G5 - A5 - - - G5 - - -
       A5 - - - C6 - - - E6 - D6 - C6 - - -
harmony ...
bass   1 - - - . . 1 - 5 - - - . . 5 -
kick   x... .... x... ....
```

One token is a sixteenth: a note (`E5`, `F#4`, `Bb3`), `-` to hold it,
`.` for silence; sixteen make a bar of 4/4. `chords` has one chord a bar,
and `bass` and `arp` play its degrees (`1 3 5 7 8`). `harmony` is a second
voice under the melody, your second lane on Hard. A line is one bar, which
repeats, or the whole section; a line starting with a space carries on the
one before; `#` starts a comment.

As you type a note on a melody line and press Space you hear it. The
editor checks the song after every key and says what is wrong and on which
line. **Ctrl-S** saves, **Ctrl-L** listens from the section the cursor is
in, **Ctrl-P** plays it from there, **Ctrl-Z** and **Ctrl-Y** undo and redo,
**Ctrl-D** copies a line and **Ctrl-K** cuts one, **Esc** leaves.

Songs are kept in the `songs` folder in Bellbeat's state folder (below),
one `.song` file each, named for the song; Bellbeat also reads a `songs`
folder beside the program. Share a song by sharing its file.

## Calibrate

**Calibrate first.** Sound takes a moment to reach your ears, more with
Bluetooth headphones. Press C on the song list and tap along to the clicks;
Bellbeat measures how late you hear them and moves every note to match.

Held notes need key releases, which kitty, foot, WezTerm, Ghostty,
Alacritty, iTerm2 and others report, and so does a gamepad. In other
terminals a held note holds itself once you hit it.

## Options

- `--demo`: watch it play itself
- `--mute`: no sound
- `--song ID`, `--difficulty easy|normal|hard`: start on a song
- `--speed N` (1 to 10), `--offset MS`: set and save these
- `--help`: everything

Best scores and settings live in `$XDG_STATE_HOME/bellbeat`
(`~/.local/state/bellbeat`), `~/Library/Application Support/bellbeat` on
macOS and `%LocalAppData%\bellbeat` on Windows.

## Licence

MIT. Game, songs and sounds by Asciiverse.
