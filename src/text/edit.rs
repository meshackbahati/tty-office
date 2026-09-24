//! Edit units and the mutation operations that record them.

use super::TextDocument;

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

impl TextDocument {
    pub(crate) fn insert_at_cursor(&mut self, text: &str) {
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
    pub(crate) fn delete_backward(&mut self) {
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
    pub(crate) fn apply_forward(&mut self, unit: &[Edit]) {
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
    pub(crate) fn apply_inverse(&mut self, unit: &[Edit]) {
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

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.word_count = None;
        let len = self.rope.len_chars();
        if self.cursor > len {
            self.cursor = len;
        }
    }

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
}
