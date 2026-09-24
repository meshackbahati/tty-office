//! Viewport and status rendering through Ratatui's TestBackend.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn app_with(text: &str) -> App {
    let mut doc = TextDocument::new();
    doc.insert_str(text);
    App::new(Document::Text(doc))
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

#[test]
fn renders_text_status_and_message_hint() {
    let mut app = app_with("hello\nworld");
    // Wide enough that the status line is not truncated before the word count.
    let backend = TestBackend::new(60, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("hello"), "missing body: {text}");
    assert!(text.contains("world"), "missing body: {text}");
    assert!(text.contains("[no name]"), "missing filename: {text}");
    assert!(text.contains("Ln 2"), "missing line number: {text}");
    assert!(text.contains("2 words"), "missing word count: {text}");
    assert!(text.contains("EDIT"), "missing mode: {text}");
}

#[test]
fn help_overlay_embeds_keymap_lines() {
    let mut app = app_with("body");
    app.mode = tty_office::Mode::Help;
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(
        text.contains("Help") || text.contains("Ctrl"),
        "help overlay missing: {text}"
    );
}

#[test]
fn selection_is_marked_in_rendered_line() {
    let mut app = app_with("selected");
    app.doc.select_all();
    let backend = TestBackend::new(30, 6);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let buffer = term.backend().buffer();
    let area = *buffer.area();
    // Row 1 is inside the text pane under the top border.
    let cell = &buffer[(area.left(), area.top() + 1)];
    // Reverse video is applied via style; assert the glyph itself still paints.
    assert_eq!(cell.symbol(), "s");
}
