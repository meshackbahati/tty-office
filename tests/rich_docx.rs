//! Feature-gated round trips through the DOCX/ODT package layer.
//!
//! Each test creates a package with the public editor surface, saves it,
//! reopens the file, and asserts that the projection and dirty flag match.

#![cfg(feature = "docx")]

use std::path::Path;

use tempfile::TempDir;
use tty_office::{open, Document, Editor, Motion, RichFormat};

fn roundtrip(ext: &str) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join(format!("roundtrip.{ext}"));

    let mut doc = open(&path).expect("open new rich path");
    assert!(!doc.is_dirty());
    assert_eq!(
        doc.path().map(Path::to_path_buf),
        Some(path.clone()),
        "missing {ext} path is adopted without reading"
    );

    doc.insert_str("alpha\nbeta");
    assert!(doc.is_dirty());
    doc.save(None).expect("first save");
    assert!(!doc.is_dirty());

    let mut again = open(&path).expect("reopen");
    assert_eq!(again.text_projection(), "alpha\nbeta");
    assert!(!again.is_dirty());

    // Reopen starts at offset 0; move to the end so the append lands after "beta".
    again.move_cursor(Motion::BufferEnd, false);
    again.insert_char('!');
    again.save(None).expect("second save");
    let final_doc = open(&path).expect("reopen after edit");
    assert_eq!(final_doc.text_projection(), "alpha\nbeta!");
}

#[test]
fn docx_create_edit_reopen_roundtrip() {
    roundtrip("docx");
}

#[test]
fn odt_create_edit_reopen_roundtrip() {
    roundtrip("odt");
}

#[test]
fn open_dispatches_docx_to_rich_variant() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("variant.docx");
    let mut doc = open(&path).expect("open");
    doc.insert_str("x");
    doc.save(None).expect("save");

    let doc = open(&path).expect("reopen");
    assert!(
        matches!(doc, Document::Rich(_)),
        "existing .docx must open as Document::Rich"
    );
}

#[test]
fn new_odt_path_selects_odt_format() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("format.odt");
    let doc = open(&path).expect("open");
    match doc {
        Document::Rich(r) => assert_eq!(r.format(), RichFormat::Odt),
        other => panic!("expected Rich, got {other:?}"),
    }
}

#[test]
fn undo_after_save_restores_pre_save_text() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("undo.docx");
    let mut doc = open(&path).expect("open");
    doc.insert_str("keep");
    doc.save(None).expect("save");
    assert!(!doc.is_dirty());
    // Save must not discard history: one undo returns to the empty buffer.
    assert!(doc.undo());
    assert_eq!(doc.text_projection(), "");
    assert!(doc.is_dirty());
}
