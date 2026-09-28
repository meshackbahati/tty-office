//! Chart overlay: numeric selections graph, empty ones report.

#![cfg(feature = "xlsx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, Mode, Motion, SheetDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
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

fn numbered_sheet() -> App {
    // A missing path adopts without touching disk, so no fixture files
    // are needed and nothing pollutes the repository.
    let mut doc = tty_office::open(std::path::Path::new("chart.xlsx")).expect("open sheet");
    if let Document::Sheet(sheet) = &mut doc {
        put(sheet, 0, 0, "hello");
        put(sheet, 0, 1, "10");
        put(sheet, 1, 1, "20");
        put(sheet, 2, 1, "30");
    }
    App::new(doc)
}

fn drawn_text(app: &mut App) -> String {
    let backend = TestBackend::new(80, 20);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    let buffer = term.backend().buffer();
    let area = *buffer.area();
    let mut text = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn alt_g_charts_the_selection() {
    let mut app = numbered_sheet();
    // Cursor to B1, then extend down across B1:B3.
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(key(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(key(KeyCode::Char('g'), KeyModifiers::ALT));
    assert_eq!(app.mode, Mode::Chart);
    let text = drawn_text(&mut app);
    assert!(text.contains("B1:B3"), "chart title missing: {text}");
}

#[test]
fn esc_closes_the_chart() {
    let mut app = numbered_sheet();
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(key(KeyCode::Char('g'), KeyModifiers::ALT));
    assert_eq!(app.mode, Mode::Chart);
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn text_selection_reports_no_numbers() {
    let mut app = numbered_sheet();
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    // A1 holds text and A2 is empty: no numbers under the selection.
    app.handle_key(key(KeyCode::Down, KeyModifiers::SHIFT));
    app.handle_key(key(KeyCode::Char('g'), KeyModifiers::ALT));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.message, "No numbers in selection");
}
