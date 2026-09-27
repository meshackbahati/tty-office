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

    /// Heading level (1-9) of a prose line, for word documents with
    /// styled paragraphs; anything else reads as no heading.
    pub fn heading_at(&self, _line: usize) -> Option<u8> {
        match self {
            Document::Text(_) => None,
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.heading_at(_line),
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => None,
        }
    }

    /// Hyperlink runs of a prose line as `(start, end, target)`: word
    /// documents resolve them, and anything else carries no links.
    pub fn link_spans(&self, _line: usize) -> Vec<(usize, usize, Option<String>)> {
        match self {
            Document::Text(_) => Vec::new(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r
                .link_spans(_line)
                .into_iter()
                .map(|s| (s.start, s.end, s.target))
                .collect(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => Vec::new(),
        }
    }

    /// Styled runs of a prose line as `(start, end, bold, italic,
    /// underline)`: word documents resolve them, and anything else
    /// carries no styles.
    pub fn run_styles(&self, _line: usize) -> Vec<(usize, usize, bool, bool, bool)> {
        match self {
            Document::Text(_) => Vec::new(),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r
                .run_styles(_line)
                .into_iter()
                .map(|s| (s.start, s.end, s.flags.0, s.flags.1, s.flags.2))
                .collect(),
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => Vec::new(),
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

    /// Family of this document for conversion routing.
    fn kind(&self) -> Kind {
        match self {
            Document::Text(_) => Kind::Text,
            #[cfg(feature = "docx")]
            Document::Rich(_) => Kind::Rich,
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => Kind::Sheet,
        }
    }

    /// Save to `target`, converting across families when the extension
    /// demands it and exporting when it names a PDF. Same-family saves
    /// keep the existing writers; unknown extensions fall through to
    /// them so their refusal messages stay specific. On a converting
    /// save the buffer itself becomes the target kind, so further
    /// edits and saves behave like a natively opened file.
    pub fn save_as_convert(&mut self, target: &Path) -> Result<(), DocumentError> {
        use crate::editor::Editor;
        let ext = target
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if ext.as_str() == "pdf" {
            return crate::print::export(self, target);
        }
        match kind_of_ext(ext.as_str()) {
            Some(want) if want == self.kind() => self.save_as(target),
            Some(_) => {
                let text = self.text_projection();
                let mut fresh = fresh_doc_for_ext(ext.as_str())?;
                match &mut fresh {
                    Document::Text(_) => {
                        fresh.insert_str(&text);
                    }
                    #[cfg(feature = "docx")]
                    Document::Rich(_) => {
                        fresh.insert_str(&text);
                    }
                    #[cfg(feature = "xlsx")]
                    Document::Sheet(sheet) => {
                        use crate::editor::Motion;
                        sheet.move_cursor(Motion::BufferStart, false);
                        let mut first = true;
                        for line in text.lines() {
                            if !first {
                                sheet.move_cursor(Motion::Down, false);
                            }
                            first = false;
                            sheet.set_cell_content(line);
                        }
                    }
                }
                fresh.save_as(target)?;
                *self = fresh;
                Ok(())
            }
            None => self.save_as(target),
        }
    }
}

/// Document family for conversion routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    #[cfg(feature = "docx")]
    Rich,
    #[cfg(feature = "xlsx")]
    Sheet,
}

/// Family implied by a path extension, if any.
fn kind_of_ext(ext: &str) -> Option<Kind> {
    match ext {
        "txt" | "md" | "markdown" => Some(Kind::Text),
        #[cfg(feature = "docx")]
        "docx" | "odt" => Some(Kind::Rich),
        #[cfg(feature = "xlsx")]
        "xlsx" | "ods" | "xls" | "csv" => Some(Kind::Sheet),
        _ => None,
    }
}

/// Empty document of the family an extension names, for conversions.
fn fresh_doc_for_ext(ext: &str) -> Result<Document, DocumentError> {
    match ext {
        "txt" | "md" | "markdown" => Ok(Document::Text(TextDocument::new())),
        #[cfg(feature = "docx")]
        "docx" => Ok(Document::Rich(Box::new(crate::RichDocument::new(
            crate::RichFormat::Docx,
        )))),
        #[cfg(feature = "docx")]
        "odt" => Ok(Document::Rich(Box::new(crate::RichDocument::new(
            crate::RichFormat::Odt,
        )))),
        #[cfg(feature = "xlsx")]
        "xlsx" => Ok(Document::Sheet(Box::new(crate::SheetDocument::new(
            crate::SheetFormat::Xlsx,
        )))),
        #[cfg(feature = "xlsx")]
        "ods" => Ok(Document::Sheet(Box::new(crate::SheetDocument::new(
            crate::SheetFormat::Ods,
        )))),
        #[cfg(feature = "xlsx")]
        "csv" => Ok(Document::Sheet(Box::new(crate::SheetDocument::new(
            crate::SheetFormat::Csv,
        )))),
        #[cfg(feature = "xlsx")]
        "xls" => Ok(Document::Sheet(Box::new(crate::SheetDocument::new(
            crate::SheetFormat::Xls,
        )))),
        _ => Err(DocumentError::UnsupportedFormat(format!(
            "cannot convert to .{ext}"
        ))),
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
        #[cfg(feature = "pdf")]
        "pdf" => {
            if path.exists() {
                view_pdf(path)
            } else {
                // A PDF cannot be written by typing, so a missing path
                // starts untitled text that Save As will rename.
                Ok(Document::Text(TextDocument::new()))
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

/// Open a PDF as extracted text for viewing. Pages extract separately
/// behind a visible separator, so multi-page documents keep their page
/// structure instead of concatenating. The buffer carries no path, so
/// saving always passes through Save As instead of overwriting the PDF
/// with plain text.
///
/// Layout-preserving rendering (columns, tables, exact spacing) needs
/// positioned glyphs, which the current extractor does not expose; the
/// roadmap covers a poppler-backed image view for graphics-capable
/// terminals and keeps this text view for pure TTY.
#[cfg(feature = "pdf")]
fn view_pdf(path: &Path) -> Result<Document, DocumentError> {
    let doc = lopdf::Document::load(path).map_err(|err| DocumentError::Parse(err.to_string()))?;
    let numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    let mut body = String::new();
    for (i, page) in numbers.iter().enumerate() {
        let text = doc
            .extract_text(&[*page])
            .map_err(|err| DocumentError::Parse(err.to_string()))?;
        if i > 0 {
            body.push_str(&format!("\n\u{2500}\u{2500} PDF page {} \u{2500}\n", i + 1));
        }
        body.push_str(&text);
    }
    let mut t = TextDocument::new();
    if !body.is_empty() {
        t.insert_str(&body);
    }
    Ok(Document::Text(t))
}
