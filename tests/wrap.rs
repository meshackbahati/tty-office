//! Word wrap: long lines break at the viewport width on word-document
//! surfaces, while plain text keeps horizontal scrolling.

#![cfg(feature = "docx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::TempDir;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn rich_app(text: &str) -> App {
    let dir = TempDir::new().expect("temp dir");
    // A missing path adopts without reading; the directory must exist
    // only for the open call itself.
    let mut doc = tty_office::open(&dir.path().join("wrap.docx")).expect("open rich");
    doc.insert_str(text);
    App::new(doc)
}

fn drawn(app: &mut App) -> Vec<String> {
    // Narrow enough that wrapping engages and the sidebar stays hidden.
    let backend = TestBackend::new(30, 10);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    let buffer = term.backend().buffer();
    let area = *buffer.area();
    let mut rows = Vec::new();
    for y in area.top()..area.bottom() {
        let mut row = String::new();
        for x in area.left()..area.right() {
            row.push_str(buffer[(x, y)].symbol());
        }
        rows.push(row.trim_end().to_string());
    }
    rows
}

fn cursor(app: &mut App) -> (usize, usize) {
    let surface = app.doc.prose_surface().expect("surface");
    (surface.cursor_line(), surface.cursor_char())
}

#[test]
fn long_lines_wrap_at_word_boundaries() {
    let mut app = rich_app("alpha beta gamma delta epsilon zeta eta theta");
    let rows = drawn(&mut app);
    // Menu row 0, hairline row 1, then the first piece of the line.
    assert!(rows[2].starts_with("│alpha"), "rows: {rows:?}");
    // The continuation is a wrapped piece, not a document line: it
    // carries text past the first content width.
    assert!(
        rows[3].starts_with('│') && rows[3].len() > 3,
        "rows: {rows:?}"
    );
    assert!(!rows[2].contains("theta"), "line did not wrap: {rows:?}");
    assert!(
        rows.iter().any(|r| r.contains("theta")),
        "text lost: {rows:?}"
    );
}

#[test]
fn vertical_motion_moves_by_display_row() {
    let line0: String = "0123456789".repeat(6);
    let mut app = rich_app(&format!("{line0}\nhi"));
    drawn(&mut app);
    // The caret starts at the end of "hi" with goal column 2. Moving up
    // must land on the last piece of line 0 at that column, which line
    // motion alone would place at character 2 instead.
    app.handle_key(key(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(cursor(&mut app), (0, 58));
    // Moving back down returns to the short line.
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(cursor(&mut app), (1, 63));
}

#[test]
fn click_lands_on_wrapped_pieces() {
    let line0: String = "0123456789".repeat(6);
    let mut app = rich_app(&line0);
    drawn(&mut app);
    // Menu row 0, hairline row 1, first piece row 2, second piece row 3;
    // column 3 is content column 2 past the page border.
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 3,
        row: 3,
        modifiers: KeyModifiers::NONE,
    });
    // Still line 0, thirty characters in: the click found the piece.
    assert_eq!(cursor(&mut app), (0, 30));
}

#[test]
fn plain_text_does_not_wrap() {
    let mut doc = TextDocument::new();
    doc.insert_str(&"x".repeat(40));
    let mut app = App::new(Document::Text(doc));
    let rows = drawn(&mut app);
    // The overlong line scrolls horizontally: the next display row is a
    // blank frame, not a continuation piece.
    assert!(rows[2].starts_with('│'), "rows: {rows:?}");
    assert!(
        rows[3].chars().all(|c| c == '│' || c == ' '),
        "plain text wrapped: {rows:?}"
    );
}
