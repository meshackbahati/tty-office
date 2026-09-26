//! Opening and naming documents: the untitled default and the file name
//! suggested the first time a pathless document is saved.

use std::path::PathBuf;

use crate::error::DocumentError;
#[cfg(not(feature = "docx"))]
use crate::text::TextDocument;
use crate::Document;

/// Open `path`, or an empty document when no path is given.
pub fn open_optional(path: Option<&PathBuf>) -> Result<Document, DocumentError> {
    match path {
        Some(p) => crate::open(p),
        // An untitled buffer in the default build is a word document:
        // the suite's primary surface, and what the first Save As will
        // name `untitled.docx` unless the user changes it.
        #[cfg(feature = "docx")]
        None => Ok(Document::Rich(Box::new(crate::RichDocument::new(
            crate::RichFormat::Docx,
        )))),
        #[cfg(not(feature = "docx"))]
        None => Ok(Document::Text(TextDocument::new())),
    }
}

/// File name suggested when a pathless document is saved for the first
/// time; the extension decides which writer handles the file.
pub(crate) fn default_save_name(doc: &Document) -> String {
    match doc {
        Document::Text(_) => "untitled.txt".to_string(),
        #[cfg(feature = "docx")]
        Document::Rich(r) => match r.format() {
            crate::RichFormat::Docx => "untitled.docx".to_string(),
            crate::RichFormat::Odt => "untitled.odt".to_string(),
        },
        #[cfg(feature = "xlsx")]
        Document::Sheet(s) => match s.format() {
            // A legacy workbook cannot be written back, so the save
            // upgrades to the closest writable format.
            crate::SheetFormat::Xls => "untitled.xlsx".to_string(),
            crate::SheetFormat::Xlsx => "untitled.xlsx".to_string(),
            crate::SheetFormat::Ods => "untitled.ods".to_string(),
        },
    }
}
