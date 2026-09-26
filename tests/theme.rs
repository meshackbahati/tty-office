//! Display themes: cycling presets and themed chrome.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use tty_office::{draw, App, Document, TextDocument, Theme};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn alt(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::ALT)
}

fn plain_app() -> App {
    App::new(Document::Text(TextDocument::new()))
}

#[test]
fn alt_t_cycles_presets_and_wraps() {
    let mut app = plain_app();
    assert_eq!(app.theme(), Theme::MONO);
    let mut names = vec!["Ocean", "Ember", "Forest", "Mono"];
    for expected in names.drain(..) {
        app.handle_key(alt('t'));
        assert_eq!(app.theme().name, expected);
        assert_eq!(app.message, format!("Theme: {expected}"));
    }
}

#[test]
fn themed_filename_uses_the_accent() {
    let mut app = plain_app();
    app.handle_key(alt('t'));
    let backend = TestBackend::new(80, 10);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Single tab on a wide frame: menu, seven body rows, status on row
    // 8, message on row 9; the filename starts at column 1.
    let cell = &term.backend().buffer()[(1, 8)];
    assert_eq!(cell.fg, Color::LightBlue);
}

#[test]
fn sidebar_shows_the_current_theme() {
    let mut app = plain_app();
    app.handle_key(alt('t'));
    let backend = TestBackend::new(80, 24);
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
    assert!(text.contains("Theme: Ocean"), "theme row missing: {text}");
}
