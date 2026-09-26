//! Format detection and enum dispatch over open documents.
//!
//! Phase 0 ships the text loader; Phase 3 adds the rich Word package loader
//! behind the `docx` feature; Phase 4 adds the spreadsheet loader behind the
//! `xlsx` feature. The enum is `#[non_exhaustive]` so those additions are not
//! breaking.

use std::path::Path;

use crate::editor::Editor;
use crate::error::DocumentError;
use crate::text::TextDocument;

#[cfg(feature = "docx")]
use crate::rich::RichDocument;

#[cfg(feature = "xlsx")]
use crate::sheet::SheetDocument;

/// Opened document, enum-dispatched so the UI need not box trait objects.
#[derive(Debug)]
#[non_exhaustive]
pub enum Document {
    /// Plain text and Markdown.
    Text(TextDocument),
    /// DOCX and ODT Word packages, when the `docx` feature is enabled.
    ///
    /// The package model is large (tens of kilobytes of styles and body
    /// content), so it is boxed to keep `Document` itself small enough to
    /// pass around without dominating stack frames.
    #[cfg(feature = "docx")]
    Rich(Box<RichDocument>),
    /// Spreadsheet grids (xlsx, ods, read-only xls), when the `xlsx` feature
    /// is enabled. Boxed for the same reason as [`Document::Rich`].
    #[cfg(feature = "xlsx")]
    Sheet(Box<SheetDocument>),
}

impl Document {
    /// Display name for the status bar.
    pub fn display_name(&self) -> String {
        match self {
            Document::Text(t) => t.display_name(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.display_name(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.display_name(),
        }
    }

    /// Owning path, when known.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Document::Text(t) => t.path(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.path(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.path(),
        }
    }

    /// Extension appended when Save As receives a bare file name, so
    /// typing `budget` for a spreadsheet saves `budget.xlsx` the way
    /// graphical suites behave instead of failing on a missing suffix.
    pub fn default_extension(&self) -> &'static str {
        match self {
            Document::Text(_) => "txt",
            #[cfg(feature = "docx")]
            Document::Rich(r) => match r.format() {
                crate::RichFormat::Docx => "docx",
                crate::RichFormat::Odt => "odt",
            },
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => match s.format() {
                crate::SheetFormat::Xlsx => "xlsx",
                crate::SheetFormat::Ods => "ods",
                crate::SheetFormat::Xls => "xlsx",
                crate::SheetFormat::Csv => "csv",
            },
        }
    }

    /// Line-oriented prose surface shared by text and rich documents.
    ///
    /// Returns `None` when the variant is not a prose editor (for example a
    /// spreadsheet pane). Callers that only understand lines use this
    /// instead of matching every prose variant.
    pub fn prose_surface(&mut self) -> Option<&mut TextDocument> {
        match self {
            Document::Text(t) => Some(t),
            #[cfg(feature = "docx")]
            Document::Rich(r) => Some(r.surface_mut()),
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => None,
        }
    }

    /// Mutable sheet document when this variant is a spreadsheet.
    #[cfg(feature = "xlsx")]
    pub fn sheet_mut(&mut self) -> Option<&mut SheetDocument> {
        match self {
            Document::Sheet(s) => Some(s),
            _ => None,
        }
    }
}

impl Editor for Document {
    fn cursor(&self) -> crate::editor::Cursor {
        match self {
            Document::Text(t) => t.cursor(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.cursor(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.cursor(),
        }
    }

    fn selection(&self) -> Option<(crate::editor::Cursor, crate::editor::Cursor)> {
        match self {
            Document::Text(t) => t.selection(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.selection(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.selection(),
        }
    }

    fn select_all(&mut self) {
        match self {
            Document::Text(t) => t.select_all(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.select_all(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.select_all(),
        }
    }

    fn clear_selection(&mut self) {
        match self {
            Document::Text(t) => t.clear_selection(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.clear_selection(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.clear_selection(),
        }
    }

    fn move_cursor(&mut self, motion: crate::editor::Motion, extend: bool) {
        match self {
            Document::Text(t) => t.move_cursor(motion, extend),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.move_cursor(motion, extend),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.move_cursor(motion, extend),
        }
    }

    fn insert_char(&mut self, c: char) {
        match self {
            Document::Text(t) => t.insert_char(c),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.insert_char(c),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.insert_char(c),
        }
    }

    fn insert_str(&mut self, s: &str) {
        match self {
            Document::Text(t) => t.insert_str(s),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.insert_str(s),
            #[cfg(feature = "xlsx")]
            Document::Sheet(sheet) => sheet.insert_str(s),
        }
    }

    fn delete_back(&mut self) {
        match self {
            Document::Text(t) => t.delete_back(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.delete_back(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.delete_back(),
        }
    }

    fn delete_forward(&mut self) {
        match self {
            Document::Text(t) => t.delete_forward(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.delete_forward(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.delete_forward(),
        }
    }

    fn is_dirty(&self) -> bool {
        match self {
            Document::Text(t) => t.is_dirty(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.is_dirty(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.is_dirty(),
        }
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError> {
        match self {
            Document::Text(t) => t.save(path),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.save(path),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.save(path),
        }
    }

    fn undo(&mut self) -> bool {
        match self {
            Document::Text(t) => t.undo(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.undo(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.undo(),
        }
    }

    fn redo(&mut self) -> bool {
        match self {
            Document::Text(t) => t.redo(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.redo(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.redo(),
        }
    }

    fn text_projection(&self) -> String {
        match self {
            Document::Text(t) => t.text_projection(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.text_projection(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.text_projection(),
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
        "docx" | "odt" => {
            if path.exists() {
                Ok(Document::Rich(Box::new(RichDocument::open(path)?)))
            } else {
                let format = match ext.as_str() {
                    "odt" => crate::rich::RichFormat::Odt,
                    _ => crate::rich::RichFormat::Docx,
                };
                let mut r = RichDocument::new(format);
                r.adopt_path(path);
                Ok(Document::Rich(Box::new(r)))
            }
        }
        #[cfg(feature = "xlsx")]
        "xlsx" | "ods" | "xls" | "csv" => {
            let format = crate::sheet::SheetFormat::from_path(path)
                .ok_or_else(|| DocumentError::UnsupportedFormat(ext.clone()))?;
            if path.exists() {
                Ok(Document::Sheet(Box::new(SheetDocument::open(path)?)))
            } else {
                let mut s = SheetDocument::new(format);
                s.adopt_path(path);
                Ok(Document::Sheet(Box::new(s)))
            }
        }
        other => {
            // Name what the suite opens so a failed attempt teaches the
            // supported set instead of ending the investigation.
            #[cfg(feature = "docx")]
            let rich = ", docx, odt";
            #[cfg(not(feature = "docx"))]
            let rich = "";
            #[cfg(feature = "xlsx")]
            let sheet = ", xlsx, ods, xls, csv";
            #[cfg(not(feature = "xlsx"))]
            let sheet = "";
            #[cfg(feature = "pdf")]
            let pdf = ", pdf";
            #[cfg(not(feature = "pdf"))]
            let pdf = "";
            let name = if other.is_empty() { "unknown" } else { other };
            Err(DocumentError::UnsupportedFormat(format!(
                ".{name} cannot be opened (open txt, md{rich}{sheet}{pdf})"
            )))
        }
    }
}
