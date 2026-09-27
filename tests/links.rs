//! Hyperlinks: detection in word packages, distinct rendering, and
//! activation paths. Opening itself spawns the system handler, so tests
//! cover detection, rendering, and every non-spawning branch.

#![cfg(feature = "docx")]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;
use tempfile::TempDir;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn linked_path(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("links.docx");
    let mut package = rdocx::Document::new();
    let rel = package.add_hyperlink_relationship("https://example.com");
    let mut paragraph = package.add_paragraph("Visit ");
    paragraph.add_hyperlink("example", &rel);
    package.save(&path).expect("save fixture");
    path
}

fn link_spans(app: &App, line: usize) -> Vec<(usize, usize, Option<String>)> {
    match &app.doc {
        Document::Rich(rich) => rich
            .link_spans(line)
            .into_iter()
            .map(|s| (s.start, s.end, s.target))
            .collect(),
        _ => panic!("rich expected"),
    }
}

#[test]
fn hyperlink_runs_detect_with_targets() {
    let dir = TempDir::new().expect("temp dir");
    let path = linked_path(&dir);
    let app = App::new(tty_office::open(&path).expect("open"));
    // "Visit " is six characters, so the link covers 6..13.
    assert_eq!(
        link_spans(&app, 0),
        vec![(6, 13, Some("https://example.com".to_string()))]
    );
    assert!(link_spans(&app, 1).is_empty());
}

#[test]
fn link_text_renders_underlined() {
    let dir = TempDir::new().expect("temp dir");
    let path = linked_path(&dir);
    let mut app = App::new(tty_office::open(&path).expect("open"));
    let backend = TestBackend::new(40, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Menu row 0, hairline row 1, first text row 2, past the page
    // border: column 7 sits inside the link text.
    let style = term.backend().buffer()[(7, 2)].style();
    assert!(
        style.add_modifier.contains(Modifier::UNDERLINED),
        "link text is not underlined"
    );
}

#[test]
fn open_link_reports_when_nothing_is_under_the_cursor() {
    let mut app = App::new(Document::Text(TextDocument::new()));
    app.handle_key(key(KeyCode::Enter, KeyModifiers::CONTROL));
    assert_eq!(app.message, "No link under cursor");
}

#[test]
fn ctrl_click_off_link_leaves_the_caret_alone() {
    let mut doc = TextDocument::new();
    doc.insert_str("plain text");
    let mut app = App::new(Document::Text(doc));
    let backend = TestBackend::new(40, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 2,
        modifiers: KeyModifiers::CONTROL,
    });
    // No link: no message, no caret motion, still editing.
    assert_eq!(app.message, "");
    assert_eq!(app.doc.prose_surface().expect("surface").cursor_char(), 10);
}
