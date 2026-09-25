//! Export paths for plain-text documents (HTML and Markdown always;
//! PDF requires the `pdf` feature).

use std::path::Path;

use tty_office::{export, Document, Editor, ExportFormat, TextDocument};

fn text_doc(body: &str) -> Document {
    let mut t = TextDocument::new();
    t.insert_str(body);
    Document::Text(t)
}

#[test]
fn export_format_from_extension() {
    assert_eq!(
        ExportFormat::from_path(Path::new("a.pdf")),
        Some(ExportFormat::Pdf)
    );
    assert_eq!(
        ExportFormat::from_path(Path::new("a.HTML")),
        Some(ExportFormat::Html)
    );
    assert_eq!(
        ExportFormat::from_path(Path::new("a.markdown")),
        Some(ExportFormat::Markdown)
    );
    assert_eq!(ExportFormat::from_path(Path::new("a.bin")), None);
}

#[test]
fn export_text_markdown_is_projection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.md");
    let mut doc = text_doc("# Title\n\nbody text\n");
    export(&mut doc, &path).expect("markdown export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert_eq!(on_disk, "# Title\n\nbody text\n");
    // Export does not adopt the path or clear the dirty flag.
    assert!(doc.is_dirty());
    assert!(doc.path().is_none());
}

#[test]
fn export_text_html_escapes_markup() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.html");
    let mut doc = text_doc("<script>alert(1)</script>\n");
    export(&mut doc, &path).expect("html export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert!(on_disk.starts_with("<!DOCTYPE html>"));
    assert!(on_disk.contains("&lt;script&gt;"));
    assert!(!on_disk.contains("<script>"));
}

#[test]
fn export_unknown_extension_is_unsupported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.bin");
    let mut doc = text_doc("x");
    let err = export(&mut doc, &path).expect_err("unknown ext fails");
    assert!(matches!(
        err,
        tty_office::DocumentError::UnsupportedFormat(_)
    ));
}

#[cfg(feature = "pdf")]
#[test]
fn export_text_pdf_writes_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.pdf");
    let mut doc = text_doc("Hello from tty-office.\nSecond line.\n");
    export(&mut doc, &path).expect("pdf export");
    let bytes = std::fs::read(&path).expect("read");
    assert!(bytes.starts_with(b"%PDF"), "expected PDF magic");
}

#[cfg(not(feature = "pdf"))]
#[test]
fn export_text_pdf_without_feature_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.pdf");
    let mut doc = text_doc("x");
    let err = export(&mut doc, &path).expect_err("pdf needs feature");
    match err {
        tty_office::DocumentError::Save { message, .. } => {
            assert!(message.contains("pdf"));
        }
        other => panic!("expected Save error, got {other:?}"),
    }
}
