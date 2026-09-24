//! Format detection and enum dispatch over open documents.
//!
//! Phase 0 ships only the text loader. DOCX, ODS, and XLSX variants appear
//! here when their features are enabled and their modules land; the enum is
//! `#[non_exhaustive]` so those additions are not breaking.

use std::path::Path;

use crate::editor::Editor;
use crate::error::DocumentError;
use crate::text::TextDocument;

/// Opened document, enum-dispatched so the UI need not box trait objects.
#[derive(Debug)]
#[non_exhaustive]
pub enum Document {
    /// Plain text and Markdown.
    Text(TextDocument),
}

impl Document {
    /// Display name for the status bar.
    pub fn display_name(&self) -> String {
        match self {
            Document::Text(t) => t.display_name(),
        }
    }

    /// Owning path, when known.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Document::Text(t) => t.path(),
        }
    }
}

impl Editor for Document {
    fn cursor(&self) -> crate::editor::Cursor {
        match self {
            Document::Text(t) => t.cursor(),
        }
    }

    fn selection(&self) -> Option<(crate::editor::Cursor, crate::editor::Cursor)> {
        match self {
            Document::Text(t) => t.selection(),
        }
    }

    fn select_all(&mut self) {
        match self {
            Document::Text(t) => t.select_all(),
        }
    }

    fn clear_selection(&mut self) {
        match self {
            Document::Text(t) => t.clear_selection(),
        }
    }

    fn move_cursor(&mut self, motion: crate::editor::Motion, extend: bool) {
        match self {
            Document::Text(t) => t.move_cursor(motion, extend),
        }
    }

    fn insert_char(&mut self, c: char) {
        match self {
            Document::Text(t) => t.insert_char(c),
        }
    }

    fn insert_str(&mut self, s: &str) {
        match self {
            Document::Text(t) => t.insert_str(s),
        }
    }

    fn delete_back(&mut self) {
        match self {
            Document::Text(t) => t.delete_back(),
        }
    }

    fn delete_forward(&mut self) {
        match self {
            Document::Text(t) => t.delete_forward(),
        }
    }

    fn is_dirty(&self) -> bool {
        match self {
            Document::Text(t) => t.is_dirty(),
        }
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError> {
        match self {
            Document::Text(t) => t.save(path),
        }
    }

    fn undo(&mut self) -> bool {
        match self {
            Document::Text(t) => t.undo(),
        }
    }

    fn redo(&mut self) -> bool {
        match self {
            Document::Text(t) => t.redo(),
        }
    }

    fn text_projection(&self) -> String {
        match self {
            Document::Text(t) => t.text_projection(),
        }
    }
}

/// Detect format from path extension and open.
///
/// A missing file is treated as an empty text document so that
/// `tty-office new.txt` starts editing immediately.
///
/// ```
/// use std::path::Path;
/// use tty_office::{open, Editor};
/// // Opening a non-existent path yields an empty, dirty-free text document.
/// let path = Path::new("/tmp/tty-office-doc-test-does-not-exist.txt");
/// if path.exists() { std::fs::remove_file(path).ok(); }
/// let doc = open(path).expect("new text files open as empty");
/// assert!(!doc.is_dirty());
/// ```
pub fn open(path: &Path) -> Result<Document, DocumentError> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "txt" | "md" | "markdown" | "" => {
            if path.exists() {
                Ok(Document::Text(TextDocument::open(path)?))
            } else {
                let mut t = TextDocument::new();
                t.adopt_path(path);
                Ok(Document::Text(t))
            }
        }
        #[cfg(feature = "docx")]
        "docx" | "odt" => Err(DocumentError::UnsupportedFormat(format!(
            "{ext} support arrives in Phase 3"
        ))),
        #[cfg(feature = "xlsx")]
        "xlsx" | "ods" | "xls" => Err(DocumentError::UnsupportedFormat(format!(
            "{ext} support arrives in Phase 4"
        ))),
        other if other.is_empty() => Err(DocumentError::UnsupportedFormat(
            "unknown extension".to_string(),
        )),
        other => Err(DocumentError::UnsupportedFormat(other.to_string())),
    }
}
