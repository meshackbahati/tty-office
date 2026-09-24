//! Feature-gated spreadsheet round trips through the xlsx/ods package layer.
//!
//! Each test creates a sheet with the public editor surface, saves it,
//! reopens the file, and asserts that the projection and dirty flag match.
//! App-level navigation is exercised through `App::handle_key`.

#![cfg(feature = "xlsx")]

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;
use tty_office::{open, App, Document, Editor, Mode, Motion, SheetDocument, SheetFormat};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn enter() -> KeyEvent {
    key(KeyCode::Enter, KeyModifiers::NONE)
}

fn tab() -> KeyEvent {
    key(KeyCode::Tab, KeyModifiers::NONE)
}

fn open_sheet(path: &Path) -> Document {
    open(path).expect("open sheet path")
}

fn as_sheet(doc: &Document) -> &SheetDocument {
    match doc {
        Document::Sheet(s) => s,
        other => panic!("expected Sheet, got {}", other.display_name()),
    }
}

fn cell_display(sheet: &SheetDocument, row: usize, col: usize) -> String {
    sheet
        .cell(row, col)
        .map(|c| c.value.display())
        .unwrap_or_default()
}

fn roundtrip(ext: &str) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join(format!("roundtrip.{ext}"));

    let mut doc = open_sheet(&path);
    assert!(!doc.is_dirty());
    assert_eq!(
        doc.path().map(Path::to_path_buf),
        Some(path.clone()),
        "missing {ext} path is adopted without reading"
    );

    {
        let sheet = doc.sheet_mut().expect("sheet");
        sheet.set_cell_content("42");
    }
    assert!(doc.is_dirty());
    doc.save(None).expect("first save");
    assert!(!doc.is_dirty());

    let again = open_sheet(&path);
    let sheet = as_sheet(&again);
    assert_eq!(cell_display(sheet, 0, 0), "42");
    assert!(!again.is_dirty());
}

#[test]
fn xlsx_create_edit_reopen_roundtrip() {
    roundtrip("xlsx");
}

#[test]
fn ods_create_edit_reopen_roundtrip() {
    roundtrip("ods");
}

#[test]
fn open_dispatches_xlsx_to_sheet_variant() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("variant.xlsx");
    let mut doc = open_sheet(&path);
    doc.sheet_mut().expect("sheet").set_cell_content("x");
    doc.save(None).expect("save");

    let doc = open_sheet(&path);
    assert!(matches!(doc, Document::Sheet(_)));
    assert_eq!(cell_display(as_sheet(&doc), 0, 0), "x");
}

#[test]
fn formula_survives_roundtrip() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("formula.xlsx");

    let mut doc = open_sheet(&path);
    doc.sheet_mut().expect("sheet").set_cell_content("1");
    doc.move_cursor(Motion::Right, false);
    doc.sheet_mut().expect("sheet").set_cell_content("2");
    doc.move_cursor(Motion::Right, false);
    doc.sheet_mut().expect("sheet").set_cell_content("=A1+B1");
    doc.save(None).expect("save with formula");

    let again = open_sheet(&path);
    let sheet = as_sheet(&again);
    assert_eq!(cell_display(sheet, 0, 2), "3");
    let formula = sheet.cell(0, 2).and_then(|c| c.formula.clone());
    assert_eq!(formula.as_deref(), Some("=A1+B1"));
}

#[test]
fn xls_save_is_refused_with_clear_message() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("book.xls");
    let mut doc = Document::Sheet(Box::new(SheetDocument::new(SheetFormat::Xls)));
    doc.sheet_mut().expect("sheet").adopt_path(&path);
    let err = doc.save(None).expect_err("xls save must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("xls cannot be written") || msg.contains("save as"),
        "unexpected message: {msg}"
    );
    // The temp directory must not contain a written workbook.
    assert!(!path.exists(), "no .xls file may be written");
}

#[test]
fn cell_edit_via_app_enter_and_typing() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("app.xlsx");
    let mut app = App::new(open_sheet(&path));

    // Enter opens CellEdit for A1; type 7 and commit. Commit moves down.
    app.handle_key(enter());
    assert!(matches!(app.mode, Mode::Prompt(_)));
    app.handle_key(key(KeyCode::Char('7'), KeyModifiers::NONE));
    app.handle_key(enter());
    assert_eq!(app.doc.text_projection(), "7");
    assert_eq!(app.doc.sheet_mut().expect("sheet").cursor_cell(), (1, 0));

    // Tab moves right onto B2; type 8 and commit.
    app.handle_key(tab());
    assert_eq!(app.doc.sheet_mut().expect("sheet").cursor_cell(), (1, 1));
    app.handle_key(enter());
    app.handle_key(key(KeyCode::Char('8'), KeyModifiers::NONE));
    app.handle_key(enter());

    let sheet = app.doc.sheet_mut().expect("sheet");
    assert_eq!(cell_display(sheet, 0, 0), "7");
    assert_eq!(cell_display(sheet, 1, 1), "8");
    // Commit moves down again, so the cursor rests on row 2 col 1.
    assert_eq!(sheet.cursor_cell(), (2, 1));
}

#[test]
fn ctrl_s_saves_and_clears_dirty() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("quick.xlsx");
    let mut app = App::new(open_sheet(&path));
    app.handle_key(enter());
    app.handle_key(key(KeyCode::Char('1'), KeyModifiers::NONE));
    app.handle_key(enter());
    assert!(app.doc.is_dirty());
    app.handle_key(ctrl('s'));
    assert!(!app.doc.is_dirty());
    assert!(app.message.contains("Wrote"), "message: {}", app.message);
}

#[test]
fn empty_cell_edit_clears_cell() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("clear.xlsx");
    let mut app = App::new(open_sheet(&path));
    app.doc.sheet_mut().expect("sheet").set_cell_content("temp");
    assert_eq!(cell_display(as_sheet(&app.doc), 0, 0), "temp");

    // Enter prefills with "temp"; backspaces clear it; empty commit removes.
    app.handle_key(enter());
    assert_eq!(app.prompt_buf, "temp");
    for _ in 0..4 {
        app.handle_key(key(KeyCode::Backspace, KeyModifiers::NONE));
    }
    assert_eq!(app.prompt_buf, "");
    app.handle_key(enter());
    let sheet = app.doc.sheet_mut().expect("sheet");
    assert_eq!(cell_display(sheet, 0, 0), "");
    assert_eq!(sheet.cell(0, 0), None);
}

#[test]
fn undo_restores_previous_cell_value() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("undo.xlsx");
    let mut doc = open_sheet(&path);
    doc.sheet_mut().expect("sheet").set_cell_content("a");
    doc.sheet_mut().expect("sheet").set_cell_content("b");
    assert_eq!(cell_display(as_sheet(&doc), 0, 0), "b");
    assert!(doc.undo());
    assert_eq!(cell_display(as_sheet(&doc), 0, 0), "a");
    assert!(doc.redo());
    assert_eq!(cell_display(as_sheet(&doc), 0, 0), "b");
}
