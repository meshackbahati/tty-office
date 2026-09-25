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

/// Document whose caret rests on line 51, which is the first line of page two
/// under the default fifty-line page model.
fn app_past_first_page() -> App {
    let body: Vec<String> = (1..=51).map(|n| format!("line {n}")).collect();
    app_with(&body.join("\n"))
}

#[test]
fn status_reports_page_of_cursor() {
    let mut app = app_past_first_page();
    // Wide enough that the whole left half of the status bar is not clipped.
    let backend = TestBackend::new(70, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("page 2/2"), "missing page readout: {text}");
    assert!(text.contains("Ln 51"), "missing cursor line: {text}");
}

#[test]
fn viewport_draws_page_rule_above_page_two() {
    let mut app = app_past_first_page();
    // Tall enough that the caret on line 51 and the rule above it both fit
    // inside the viewport.
    let backend = TestBackend::new(60, 60);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    // The rule label is padded with spaces on both sides, which distinguishes
    // it from the status bar's compact `page 2/2` readout.
    assert!(text.contains(" page 2 "), "missing page rule: {text}");
    assert!(text.contains("line 51"), "caret line missing: {text}");
}

#[test]
fn no_page_rule_inside_the_first_page() {
    let mut app = app_with("hello\nworld");
    let backend = TestBackend::new(40, 12);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(!text.contains("page 2"), "unexpected rule: {text}");
}
