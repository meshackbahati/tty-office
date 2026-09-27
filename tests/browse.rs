//! File browser: listing, filtering, descent, opening, and clicks.
//!
//! The working directory is the package root under test, which owns
//! `Cargo.toml` and a `src` directory; both are permanent fixtures for
//! these assertions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Mode, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
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

fn drawn(app: &mut App) -> String {
    // Tall enough that files below the directory run stay visible.
    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    buffer_text(&mut term)
}

fn filter_to(app: &mut App, needle: &str) {
    for c in needle.chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

#[test]
fn ctrl_o_lists_the_working_directory() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    assert_eq!(app.mode, Mode::Browse);
    let text = drawn(&mut app);
    assert!(text.contains("Cargo.toml"), "listing missing: {text}");
    assert!(text.contains("BROWSE"), "mode missing: {text}");
}

#[test]
fn filter_narrows_and_enter_opens_a_tab() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    // README.md opens as text; the filter matches nothing else.
    filter_to(&mut app, "READM");
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.message, "Opened README.md");
}

#[test]
fn esc_leaves_the_browser() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.tab_count(), 1);
}

#[test]
fn enter_descends_and_backspace_ascends() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    filter_to(&mut app, "src");
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Browse);
    let descended = drawn(&mut app);
    assert!(
        descended.contains("main.rs"),
        "src listing missing: {descended}"
    );
    // Empty filter, so Backspace ascends instead of editing the filter.
    app.handle_key(key(KeyCode::Backspace, KeyModifiers::NONE));
    let ascended = drawn(&mut app);
    assert!(ascended.contains("Cargo.toml"), "root missing: {ascended}");
}

#[test]
fn click_activates_the_selected_row() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    filter_to(&mut app, "READM");
    drawn(&mut app);
    // Single tab on a wide frame: the body starts at row 1, so the
    // first content row of the overlay is row 2.
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 2,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.tab_count(), 2);
}

#[test]
fn wheel_scrolls_the_browser_selection() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    // Narrow frame, long listing: the wheel must move the selection.
    let backend = TestBackend::new(80, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 5,
        row: 4,
        modifiers: KeyModifiers::NONE,
    });
    // One notch moves three rows from .git to docs; entering the
    // directory proves the selection followed the wheel rather than
    // the document scroll.
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Browse);
    let text = drawn(&mut app);
    assert!(text.contains("FORMATS.md"), "wheel did not move: {text}");
}
