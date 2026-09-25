//! Cursor motion, selection placement, and viewport scrolling.

use crate::editor::Motion;
use unicode_width::UnicodeWidthChar;

use super::TextDocument;

impl TextDocument {
    /// Display column for a character offset `char_off` into line `line_idx`.
    pub(crate) fn line_display_col(&self, line_idx: usize, char_off: usize) -> usize {
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
    pub(crate) fn char_off_for_display_col(&self, line_idx: usize, target: usize) -> usize {
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
        let len = line.len_chars();
        if len == 0 {
            // An empty document has a single zero-length line, and indexing
            // its would-be last character would panic in ropey.
            return 0;
        }
        len - usize::from(line.char(len - 1) == '\n')
    }

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

    pub(crate) fn set_cursor(&mut self, char_idx: usize, extend: bool) {
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

    /// Place the caret at `line` and display column `col` for mouse clicks,
    /// clamping both to the buffer and clearing any selection.
    pub fn click_at(&mut self, line: usize, col: usize) {
        let line = line.min(self.line_count().saturating_sub(1));
        let off = self.char_off_for_display_col(line, col);
        let at = self.line_char_start(line) + off;
        self.set_cursor(at, false);
    }

    /// Scroll the viewport by whole lines without moving the caret.
    pub fn scroll_lines(&mut self, delta: i32) {
        let max = self.line_count().saturating_sub(1) as i32;
        self.rowoff = (self.rowoff as i32 + delta).clamp(0, max) as usize;
    }

    pub(crate) fn apply_motion(&mut self, motion: Motion, extend: bool, view_h: usize) {
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

    pub(crate) fn move_vertical(&mut self, delta: i32, extend: bool) {
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
}
