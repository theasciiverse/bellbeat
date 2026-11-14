//! Bellbeat: a rhythm game for the terminal, named after BEL (^G, 0x07),
//! the one sound a terminal was ever built to make.
//!
//! Notes fall down four lanes to the line; hit them on the beat. Every note
//! you hit is a note of the song's melody, played the moment you hit it,
//! and every note you miss is a hole in the tune. The songs are our own,
//! synthesised as they play.
//!
//! `--demo` lets the game play itself; `--shot` plays some seconds headless
//! and prints the screen; `--bench` times frames without a terminal.

mod app;
mod chart;
mod font;
mod input;
mod play;
mod save;
mod song;
mod synth;
mod ui;

use app::{App, Screen};
use chart::Difficulty;
use input::Input;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Rect;
use std::io;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_micros(8_333);
const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);

const HELP: &str = "bellbeat [--demo] [--mute] [--song ID] [--difficulty easy|normal|hard] [--speed 1-10] [--offset MS]

A rhythm game. Notes fall down four lanes; hit each one as it reaches the
line. Every note you hit plays that note of the melody, and every note you
miss leaves a hole in the tune. Long notes are held.

  D F J K         the four lanes   (or the arrow keys \u{2190} \u{2193} \u{2191} \u{2192})
  gamepad         d-pad, X A Y B, or LB LT RT RB, each laid out as the lanes
  Esc / p         pause            (a pad's Start)
  m               sound on/off

On the song list: \u{2191} \u{2193} choose a song, \u{2190} \u{2192} the difficulty, Enter
play, [ and ] how fast the notes fall, - and + the sound offset, c
calibrate the offset by tapping along to clicks, p practise a section,
Esc quit.

Practice: \u{2191} \u{2193} choose a section, \u{2190} \u{2192} slower or faster, Tab the
difficulty, Enter play it round and round (no groove meter, no losing),
Esc back.

Held notes need key releases: kitty, foot, WezTerm, Ghostty, Alacritty,
iTerm2 and others report them, and so does a gamepad. Elsewhere a held
note holds itself once you hit it.

Songs: lullaby-for-a-daemon, hello-world, waltz-for-a-spinning-disk,
cron-at-midnight, carriage-return, tab-complete-shuffle, null-pointer-samba,
escape-sequence, kernel-panic.

  --demo                     watch the game play (any of Esc, p, Start stops it)
  --mute                     no sound
  --song ID                  start on this song
  --difficulty D             easy, normal or hard
  --speed N                  how fast notes fall, 1 to 10 (saved)
  --offset MS                sound offset in milliseconds (saved)
  --fps N                    draw N frames a second (60; the game runs at 120)
  --shot SECS [--size WxH]   play headless on the demo, print the screen
  --bench FRAMES [--size WxH]  time frames without a terminal

Best scores and settings: $XDG_STATE_HOME/bellbeat (~/.local/state/bellbeat).
Session log: RUST_LOG=debug bellbeat 2>bellbeat.log";

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("bellbeat {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let arg = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str);
    let size = arg("--size").and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some((w.parse::<u16>().ok()?.clamp(20, 1000), h.parse::<u16>().ok()?.clamp(10, 500)))
    });
    let song_id = arg("--song");
    let diff = arg("--difficulty").and_then(Difficulty::from_name);
    let demo = args.iter().any(|a| a == "--demo");
    let mut settings = save::load_settings();
    if let Some(s) = arg("--speed").and_then(|v| v.parse::<u32>().ok()) {
        settings.speed = s.clamp(1, 10);
        save::save_settings(&settings);
    }
    if let Some(o) = arg("--offset").and_then(|v| v.parse::<i32>().ok()) {
        settings.offset_ms = o.clamp(-300, 300);
        save::save_settings(&settings);
    }

    if let Some(secs) = arg("--shot") {
        shot(secs.parse().unwrap_or(10.0), size.unwrap_or((100, 30)), song_id.unwrap_or("kernel-panic"), diff.unwrap_or(Difficulty::Hard), settings);
        return Ok(());
    }
    if let Some(frames) = arg("--bench") {
        bench(frames.parse().unwrap_or(600), size.unwrap_or((100, 30)), settings);
        return Ok(());
    }

    let mut terminal = ratatui::init();
    // the demo takes no lane keys, so it skips the query (a recorder never
    // answers it, and the query then waits two seconds)
    let kitty = !demo && supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    let _ = execute!(io::stdout(), EnableFocusChange);
    let mut input = Input::new(kitty);
    input.open_pad();
    let screen = terminal.get_frame().area();
    let audio = synth::Audio::new(args.iter().any(|a| a == "--mute"));
    log::info!("start {}x{} kitty_keyboard={kitty} pad={:?} sound device {} offset {} ms speed {}", screen.width, screen.height, input.pad_name, audio.device(), settings.offset_ms, settings.speed);
    let mut app = App::new(audio, settings, save::load_scores(), kitty || input.pad_name.is_some());
    app.demo = demo;
    let song = song_id.and_then(|id| app.find(id));
    if let Some(s) = song {
        app.sel = s;
    }
    if let Some(d) = diff {
        app.diff = d;
    }
    if demo {
        let (s, d) = (song.or(app.find("kernel-panic")).unwrap_or(0), diff.unwrap_or(Difficulty::Hard));
        app.sel = s;
        app.diff = d;
        app.start(app.songs[s].clone(), d);
    } else {
        app.to_select();
    }
    let fps = arg("--fps").and_then(|v| v.parse::<u32>().ok()).unwrap_or(60).clamp(1, 120);
    let result = run(&mut terminal, &mut app, &mut input, fps);
    let _ = execute!(io::stdout(), DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    log::info!("quit");
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, input: &mut Input, fps: u32) -> io::Result<()> {
    let mut next = Instant::now();
    // the game runs 120 times a second; the screen is drawn `fps` times
    let every = (120 / fps).max(1) as u64;
    let mut frame = 0u64;
    let mut snapshot = Instant::now();
    loop {
        // read keys right up to the frame, each stamped as it arrives
        loop {
            let wait = next.saturating_duration_since(Instant::now());
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(k) => {
                    let at = Instant::now();
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    input.on_key(k, at);
                }
                Event::FocusLost => {
                    log::debug!("focus lost");
                    if let Screen::Play { play, paused: false, .. } = &app.screen {
                        if !play.demo {
                            input.presses.push(input::Press { key: input::K::Esc, down: true, at: Instant::now(), exact: true });
                        }
                    }
                }
                Event::Resize(w, h) => log::info!("resize {w}x{h}"),
                _ => {}
            }
        }
        next += FRAME;
        let now = Instant::now();
        if next < now {
            next = now + FRAME;
        }
        input.poll_pad();
        let presses = std::mem::take(&mut input.presses);
        app.exact = input.exact || input.pad_name.is_some();
        app.frame(presses, Instant::now());
        if app.quit {
            return Ok(());
        }
        frame += 1;
        if frame % every == 0 {
            terminal.draw(|f| {
                let area = f.area();
                ui::draw(app, f.buffer_mut(), area);
            })?;
        }
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            app.snapshot();
        }
    }
}

/// Play `secs` of a song on the demo without a terminal or sound, and print
/// the screen.
fn shot(secs: f64, (w, h): (u16, u16), song: &str, diff: Difficulty, settings: save::Settings) {
    let audio = synth::Audio::headless();
    let mut app = App::new(audio, settings, Vec::new(), true);
    app.demo = true;
    let Some(i) = app.find(song) else {
        eprintln!("no song {song}");
        return;
    };
    app.sel = i;
    app.start(app.songs[i].clone(), diff);
    let start = Instant::now();
    // drive the clock by hand, a frame at a time, rendering the synth to match
    let frames = (secs * 60.0) as u32;
    let mut buf_s = vec![0.0f32; synth::SR as usize / 60];
    for _ in 0..frames {
        app.audio.with(|s| s.render(&mut buf_s));
        app.clock.t += 1.0 / 60.0;
        app.frame(Vec::new(), start);
    }
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    ui::draw(&app, &mut buf, area);
    for y in 0..h {
        let row: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
        println!("{}", row.trim_end());
    }
}

fn bench(frames: u32, (w, h): (u16, u16), settings: save::Settings) {
    let audio = synth::Audio::headless();
    let mut app = App::new(audio, settings, Vec::new(), true);
    app.demo = true;
    let i = app.find("kernel-panic").unwrap_or(0);
    app.start(app.songs[i].clone(), Difficulty::Hard);
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    let start = Instant::now();
    let mut buf_s = vec![0.0f32; synth::SR as usize / 60];
    let (mut upd, mut drw, mut snd) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    for _ in 0..frames {
        let t = Instant::now();
        app.audio.with(|s| s.render(&mut buf_s));
        snd += t.elapsed();
        let t = Instant::now();
        app.clock.t += 1.0 / 60.0;
        app.frame(Vec::new(), start);
        upd += t.elapsed();
        let t = Instant::now();
        ui::draw(&app, &mut buf, area);
        drw += t.elapsed();
    }
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / frames as f64;
    println!("{w}x{h}, {frames} frames: sound {:.3} ms, update {:.3} ms, draw {:.3} ms a frame (budget 16.7)", ms(snd), ms(upd), ms(drw));
}
