//! DOCX and ODT documents behind the shared editing surface.
//!
//! Structure and styles live in an `rdocx` model. Interactive editing runs on
//! a plain-text [`TextDocument`] projection of body paragraphs so cursor,
//! selection, undo, and the viewport stay identical to TXT and Markdown.
//! On save the projection is written back into body paragraphs, then the
//! `rdocx` model is serialized, which preserves tables, headers, and producer
//! XML that the projection does not address.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::editor::{Cursor, Editor, Motion};
use crate::error::DocumentError;
use crate::text::TextDocument;

mod model;

use model::{load_model, project_body_paragraphs, set_paragraph_text};

/// On-disk package kind for a rich document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichFormat {
    /// Office Open XML Word package (`.docx`).
    Docx,
    /// OpenDocument Text package (`.odt`).
    Odt,
}

impl RichFormat {
    /// Format implied by a path extension, if any.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "docx" => Some(Self::Docx),
            "odt" => Some(Self::Odt),
            _ => None,
        }
    }
}

/// Word-processing document: `rdocx` storage plus a text editing surface.
///
/// Callers drive this type through [`Editor`] and the line-oriented accessors
/// that mirror [`TextDocument`]. The underlying `rdocx::Document` is never
/// exposed, which keeps third-party types out of the public semver surface.
pub struct RichDocument {
    format: RichFormat,
    path: Option<PathBuf>,
    /// Format model: source of truth for structure, styles, and serialization.
    model: rdocx::Document,
    /// Line-oriented editing surface: cursor, selection, undo, and viewport.
    surface: TextDocument,
}

impl fmt::Debug for RichDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RichDocument")
            .field("format", &self.format)
            .field("path", &self.path)
            .field("dirty", &self.is_dirty())
            .finish_non_exhaustive()
    }
}

impl RichDocument {
    /// Create an empty Word document that will be saved as `format`.
    pub fn new(format: RichFormat) -> Self {
        let mut surface = TextDocument::new();
        // Word documents wrap at the viewport width; plain text keeps
        // the horizontal scroll contract.
        surface.set_wrap(true);
        Self {
            format,
            path: None,
            model: rdocx::Document::new(),
            surface,
        }
    }

    /// Open a `.docx` or `.odt` package from disk.
    pub fn open(path: &Path) -> Result<Self, DocumentError> {
        let format = RichFormat::from_path(path).ok_or_else(|| {
            DocumentError::UnsupportedFormat(
                path.extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
        })?;
        let model = load_model(path, format)?;
        let projection = project_body_paragraphs(&model);
        let mut surface = TextDocument::new();
        surface.set_wrap(true);
        surface.load_clean(&projection);
        Ok(Self {
            format,
            path: Some(path.to_path_buf()),
            model,
            surface,
        })
    }

    /// Adopt `path` as the save destination without reading or writing it.
    ///
    /// Used when the user names a new rich file that does not exist yet; the
    /// document stays clean until the first edit.
    pub fn adopt_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
        if let Some(fmt) = RichFormat::from_path(path) {
            self.format = fmt;
        }
    }

    /// Path this document was opened from, when known.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Package format used for the next save.
    pub fn format(&self) -> RichFormat {
        self.format
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

    /// Mutable access to the text surface used by the viewport and search.
    pub fn surface_mut(&mut self) -> &mut TextDocument {
        &mut self.surface
    }

    /// Immutable access to the text surface.
    pub fn surface(&self) -> &TextDocument {
        &self.surface
    }

    /// Heading level (1-9) of the paragraph behind surface `line`, from
    /// the package style id. Documents that never styled the paragraph
    /// read as no heading.
    pub fn heading_at(&self, line: usize) -> Option<u8> {
        model::paragraph_heading(&self.model, line)
    }

    /// Hyperlink runs of the paragraph behind surface `line`, with
    /// paragraph-relative ranges matching surface character indices.
    pub fn link_spans(&self, line: usize) -> Vec<model::LinkSpan> {
        model::paragraph_links(&self.model, line)
    }

    /// Styled runs of the paragraph behind surface `line`, with
    /// paragraph-relative ranges matching surface character indices.
    pub fn run_styles(&self, line: usize) -> Vec<model::RunStyle> {
        model::paragraph_run_styles(&self.model, line)
    }

    /// Apply Heading `level` to the cursor paragraph. Unsaved surface
    /// text syncs into the model first so fresh documents style
    /// correctly; the surface stays dirty so the style saves with
    /// the next write.
    pub fn apply_heading(&mut self, level: u8) -> Result<(), String> {
        if self.surface.is_dirty() {
            self.sync_surface_into_model()
                .map_err(|err| err.to_string())?;
        }
        let line = self.surface.cursor_line();
        model::set_paragraph_heading(&mut self.model, line, level)?;
        self.surface.mark_dirty();
        Ok(())
    }

    /// First visible line index.
    pub fn rowoff(&self) -> usize {
        self.surface.rowoff()
    }

    /// First visible display column.
    pub fn coloff(&self) -> usize {
        self.surface.coloff()
    }

    /// Zero-based line containing the cursor.
    pub fn cursor_line(&self) -> usize {
        self.surface.cursor_line()
    }

    /// Zero-based display column of the cursor within its line.
    pub fn cursor_display_col(&self) -> usize {
        self.surface.cursor_display_col()
    }

    /// Number of lines in the editing surface. Always at least one.
    pub fn line_count(&self) -> usize {
        self.surface.line_count()
    }

    /// Text of line `idx` without the trailing newline.
    pub fn line_text(&self, idx: usize) -> String {
        self.surface.line_text(idx)
    }

    /// Character index of the start of line `idx`.
    pub fn line_char_start(&self, idx: usize) -> usize {
        self.surface.rope().line_to_char(idx)
    }

    /// Normalized selection range as inclusive-exclusive character indices.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.surface.selection_range()
    }

    /// Clamp scroll offsets so the cursor stays inside the viewport.
    pub fn scroll_to_cursor(&mut self, view_h: usize, view_w: usize) {
        self.surface.scroll_to_cursor(view_h, view_w);
    }

    /// Character index of the cursor head.
    pub fn cursor_char(&self) -> usize {
        self.surface.cursor_char()
    }

    /// Search forward from `from` for `needle`, wrapping once.
    pub fn find(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        self.surface.find(needle, from)
    }

    /// Search forward from `from` without wrapping.
    pub fn find_no_wrap(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        self.surface.find_no_wrap(needle, from)
    }

    /// Place the cursor at `start` with `end` as the exclusive bound.
    pub fn set_cursor_range(&mut self, start: usize, end: usize) {
        self.surface.set_cursor_range(start, end);
    }

    /// Replace character range `[start, end)` as one undo unit.
    pub fn replace_range(&mut self, start: usize, end: usize, replacement: &str) {
        self.surface.replace_range(start, end, replacement);
    }

    /// Cut the entire current line into `cutbuffer`.
    pub fn cut_current_line(&mut self, cutbuffer: &mut String) {
        self.surface.cut_current_line(cutbuffer);
    }

    /// Slice of the projection as characters `[start, end)`.
    pub fn slice_chars(&self, start: usize, end: usize) -> String {
        let text = self.surface.text_projection();
        text.chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect()
    }

    /// Full-document word count, cached until the next edit.
    pub fn word_count(&mut self) -> usize {
        self.surface.word_count()
    }

    /// Replace a string in the package model as well as the editing surface.
    ///
    /// Returns the number of replacements reported by the package layer.
    /// Surface matches are updated so the viewport stays consistent with the
    /// model after a template substitution.
    pub fn replace_all_model(&mut self, needle: &str, replacement: &str) -> usize {
        if needle.is_empty() {
            return 0;
        }
        let mut map = std::collections::HashMap::new();
        map.insert(needle, replacement);
        let count = self.model.replace_all(&map);
        let mut spans = Vec::new();
        let mut cursor = 0usize;
        while let Some((a, b)) = self.surface.find_no_wrap(needle, cursor) {
            spans.push((a, b));
            cursor = b.max(a + 1);
        }
        for (a, b) in spans.into_iter().rev() {
            self.surface.replace_range(a, b, replacement);
        }
        count
    }

    /// Write surface lines back into body paragraphs, then serialize.
    fn save_to(&mut self, target: &Path) -> Result<(), DocumentError> {
        let detected = RichFormat::from_path(target).ok_or_else(|| DocumentError::Save {
            path: target.to_path_buf(),
            message: "destination extension must be .docx or .odt".to_string(),
        })?;
        self.format = detected;
        if self.surface.is_dirty() {
            self.sync_surface_into_model()?;
        }
        match self.format {
            RichFormat::Docx => self.model.save(target).map_err(|err| DocumentError::Save {
                path: target.to_path_buf(),
                message: err.to_string(),
            })?,
            RichFormat::Odt => {
                self.model
                    .save_odt(target)
                    .map_err(|err| DocumentError::Save {
                        path: target.to_path_buf(),
                        message: err.to_string(),
                    })?;
            }
        }
        self.path = Some(target.to_path_buf());
        // The surface text is what was written into the package; only the
        // dirty flag needs clearing. History and the caret are kept so undo
        // after save matches plain text behaviour.
        self.surface.mark_saved();
        Ok(())
    }

    /// Copy each surface line into the corresponding body paragraph.
    ///
    /// Paragraph count grows or shrinks to match the projection. Tables and
    /// other non-paragraph body content are left in place; only paragraph
    /// runs are rewritten, and a paragraph that cannot be rewritten in place
    /// (for example because it holds a drawing) is reported as a save error
    /// rather than being destroyed.
    fn sync_surface_into_model(&mut self) -> Result<(), DocumentError> {
        let projection = self.surface.text_projection();
        // Split on newlines only: a projection that ends with '\n' yields a
        // final empty string, which is a real empty trailing paragraph and
        // must not be discarded.
        let lines: Vec<String> = projection.split('\n').map(str::to_string).collect();

        let existing = self.model.paragraph_count();
        for (idx, line) in lines.iter().enumerate() {
            if idx < existing {
                set_paragraph_text(&mut self.model, idx, line).map_err(|message| {
                    DocumentError::Save {
                        path: self
                            .path
                            .clone()
                            .unwrap_or_else(|| PathBuf::from("[no name]")),
                        message,
                    }
                })?;
            } else {
                self.model.add_paragraph(line);
            }
        }
        // Remove surplus paragraphs from the end so structural deletes survive
        // a round trip. Content indices are recomputed each iteration because
        // removing one shifts the rest.
        while self.model.paragraph_count() > lines.len() {
            let last_para_index = self.model.paragraph_count() - 1;
            match self.model.content_index_of_paragraph(last_para_index) {
                Some(index) => {
                    if !self.model.remove_content(index) {
                        break;
                    }
                }
                None => break,
            }
        }
        Ok(())
    }
}

impl Default for RichDocument {
    fn default() -> Self {
        Self::new(RichFormat::Docx)
    }
}

impl Editor for RichDocument {
    fn cursor(&self) -> Cursor {
        self.surface.cursor()
    }

    fn selection(&self) -> Option<(Cursor, Cursor)> {
        self.surface.selection()
    }

    fn select_all(&mut self) {
        self.surface.select_all();
    }

    fn clear_selection(&mut self) {
        self.surface.clear_selection();
    }

    fn move_cursor(&mut self, motion: Motion, extend: bool) {
        self.surface.move_cursor(motion, extend);
    }

    fn insert_char(&mut self, c: char) {
        self.surface.insert_char(c);
    }

    fn insert_str(&mut self, s: &str) {
        self.surface.insert_str(s);
    }

    fn delete_back(&mut self) {
        self.surface.delete_back();
    }

    fn delete_forward(&mut self) {
        self.surface.delete_forward();
    }

    fn is_dirty(&self) -> bool {
        self.surface.is_dirty()
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError> {
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => self.path.clone().ok_or_else(|| DocumentError::Save {
                path: PathBuf::from("[no name]"),
                message: "no path; use save_as".to_string(),
            })?,
        };
        self.save_to(&target)
    }

    fn undo(&mut self) -> bool {
        self.surface.undo()
    }

    fn redo(&mut self) -> bool {
        self.surface.redo()
    }

    fn text_projection(&self) -> String {
        self.surface.text_projection()
    }
}

impl RichDocument {
    /// Flush the editing surface into the package model so export sees
    /// current text without clearing the dirty flag.
    pub(crate) fn prepare_export(&mut self) -> Result<(), DocumentError> {
        if self.surface.is_dirty() {
            self.sync_surface_into_model()?;
        }
        Ok(())
    }

    /// Serialize the package model to PDF via `rdocx`.
    pub(crate) fn export_pdf(&self, path: &Path) -> Result<(), DocumentError> {
        self.model
            .save_pdf(path)
            .map_err(|err| DocumentError::Save {
                path: path.to_path_buf(),
                message: err.to_string(),
            })
    }

    /// Full HTML document for the package model.
    pub(crate) fn export_html(&self) -> String {
        self.model.to_html()
    }

    /// Markdown body for the package model.
    pub(crate) fn export_markdown(&self) -> String {
        self.model.to_markdown()
    }

    /// Character-index revision of the editing surface, for proof scheduling.
    #[cfg(feature = "proof")]
    pub(crate) fn revision(&self) -> u64 {
        self.surface.revision()
    }
}
