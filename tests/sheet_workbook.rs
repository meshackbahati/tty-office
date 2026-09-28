//! Multi-sheet workbooks: every sheet opens, switches keep per-sheet
//! state, and saves preserve all sheets.

#![cfg(feature = "xlsx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::TempDir;
use tty_office::{draw, App, Document, Editor};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn two_sheet_ods(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("book.ods");
    let mut book = spreadsheet_ods::WorkBook::new_empty();
    let mut first = spreadsheet_ods::Sheet::new("Alpha");
    first.set_value(0, 0, "a1");
    book.push_sheet(first);
    let mut second = spreadsheet_ods::Sheet::new("Beta");
    second.set_value(0, 0, "b1");
    book.push_sheet(second);
    spreadsheet_ods::write_ods(&mut book, &path).expect("write fixture");
    path
}

fn as_sheet(doc: &Document) -> &tty_office::SheetDocument {
    match doc {
        Document::Sheet(sheet) => sheet,
        _ => panic!("sheet expected"),
    }
}

fn value(app: &App, row: usize, col: usize) -> String {
    as_sheet(&app.doc)
        .cell(row, col)
        .map(|c| c.value.display())
        .unwrap_or_default()
}

#[test]
fn opens_all_sheets_with_names() {
    let dir = TempDir::new().expect("temp dir");
    let app = App::new(tty_office::open(&two_sheet_ods(&dir)).expect("open"));
    let sheet = as_sheet(&app.doc);
    assert_eq!(
        sheet.sheet_names(),
        vec!["Alpha".to_string(), "Beta".to_string()]
    );
    assert_eq!(sheet.active_sheet(), 0);
    assert_eq!(value(&app, 0, 0), "a1");
}

#[test]
fn switch_preserves_per_sheet_state() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = App::new(tty_office::open(&two_sheet_ods(&dir)).expect("open"));
    // Move away from the origin on the first sheet, then switch twice.
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
    let shift_right = key(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    app.handle_key(shift_right);
    assert_eq!(as_sheet(&app.doc).active_sheet(), 1);
    assert_eq!(value(&app, 0, 0), "b1");
    let shift_left = key(KeyCode::Left, KeyModifiers::ALT | KeyModifiers::SHIFT);
    app.handle_key(shift_left);
    assert_eq!(as_sheet(&app.doc).active_sheet(), 0);
    assert_eq!(as_sheet(&app.doc).cursor_cell(), (1, 1));
    assert_eq!(value(&app, 0, 0), "a1");
}

#[test]
fn save_preserves_all_sheets() {
    let dir = TempDir::new().expect("temp dir");
    let path = two_sheet_ods(&dir);
    let mut app = App::new(tty_office::open(&path).expect("open"));
    let shift_right = key(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    app.handle_key(shift_right);
    app.doc.insert_str("edited");
    app.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
    let again = tty_office::open(&path).expect("reopen");
    let sheet = as_sheet(&again);
    assert_eq!(
        sheet.sheet_names(),
        vec!["Alpha".to_string(), "Beta".to_string()]
    );
    let beta = {
        let mut clone = App::new(again);
        clone.handle_key(shift_right);
        value(&clone, 0, 0)
    };
    assert_eq!(beta, "edited");
    // First sheet untouched.
    let app = App::new(tty_office::open(&path).expect("reopen"));
    assert_eq!(value(&app, 0, 0), "a1");
}

#[test]
fn strip_renders_sheet_names() {
    let dir = TempDir::new().expect("temp dir");
    let mut app = App::new(tty_office::open(&two_sheet_ods(&dir)).expect("open"));
    let backend = TestBackend::new(60, 12);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let buffer = term.backend().buffer();
    let area = *buffer.area();
    let mut text = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    assert!(text.contains("Alpha"), "strip missing: {text}");
    assert!(text.contains("Beta"), "strip missing: {text}");
}

#[test]
fn strip_click_switches_sheets() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tty_office::draw;

    let dir = TempDir::new().expect("temp dir");
    let mut app = App::new(tty_office::open(&two_sheet_ods(&dir)).expect("open"));
    // Start on Beta through the chord, then click back on Alpha.
    let shift_right = key(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    app.handle_key(shift_right);
    assert_eq!(as_sheet(&app.doc).active_sheet(), 1);
    let backend = TestBackend::new(80, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Single document tab with sidebar: the body starts at row 1 with
    // five rows, so the strip sits on row 5; the Alpha cell starts at
    // the pane edge, column 22.
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 24,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(as_sheet(&app.doc).active_sheet(), 0);
}
