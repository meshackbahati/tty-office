//! Cross-kind Save As conversion: the destination extension decides the
//! bytes, so text, word, and sheet content convert instead of mislabeling.

use std::path::PathBuf;

use tempfile::TempDir;
use tty_office::{open, Document, Editor};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A `.docx` is a zip package; the central directory signature must be
/// present or the file is corrupt text with the wrong name.
fn assert_valid_zip(path: &std::path::Path) {
    let bytes = std::fs::read(path).expect("read output");
    assert!(
        bytes.windows(4).any(|w| w == [0x50, 0x4B, 0x03, 0x04]),
        "missing zip local-file signature"
    );
    assert!(
        bytes.windows(4).any(|w| w == [0x50, 0x4B, 0x05, 0x06]),
        "missing zip central-directory signature"
    );
}

#[test]
#[cfg(feature = "docx")]
fn text_converts_to_valid_docx() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("memo.docx");
    let mut doc = open(&fixture("plain.txt")).expect("open txt");
    doc.save_as_convert(&out).expect("convert to docx");
    assert_valid_zip(&out);
    assert!(!matches!(doc, Document::Text(_)));
    let again = open(&out).expect("reopen converted");
    assert_eq!(
        again.text_projection(),
        "Plain notes\nA second line for the corpus.\n\nTrailing paragraph."
    );
}

#[test]
fn same_kind_save_is_untouched() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("copy.txt");
    let mut doc = open(&fixture("plain.txt")).expect("open txt");
    doc.save_as_convert(&out).expect("save txt");
    assert!(matches!(doc, Document::Text(_)));
    let back = std::fs::read_to_string(&out).expect("read back");
    assert!(back.contains("Plain notes"));
}

#[test]
#[cfg(feature = "xlsx")]
fn sheet_converts_to_text() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("grid.txt");
    let mut doc = open(&fixture("budget.xlsx")).expect("open sheet");
    doc.save_as_convert(&out).expect("convert to text");
    assert!(matches!(doc, Document::Text(_)));
    let back = std::fs::read_to_string(&out).expect("read back");
    assert!(back.contains("region"), "projection missing: {back}");
}

#[test]
#[cfg(feature = "pdf")]
fn text_converts_to_pdf_by_export() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("memo.pdf");
    let mut doc = open(&fixture("plain.txt")).expect("open txt");
    doc.save_as_convert(&out).expect("convert to pdf");
    let bytes = std::fs::read(&out).expect("read pdf");
    assert!(
        bytes.starts_with(b"%PDF"),
        "export did not write a pdf document"
    );
}
