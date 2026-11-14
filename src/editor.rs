//! The song editor: a song file as text, in the terminal. Type the melody
//! in the song notation and hear each note as you finish it; listen to the
//! song from where the cursor is; play it from there; save it to the songs
//! folder, where the song list finds it.
//!
//! The text is parsed again after every change, so the editor always says
//! whether the song is whole, or what is wrong and on which line.

use crate::input::K;
use crate::song::{self, parse_note, Song, SongError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Longest line and most lines the editor takes (a song file is far less).
const MAX_LINE: usize = 400;
const MAX_LINES: usize = 4000;
/// Tab moves to the next multiple of this, where the notes start in the
/// built-in songs (`lead   E5 ...`).
const TAB: usize = 7;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    None,
    /// The text changed; with a note's MIDI number when one was just typed.
    Edited(Option<i32>),
    Save,
    Listen,
    Play,
    Leave,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Type,
    Delete,
    Other,
}

#[derive(Clone)]
struct Snap {
    lines: Vec<Vec<char>>,
    row: usize,
    col: usize,
}

pub struct Editor {
    pub path: PathBuf,
    pub id: String,
    pub lines: Vec<Vec<char>>,
    pub row: usize,
    pub col: usize,
    /// The first line and column on screen.
    pub top: usize,
    pub left: usize,
    pub dirty: bool,
    /// The text as last saved (or opened), to tell whether it changed.
    saved: String,
    pub parsed: Result<Song, SongError>,
    pub listening: bool,
    /// A message for the status line: text, whether it is bad news, since.
    pub message: Option<(String, bool, Instant)>,
    /// Esc pressed once with changes unsaved: the next Esc leaves.
    pub leave_armed: bool,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Option<(Kind, Instant)>,
}

/// Text as editor lines: tabs as spaces, any other control character as `?`.
fn to_lines(text: &str) -> Vec<Vec<char>> {
    let mut lines: Vec<Vec<char>> = text
        .lines()
        .take(MAX_LINES)
        .map(|l| {
            let mut out = Vec::new();
            for c in l.chars() {
                match c {
                    '\t' => {
                        out.push(' ');
                        while out.len() % TAB != 0 {
                            out.push(' ');
                        }
                    }
                    c if c.is_control() => out.push('?'),
                    c => out.push(c),
                }
            }
            out.truncate(MAX_LINE);
            out
        })
        .collect();
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

impl Editor {
    pub fn new(path: PathBuf, text: &str) -> Editor {
        let id = song::id_from_path(&path);
        let lines = to_lines(text);
        let mut e = Editor {
            parsed: Err(SongError { line: 0, msg: String::new() }),
            path,
            id,
            lines,
            row: 0,
            col: 0,
            top: 0,
            left: 0,
            dirty: false,
            saved: String::new(),
            listening: false,
            message: None,
            leave_armed: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last: None,
        };
        e.reparse();
        e.saved = e.text();
        e
    }

    /// Not on disk yet: unsaved until saved, whatever is typed.
    pub fn mark_new(&mut self) {
        self.saved.clear();
        self.dirty = true;
    }

    /// Whether the text differs from the file, after an edit.
    pub fn changed(&mut self) {
        self.dirty = self.text() != self.saved;
    }

    /// Open a song file, as safely as the song list reads it.
    pub fn open(path: &Path) -> Result<Editor, String> {
        let meta = std::fs::metadata(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        if meta.len() > song::MAX_FILE {
            return Err(format!("{} is too long to be a song", path.display()));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut e = Editor::new(path.to_path_buf(), &text);
        if std::str::from_utf8(&bytes).is_err() {
            e.say("this file was not UTF-8 text: odd characters are shown as \u{fffd}", true);
        }
        Ok(e)
    }

    pub fn text(&self) -> String {
        let mut s: String = self.lines.iter().map(|l| l.iter().collect::<String>() + "\n").collect();
        while s.ends_with("\n\n") {
            s.pop();
        }
        s
    }

    pub fn reparse(&mut self) {
        self.parsed = song::parse(&self.text(), &self.id).map(|mut s| {
            s.path = Some(self.path.clone());
            s
        });
    }

    pub fn say(&mut self, s: impl Into<String>, bad: bool) {
        self.message = Some((s.into(), bad, Instant::now()));
    }

    /// Write the file (beside it first, then over it).
    pub fn save(&mut self) -> Result<(), String> {
        let dir = self.path.parent().ok_or("no folder to save in")?;
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        let tmp = self.path.with_extension("song.saving");
        std::fs::write(&tmp, self.text()).and_then(|_| std::fs::rename(&tmp, &self.path)).map_err(|e| format!("not saved: {e}"))?;
        self.dirty = false;
        self.saved = self.text();
        self.leave_armed = false;
        log::info!("saved song {}", self.path.display());
        Ok(())
    }

    fn snapshot(&mut self, kind: Kind) {
        let now = Instant::now();
        // a run of typing (or of deleting) undoes in one go
        let joined = kind != Kind::Other && self.last.is_some_and(|(k, at)| k == kind && now - at < Duration::from_millis(1200));
        if !joined {
            self.undo.push(Snap { lines: self.lines.clone(), row: self.row, col: self.col });
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some((kind, now));
    }

    fn restore(&mut self, from_undo: bool) -> bool {
        let (src, dst) = if from_undo { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        let Some(s) = src.pop() else { return false };
        dst.push(Snap { lines: self.lines.clone(), row: self.row, col: self.col });
        self.lines = s.lines;
        self.row = s.row.min(self.lines.len() - 1);
        self.col = s.col.min(self.lines[self.row].len());
        self.last = None;
        true
    }

    fn line_len(&self) -> usize {
        self.lines[self.row].len()
    }

    fn total(&self) -> usize {
        self.lines.iter().map(|l| l.len() + 1).sum()
    }

    /// The key a line belongs to: its own first word, or for a carried-on
    /// line the first word of the line it carries on.
    fn key_of(&self, row: usize) -> Option<String> {
        for r in (0..=row).rev() {
            let l = &self.lines[r];
            let t: String = l.iter().collect();
            let tt = t.trim();
            if tt.is_empty() || tt.starts_with('#') {
                if r == row {
                    return None;
                }
                continue;
            }
            if l.first().is_some_and(|c| *c == ' ') {
                continue;
            }
            return tt.split_whitespace().next().map(|w| w.to_ascii_lowercase());
        }
        None
    }

    /// The note just before the cursor, on a melody line.
    fn note_before(&self) -> Option<i32> {
        if !matches!(self.key_of(self.row).as_deref(), Some("lead" | "harmony")) {
            return None;
        }
        let l = &self.lines[self.row];
        let end = self.col.min(l.len());
        let start = l[..end].iter().rposition(|c| c.is_whitespace()).map_or(0, |i| i + 1);
        let tok: String = l[start..end].iter().collect();
        parse_note(&tok).filter(|m| (24..=108).contains(m))
    }

    /// The section the cursor is in, by its first place in the song's order.
    pub fn section_at_cursor(&self) -> usize {
        let Ok(song) = &self.parsed else { return 0 };
        let mut name = None;
        for l in &self.lines[..=self.row] {
            let t: String = l.iter().collect();
            if let Some(rest) = t.trim().strip_prefix('[') {
                if let Some((n, _)) = rest.split_once(']') {
                    name = Some(n.trim().to_string());
                }
            }
        }
        let Some(name) = name else { return 0 };
        let Some(si) = song.sections.iter().position(|s| s.name == name) else { return 0 };
        song.order.iter().position(|&o| o == si).unwrap_or(0)
    }

    fn insert(&mut self, c: char) -> bool {
        if self.line_len() >= MAX_LINE || self.total() as u64 >= song::MAX_FILE {
            self.say("that line is as long as a line can be", true);
            return false;
        }
        let col = self.col.min(self.line_len());
        self.lines[self.row].insert(col, c);
        self.col = col + 1;
        true
    }

    /// A key pressed in the editor.
    pub fn key(&mut self, k: K) -> Action {
        if k != K::Esc && k != K::Ctrl('q') {
            self.leave_armed = false;
        }
        let edited = |note| Action::Edited(note);
        match k {
            K::Esc | K::Ctrl('q') => return Action::Leave,
            K::Ctrl('s') | K::F(2) => return Action::Save,
            K::Ctrl('l') | K::F(5) => return Action::Listen,
            K::Ctrl('p') | K::F(6) => return Action::Play,
            K::Ctrl('z') => {
                if self.restore(true) {
                    self.dirty = true;
                    return edited(None);
                }
                self.say("nothing to undo", false);
            }
            K::Ctrl('y') => {
                if self.restore(false) {
                    self.dirty = true;
                    return edited(None);
                }
                self.say("nothing to redo", false);
            }
            K::Ctrl('d') => {
                // the line again, below: the quickest way to a new bar
                if self.lines.len() < MAX_LINES && self.total() + self.line_len() < song::MAX_FILE as usize {
                    self.snapshot(Kind::Other);
                    let l = self.lines[self.row].clone();
                    self.lines.insert(self.row + 1, l);
                    self.row += 1;
                    self.dirty = true;
                    return edited(None);
                }
            }
            K::Ctrl('k') => {
                self.snapshot(Kind::Other);
                if self.lines.len() > 1 {
                    self.lines.remove(self.row);
                    self.row = self.row.min(self.lines.len() - 1);
                } else {
                    self.lines[0].clear();
                }
                self.col = self.col.min(self.line_len());
                self.dirty = true;
                return edited(None);
            }
            K::Char(c) if !c.is_control() => {
                self.snapshot(Kind::Type);
                let note = if c == ' ' { self.note_before() } else { None };
                if self.insert(c) {
                    self.dirty = true;
                    return edited(note);
                }
            }
            K::Tab => {
                self.snapshot(Kind::Type);
                let note = self.note_before();
                let mut any = false;
                while self.insert(' ') {
                    any = true;
                    if self.col % TAB == 0 {
                        break;
                    }
                }
                if any {
                    self.dirty = true;
                    return edited(note);
                }
            }
            K::Enter => {
                if self.lines.len() >= MAX_LINES {
                    self.say("that is as many lines as a song can have", true);
                    return Action::None;
                }
                self.snapshot(Kind::Other);
                let note = self.note_before();
                let col = self.col.min(self.line_len());
                let rest = self.lines[self.row].split_off(col);
                // carry on a melody: a new line under a key line starts
                // where its notes do
                let indent = if rest.is_empty() && self.key_of(self.row).is_some() {
                    let l = &self.lines[self.row];
                    let first_ws = l.iter().position(|c| c.is_whitespace());
                    match first_ws {
                        Some(i) if l.first().is_some_and(|c| !c.is_whitespace()) => i + l[i..].iter().take_while(|c| **c == ' ').count(),
                        _ => l.iter().take_while(|c| **c == ' ').count(),
                    }
                    .min(MAX_LINE / 2)
                } else {
                    0
                };
                let mut new = vec![' '; indent];
                new.extend(rest);
                self.lines.insert(self.row + 1, new);
                self.row += 1;
                self.col = indent;
                self.dirty = true;
                return edited(note);
            }
            K::Back => {
                let col = self.col.min(self.line_len());
                if col > 0 {
                    self.snapshot(Kind::Delete);
                    self.lines[self.row].remove(col - 1);
                    self.col = col - 1;
                } else if self.row > 0 {
                    self.snapshot(Kind::Other);
                    let l = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.line_len();
                    self.lines[self.row].extend(l);
                    self.lines[self.row].truncate(MAX_LINE);
                } else {
                    return Action::None;
                }
                self.dirty = true;
                return edited(None);
            }
            K::Delete => {
                let col = self.col.min(self.line_len());
                if col < self.line_len() {
                    self.snapshot(Kind::Delete);
                    self.lines[self.row].remove(col);
                } else if self.row + 1 < self.lines.len() {
                    self.snapshot(Kind::Other);
                    let l = self.lines.remove(self.row + 1);
                    self.lines[self.row].extend(l);
                    self.lines[self.row].truncate(MAX_LINE);
                } else {
                    return Action::None;
                }
                self.col = col;
                self.dirty = true;
                return edited(None);
            }
            K::Left => {
                if self.col > 0 {
                    self.col = self.col.min(self.line_len()) - 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.line_len();
                }
            }
            K::Right => {
                if self.col < self.line_len() {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            }
            K::Up => self.row = self.row.saturating_sub(1),
            K::Down => self.row = (self.row + 1).min(self.lines.len() - 1),
            K::PageUp => self.row = self.row.saturating_sub(15),
            K::PageDown => self.row = (self.row + 15).min(self.lines.len() - 1),
            K::Home => {
                // to the first word, then to the very start
                let first = self.lines[self.row].iter().position(|c| !c.is_whitespace()).unwrap_or(0);
                self.col = if self.col == first { 0 } else { first };
            }
            K::End => self.col = self.line_len(),
            _ => {}
        }
        Action::None
    }

    /// Scroll so the cursor shows in a text area `w` by `h`.
    pub fn scroll(&mut self, w: usize, h: usize) {
        let h = h.max(1);
        let w = w.max(4);
        if self.row < self.top {
            self.top = self.row;
        } else if self.row >= self.top + h {
            self.top = self.row + 1 - h;
        }
        let col = self.col.min(self.line_len());
        if col < self.left {
            self.left = col.saturating_sub(w / 4);
        } else if col >= self.left + w {
            self.left = col + 1 - w + w / 4;
        }
    }

    /// The new song's file: `my-song.song`, or with a number when taken.
    pub fn new_path(dir: &Path, base: &str) -> PathBuf {
        let mut p = dir.join(format!("{base}.song"));
        let mut n = 2;
        while p.exists() && n < 1000 {
            p = dir.join(format!("{base}-{n}.song"));
            n += 1;
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str) -> Editor {
        Editor::new(PathBuf::from("/nowhere/test-song.song"), text)
    }

    fn typing(e: &mut Editor, s: &str) -> Vec<Action> {
        s.chars().map(|c| e.key(if c == '\n' { K::Enter } else { K::Char(c) })).collect()
    }

    #[test]
    fn the_template_opens_whole() {
        let e = ed(song::TEMPLATE);
        assert!(e.parsed.is_ok(), "{:?}", e.parsed.as_ref().err());
        assert_eq!(e.text(), song::TEMPLATE);
        assert_eq!(e.id, "test-song");
    }

    #[test]
    fn typing_a_note_on_a_melody_line_sounds_it() {
        let mut e = ed("title T\nbpm 100\n[a] 1 bar\nchords C\nlead \n");
        e.row = 4;
        e.col = 5;
        let acts = typing(&mut e, "C5 - E5 ");
        assert_eq!(acts[2], Action::Edited(Some(72)));
        assert_eq!(acts[4], Action::Edited(None));
        assert_eq!(acts[7], Action::Edited(Some(76)));
        // not on a chord line
        e.row = 3;
        e.col = 8;
        assert_eq!(typing(&mut e, " G ")[2], Action::Edited(None));
        assert!(e.dirty);
    }

    #[test]
    fn enter_carries_a_melody_on_under_its_notes() {
        let mut e = ed("lead   C5 - - -");
        e.col = e.lines[0].len();
        e.key(K::Enter);
        assert_eq!(e.row, 1);
        assert_eq!(e.col, 7);
        assert_eq!(e.lines[1], vec![' '; 7]);
    }

    #[test]
    fn edits_undo_and_redo() {
        let mut e = ed("title T\n");
        e.col = 7;
        typing(&mut e, "ea");
        assert_eq!(e.text(), "title Tea\n");
        e.key(K::Ctrl('z'));
        assert_eq!(e.text(), "title T\n");
        e.key(K::Ctrl('y'));
        assert_eq!(e.text(), "title Tea\n");
        e.key(K::Home);
        e.key(K::Home);
        assert_eq!(e.col, 0);
        e.key(K::Back);
        e.key(K::End);
        e.key(K::Delete);
        e.key(K::Ctrl('d'));
        assert_eq!(e.lines.len(), 2);
        e.key(K::Ctrl('k'));
        e.key(K::Ctrl('k'));
        e.key(K::Ctrl('k'));
        assert_eq!(e.text(), "\n");
    }

    #[test]
    fn the_cursor_knows_its_section() {
        let mut e = ed(song::TEMPLATE);
        e.row = e.lines.iter().position(|l| l.iter().collect::<String>().starts_with("lead")).unwrap();
        // [tune] is the second section played
        assert_eq!(e.section_at_cursor(), 1);
        e.row = 0;
        assert_eq!(e.section_at_cursor(), 0);
    }

    /// Random keys never panic, and the text stays within bounds.
    #[test]
    fn random_keys_never_panic() {
        let mut e = ed(song::TEMPLATE);
        let keys = [K::Left, K::Right, K::Up, K::Down, K::Home, K::End, K::PageUp, K::PageDown, K::Back, K::Delete, K::Enter, K::Tab, K::Char('C'), K::Char('5'), K::Char(' '), K::Char('-'), K::Char('\u{e9}'), K::Ctrl('z'), K::Ctrl('y'), K::Ctrl('d'), K::Ctrl('k')];
        let mut seed = 0x2545_f491u32;
        for _ in 0..20_000 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let k = keys[seed as usize % keys.len()];
            let _ = e.key(k);
            if seed % 7 == 0 {
                e.reparse();
                let _ = e.section_at_cursor();
            }
            e.scroll(60, 20);
            assert!(e.row < e.lines.len());
            assert!(e.lines.iter().all(|l| l.len() <= MAX_LINE));
        }
    }

    #[test]
    fn control_characters_never_reach_the_screen() {
        let e = ed("title \u{1b}[2J\tx\n");
        assert!(e.lines[0].iter().all(|c| !c.is_control()));
    }
}
