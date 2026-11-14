//! Best scores and settings, in the player's state folder:
//! `$XDG_STATE_HOME/bellbeat`, else `~/.local/state/bellbeat` (Linux),
//! `~/Library/Application Support/bellbeat` (macOS), `%LocalAppData%\bellbeat`
//! (Windows).
//!
//! Both files are plain text: `scores` has one line per song and
//! difficulty (`<song> <difficulty> <score> <accuracy per mille> <best
//! combo> <full combo 0/1> <grade>`), `settings` has `offset_ms` and `speed`.

use std::path::PathBuf;

pub fn dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(d) = env("BELLBEAT_STATE") {
        return Some(d);
    }
    // tests never touch the player's own folder
    if cfg!(test) {
        return None;
    }
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_STATE_HOME").or_else(|| env("HOME").map(|h| h.join(".local/state")))
    };
    Some(base?.join("bellbeat"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Best {
    pub song: String,
    pub diff: String,
    pub score: u64,
    pub accuracy: u32,
    pub combo: u32,
    pub full_combo: bool,
    pub grade: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// How late the sound reaches the player, in milliseconds.
    pub offset_ms: i32,
    /// How fast the notes fall, 1 to 10.
    pub speed: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { offset_ms: 0, speed: 5 }
    }
}

fn write(name: &str, text: &str) {
    let Some(d) = dir() else { return };
    let p = d.join(name);
    match std::fs::create_dir_all(&d).and_then(|_| std::fs::write(&p, text)) {
        Ok(()) => log::info!("saved {}", p.display()),
        Err(e) => log::warn!("not saved to {}: {e}", p.display()),
    }
}

fn read(name: &str) -> String {
    dir().and_then(|d| std::fs::read_to_string(d.join(name)).ok()).unwrap_or_default()
}

pub fn parse_scores(text: &str) -> Vec<Best> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 7 {
                return None;
            }
            Some(Best {
                song: f[0].into(),
                diff: f[1].into(),
                score: f[2].parse().ok()?,
                accuracy: f[3].parse().ok()?,
                combo: f[4].parse().ok()?,
                full_combo: f[5] == "1",
                grade: f[6].into(),
            })
        })
        .collect()
}

pub fn format_scores(all: &[Best]) -> String {
    all.iter().map(|b| format!("{} {} {} {} {} {} {}\n", b.song, b.diff, b.score, b.accuracy, b.combo, b.full_combo as u8, b.grade)).collect()
}

pub fn load_scores() -> Vec<Best> {
    parse_scores(&read("scores"))
}

/// Keep `b` if it beats what is there; true when it did.
pub fn record(all: &mut Vec<Best>, b: Best) -> bool {
    match all.iter_mut().find(|x| x.song == b.song && x.diff == b.diff) {
        Some(x) if x.score >= b.score => {
            // a full combo is kept even on a lower score
            if b.full_combo && !x.full_combo {
                x.full_combo = true;
                write("scores", &format_scores(all));
            }
            false
        }
        Some(x) => {
            let fc = x.full_combo || b.full_combo;
            *x = b;
            x.full_combo = fc;
            write("scores", &format_scores(all));
            true
        }
        None => {
            all.push(b);
            write("scores", &format_scores(all));
            true
        }
    }
}

pub fn parse_settings(text: &str) -> Settings {
    let mut s = Settings::default();
    for l in text.lines() {
        let mut f = l.split_whitespace();
        match (f.next(), f.next().and_then(|v| v.parse::<i64>().ok())) {
            (Some("offset_ms"), Some(v)) => s.offset_ms = v.clamp(-300, 300) as i32,
            (Some("speed"), Some(v)) => s.speed = v.clamp(1, 10) as u32,
            _ => {}
        }
    }
    s
}

pub fn load_settings() -> Settings {
    parse_settings(&read("settings"))
}

pub fn save_settings(s: &Settings) {
    write("settings", &format!("offset_ms {}\nspeed {}\n", s.offset_ms, s.speed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_round_trip_and_only_improve() {
        let b = |score, fc| Best { song: "hello-world".into(), diff: "Normal".into(), score, accuracy: 950, combo: 80, full_combo: fc, grade: "S".into() };
        let text = format_scores(&[b(1234, true)]);
        assert_eq!(parse_scores(&text), vec![b(1234, true)]);
        std::env::set_var("BELLBEAT_STATE", std::env::temp_dir().join(format!("bellbeat-test-{}", std::process::id())));
        let mut all = vec![b(5000, false)];
        assert!(!record(&mut all, b(4000, true)));
        assert!(all[0].full_combo && all[0].score == 5000);
        assert!(record(&mut all, b(6000, false)));
        assert!(all[0].full_combo && all[0].score == 6000);
        let _ = std::fs::remove_dir_all(dir().unwrap());
    }

    #[test]
    fn settings_parse_and_clamp() {
        assert_eq!(parse_settings("offset_ms 45\nspeed 7\n"), Settings { offset_ms: 45, speed: 7 });
        assert_eq!(parse_settings("offset_ms 9000\nspeed 0\n"), Settings { offset_ms: 300, speed: 1 });
        assert_eq!(parse_settings("junk"), Settings::default());
    }
}
