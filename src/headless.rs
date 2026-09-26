//! Headless operation: read and convert documents without the terminal
//! interface, for scripts and pipelines. The terminal UI never calls
//! these; the binary subcommands print their returned strings.

use std::path::Path;

use crate::editor::Editor;
use crate::error::DocumentError;
use crate::Document;

/// Text projection of `file` for `cat`, without opening the interface.
pub fn cat_text(file: &Path) -> Result<String, DocumentError> {
    let doc = crate::open(file)?;
    Ok(doc.text_projection())
}

/// Metadata summary of `file` for `info`, one `key: value` line each.
pub fn info_text(file: &Path) -> Result<String, DocumentError> {
    let doc = crate::open(file)?;
    let bytes = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    let mut out = format!("path: {}\n", file.display());
    match &doc {
        Document::Text(t) => {
            out.push_str("kind: text\n");
            out.push_str(&format!("bytes: {bytes}\n"));
            out.push_str(&format!("lines: {}\n", t.line_count()));
        }
        #[cfg(feature = "docx")]
        Document::Rich(r) => {
            let format = match r.format() {
                crate::RichFormat::Docx => "docx",
                crate::RichFormat::Odt => "odt",
            };
            out.push_str(&format!("kind: rich ({format})\n"));
            out.push_str(&format!("bytes: {bytes}\n"));
            out.push_str(&format!("lines: {}\n", r.line_count()));
        }
        #[cfg(feature = "xlsx")]
        Document::Sheet(s) => {
            let format = match s.format() {
                crate::SheetFormat::Xlsx => "xlsx",
                crate::SheetFormat::Ods => "ods",
                crate::SheetFormat::Xls => "xls",
            };
            out.push_str(&format!("kind: spreadsheet ({format})\n"));
            out.push_str(&format!("bytes: {bytes}\n"));
            let (rows, cols) = s.used_range();
            out.push_str(&format!("rows: {}\n", rows + 1));
            out.push_str(&format!("cols: {}\n", cols + 1));
            out.push_str(&format!("formulas: {}\n", s.formula_count()));
        }
    }
    Ok(out)
}

/// Convert `input` to `output` for `convert`, inferring both formats
/// from their extensions through the same open and save paths the
/// interface uses.
pub fn convert_files(input: &Path, output: &Path) -> Result<String, DocumentError> {
    let mut doc = crate::open(input)?;
    doc.save_as(output)?;
    Ok(format!(
        "Converted {} to {}\n",
        input.display(),
        output.display()
    ))
}
