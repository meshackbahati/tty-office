//! Headings: applying levels by shortcut, persisting through the
//! package, and rendering distinctly.

#![cfg(feature = "docx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;
use tempfile::TempDir;
use tty_office::{draw, App, Document, Editor};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn rich_app(text: &str) -> App {
    let dir = TempDir::new().expect("temp dir");
    let mut doc = tty_office::open(&dir.path().join("headings.docx")).expect("open rich");
    doc.insert_str(text);
    App::new(doc)
}

fn heading_of(app: &App, line: usize) -> Option<u8> {
    match &app.doc {
        Document::Rich(rich) => rich.heading_at(line),
        _ => None,
    }
}

#[test]
fn ctrl_digit_applies_heading_to_the_cursor_line() {
    let mut app = rich_app("Title\nbody text");
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Char('1'), KeyModifiers::CONTROL));
    assert_eq!(app.message, "Heading 1");
    assert_eq!(heading_of(&app, 0), Some(1));
    assert_eq!(heading_of(&app, 1), None);
}

#[test]
fn headings_persist_through_save_and_reopen() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("persist.docx");
    let mut doc = tty_office::open(&path).expect("open rich");
    doc.insert_str("Title\nbody");
    let mut app = App::new(doc);
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Char('2'), KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
    let again = tty_office::open(&path).expect("reopen");
    let Document::Rich(rich) = &again else {
        panic!("rich expected");
    };
    assert_eq!(rich.heading_at(0), Some(2));
    assert_eq!(rich.heading_at(1), None);
}

#[test]
fn headings_need_a_word_document() {
    let mut app = App::new(Document::Text(tty_office::TextDocument::new()));
    app.handle_key(key(KeyCode::Char('1'), KeyModifiers::CONTROL));
    assert_eq!(app.message, "Headings need a word document");
}

#[test]
fn heading_lines_render_bold() {
    let mut app = rich_app("Title\nbody text");
    app.handle_key(key(KeyCode::Home, KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Char('1'), KeyModifiers::CONTROL));
    let backend = TestBackend::new(30, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Menu row 0, hairline row 1, first text row 2, past the page border.
    let style = term.backend().buffer()[(1, 2)].style();
    assert!(
        style.add_modifier.contains(Modifier::BOLD),
        "heading line is not bold"
    );
    let plain = term.backend().buffer()[(1, 3)].style();
    assert!(
        !plain.add_modifier.contains(Modifier::BOLD),
        "body line should not be bold"
    );
}
