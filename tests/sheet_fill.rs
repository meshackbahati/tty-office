//! Fill down and right: leading content copies across with relative
//! formula references shifted per row or column.

#![cfg(feature = "xlsx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tty_office::{App, Document, Editor, Motion, SheetDocument, SheetFormat};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn fresh_sheet() -> SheetDocument {
    SheetDocument::new(SheetFormat::Xlsx)
}

fn put(sheet: &mut SheetDocument, row: usize, col: usize, input: &str) {
    sheet.move_cursor(Motion::BufferStart, false);
    for _ in 0..row {
        sheet.move_cursor(Motion::Down, false);
    }
    for _ in 0..col {
        sheet.move_cursor(Motion::Right, false);
    }
    sheet.set_cell_content(input);
}

fn value(sheet: &SheetDocument, row: usize, col: usize) -> String {
    sheet
        .cell(row, col)
        .map(|c| c.value.display())
        .unwrap_or_default()
}

fn formula(sheet: &SheetDocument, row: usize, col: usize) -> String {
    sheet
        .cell(row, col)
        .and_then(|c| c.formula.clone())
        .unwrap_or_default()
}

fn app_with(sheet: SheetDocument) -> App {
    App::new(Document::Sheet(Box::new(sheet)))
}

#[test]
fn fill_down_copies_formula_with_shifted_refs() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 0, "1");
    put(&mut sheet, 0, 1, "2");
    put(&mut sheet, 0, 2, "=SUM(A1,B1)");
    put(&mut sheet, 1, 0, "3");
    put(&mut sheet, 1, 1, "4");
    // put() leaves the cursor behind, so step onto C2 before filling.
    sheet.move_cursor(Motion::BufferStart, false);
    sheet.move_cursor(Motion::Down, false);
    sheet.move_cursor(Motion::Right, false);
    sheet.move_cursor(Motion::Right, false);
    let mut app = app_with(sheet);
    app.handle_key(ctrl('d'));
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(formula(sheet, 1, 2), "=SUM(A2,B2)");
    assert_eq!(value(sheet, 1, 2), "7");
    assert!(
        app.message.starts_with("Filled "),
        "message: {}",
        app.message
    );
}

#[test]
fn fill_down_pins_absolute_markers() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 3, "=$A$1+$B2+C$3+D4");
    sheet.move_cursor(Motion::BufferStart, false);
    sheet.move_cursor(Motion::Down, false);
    for _ in 0..3 {
        sheet.move_cursor(Motion::Right, false);
    }
    let mut app = app_with(sheet);
    app.handle_key(ctrl('d'));
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(formula(sheet, 1, 3), "=$A$1+$B3+C$3+D5");
}

#[test]
fn fill_down_range_seeds_from_the_top_row() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 2, "=A1*10");
    put(&mut sheet, 0, 0, "2");
    put(&mut sheet, 1, 0, "3");
    put(&mut sheet, 2, 0, "4");
    // Cursor back to C1, then extend down across C1:C3.
    sheet.move_cursor(Motion::BufferStart, false);
    sheet.move_cursor(Motion::Right, false);
    sheet.move_cursor(Motion::Right, false);
    let mut app = app_with(sheet);
    let shift_down = key(KeyCode::Down, KeyModifiers::SHIFT);
    app.handle_key(shift_down);
    app.handle_key(shift_down);
    app.handle_key(ctrl('d'));
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(formula(sheet, 1, 2), "=A2*10");
    assert_eq!(value(sheet, 1, 2), "30");
    assert_eq!(formula(sheet, 2, 2), "=A3*10");
    assert_eq!(value(sheet, 2, 2), "40");
}

#[test]
fn fill_right_copies_across_with_shifted_refs() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 0, "5");
    put(&mut sheet, 0, 1, "=A1*2");
    // Step onto B1, extend across B1:C1, then fill rightward from the
    // leading cell B1.
    sheet.move_cursor(Motion::BufferStart, false);
    sheet.move_cursor(Motion::Right, false);
    let mut app = app_with(sheet);
    let shift_right = key(KeyCode::Right, KeyModifiers::SHIFT);
    app.handle_key(shift_right);
    // Ctrl+Shift+R arrives as uppercase R without a SHIFT flag on most
    // terminals, which the resolver scores above Ctrl+R read-file.
    app.handle_key(key(KeyCode::Char('R'), KeyModifiers::CONTROL));
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(formula(sheet, 0, 2), "=B1*2");
    assert_eq!(value(sheet, 0, 2), "20");
}

#[test]
fn fill_from_vacant_refuses_with_a_message() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 5, 0, "kept");
    // Step up to row 4, whose cell above is vacant.
    sheet.move_cursor(Motion::BufferStart, false);
    for _ in 0..4 {
        sheet.move_cursor(Motion::Down, false);
    }
    let mut app = app_with(sheet);
    app.handle_key(ctrl('d'));
    assert!(
        app.message.contains("Nothing to fill"),
        "message: {}",
        app.message
    );
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(value(sheet, 4, 0), "");
    assert_eq!(value(sheet, 5, 0), "kept");
}

#[test]
fn undo_restores_filled_cells() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 2, "=1+1");
    sheet.move_cursor(Motion::BufferStart, false);
    sheet.move_cursor(Motion::Down, false);
    sheet.move_cursor(Motion::Right, false);
    sheet.move_cursor(Motion::Right, false);
    let mut app = app_with(sheet);
    app.handle_key(ctrl('d'));
    app.handle_key(ctrl('z'));
    let Document::Sheet(sheet) = &app.doc else {
        panic!("sheet expected");
    };
    assert_eq!(value(sheet, 1, 2), "");
}
