//! Feature-gated round trips through the DOCX/ODT package layer.
//!
//! Each test creates a package with the public editor surface, saves it,
//! reopens the file, and asserts that the projection and dirty flag match.

#![cfg(feature = "docx")]

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;
use tty_office::{open, App, Document, Editor, Motion, RichFormat};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

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

#[test]
fn save_as_txt_converts_word_to_text() {
    use tty_office::DocumentError;

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("roundtrip.docx");
    let mut doc = open(&path).expect("open new rich path");
    doc.insert_str("words");
    // Saving word content under a text name converts instead of
    // mislabeling package bytes.
    doc.save_as_convert(&dir.path().join("words.txt"))
        .expect("convert to text");
    assert!(matches!(doc, Document::Text(_)));
    let back = std::fs::read_to_string(dir.path().join("words.txt")).expect("read back");
    assert_eq!(back, "words");
    // The direct writer still refuses mismatched extensions loudly.
    let mut direct = open(&path).expect("open new rich path");
    direct.insert_str("words");
    let err = direct
        .save_as(&dir.path().join("direct.txt"))
        .expect_err("txt save must refuse");
    match err {
        DocumentError::Save { message, .. } => {
            assert!(
                message.contains(".docx") && message.contains(".odt"),
                "message names the writable extensions: {message}"
            );
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert!(!dir.path().join("direct.txt").exists());
}

#[test]
fn save_as_bare_name_gains_the_document_extension() {
    use tty_office::{RichDocument, RichFormat};

    let dir = TempDir::new().expect("temp dir");
    let bare = dir.path().join("budget").to_string_lossy().into_owned();
    let mut doc = Document::Rich(Box::new(RichDocument::new(RichFormat::Docx)));
    doc.insert_str("words");
    let mut app = App::new(doc);
    // Pathless documents fold Ctrl+S into Save As; replace the
    // suggested name with an extensionless absolute path.
    app.handle_key(ctrl('s'));
    app.prompt_buf.clear();
    for c in bare.chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.message.starts_with("Wrote "),
        "message: {}",
        app.message
    );
    assert!(dir.path().join("budget.docx").is_file());
    assert!(!dir.path().join("budget").exists());
}
