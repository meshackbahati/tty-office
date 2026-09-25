//! Export paths for rich DOCX/ODT packages (HTML, Markdown, PDF via rdocx).

#![cfg(feature = "docx")]

use std::path::Path;

use tty_office::{export, Document, Editor, ExportFormat, RichDocument, RichFormat};

fn rich_doc(body: &str) -> Document {
    let mut r = RichDocument::new(RichFormat::Docx);
    r.insert_str(body);
    Document::Rich(Box::new(r))
}

#[test]
fn export_rich_markdown_contains_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.md");
    let mut doc = rich_doc("Hello rich export");
    export(&mut doc, &path).expect("markdown export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert!(on_disk.contains("Hello rich export"));
}

#[test]
fn export_rich_html_is_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.html");
    let mut doc = rich_doc("Html body");
    export(&mut doc, &path).expect("html export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert!(on_disk.contains("<html") || on_disk.contains("<HTML"));
    assert!(on_disk.contains("Html body"));
}

#[test]
fn export_rich_pdf_writes_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.pdf");
    let mut doc = rich_doc("PDF body");
    export(&mut doc, &path).expect("pdf export via rdocx");
    let bytes = std::fs::read(&path).expect("read");
    assert!(
        bytes.starts_with(b"%PDF"),
        "rdocx save_pdf should emit PDF magic"
    );
    let _ = ExportFormat::Pdf;
    let _ = Path::new(".");
}
