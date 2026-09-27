//! Keyboard focus: F6 toggles the sidebar, arrows move its highlight,
//! Enter activates, and anything else falls back to the text.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, Mode, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn plain_app() -> App {
    App::new(Document::Text(TextDocument::new()))
}

fn drawn(app: &mut App, width: u16) {
    let backend = TestBackend::new(width, 12);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
}

fn f6() -> KeyEvent {
    key(KeyCode::F(6), KeyModifiers::NONE)
}

#[test]
fn f6_focuses_sidebar_and_enter_activates() {
    let mut app = plain_app();
    drawn(&mut app, 80);
    app.handle_key(f6());
    // Focus lands on the first actionable row (New text file).
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Text(_)));
}

#[test]
#[cfg(feature = "docx")]
fn sidebar_arrows_skip_headers() {
    let mut app = plain_app();
    drawn(&mut app, 80);
    app.handle_key(f6());
    // Down from New text file lands on New document, skipping nothing
    // actionable; Enter opens a word document tab.
    app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Rich(_)));
}

#[test]
fn esc_returns_focus_to_text() {
    let mut app = plain_app();
    drawn(&mut app, 80);
    app.handle_key(f6());
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.doc.text_projection(), "x");
}

#[test]
fn typing_falls_through_to_text() {
    let mut app = plain_app();
    drawn(&mut app, 80);
    app.handle_key(f6());
    for c in "hi".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.doc.text_projection(), "hi");
}

#[test]
fn hidden_sidebar_keeps_text_focus() {
    let mut app = plain_app();
    drawn(&mut app, 50);
    app.handle_key(f6());
    // No sidebar to focus: Enter still inserts a newline in the text.
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.doc.text_projection(), "\n");
    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.mode, Mode::Normal);
}
