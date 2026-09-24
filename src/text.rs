//! Rope-backed plain text and Markdown document.
//!
//! Character indices address the rope directly; display columns are computed
//! with `unicode-width` only when the viewport needs them, which keeps edit
//! paths free of width calculations.

use std::path::{Path, PathBuf};

use ropey::Rope;
use unicode_width::UnicodeWidthChar;

use crate::editor::{Cursor, Editor, Motion};
use crate::error::DocumentError;
use crate::history::History;
use crate::io::{load_rope, save_rope_atomic};

/// A single reversible edit against the rope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Text was inserted at character index `at`.
    Insert {
        /// Character index of the insertion point.
        at: usize,
        /// Text that was inserted.
        text: String,
    },
    /// Text was removed starting at character index `at`.
    Delete {
        /// Character index of the deletion point.
        at: usize,
        /// Text that was removed, retained for undo.
        text: String,
    },
}

impl Edit {
    /// The edit that reverses `self`.
    pub fn inverted(&self) -> Edit {
        match self {
            Edit::Insert { at, text } => Edit::Delete {
                at: *at,
                text: text.clone(),
            },
            Edit::Delete { at, text } => Edit::Insert {
                at: *at,
                text: text.clone(),
            },
        }
    }
}

/// Plain-text document with selection, viewport scroll, and undo history.
#[derive(Debug)]
pub struct TextDocument {
    rope: Rope,
    path: Option<PathBuf>,
    dirty: bool,
    /// Head of the cursor, as a character index.
    cursor: usize,
    /// Selection anchor, or `None` when the selection is collapsed.
    anchor: Option<usize>,
    /// First visible line.
    rowoff: usize,
    /// First visible display column.
    coloff: usize,
    /// Display column preferred by vertical motion.
    goal_col: usize,
    history: History,
    /// Cached full-document word count; `None` when invalid.
    word_count: Option<usize>,
}

impl TextDocument {
    /// Create an empty document with no path.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            dirty: false,
            cursor: 0,
            anchor: None,
            rowoff: 0,
            coloff: 0,
            goal_col: 0,
            history: History::new(),
            word_count: None,
        }
    }

    /// Open a text file from disk.
    pub fn open(path: &Path) -> Result<Self, DocumentError> {
        let rope = load_rope(path)?;
        Ok(Self {
            rope,
            path: Some(path.to_path_buf()),
            dirty: false,
            cursor: 0,
            anchor: None,
            rowoff: 0,
            coloff: 0,
            goal_col: 0,
            history: History::new(),
            word_count: None,
        })
    }

    /// Replace the buffer with `text`, leaving the document clean.
    ///
    /// Used when a rich document rebuilds its editing surface from a package
    /// projection, so that load is not reported as an edit.
    pub fn load_clean(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.path = None;
        self.dirty = false;
        self.cursor = 0;
        self.anchor = None;
        self.rowoff = 0;
        self.coloff = 0;
        self.goal_col = 0;
        self.history = History::new();
        self.word_count = None;
    }

    /// Clear the dirty flag after the caller has persisted this buffer's
    /// content through another path (for example, writing the projection back
    /// into a rich package). History, cursor, and scroll are left untouched so
    /// undo after save behaves as it does for a plain text save.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Path this document was opened from, when known.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Adopt `path` as the save destination without reading or writing it.
    ///
    /// Used when the user names a new file that does not exist yet; the
    /// document stays clean until the first edit.
    pub fn adopt_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
    }

    /// Display name for the status bar.
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            None => "[no name]".to_string(),
        }
    }

    /// Number of lines in the document. Always at least one.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// First visible line index.
    pub fn rowoff(&self) -> usize {
        self.rowoff
    }

    /// First visible display column.
    pub fn coloff(&self) -> usize {
        self.coloff
    }

    /// Character index of the cursor head.
    pub fn cursor_char(&self) -> usize {
        self.cursor
    }

    /// Zero-based line containing the cursor.
    pub fn cursor_line(&self) -> usize {
        self.rope.char_to_line(self.cursor)
    }

    /// Zero-based display column of the cursor within its line.
    pub fn cursor_display_col(&self) -> usize {
        let line_idx = self.cursor_line();
        let line_start = self.rope.line_to_char(line_idx);
        let offset = self.cursor - line_start;
        self.line_display_col(line_idx, offset)
    }

    /// Read-only access to the rope for tests and internal helpers.
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Text of line `idx` without the trailing newline.
    pub fn line_text(&self, idx: usize) -> String {
        if idx >= self.rope.len_lines() {
            return String::new();
        }
        let mut s = self.rope.line(idx).to_string();
        if s.ends_with('\n') {
            s.pop();
            if s.ends_with('\r') {
                s.pop();
            }
        }
        s
    }

    /// Display column for a character offset `char_off` into line `line_idx`.
    fn line_display_col(&self, line_idx: usize, char_off: usize) -> usize {
        let line = self.rope.line(line_idx);
        let mut col = 0usize;
        for (i, ch) in line.chars().enumerate() {
            if i >= char_off {
                break;
            }
            if ch == '\n' || ch == '\r' {
                break;
            }
            col += ch.width().unwrap_or(0);
        }
        col
    }

    /// Character offset within `line_idx` that starts display column `target`.
    fn char_off_for_display_col(&self, line_idx: usize, target: usize) -> usize {
        let line = self.rope.line(line_idx);
        let mut col = 0usize;
        for (i, ch) in line.chars().enumerate() {
            if ch == '\n' || ch == '\r' {
                return i;
            }
            let w = ch.width().unwrap_or(0);
            if col + w > target {
                return i;
            }
            col += w;
            if col == target {
                return i + 1;
            }
        }
        line.len_chars().saturating_sub(usize::from(
            line.char(line.len_chars().saturating_sub(1)) == '\n',
        ))
    }

    /// Character index of the start of line `idx`.
    pub fn line_char_start(&self, idx: usize) -> usize {
        self.rope.line_to_char(idx)
    }

    /// Normalized selection range as inclusive-exclusive character indices.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor <= self.cursor {
            Some((anchor, self.cursor))
        } else {
            Some((self.cursor, anchor))
        }
    }

    /// Replace the selection or insert at the cursor, recording one unit.
    fn insert_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut unit = Vec::new();
        if let Some((start, end)) = self.selection_range() {
            if start < end {
                let removed = self.rope.slice(start..end).to_string();
                self.rope.remove(start..end);
                unit.push(Edit::Delete {
                    at: start,
                    text: removed,
                });
                self.cursor = start;
            }
        }
        let at = self.cursor;
        self.rope.insert(at, text);
        unit.push(Edit::Insert {
            at,
            text: text.to_string(),
        });
        self.cursor = at + text.chars().count();
        self.anchor = None;
        self.goal_col = self.cursor_display_col();
        self.history.push(unit);
        self.mark_dirty();
    }

    /// Delete the selection, or the character before the cursor.
    fn delete_backward(&mut self) {
        let mut unit = Vec::new();
        if let Some((start, end)) = self.selection_range() {
            if start < end {
                let removed = self.rope.slice(start..end).to_string();
                self.rope.remove(start..end);
                unit.push(Edit::Delete {
                    at: start,
                    text: removed,
                });
                self.cursor = start;
                self.anchor = None;
                self.history.push(unit);
                self.mark_dirty();
                return;
            }
        }
        if self.cursor == 0 {
            return;
        }
        let prev = self.cursor - 1;
        let ch = self.rope.char(prev);
        let mut removed = String::new();
        removed.push(ch);
        // Treat a CRLF pair as one deletion so newline handling stays atomic.
        if ch == '\n' && prev > 0 && self.rope.char(prev - 1) == '\r' {
            removed.insert(0, '\r');
            self.rope.remove((prev - 1)..self.cursor);
            unit.push(Edit::Delete {
                at: prev - 1,
                text: removed,
            });
            self.cursor = prev - 1;
        } else {
            self.rope.remove(prev..self.cursor);
            unit.push(Edit::Delete {
                at: prev,
                text: removed,
            });
            self.cursor = prev;
        }
        self.anchor = None;
        self.goal_col = self.cursor_display_col();
        self.history.push(unit);
        self.mark_dirty();
    }

    /// Apply an edit unit forward.
    fn apply_forward(&mut self, unit: &[Edit]) {
        for edit in unit {
            match edit {
                Edit::Insert { at, text } => {
                    self.rope.insert(*at, text);
                    self.cursor = *at + text.chars().count();
                }
                Edit::Delete { at, text } => {
                    let end = *at + text.chars().count();
                    self.rope.remove(*at..end);
                    self.cursor = *at;
                }
            }
        }
        self.anchor = None;
        self.mark_dirty();
    }

    /// Apply an edit unit in reverse (the inverse of each edit, last first).
    fn apply_inverse(&mut self, unit: &[Edit]) {
        for edit in unit.iter().rev() {
            let inv = edit.inverted();
            match &inv {
                Edit::Insert { at, text } => {
                    self.rope.insert(*at, text);
                    self.cursor = *at + text.chars().count();
                }
                Edit::Delete { at, text } => {
                    let end = *at + text.chars().count();
                    self.rope.remove(*at..end);
                    self.cursor = *at;
                }
            }
        }
        self.anchor = None;
        self.mark_dirty();
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.word_count = None;
        let len = self.rope.len_chars();
        if self.cursor > len {
            self.cursor = len;
        }
    }

    /// Clamp scroll offsets so the cursor stays inside the viewport of the
    /// given size.
    pub fn scroll_to_cursor(&mut self, view_h: usize, view_w: usize) {
        if view_h == 0 || view_w == 0 {
            return;
        }
        let line = self.cursor_line();
        if line < self.rowoff {
            self.rowoff = line;
        } else if line >= self.rowoff + view_h {
            self.rowoff = line + 1 - view_h;
        }
        let col = self.cursor_display_col();
        if col < self.coloff {
            self.coloff = col;
        } else if col >= self.coloff + view_w {
            self.coloff = col + 1 - view_w;
        }
        let max_row = self.line_count().saturating_sub(1);
        if self.rowoff > max_row {
            self.rowoff = max_row;
        }
    }

    /// Search forward from `from` for `needle`, wrapping once.
    ///
    /// `from` is a character index into the rope. Returns the match range as
    /// character indices.
    pub fn find(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let text = self.rope.to_string();
        // Convert the character offset to a byte offset: `str::find` works in
        // bytes, while callers pass rope character indices.
        let byte_start = text
            .char_indices()
            .nth(from)
            .map(|(b, _)| b)
            .unwrap_or(text.len());
        if let Some(rel) = text[byte_start..].find(needle) {
            let abs = byte_start + rel;
            let char_at = text[..abs].chars().count();
            return Some((char_at, char_at + needle.chars().count()));
        }
        if byte_start > 0 {
            if let Some(rel) = text[..byte_start].find(needle) {
                let char_at = text[..rel].chars().count();
                return Some((char_at, char_at + needle.chars().count()));
            }
        }
        None
    }

    /// Search forward from `from` without wrapping. Same return shape as
    /// [`TextDocument::find`].
    pub fn find_no_wrap(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let text = self.rope.to_string();
        let byte_start = text
            .char_indices()
            .nth(from)
            .map(|(b, _)| b)
            .unwrap_or(text.len());
        let rel = text[byte_start..].find(needle)?;
        let abs = byte_start + rel;
        let char_at = text[..abs].chars().count();
        Some((char_at, char_at + needle.chars().count()))
    }

    /// Move the cursor head to `char_idx`, clearing or keeping selection.
    fn set_cursor(&mut self, char_idx: usize, extend: bool) {
        let len = self.rope.len_chars();
        let target = char_idx.min(len);
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = target;
        self.goal_col = self.cursor_display_col();
    }

    /// Place the cursor at `start` with `end` as the exclusive bound, selecting
    /// that range. Used by find to highlight a match.
    pub fn set_cursor_range(&mut self, start: usize, end: usize) {
        let len = self.rope.len_chars();
        let start = start.min(len);
        let end = end.min(len).max(start);
        self.anchor = Some(start);
        self.cursor = end;
        self.goal_col = self.cursor_display_col();
    }

    /// Replace character range `[start, end)` with `replacement` as one undo unit.
    pub fn replace_range(&mut self, start: usize, end: usize, replacement: &str) {
        let len = self.rope.len_chars();
        let start = start.min(len);
        let end = end.min(len).max(start);
        let mut unit = Vec::new();
        if start < end {
            let removed = self.rope.slice(start..end).to_string();
            self.rope.remove(start..end);
            unit.push(Edit::Delete {
                at: start,
                text: removed,
            });
        }
        if !replacement.is_empty() {
            self.rope.insert(start, replacement);
            unit.push(Edit::Insert {
                at: start,
                text: replacement.to_string(),
            });
        }
        if !unit.is_empty() {
            self.history.push(unit);
        }
        self.cursor = start + replacement.chars().count();
        self.anchor = None;
        self.goal_col = self.cursor_display_col();
        self.mark_dirty();
    }

    /// Cut the entire current line, including its trailing newline when present,
    /// into `cutbuffer`, removing it from the document as one undo unit.
    pub fn cut_current_line(&mut self, cutbuffer: &mut String) {
        let line = self.cursor_line();
        let start = self.rope.line_to_char(line);
        let end = start + self.rope.line(line).len_chars();
        if start >= end {
            return;
        }
        let text = self.rope.slice(start..end).to_string();
        *cutbuffer = text.clone();
        self.rope.remove(start..end);
        self.history.push(vec![Edit::Delete { at: start, text }]);
        self.cursor = start.min(self.rope.len_chars());
        self.anchor = None;
        self.goal_col = self.cursor_display_col();
        self.mark_dirty();
    }

    fn apply_motion(&mut self, motion: Motion, extend: bool, view_h: usize) {
        match motion {
            Motion::Left => {
                if !extend {
                    if let Some((start, end)) = self.selection_range() {
                        if start < end {
                            self.cursor = start;
                            self.anchor = None;
                            self.goal_col = self.cursor_display_col();
                            return;
                        }
                    }
                }
                if self.cursor > 0 {
                    self.set_cursor(self.cursor - 1, extend);
                }
            }
            Motion::Right => {
                if !extend {
                    if let Some((start, end)) = self.selection_range() {
                        if start < end {
                            self.cursor = end;
                            self.anchor = None;
                            self.goal_col = self.cursor_display_col();
                            return;
                        }
                    }
                }
                if self.cursor < self.rope.len_chars() {
                    self.set_cursor(self.cursor + 1, extend);
                }
            }
            Motion::Up => self.move_vertical(-1, extend),
            Motion::Down => self.move_vertical(1, extend),
            Motion::LineStart => {
                let line = self.cursor_line();
                self.set_cursor(self.rope.line_to_char(line), extend);
            }
            Motion::LineEnd => {
                let line = self.cursor_line();
                let line_len = self.rope.line(line).len_chars();
                // Exclude the trailing newline from the end position.
                let mut end = self.rope.line_to_char(line) + line_len;
                if line_len > 0 {
                    let last = self.rope.line(line).char(line_len - 1);
                    if last == '\n' {
                        end -= 1;
                        if line_len > 1 && self.rope.line(line).char(line_len - 2) == '\r' {
                            end -= 1;
                        }
                    }
                }
                self.set_cursor(end, extend);
            }
            Motion::PageUp => {
                let step = view_h.max(1);
                for _ in 0..step {
                    self.move_vertical(-1, extend);
                }
            }
            Motion::PageDown => {
                let step = view_h.max(1);
                for _ in 0..step {
                    self.move_vertical(1, extend);
                }
            }
            Motion::BufferStart => self.set_cursor(0, extend),
            Motion::BufferEnd => self.set_cursor(self.rope.len_chars(), extend),
        }
    }

    fn move_vertical(&mut self, delta: i32, extend: bool) {
        let line = self.cursor_line() as i64;
        let target = (line + i64::from(delta)).clamp(0, self.line_count() as i64 - 1) as usize;
        let goal = self.goal_col;
        let off = self.char_off_for_display_col(target, goal);
        // Restore the goal column after set_cursor recomputes it from the
        // clamped position, so a short line does not permanently narrow the
        // preferred column.
        self.set_cursor(self.rope.line_to_char(target) + off, extend);
        self.goal_col = goal;
    }

    /// Full-document word count, cached until the next edit.
    pub fn word_count(&mut self) -> usize {
        if let Some(n) = self.word_count {
            return n;
        }
        let mut count = 0usize;
        let mut in_word = false;
        for ch in self.rope.to_string().chars() {
            if ch.is_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                count += 1;
            }
        }
        self.word_count = Some(count);
        count
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor for TextDocument {
    fn cursor(&self) -> Cursor {
        Cursor::Char(self.cursor)
    }

    fn selection(&self) -> Option<(Cursor, Cursor)> {
        self.selection_range()
            .map(|(a, b)| (Cursor::Char(a), Cursor::Char(b)))
    }

    fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.rope.len_chars();
        self.goal_col = self.cursor_display_col();
    }

    fn clear_selection(&mut self) {
        self.anchor = None;
    }

    fn move_cursor(&mut self, motion: Motion, extend: bool) {
        // Viewport height is unknown at this layer; PageUp and PageDown use a
        // fixed step that the UI can refine by calling scroll_to_cursor.
        self.apply_motion(motion, extend, 40);
    }

    fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.insert_at_cursor(c.encode_utf8(&mut buf));
    }

    fn insert_str(&mut self, s: &str) {
        self.insert_at_cursor(s);
    }

    fn delete_back(&mut self) {
        self.delete_backward();
    }

    fn delete_forward(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            if start < end {
                let removed = self.rope.slice(start..end).to_string();
                self.rope.remove(start..end);
                self.history.push(vec![Edit::Delete {
                    at: start,
                    text: removed,
                }]);
                self.cursor = start;
                self.anchor = None;
                self.mark_dirty();
                return;
            }
        }
        if self.cursor >= self.rope.len_chars() {
            return;
        }
        let ch = self.rope.char(self.cursor);
        let mut removed = String::new();
        removed.push(ch);
        let mut end = self.cursor + 1;
        if ch == '\r' && end < self.rope.len_chars() && self.rope.char(end) == '\n' {
            removed.push('\n');
            end += 1;
        }
        self.rope.remove(self.cursor..end);
        self.history.push(vec![Edit::Delete {
            at: self.cursor,
            text: removed,
        }]);
        self.mark_dirty();
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError> {
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => self.path.clone().ok_or_else(|| DocumentError::Save {
                path: PathBuf::from("[no name]"),
                message: "no path; use save_as".to_string(),
            })?,
        };
        save_rope_atomic(&target, &self.rope)?;
        self.path = Some(target);
        self.dirty = false;
        Ok(())
    }

    fn undo(&mut self) -> bool {
        match self.history.pop_undo() {
            Some(unit) => {
                self.apply_inverse(&unit);
                self.history.push_redo(unit);
                true
            }
            None => false,
        }
    }

    fn redo(&mut self) -> bool {
        match self.history.pop_redo() {
            Some(unit) => {
                self.apply_forward(&unit);
                self.history.push_undo(unit);
                true
            }
            None => false,
        }
    }

    fn text_projection(&self) -> String {
        self.rope.to_string()
    }
}
