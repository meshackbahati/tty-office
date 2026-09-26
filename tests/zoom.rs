//! Display zoom: plus and minus adjust line rhythm, the View menu lists
//! the controls, the status bar shows the level, and clicks account for
//! the inserted blank rows.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::ALT)
}

fn app_with(text: &str) -> App {
    let mut doc = TextDocument::new();
    doc.insert_str(text);
    App::new(Document::Text(doc))
}

fn buffer_lines(term: &mut Terminal<TestBackend>) -> Vec<String> {
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

fn drawn(app: &mut App) -> Vec<String> {
    // Narrow enough that the sidebar stays hidden and rows map directly.
    let backend = TestBackend::new(30, 10);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    buffer_lines(&mut term)
}

#[test]
fn zoom_keys_adjust_level_and_reset() {
    let mut app = app_with("body");
    assert_eq!(app.zoom_percent(), 100);
    app.handle_key(ctrl('='));
    assert_eq!(app.zoom_percent(), 125);
    app.handle_key(ctrl('+'));
    assert_eq!(app.zoom_percent(), 150);
    app.handle_key(ctrl('-'));
    assert_eq!(app.zoom_percent(), 125);
    app.handle_key(ctrl('0'));
    assert_eq!(app.zoom_percent(), 100);
}

#[test]
fn zoom_inserts_blank_rows_between_lines() {
    let mut app = app_with("a\nb");
    let plain = drawn(&mut app);
    assert!(plain[2].starts_with("│a"), "rows: {plain:?}");
    assert!(plain[3].starts_with("│b"), "rows: {plain:?}");
    app.handle_key(ctrl('='));
    let zoomed = drawn(&mut app);
    // Menu row 0, hairline row 1, then each line with one blank framed
    // row after it; rows are trimmed on the right but keep both borders.
    assert!(zoomed[2].starts_with("│a"), "rows: {zoomed:?}");
    assert!(zoomed[2].ends_with('│'), "rows: {zoomed:?}");
    assert_eq!(
        zoomed[3].chars().filter(|&c| c == '│').count(),
        2,
        "rows: {zoomed:?}"
    );
    assert!(!zoomed[3].contains(['a', 'b']), "rows: {zoomed:?}");
    assert!(zoomed[4].starts_with("│b"), "rows: {zoomed:?}");
}

#[test]
fn zoom_moves_the_cursor_with_its_line() {
    let mut app = app_with("a\nb");
    app.handle_key(ctrl('='));
    let backend = TestBackend::new(30, 10);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // The caret follows "b" at the end of the buffer: line 1 renders at
    // display row 2 with one zoom row per line, below menu and hairline,
    // one column right for the page border.
    term.backend_mut().assert_cursor_position((2, 4));
}

#[test]
fn view_menu_lists_zoom_controls() {
    let mut app = app_with("body");
    app.handle_key(alt('v'));
    let backend = TestBackend::new(60, 14);
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
    assert!(text.contains("Zoom in"), "menu missing: {text}");
    assert!(text.contains("Reset zoom"), "menu missing: {text}");
}

#[test]
fn status_shows_zoom_percent_while_zoomed() {
    let mut app = app_with("body");
    app.handle_key(ctrl('='));
    app.handle_key(ctrl('='));
    // The zoom segment rides at the end of the status bar, past the point
    // where narrow frames truncate, so this draws wide.
    let backend = TestBackend::new(80, 10);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let rows = buffer_lines(&mut term);
    assert!(
        rows.iter().any(|r| r.contains("150%")),
        "status missing: {rows:?}"
    );
}

#[test]
fn mouse_click_accounts_for_zoom_rows() {
    let mut app = app_with("a\nb");
    app.handle_key(ctrl('='));
    drawn(&mut app);
    // Line 1 ("b") sits at display row 2 once zoom inserts a blank row,
    // which is frame row 4 below the menu and the hairline; column 1 is
    // the first content column past the page border.
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 4,
        modifiers: KeyModifiers::NONE,
    });
    let at = app.doc.prose_surface().expect("surface").cursor_char();
    assert_eq!(at, 2);
}
