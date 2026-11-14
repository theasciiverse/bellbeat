//! Drawing every screen into a ratatui buffer.
//!
//! The lanes are drawn two pixels a cell: each cell is an upper half block
//! whose foreground is its top half and background its bottom half, so the
//! notes fall smoothly by half rows. The scope beside them draws the very
//! samples the synth just made, in Braille.

use crate::app::{App, Cal, Screen, CAL_TAPS, EDITOR_PAUSE_ITEMS, PAUSE_ITEMS, PRACTICE_PAUSE_ITEMS, SPEEDS};
use crate::editor::Editor;
use crate::chart::{Difficulty, Sound, LANES};
use crate::font;
use crate::play::{Hold, Judge, Play};
use crate::synth::SCOPE;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

const BG: Color = Color::Rgb(6, 9, 8);
const DIM: Color = Color::Rgb(70, 92, 80);
const TEXT: Color = Color::Rgb(170, 220, 185);
const BRIGHT: Color = Color::Rgb(220, 255, 230);
const AMBER: Color = Color::Rgb(255, 184, 64);
const GREEN: Color = Color::Rgb(90, 255, 130);
const RED: Color = Color::Rgb(255, 80, 80);
const MISSED: Color = Color::Rgb(70, 70, 76);

/// The four lanes' colours: cyan, green, amber, rose.
pub const LANE: [(u8, u8, u8); LANES] = [(80, 210, 255), (100, 255, 140), (255, 196, 70), (255, 110, 200)];

fn rgb((r, g, b): (u8, u8, u8), k: f32) -> Color {
    let f = |c: u8| (c as f32 * k).clamp(0.0, 255.0) as u8;
    Color::Rgb(f(r), f(g), f(b))
}

/// Text at (x, y), clipped to `area`; returns the column after it.
fn put(buf: &mut Buffer, area: Rect, x: u16, y: u16, s: &str, fg: Color) -> u16 {
    let mut cx = x;
    if y < area.y || y >= area.bottom() {
        return x;
    }
    for ch in s.chars() {
        if cx >= area.right() {
            break;
        }
        if cx >= area.x {
            let mut tmp = [0u8; 4];
            buf[(cx, y)].set_symbol(ch.encode_utf8(&mut tmp)).set_fg(fg);
        }
        cx += 1;
    }
    cx
}

fn centre(buf: &mut Buffer, area: Rect, y: u16, s: &str, fg: Color) {
    let w = s.chars().count() as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    put(buf, area, x, y, s, fg);
}

fn fill(buf: &mut Buffer, r: Rect, bg: Color) {
    let r = r.intersection(buf.area);
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(bg);
        }
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn stars(density: f64) -> usize {
    match density {
        d if d < 1.0 => 1,
        d if d < 1.8 => 2,
        d if d < 2.8 => 3,
        d if d < 4.0 => 4,
        _ => 5,
    }
}

fn note_name(midi: i32) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[midi.rem_euclid(12) as usize], midi / 12 - 1)
}

/// One line for the panel's log of judgements.
pub fn log_line(play: &Play, j: Judge, at: f64, off: f64) -> String {
    let i = play.judged.iter().zip(play.notes()).position(|(x, _)| x.is_some_and(|(jj, a)| jj == j && a == at));
    let what = match i.map(|i| play.notes()[i].sound) {
        Some(Sound::Lead(k)) => note_name(play.score_sheet.lead[k].midi),
        Some(Sound::Harmony(k)) => note_name(play.score_sheet.harmony[k].midi),
        Some(Sound::Snare) => "snare".into(),
        None => "?".into(),
    };
    match j {
        Judge::Miss => format!("{what:<5} miss"),
        _ => format!("{what:<5} {:<7} {:+4.0}ms", if j == Judge::Perfect { "perfect" } else { "good" }, off * 1000.0),
    }
}

pub fn draw(app: &App, buf: &mut Buffer, area: Rect) {
    fill(buf, area, BG);
    match &app.screen {
        Screen::Select => draw_select(app, buf, area),
        Screen::Play { play, paused, item, .. } => {
            draw_play(app, play, buf, area);
            if *paused {
                let items = if !play.practice {
                    PAUSE_ITEMS
                } else if app.practice.as_ref().is_some_and(|p| p.editor) {
                    EDITOR_PAUSE_ITEMS
                } else {
                    PRACTICE_PAUSE_ITEMS
                };
                draw_pause(buf, area, *item, items);
            }
        }
        Screen::Results { play, new_best, shown } => draw_results(app, play, *new_best, shown.elapsed().as_secs_f64(), buf, area),
        Screen::Calibrate(cal) => draw_calibrate(app, cal, buf, area),
        Screen::Practice => draw_practice(app, buf, area),
        Screen::Edit => {
            if let Some(e) = &app.editor {
                draw_editor(app, e, buf, area);
            }
        }
    }
}

// ------------------------------------------------------------------ scope

/// The last samples as a Braille trace in `r`, steadied on a rising zero
/// crossing.
fn scope(app: &App, buf: &mut Buffer, r: Rect, col: (u8, u8, u8)) {
    if r.width < 2 || r.height < 1 {
        return;
    }
    let samples: Vec<f32> = app
        .audio
        .with(|s| {
            let mut v = Vec::with_capacity(SCOPE);
            for i in 0..SCOPE {
                v.push(s.scope[(s.scope_at + i) % SCOPE]);
            }
            v
        })
        .unwrap_or_default();
    if samples.is_empty() {
        return;
    }
    let (w, h) = (r.width as usize * 2, r.height as usize * 4);
    let span = (w * 2).min(SCOPE / 2);
    // the latest rising zero crossing that leaves room for a full trace
    let mut start = SCOPE - span;
    for i in (1..SCOPE - span).rev() {
        if samples[i - 1] < 0.0 && samples[i] >= 0.0 {
            start = i;
            break;
        }
    }
    let mut dots = vec![0u8; r.width as usize * r.height as usize];
    let mut set = |x: usize, y: usize| {
        if x < w && y < h {
            let bit = [[0x01u8, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]][x % 2][y % 4];
            dots[(y / 4) * r.width as usize + x / 2] |= bit;
        }
    };
    let ypix = |v: f32| (((1.0 - (v * 1.4).clamp(-1.0, 1.0)) * 0.5) * (h - 1) as f32).round() as usize;
    let mut prev = ypix(samples[start]);
    for x in 0..w {
        let v = samples[start + x * span / w];
        let y = ypix(v);
        let (a, b) = if y < prev { (y, prev) } else { (prev, y) };
        for yy in a..=b {
            set(x, yy);
        }
        prev = y;
    }
    for cy in 0..r.height {
        for cx in 0..r.width {
            let d = dots[cy as usize * r.width as usize + cx as usize];
            if d != 0 {
                let ch = char::from_u32(0x2800 + d as u32).unwrap_or(' ');
                let mut tmp = [0u8; 4];
                buf[(r.x + cx, r.y + cy)].set_symbol(ch.encode_utf8(&mut tmp)).set_fg(rgb(col, 0.9));
            }
        }
    }
}

// ------------------------------------------------------------------ play

pub struct Geo {
    pub x0: u16,
    pub lw: u16,
    pub fw: u16,
    pub top: u16,
    /// The hit line's row.
    pub hy: u16,
    pub left: Rect,
    pub right: Rect,
}

pub fn geo(area: Rect) -> Geo {
    let lw = match area.width {
        w if w >= 110 => 9,
        w if w >= 90 => 8,
        w if w >= 66 => 7,
        _ => 5,
    };
    let fw = lw * LANES as u16 + LANES as u16 + 1;
    let x0 = area.x + area.width.saturating_sub(fw) / 2;
    let top = area.y + 1;
    let hy = (area.y + area.height).saturating_sub(4).max(top + 4);
    let lw_panel = x0.saturating_sub(area.x + 2);
    let left = Rect::new(area.x + 1, top, lw_panel, hy.saturating_sub(top) + 2);
    let rx = x0 + fw + 1;
    let right = Rect::new(rx, top, area.right().saturating_sub(rx + 1), hy.saturating_sub(top) + 2);
    Geo { x0, lw, fw, top, hy, left, right }
}

fn lane_x(g: &Geo, lane: usize) -> u16 {
    g.x0 + 1 + lane as u16 * (g.lw + 1)
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b) ^ c.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// Half rows a second the notes fall at.
fn fall(speed: u32) -> f64 {
    2.0 * (6.0 + 3.0 * speed as f64)
}

fn draw_play(app: &App, play: &Play, buf: &mut Buffer, area: Rect) {
    let g = geo(area);
    let now = play.now;
    let hr_per_s = fall(app.settings.speed);
    let hit_hr = g.hy as f64 * 2.0 + 1.0;
    let top_hr = g.top as f64 * 2.0;
    let rows = (g.hy - g.top + 1) as usize;
    let fever = play.combo >= 50;

    // the lanes, half row by half row: 0 empty, 1 note, 2 tail, 3 missed, 4 dropped tail
    for lane in 0..LANES {
        let lx = lane_x(&g, lane);
        let col = LANE[lane];
        let mut head = vec![0u8; rows * 2];
        let mut tail = vec![0u8; rows * 2];
        let mark = |v: &mut Vec<u8>, from: f64, to: f64, k: u8| {
            let (a, b) = (from.min(to), from.max(to));
            let a = ((a - top_hr).floor().max(0.0)) as usize;
            let b = ((b - top_hr).ceil().max(0.0) as usize).min(rows * 2);
            for x in v.iter_mut().take(b).skip(a) {
                if *x == 0 || k < *x {
                    *x = k;
                }
            }
        };
        for (i, n) in play.notes().iter().enumerate() {
            if n.lane != lane {
                continue;
            }
            let dt = n.t - now;
            if dt > 4.0 {
                break;
            }
            let end_dt = n.t + n.hold - now;
            if end_dt < -1.0 && dt < -1.0 {
                continue;
            }
            let judged = play.judged[i];
            let hit = judged.is_some_and(|(j, _)| j != Judge::Miss);
            let missed = judged.is_some_and(|(j, _)| j == Judge::Miss);
            let y = hit_hr - dt * hr_per_s;
            if n.hold > 0.0 {
                let ye = hit_hr - end_dt * hr_per_s;
                match play.hold[i] {
                    Hold::Holding => mark(&mut tail, ye, hit_hr, 2),
                    Hold::Done => {}
                    Hold::Dropped => mark(&mut tail, ye, y.min(hit_hr), 4),
                    Hold::None => mark(&mut tail, ye, y - 1.0, if missed { 4 } else { 2 }),
                }
                if play.hold[i] == Hold::Holding {
                    mark(&mut head, hit_hr - 1.0, hit_hr, 1);
                }
            }
            if !hit {
                mark(&mut head, y - 1.0, y, if missed { 3 } else { 1 });
            }
        }
        let held = play.down[lane];
        let since = now - play.pressed_at[lane];
        let glow = if held { 0.22 } else { (0.22 - since * 1.5).max(0.0) as f32 };
        let lane_bg = rgb(col, 0.05 + glow * 0.4 + if fever { 0.03 } else { 0.0 });
        let colour = |k: u8, row_hr: usize| -> Color {
            match k {
                1 => {
                    // notes brighten as they near the line
                    let y = top_hr + row_hr as f64;
                    let near = (1.0 - ((hit_hr - y) / 30.0).clamp(0.0, 1.0)) as f32;
                    rgb(col, 0.75 + 0.25 * near)
                }
                2 => rgb(col, 0.45),
                3 => MISSED,
                4 => Color::Rgb(45, 45, 50),
                _ => lane_bg,
            }
        };
        let tail_w = if g.lw >= 8 { 3 } else { 1 };
        let tail_x0 = lx + (g.lw - tail_w) / 2;
        for r in 0..rows {
            let y = g.top + r as u16;
            for cx in 0..g.lw {
                let x = lx + cx;
                let inner = cx >= 1 && cx + 1 < g.lw;
                let in_tail = x >= tail_x0 && x < tail_x0 + tail_w;
                let pick = |h: usize| {
                    let a = if inner { head[h] } else { 0 };
                    let b = if in_tail { tail[h] } else { 0 };
                    match (a, b) {
                        (0, b) => b,
                        (a, 0) => a,
                        (a, b) => a.min(b),
                    }
                };
                let (t, b) = (pick(r * 2), pick(r * 2 + 1));
                let cell = &mut buf[(x, y)];
                cell.reset();
                if t == 0 && b == 0 {
                    cell.set_bg(lane_bg);
                    continue;
                }
                cell.set_symbol("\u{2580}").set_fg(colour(t, r * 2)).set_bg(colour(b, r * 2 + 1));
            }
        }
        // the lane's edges
        for r in 0..rows {
            let y = g.top + r as u16;
            buf[(lx - 1, y)].set_symbol("\u{2502}").set_fg(Color::Rgb(30, 44, 38)).set_bg(BG);
        }
    }
    let right_edge = g.x0 + g.fw - 1;
    for r in 0..rows {
        buf[(right_edge, g.top + r as u16)].set_symbol("\u{2502}").set_fg(Color::Rgb(30, 44, 38)).set_bg(BG);
    }

    // beat lines falling with the notes, brighter on the bar
    let step = play.def.step_secs();
    let beat = step * 4.0;
    let first = ((now - 0.5) / beat).floor() as i64;
    for b in first..first + 40 {
        let tb = b as f64 * beat;
        let y_hr = hit_hr - (tb - now) * hr_per_s;
        if y_hr < top_hr {
            break;
        }
        let row = (y_hr / 2.0).floor() as i64;
        if row < g.top as i64 || row >= g.hy as i64 || b < 0 {
            continue;
        }
        let bar = b % play.def.meter as i64 == 0;
        for lane in 0..LANES {
            let lx = lane_x(&g, lane);
            for cx in 0..g.lw {
                let cell = &mut buf[(lx + cx, row as u16)];
                if cell.symbol() == " " {
                    cell.set_symbol(if bar { "\u{2500}" } else { "\u{2508}" }).set_fg(rgb(LANE[lane], if bar { 0.28 } else { 0.16 }));
                }
            }
        }
    }

    // sparks rising from the line
    for s in &play.sparks {
        let age = now - s.born;
        let y_hr = hit_hr - age * s.vy as f64 * 2.0;
        let row = (y_hr / 2.0) as i64;
        if row < g.top as i64 || row >= g.hy as i64 {
            continue;
        }
        let x = lane_x(&g, s.lane) + 1 + (s.x * (g.lw - 2) as f32) as u16;
        let cell = &mut buf[(x, row as u16)];
        if cell.symbol() == " " || cell.symbol() == "\u{2508}" || cell.symbol() == "\u{2500}" {
            let ch = if age < 0.15 { "\u{2726}" } else if age < 0.35 { "\u{2022}" } else { "\u{00b7}" };
            cell.set_symbol(ch).set_fg(rgb(LANE[s.lane], 1.0 - age as f32));
        }
    }

    // a miss scrambles the lane for a moment
    const JUNK: [&str; 14] = ["\u{2593}", "\u{2592}", "\u{2591}", "#", "%", "&", "$", "@", "\u{00a4}", "\u{00a7}", "\u{00bf}", "\u{00d0}", "\u{00c6}", "\u{00c3}"];
    let tick = (now * 24.0) as u32;
    for lane in 0..LANES {
        if now > play.glitch[lane] {
            continue;
        }
        let lx = lane_x(&g, lane);
        for r in g.hy.saturating_sub(7).max(g.top)..g.hy {
            for cx in 0..g.lw {
                let h = hash(tick, r as u32, (lx + cx) as u32);
                if h % 3 != 0 {
                    continue;
                }
                let cell = &mut buf[(lx + cx, r)];
                if cell.symbol() == " " || cell.symbol() == "\u{2508}" || cell.symbol() == "\u{2500}" {
                    cell.set_symbol(JUNK[(h / 3) as usize % JUNK.len()]).set_fg(Color::Rgb(150, 40, 50));
                }
            }
        }
    }

    // the hit line and the keys under it
    let labels = ["D", "F", "J", "K"];
    for lane in 0..LANES {
        let lx = lane_x(&g, lane);
        let held = play.down[lane];
        let since = now - play.pressed_at[lane];
        let k = if held { 1.0 } else { (0.55 + 0.45 * (1.0 - since * 6.0)).clamp(0.55, 1.0) as f32 };
        for cx in 0..g.lw {
            let cell = &mut buf[(lx + cx, g.hy)];
            if cell.symbol() == " " || cell.symbol() == "\u{2508}" || cell.symbol() == "\u{2500}" {
                cell.set_symbol("\u{2501}").set_fg(rgb(LANE[lane], k));
            }
        }
        let ky = g.hy + 1;
        if ky < area.bottom() {
            let label = format!("[{}]", labels[lane]);
            let x = lx + (g.lw - 3) / 2;
            let fg = if held { BG } else { rgb(LANE[lane], 0.8) };
            let x2 = put(buf, area, x, ky, &label, fg);
            if held {
                for xx in x..x2 {
                    buf[(xx, ky)].set_bg(rgb(LANE[lane], 0.9));
                }
            }
        }
    }

    // the judgement over the lanes
    let field = Rect::new(g.x0, g.top, g.fw, g.hy - g.top);
    if let Some((j, at, off)) = play.last {
        let age = now - at;
        if age < 0.6 {
            let (word, fg) = match j {
                Judge::Perfect => ("P E R F E C T", AMBER),
                Judge::Good => ("G O O D", GREEN),
                Judge::Miss => ("M I S S", RED),
            };
            let y = g.hy.saturating_sub(6).max(g.top);
            centre(buf, field, y, word, fg);
            for x in field.x..field.right() {
                if buf[(x, y)].symbol() != " " && buf[(x, y)].fg == fg {
                    buf[(x, y)].set_style(Modifier::BOLD);
                }
            }
            if j == Judge::Good {
                centre(buf, field, y + 1, if off < 0.0 { "early" } else { "late" }, DIM);
            }
        }
    }
    if play.combo >= 5 {
        let y = g.hy.saturating_sub(4).max(g.top);
        centre(buf, field, y, &format!("{} combo", play.combo), if fever { AMBER } else { TEXT });
    }
    if play.now < play.start - 0.3 {
        let n = (play.start - play.now).ceil() as i32;
        centre(buf, field, g.top + (g.hy - g.top) / 3, &format!("{n}"), BRIGHT);
        centre(buf, field, g.top + (g.hy - g.top) / 3 + 2, "ready", DIM);
    }
    if play.failed {
        let y = g.top + (g.hy - g.top) / 3;
        centre(buf, field, y, " SIGNAL LOST ", RED);
        centre(buf, field, y + 1, " the groove ran out ", DIM);
    }

    // the panels
    let wide = g.left.width >= 16;
    let l = g.left;
    if wide {
        let mut y = l.y;
        put(buf, l, l.x + 1, y, "\u{2407} BELLBEAT", AMBER);
        y += 2;
        put(buf, l, l.x + 1, y, &play.def.title, BRIGHT);
        y += 1;
        put(buf, l, l.x + 1, y, &format!("{}  {:.0} bpm", play.diff.name(), play.def.bpm), DIM);
        y += 2;
        if play.practice {
            if let Some(pr) = &app.practice {
                let name = pr.parts().get(pr.section).map(|p| p.0.clone()).unwrap_or_default();
                let what = if pr.editor { format!("trying it from {name}") } else { format!("practice {name} {:.0}%", SPEEDS[pr.speed] * 100.0) };
                put(buf, l, l.x + 1, y - 1, &what, AMBER);
                y += 1;
            }
        }
        put(buf, l, l.x + 1, y, "score", DIM);
        y += 1;
        put(buf, l, l.x + 1, y, &thousands(play.score()), BRIGHT);
        y += 2;
        let x = put(buf, l, l.x + 1, y, &format!("combo {:<4}", play.combo), TEXT);
        put(buf, l, x + 1, y, &format!("x{}", play.multiplier()), if play.multiplier() >= 4 { AMBER } else { TEXT });
        y += 1;
        put(buf, l, l.x + 1, y, &format!("acc   {:.1}%", play.accuracy() * 100.0), TEXT);
        y += 2;
        if y + 3 < l.bottom() {
            put(buf, l, l.x + 1, y, &format!("perfect {}", play.counts[0]), AMBER);
            put(buf, l, l.x + 1, y + 1, &format!("good    {}", play.counts[1]), GREEN);
            put(buf, l, l.x + 1, y + 2, &format!("miss    {}", play.counts[2]), RED);
            y += 4;
        }
        if play.practice {
            if let (Some(pr), true) = (app.practice.as_ref().filter(|p| !p.editor), y + 1 < l.bottom()) {
                put(buf, l, l.x + 1, y, &format!("round {}", pr.runs + 1), TEXT);
                if let Some(a) = pr.last {
                    put(buf, l, l.x + 1, y + 1, &format!("last  {:.1}%", a * 100.0), DIM);
                }
            }
        } else if y + 1 < l.bottom() {
            put(buf, l, l.x + 1, y, "groove", DIM);
            let w = l.width.saturating_sub(3);
            let filled = (play.groove * w as f64).round() as u16;
            let fg = if play.groove > 0.5 { GREEN } else if play.groove > 0.25 { AMBER } else { RED };
            for i in 0..w {
                let (ch, c) = if i < filled { ("\u{2588}", fg) } else { ("\u{2591}", Color::Rgb(40, 52, 46)) };
                buf[(l.x + 1 + i, y + 1)].set_symbol(ch).set_fg(c);
            }
        }
    } else {
        // narrow: one line over the lanes
        let s = format!("{}  {}  x{}  {:.0}%", thousands(play.score()), play.combo, play.multiplier(), play.accuracy() * 100.0);
        centre(buf, Rect::new(area.x, area.y, area.width, 1), area.y, &s, TEXT);
    }
    let r = g.right;
    if r.width >= 14 {
        put(buf, r, r.x + 1, r.y, "signal", DIM);
        let sh = ((r.height as f32) * 0.35).clamp(3.0, 8.0) as u16;
        let sr = Rect::new(r.x + 1, r.y + 1, r.width.saturating_sub(2), sh);
        scope(app, buf, sr, if fever { (255, 190, 80) } else { (90, 255, 140) });
        let ly = sr.bottom() + 1;
        if ly < r.bottom() {
            put(buf, r, r.x + 1, ly, "$ tail -f notes", DIM);
            let room = r.bottom().saturating_sub(ly + 1) as usize;
            let lines: Vec<&String> = app.log.iter().rev().take(room).collect();
            for (i, line) in lines.iter().rev().enumerate() {
                let fg = if line.contains("miss") { RED } else if line.contains("perfect") { TEXT } else { GREEN };
                let s: String = line.chars().take(r.width.saturating_sub(2) as usize).collect();
                put(buf, r, r.x + 1, ly + 1 + i as u16, &s, fg);
            }
        }
    }

    // the song's progress along the bottom
    let by = area.bottom().saturating_sub(1);
    if by > g.hy + 1 {
        let t = format!(" {} / {} ", mmss(play.now - play.start), mmss(play.length - play.start));
        let w = area.width.saturating_sub(t.len() as u16 + 4);
        let done = (play.progress() * w as f64) as u16;
        for i in 0..w {
            let (ch, c) = if i < done { ("\u{2501}", Color::Rgb(90, 160, 110)) } else { ("\u{2500}", Color::Rgb(36, 48, 42)) };
            buf[(area.x + 2 + i, by)].set_symbol(ch).set_fg(c);
        }
        put(buf, area, area.x + 2 + w, by, &t, DIM);
        if play.demo {
            put(buf, area, area.x + 2, by.saturating_sub(0), " DEMO ", AMBER);
        }
    }
}

fn draw_pause(buf: &mut Buffer, area: Rect, item: usize, items: [&str; 3]) {
    let w = 28.min(area.width);
    let h = 7.min(area.height);
    let r = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    fill(buf, r, Color::Rgb(16, 24, 20));
    centre(buf, r, r.y + 1, "paused", AMBER);
    for (i, s) in items.iter().enumerate() {
        let sel = i == item;
        let text = if sel { format!("\u{25b8} {s}") } else { format!("  {s}") };
        put(buf, r, r.x + 4, r.y + 3 + i as u16, &text, if sel { BRIGHT } else { DIM });
    }
}

// ------------------------------------------------------------------ menus

fn draw_title(buf: &mut Buffer, area: Rect, y: u16) -> u16 {
    let title = "BELLBEAT";
    if area.width >= font::width(title) + 4 && area.height >= 22 {
        let x = area.x + (area.width - font::width(title)) / 2;
        font::draw(buf, x, y, title, AMBER);
        y + font::ROWS + 1
    } else {
        centre(buf, area, y, "\u{2407}  B E L L B E A T", AMBER);
        y + 2
    }
}

fn draw_select(app: &App, buf: &mut Buffer, area: Rect) {
    let mut y = draw_title(buf, area, area.y + 1);
    centre(buf, area, y, "every note you hit is a note of the song", DIM);
    y += 2;
    let lw = 72.min(area.width.saturating_sub(4));
    let lx = area.x + (area.width - lw) / 2;
    // the list scrolls when it is longer than the room for it
    let below = 10;
    let room = area.bottom().saturating_sub(y + below).max(3) as usize;
    let n = app.entries();
    let rows = n.min(room);
    let first = if app.sel < rows / 2 { 0 } else { (app.sel - rows / 2).min(n - rows) };
    let list = Rect::new(lx, y, lw, rows as u16);
    let d = app.diff;
    for (row_i, i) in (first..first + rows).enumerate() {
        let sel = i == app.sel;
        let row = y + row_i as u16;
        if row >= area.bottom() {
            break;
        }
        if sel {
            for x in list.x..list.right() {
                buf[(x, row)].set_bg(Color::Rgb(20, 40, 30));
            }
        }
        let Some(s) = app.songs.get(i) else {
            // a song file with an error
            if let Some(b) = app.broken.get(i - app.songs.len()) {
                put(buf, list, lx, row, if sel { "\u{25b8}" } else { " " }, AMBER);
                let name = b.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let name: String = name.chars().filter(|c| !c.is_control()).take(lw.saturating_sub(24) as usize).collect();
                put(buf, list, lx + 2, row, &name, if sel { BRIGHT } else { DIM });
                let tag = "has an error  Enter mends it";
                put(buf, list, list.right().saturating_sub(tag.len() as u16), row, tag, RED);
            }
            continue;
        };
        let user = if s.builtin { "" } else { " \u{270e}" };
        let fg = if sel { BRIGHT } else { TEXT };
        let marker = if sel {
            "\u{25b8}"
        } else if (row_i == 0 && first > 0) || (row_i + 1 == rows && first + rows < n) {
            "\u{22ee}"
        } else {
            " "
        };
        put(buf, list, lx, row, marker, AMBER);
        let x = put(buf, list, lx + 2, row, &s.title, fg);
        put(buf, list, x, row, user, DIM);
        let st = stars(app.density.get(i).map_or(0.0, |x| x[d.index()]));
        let star: String = (0..5).map(|k| if k < st { '\u{2605}' } else { '\u{2606}' }).collect();
        let meter = if s.meter != 4 { format!("{}/4", s.meter) } else if s.swing > 0.55 { "swing".into() } else { String::new() };
        let info = format!("{meter:>5} {:>3.0} bpm  {}  {}", s.bpm, mmss(s.secs()), star);
        let best = app.best(&s.id, d).map(|b| format!("  {} {:>9}{}", b.grade, thousands(b.score), if b.full_combo { "\u{2713}" } else { " " })).unwrap_or_else(|| format!("  {:>12}", "-"));
        // as much of it as there is room for: the best score goes first
        let title_end = lx + 3 + s.title.chars().count() as u16 + user.chars().count() as u16;
        let full = list.right().saturating_sub((info.chars().count() + best.chars().count()) as u16);
        let short = list.right().saturating_sub(info.chars().count() as u16);
        if full > title_end {
            let x = put(buf, list, full, row, &info, if sel { TEXT } else { DIM });
            put(buf, list, x, row, &best, if sel { AMBER } else { DIM });
        } else if short > title_end {
            put(buf, list, short, row, &info, if sel { TEXT } else { DIM });
        }
    }
    y += rows as u16 + 1;
    if let Some(s) = app.songs.get(app.sel) {
        centre(buf, area, y, &s.blurb, DIM);
    } else if let Some(b) = app.sel.checked_sub(app.songs.len()).and_then(|i| app.broken.get(i)) {
        let msg: String = b.err.to_string().chars().take(area.width.saturating_sub(4) as usize).collect();
        centre(buf, area, y, &msg, RED);
    }
    y += 2;
    // the difficulty
    let mut parts = Vec::new();
    for x in Difficulty::ALL {
        parts.push(if x == d { format!("[{}]", x.name()) } else { format!(" {} ", x.name()) });
    }
    let line = format!("\u{25c2}  {}  \u{25b8}", parts.join("  "));
    let w = line.chars().count() as u16;
    let x0 = area.x + area.width.saturating_sub(w) / 2;
    put(buf, area, x0, y, &line, TEXT);
    let pos = line.find('[').map(|b| line[..b].chars().count() as u16).unwrap_or(0);
    put(buf, area, x0 + pos, y, &format!("[{}]", d.name()), AMBER);
    y += 2;
    centre(buf, area, y, difficulty_text(d), DIM);
    y += 2;
    // a little scope of the preview
    if area.bottom() > y + 6 {
        let sr = Rect::new(lx, y, lw, 2);
        scope(app, buf, sr, (90, 200, 130));
        y += 3;
    }
    let sound = if app.audio.muted { "sound off" } else if !app.audio.device() { "no sound device" } else { "sound on" };
    let settings = format!("speed {}  [ ]     offset {:+} ms  - +     c calibrate     m {sound}", app.settings.speed, app.settings.offset_ms);
    if y < area.bottom() {
        centre(buf, area, y, &settings, TEXT);
    }
    let keys = if app.exact { "D F J K or \u{2190} \u{2193} \u{2191} \u{2192}, or a gamepad" } else { "D F J K or \u{2190} \u{2193} \u{2191} \u{2192}, or a gamepad (held notes hold themselves here)" };
    if y + 1 < area.bottom() {
        centre(buf, area, y + 1, &format!("play with {keys}"), DIM);
    }
    if y + 2 < area.bottom() {
        centre(buf, area, y + 2, "Enter play  \u{2191}\u{2193} song  \u{2190}\u{2192} difficulty  p practise  e edit  n new song  Esc quit", DIM);
    }
    if let Some((s, at)) = &app.note {
        if at.elapsed().as_secs_f64() < 2.0 {
            centre(buf, area, area.bottom().saturating_sub(1), s, AMBER);
        }
    }
}

fn difficulty_text(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Easy => "the melody's notes on the beat; the rest plays itself",
        Difficulty::Normal => "the melody's notes on every eighth",
        Difficulty::Hard => "the whole melody, its chords, and the snare with your other hand",
    }
}

fn draw_practice(app: &App, buf: &mut Buffer, area: Rect) {
    let Some(pr) = &app.practice else { return };
    let mut y = area.y + 1;
    centre(buf, area, y, "P R A C T I C E", AMBER);
    y += 2;
    centre(buf, area, y, &pr.song.title, BRIGHT);
    y += 1;
    centre(buf, area, y, "one section on repeat, slower if you like; no groove meter", DIM);
    y += 2;
    let parts = pr.parts();
    let below = 8;
    let room = area.bottom().saturating_sub(y + below).max(3) as usize;
    let rows = parts.len().min(room);
    let first = if pr.section < rows / 2 { 0 } else { (pr.section - rows / 2).min(parts.len() - rows) };
    let lw = 40.min(area.width.saturating_sub(4));
    let lx = area.x + (area.width - lw) / 2;
    for (row_i, i) in (first..first + rows).enumerate() {
        let (name, at, steps) = &parts[i];
        let row = y + row_i as u16;
        let sel = i == pr.section;
        if sel {
            for x in lx..lx + lw {
                buf[(x, row)].set_bg(Color::Rgb(20, 40, 30));
            }
        }
        put(buf, area, lx, row, if sel { "\u{25b8}" } else { " " }, AMBER);
        put(buf, area, lx + 2, row, &format!("{:>2}  {name}", i + 1), if sel { BRIGHT } else { TEXT });
        let info = format!("{} bars  {}", steps / pr.song.bar(), mmss(pr.song.time(*at)));
        put(buf, area, (lx + lw).saturating_sub(info.chars().count() as u16 + 1), row, &info, if sel { TEXT } else { DIM });
    }
    y += rows as u16 + 1;
    let mut parts_s = Vec::new();
    for (i, k) in SPEEDS.iter().enumerate().rev() {
        let t = format!("{:.0}%", k * 100.0);
        parts_s.push(if i == pr.speed { format!("[{t}]") } else { format!(" {t} ") });
    }
    centre(buf, area, y, &format!("tempo  {}", parts_s.join(" ")), TEXT);
    y += 1;
    centre(buf, area, y, &format!("{:.0} bpm", pr.song.bpm * SPEEDS[pr.speed]), DIM);
    y += 2;
    centre(buf, area, y, &format!("{}: {}", app.diff.name(), difficulty_text(app.diff)), TEXT);
    y += 2;
    if let Some(a) = pr.last {
        centre(buf, area, y, &format!("{} rounds, the last at {:.1}%", pr.runs, a * 100.0), DIM);
    }
    y += 1;
    if y < area.bottom() {
        centre(buf, area, area.bottom() - 1, "Enter play  \u{2191}\u{2193} section  \u{2190}\u{2192} tempo  Tab difficulty  Esc back", DIM);
    }
}

/// What a token in a song file is, for its colour.
fn token_colour(word: &str, first: bool, key: Option<&str>) -> Color {
    if first {
        return AMBER;
    }
    match word {
        "-" => Color::Rgb(90, 120, 105),
        "." => Color::Rgb(55, 70, 62),
        _ => match key {
            Some("lead") if crate::song::parse_note(word).is_some() => BRIGHT,
            Some("harmony") if crate::song::parse_note(word).is_some() => Color::Rgb(120, 220, 255),
            Some("kick" | "snare" | "hat") => Color::Rgb(255, 150, 120),
            Some("chords") => GREEN,
            _ => TEXT,
        },
    }
}

/// The editor's text area for a screen: columns and rows.
pub fn editor_text_size(area: Rect) -> (usize, usize) {
    (area.width.saturating_sub(6) as usize, area.height.saturating_sub(4) as usize)
}

fn draw_editor(app: &App, e: &Editor, buf: &mut Buffer, area: Rect) {
    if area.height < 8 || area.width < 30 {
        centre(buf, area, area.y, "the editor needs 30x8 at least", AMBER);
        return;
    }
    // the header
    let file = e.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let title = match &e.parsed {
        Ok(s) => s.title.clone(),
        Err(_) => e.id.clone(),
    };
    let x = put(buf, area, area.x + 1, area.y, "\u{2407} EDIT  ", AMBER);
    let x = put(buf, area, x, area.y, &title, BRIGHT);
    let x = put(buf, area, x + 2, area.y, &file, DIM);
    if e.dirty {
        put(buf, area, x + 2, area.y, "not saved", AMBER);
    }
    if e.listening {
        let s = "\u{266a} listening";
        put(buf, area, area.right().saturating_sub(s.chars().count() as u16 + 1), area.y, s, GREEN);
    }
    // the text, with line numbers
    let gutter = 5u16;
    let text_top = area.y + 1;
    let text_h = area.height.saturating_sub(4);
    let text_w = area.width.saturating_sub(gutter + 1);
    let err_line = e.parsed.as_ref().err().map(|x| x.line).filter(|l| *l > 0);
    let mut key: Option<String> = None;
    // the key in force at the first line shown
    for l in &e.lines[..e.top.min(e.lines.len())] {
        let t: String = l.iter().collect();
        if t.trim().is_empty() || t.trim_start().starts_with('#') {
            continue;
        }
        if !t.starts_with(' ') {
            key = t.split_whitespace().next().map(|w| w.to_ascii_lowercase());
        }
    }
    for r in 0..text_h as usize {
        let li = e.top + r;
        let y = text_top + r as u16;
        let Some(line) = e.lines.get(li) else { break };
        let is_err = err_line == Some(li + 1);
        put(buf, area, area.x, y, &format!("{:>4}", li + 1), if is_err { RED } else { Color::Rgb(50, 66, 58) });
        if is_err {
            buf[(area.x + 4, y)].set_symbol("\u{25b8}").set_fg(RED);
        }
        let t: String = line.iter().collect();
        let comment = t.trim_start().starts_with('#');
        let header = t.trim_start().starts_with('[');
        if !comment && !t.trim().is_empty() && !t.starts_with(' ') {
            key = t.split_whitespace().next().map(|w| w.to_ascii_lowercase());
        }
        // colour word by word
        let mut colours = vec![TEXT; line.len()];
        if comment {
            colours.fill(DIM);
        } else if header {
            colours.fill(AMBER);
        } else {
            let mut i = 0;
            let mut first = !t.starts_with(' ');
            while i < line.len() {
                if line[i].is_whitespace() {
                    i += 1;
                    continue;
                }
                let j = (i..line.len()).find(|&j| line[j].is_whitespace()).unwrap_or(line.len());
                let w: String = line[i..j].iter().collect();
                let c = token_colour(&w, first, key.as_deref());
                colours[i..j].fill(c);
                first = false;
                i = j;
            }
        }
        for cx in 0..text_w as usize {
            let ci = e.left + cx;
            let x = area.x + gutter + cx as u16;
            let cursor = li == e.row && ci == e.col.min(line.len());
            if let Some(&ch) = line.get(ci) {
                let mut tmp = [0u8; 4];
                buf[(x, y)].set_symbol(ch.encode_utf8(&mut tmp)).set_fg(colours[ci]);
            }
            if cursor {
                buf[(x, y)].set_bg(AMBER).set_fg(BG);
            }
        }
        if e.left > 0 && !line.is_empty() {
            buf[(area.x + gutter, y)].set_symbol("\u{2039}").set_fg(DIM);
        }
    }
    // the status lines
    let sy = area.bottom() - 3;
    for x in area.x..area.right() {
        buf[(x, sy)].set_symbol("\u{2500}").set_fg(Color::Rgb(30, 44, 38));
    }
    let w = area.width.saturating_sub(2) as usize;
    let clip = |s: String| -> String { s.chars().take(w).collect() };
    match &e.message {
        Some((m, bad, at)) if at.elapsed().as_secs_f64() < 4.0 => {
            put(buf, area, area.x + 1, sy + 1, &clip(m.clone()), if *bad { RED } else { GREEN });
        }
        _ => match &e.parsed {
            Ok(s) => {
                let sc = s.score();
                let hard = crate::chart::build(s, &sc, Difficulty::Hard).notes.len();
                let feel = if s.swing > 0.55 { format!(", swing {:.0}", s.swing * 100.0) } else { String::new() };
                let m = format!("\u{2713} {}/4, {:.0} bpm{feel}, {}, {} melody notes, {hard} to play on Hard", s.meter, s.bpm, mmss(s.secs()), sc.lead.len());
                put(buf, area, area.x + 1, sy + 1, &clip(m), GREEN);
            }
            Err(err) => {
                put(buf, area, area.x + 1, sy + 1, &clip(format!("\u{2717} {err}")), RED);
            }
        },
    }
    let help = if area.width >= 96 {
        "Ctrl-S save  Ctrl-L listen  Ctrl-P play from here  Ctrl-Z undo  Ctrl-D/K copy/cut line  Esc leave"
    } else if area.width >= 72 {
        "^S save  ^L listen  ^P play from here  ^Z undo  ^D/^K copy/cut line  Esc leave"
    } else {
        "^S save  ^L listen  ^P play  ^Z undo  Esc leave"
    };
    let _ = app;
    put(buf, area, area.x + 1, sy + 2, &clip(help.into()), DIM);
}

fn grade_colour(g: &str) -> Color {
    match g {
        "S" => AMBER,
        "A" => GREEN,
        "B" => Color::Rgb(80, 210, 255),
        "C" | "D" => TEXT,
        _ => RED,
    }
}

fn draw_results(app: &App, play: &Play, new_best: bool, age: f64, buf: &mut Buffer, area: Rect) {
    let mut y = area.y + 1;
    centre(buf, area, y, &play.def.title, BRIGHT);
    y += 1;
    centre(buf, area, y, &format!("{}  {} notes", play.diff.name(), play.notes().len()), DIM);
    y += 2;
    let g = play.grade();
    if area.height >= 20 {
        let x = area.x + (area.width.saturating_sub(font::width(g))) / 2;
        font::draw(buf, x, y, g, grade_colour(g));
        y += font::ROWS + 1;
    } else {
        centre(buf, area, y, &format!("grade {g}"), grade_colour(g));
        y += 2;
    }
    if play.failed {
        centre(buf, area, y, "the groove ran out", RED);
        y += 2;
    } else if play.full_combo() && age.fract() < 0.7 {
        centre(buf, area, y, "FULL COMBO!", AMBER);
        y += 2;
    } else {
        y += 2;
    }
    let lines = [
        (format!("score       {:>10}", thousands(play.score())), BRIGHT),
        (format!("accuracy    {:>9.1}%", play.accuracy() * 100.0), TEXT),
        (format!("best combo  {:>10}", play.max_combo), TEXT),
        (format!("perfect     {:>10}", play.counts[0]), AMBER),
        (format!("good        {:>10}", play.counts[1]), GREEN),
        (format!("miss        {:>10}", play.counts[2]), RED),
    ];
    for (s, c) in lines {
        if y >= area.bottom() {
            break;
        }
        centre(buf, area, y, &s, c);
        y += 1;
    }
    y += 1;
    if new_best && y < area.bottom() {
        centre(buf, area, y, if age.fract() < 0.6 { "NEW BEST!" } else { "" }, AMBER);
        y += 2;
    } else if let Some(b) = app.best(&play.def.id, play.diff) {
        if !play.demo && y < area.bottom() {
            centre(buf, area, y, &format!("your best: {} {}", b.grade, thousands(b.score)), DIM);
        }
        y += 2;
    }
    if y < area.bottom() {
        centre(buf, area, y, if play.demo { "the demo goes on to the next song; Esc stops it" } else { "Enter songs    R play again" }, DIM);
    }
}

fn draw_calibrate(app: &App, cal: &Cal, buf: &mut Buffer, area: Rect) {
    let mut y = area.y + 1;
    centre(buf, area, y, "C A L I B R A T E", AMBER);
    y += 2;
    for s in [
        "Sound takes a moment to reach your ears, more on some machines.",
        "Tap any lane key (or Space) on every click you hear;",
        "close your eyes if it helps. Sixteen taps.",
    ] {
        centre(buf, area, y, s, TEXT);
        y += 1;
    }
    y += 1;
    // the beat
    let since = app.clock.t - cal.flash;
    let on = app.clock.t >= 0.0 && since < 0.12;
    centre(buf, area, y, if on { "\u{25cf}" } else { "\u{25cb}" }, if on { AMBER } else { DIM });
    if app.clock.t < 0.0 {
        centre(buf, area, y + 1, "get ready", DIM);
    }
    y += 3;
    // where the taps fell, from 150 ms early to 150 ms late
    let w = 61.min(area.width.saturating_sub(4));
    let x0 = area.x + (area.width - w) / 2;
    for i in 0..w {
        buf[(x0 + i, y)].set_symbol("\u{2500}").set_fg(Color::Rgb(40, 60, 50));
    }
    buf[(x0 + w / 2, y)].set_symbol("\u{253c}").set_fg(TEXT);
    put(buf, area, x0, y + 1, "early", DIM);
    put(buf, area, x0 + w.saturating_sub(4), y + 1, "late", DIM);
    for (i, off) in cal.taps.iter().enumerate() {
        let k = ((off + 0.15) / 0.3).clamp(0.0, 1.0);
        let x = x0 + (k * (w - 1) as f64).round() as u16;
        let newest = i + 1 == cal.taps.len();
        buf[(x, y)].set_symbol("\u{25cf}").set_fg(if newest { AMBER } else { GREEN });
    }
    y += 3;
    centre(buf, area, y, &format!("taps {}/{}", cal.taps.len(), CAL_TAPS), TEXT);
    y += 2;
    if let Some(ms) = cal.median_ms() {
        let s = if cal.done { format!("your offset: {ms:+} ms") } else { format!("so far: {ms:+} ms") };
        centre(buf, area, y, &s, if cal.done { AMBER } else { TEXT });
    }
    y += 2;
    let help = if cal.done {
        format!("Enter keeps it    R again    Esc leaves it at {:+} ms", app.settings.offset_ms)
    } else {
        "Esc back to the songs".to_string()
    };
    centre(buf, area, y, &help, DIM);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_helpers() {
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(12), "12");
        assert_eq!(mmss(75.4), "1:15");
        assert_eq!(note_name(69), "A4");
        assert_eq!(stars(0.5), 1);
        assert_eq!(stars(6.0), 5);
    }

    fn text(buf: &Buffer) -> String {
        let a = buf.area;
        (0..a.height).map(|y| (0..a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string() + "\n").collect()
    }

    #[test]
    fn every_screen_draws_at_every_size() {
        use crate::app::{App, Cal, Screen};
        use crate::synth::Audio;
        let mut app = App::new(Audio::headless(), crate::save::Settings::default(), Vec::new(), true);
        let def = app.songs[2].clone();
        let mut p = Play::new(def, Difficulty::Hard, None, 2.5);
        p.demo = true;
        let mut t = -3.0;
        while !p.finished {
            t += 1.0 / 30.0;
            p.update(t);
        }
        let screens = [
            Screen::Select,
            Screen::Play { play: Box::new(Play::new(app.songs[0].clone(), Difficulty::Easy, None, 2.5)), paused: true, item: 1, over_at: None },
            Screen::Results { play: Box::new(p), new_best: true, shown: std::time::Instant::now() },
            Screen::Calibrate(Cal { bpm: 100.0, taps: vec![0.02, 0.04, -0.01], flash: 0.0, done: false }),
            Screen::Practice,
            Screen::Edit,
        ];
        let mut ed = Editor::new(std::path::PathBuf::from("/nowhere/a.song"), crate::song::TEMPLATE);
        ed.row = 20;
        ed.col = 30;
        ed.scroll(50, 10);
        app.editor = Some(Box::new(ed));
        app.practice = Some(crate::app::Practice { song: app.songs[8].clone(), section: 2, speed: 3, runs: 2, last: Some(0.9), editor: false });
        for (i, s) in screens.into_iter().enumerate() {
            app.screen = s;
            for (w, h) in [(60, 20), (80, 24), (100, 30), (160, 50)] {
                let a = Rect::new(0, 0, w, h);
                let mut buf = Buffer::empty(a);
                draw(&app, &mut buf, a);
                let out = text(&buf);
                if std::env::var("BELLBEAT_PRINT").is_ok_and(|v| v == format!("{i}-{w}")) {
                    println!("{out}");
                }
                assert!(out.chars().any(|c| c.is_alphanumeric()), "screen {i} at {w}x{h} is empty");
            }
        }
    }

    #[test]
    fn the_field_fits_small_and_large_terminals() {
        for (w, h) in [(60, 20), (80, 24), (100, 30), (200, 60)] {
            let a = Rect::new(0, 0, w, h);
            let g = geo(a);
            assert!(g.x0 + g.fw <= w, "{w}x{h}");
            assert!(g.hy + 1 < h, "{w}x{h}");
        }
    }
}
