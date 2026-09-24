//! Editing surface shared by every document domain.
//!
//! The UI drives this trait and never branches on concrete format types, which
//! keeps the keymap and render loop stable as DOCX and spreadsheet backends
//! arrive behind feature gates.

use std::path::Path;

use crate::error::DocumentError;

/// A cursor position in domain-specific coordinates.
///
/// Text documents use character offsets into the rope; sheet documents will
/// use row and column indices once Phase 4 lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// Character index from the start of the document.
    Char(usize),
    /// Zero-based cell coordinates for grid documents.
    Cell {
        /// Row index.
        row: usize,
        /// Column index.
        col: usize,
    },
}

/// Directional cursor motion, independent of document domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motion {
    /// One character or cell left.
    Left,
    /// One character or cell right.
    Right,
    /// One line up, preserving the goal column.
    Up,
    /// One line down, preserving the goal column.
    Down,
    /// Start of the current line or row.
    LineStart,
    /// End of the current line or row.
    LineEnd,
    /// One viewport height up.
    PageUp,
    /// One viewport height down.
    PageDown,
    /// Start of the document.
    BufferStart,
    /// End of the document.
    BufferEnd,
}

/// The editing surface that the UI drives, independent of concrete format.
///
/// Implementors own their storage. Callers never see underlying crate types
/// such as `ropey::Rope`, so those crates remain free to change without a
/// semver event on this trait.
pub trait Editor {
    /// Current cursor position in domain coordinates.
    fn cursor(&self) -> Cursor;

    /// Current selection as anchor plus head, or `None` when collapsed.
    ///
    /// The head equals the cursor. Motion with `extend` keeps the anchor and
    /// moves only the head; motion without `extend` clears the selection.
    fn selection(&self) -> Option<(Cursor, Cursor)>;

    /// Select the entire document.
    fn select_all(&mut self);

    /// Drop any active selection, leaving a collapsed cursor.
    fn clear_selection(&mut self);

    /// Move the head by `motion`; when `extend` is true the anchor is kept.
    fn move_cursor(&mut self, motion: Motion, extend: bool);

    /// Insert a character at the cursor, replacing any active selection.
    fn insert_char(&mut self, c: char);

    /// Insert a string at the cursor, replacing any active selection.
    fn insert_str(&mut self, s: &str);

    /// Delete the selection, or the character before the cursor when the
    /// selection is collapsed.
    fn delete_back(&mut self);

    /// Delete the selection, or the character after the cursor when the
    /// selection is collapsed.
    fn delete_forward(&mut self);

    /// Whether unsaved changes exist.
    fn is_dirty(&self) -> bool;

    /// Serialize to the path this document was opened from, or to `path` if
    /// given.
    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError>;

    /// Serialize to an explicit destination path and adopt that path.
    ///
    /// Provided as sugar over [`Editor::save`] so callers that always know the
    /// destination need not build an `Option`.
    fn save_as(&mut self, path: &Path) -> Result<(), DocumentError> {
        self.save(Some(path))
    }

    /// Undo the last edit unit. Returns false when history is empty.
    fn undo(&mut self) -> bool;

    /// Redo the last undone unit. Returns false when redo is empty.
    fn redo(&mut self) -> bool;

    /// Plain-text projection for search, word count, and status display.
    fn text_projection(&self) -> String;
}
