//! PDF viewing: existing files open as extracted text, missing paths
//! start untitled text, and corrupt files error instead of panicking.

#![cfg(feature = "pdf")]

use std::path::PathBuf;

use tempfile::TempDir;
use tty_office::{open, Document, Editor};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn pdf_opens_as_extracted_text_without_a_path() {
    let doc = open(&fixture("sample.pdf")).expect("open pdf");
    let Document::Text(t) = &doc else {
        panic!("pdf must open as text");
    };
    assert!(
        t.text_projection().contains("Plain notes"),
        "extraction missing source text"
    );
    // No path is adopted, so saving always passes through Save As
    // instead of overwriting the PDF with plain text.
    assert!(doc.path().is_none());
    assert!(doc.is_dirty());
}

#[test]
fn missing_pdf_starts_untitled_text() {
    let dir = TempDir::new().expect("temp dir");
    let doc = open(&dir.path().join("missing.pdf")).expect("open missing");
    assert!(matches!(doc, Document::Text(_)));
}

#[test]
fn corrupt_pdf_errors() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("corrupt.pdf");
    std::fs::write(&path, "not a pdf at all").expect("write fixture");
    assert!(open(&path).is_err());
}
