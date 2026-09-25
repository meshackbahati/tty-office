//! Rope-backed plain text and Markdown document.
//!
//! Character indices address the rope directly; display columns are computed
//! with `unicode-width` only when the viewport needs them, which keeps edit
//! paths free of width calculations.

mod edit;
mod find;
mod motion;

use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::editor::{Cursor, Editor, Motion};
use crate::error::DocumentError;
use crate::history::History;
use crate::io::{load_rope, save_rope_atomic};

pub use edit::Edit;

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
    /// Bumped on every content mutation so background jobs (proofing) can
    /// skip work when the projection has not changed since they last ran.
    revision: u64,
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
            revision: 0,
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
            revision: 0,
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
        self.revision = self.revision.wrapping_add(1);
    }

    /// Monotonic content revision; changes on every edit and on load.
    pub fn revision(&self) -> u64 {
        self.revision
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
