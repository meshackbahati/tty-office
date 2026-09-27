//! Sidebar: toggle, creation entries, and the tab list.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

fn plain_app() -> App {
    App::new(Document::Text(TextDocument::new()))
}

fn buffer_text(term: &mut Terminal<TestBackend>) -> String {
    let buffer = term.backend().buffer();
    let area = *buffer.area();
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn drawn(app: &mut App, width: u16) -> String {
    let backend = TestBackend::new(width, 12);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    buffer_text(&mut term)
}

#[test]
fn ctrl_b_toggles_the_sidebar() {
    let mut app = plain_app();
    assert!(drawn(&mut app, 80).contains("Files"));
    app.handle_key(ctrl('b'));
    assert!(!drawn(&mut app, 80).contains("Files"));
    app.handle_key(ctrl('b'));
    assert!(drawn(&mut app, 80).contains("Files"));
}

#[test]
fn narrow_terminals_hide_the_sidebar() {
    let mut app = plain_app();
    assert!(!drawn(&mut app, 50).contains("Files"));
}

#[test]
fn ctrl_shift_n_creates_a_text_tab() {
    let mut app = plain_app();
    app.handle_key(key(
        KeyCode::Char('n'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Text(_)));
    assert!(app.message.starts_with("New "), "message: {}", app.message);
}

#[test]
fn file_menu_creates_a_text_file() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Text(_)));
}

#[test]
#[cfg(feature = "xlsx")]
fn file_menu_creates_a_spreadsheet() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Sheet(_)));
}

#[test]
fn sidebar_lists_open_tabs() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    for c in "READM".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    // The full row list needs a tall frame; short terminals truncate the
    // sidebar at the body height instead of spilling onto the status bar.
    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("Tabs"), "section missing: {text}");
    assert!(text.contains("README.md"), "tab missing: {text}");
    assert!(text.contains("[no name]"), "first tab missing: {text}");
}

#[test]
fn sidebar_shifts_the_text_pane_right() {
    let mut doc = TextDocument::new();
    doc.insert_str("hello");
    let mut app = App::new(Document::Text(doc));
    let backend = TestBackend::new(80, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Menu row 0, hairline row 1, first text row 2; the 22-column
    // sidebar plus the page border pushes the caret, which follows
    // "hello", to column 28.
    // The mouse step relies on this geometry contract.
    term.backend_mut().assert_cursor_position((28, 2));
}
