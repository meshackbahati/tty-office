//! Bounded undo and redo history shared by all document domains.
//!
//! Units are inverse-applied edit sequences. Consecutive single-character
//! inserts coalesce so that typing one word is one undo step rather than one
//! step per keystroke.

use crate::text::Edit;

/// Maximum number of undo units retained; older units are dropped.
pub const HISTORY_CAP: usize = 1000;

/// Undo and redo stacks with coalescing for typing bursts.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
}

impl History {
    /// Create an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed edit unit and clear the redo stack.
    ///
    /// A unit that is a single insert adjacent to the previous unit's insert
    /// is merged into that unit, which matches the usual editor expectation
    /// that undo reverses a typing run rather than a single glyph.
    pub fn push(&mut self, unit: Vec<Edit>) {
        if unit.is_empty() {
            return;
        }
        self.redo.clear();
        if let Some(last) = self.undo.last_mut() {
            if can_coalesce(last, &unit) {
                last.extend(unit);
                return;
            }
        }
        self.undo.push(unit);
        if self.undo.len() > HISTORY_CAP {
            let excess = self.undo.len() - HISTORY_CAP;
            self.undo.drain(0..excess);
        }
    }

    /// Pop the newest undo unit, or `None` when there is nothing to undo.
    pub fn pop_undo(&mut self) -> Option<Vec<Edit>> {
        self.undo.pop()
    }

    /// Push a unit back onto the undo stack, used when redo re-applies it.
    pub fn push_undo(&mut self, unit: Vec<Edit>) {
        self.undo.push(unit);
    }

    /// Pop the newest redo unit, or `None` when there is nothing to redo.
    pub fn pop_redo(&mut self) -> Option<Vec<Edit>> {
        self.redo.pop()
    }

    /// Push a unit onto the redo stack, used when undo reverses it.
    pub fn push_redo(&mut self, unit: Vec<Edit>) {
        self.redo.push(unit);
    }

    /// Whether any undo unit remains.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether any redo unit remains.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn can_coalesce(prev: &[Edit], next: &[Edit]) -> bool {
    if next.len() != 1 {
        return false;
    }
    match &next[0] {
        Edit::Insert { at, text } => {
            if text.chars().count() != 1 || text == "\n" || text == "\r\n" {
                return false;
            }
            // The new insert must continue from the end of the previous unit's
            // final insert. Units grow as typing coalesces, so only the last
            // edit of the previous unit is relevant.
            match prev.last() {
                Some(Edit::Insert {
                    at: prev_at,
                    text: prev_text,
                }) => *at == *prev_at + prev_text.chars().count(),
                _ => false,
            }
        }
        Edit::Delete { .. } => false,
    }
}
