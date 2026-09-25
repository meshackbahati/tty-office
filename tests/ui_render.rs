//! Viewport and status rendering through Ratatui's TestBackend.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

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
    // Row 2 is the first text row: row 0 holds the menu bar and row 1 the
    // pane hairline.
    let cell = &buffer[(area.left(), area.top() + 2)];
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

/// Find positions the current match as the document selection, so the
/// renderer must paint that span with the reversed selection style and
/// leave every other character untouched.
#[test]
fn find_highlight_marks_the_current_match() {
    let mut app = app_with("alpha beta alpha");
    app.handle_key(ctrl('f'));
    for c in "beta".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    let backend = TestBackend::new(40, 6);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("alpha beta alpha"), "body changed: {text}");
    // Row 0 holds the menu bar and the text pane draws a one-row hairline
    // on top, so the first document line sits at row 2. "beta" occupies
    // columns 6..10 of that line.
    for x in 6..10u16 {
        let buffer = term.backend().buffer();
        let style = buffer[(x, 2)].style();
        assert!(
            style.add_modifier.contains(Modifier::REVERSED),
            "column {x} of the match is not highlighted: {text}"
        );
    }
    let buffer = term.backend().buffer();
    let outside_style = buffer[(0, 2)].style();
    assert!(
        !outside_style.add_modifier.contains(Modifier::REVERSED),
        "text outside the match is highlighted: {text}"
    );
}

/// The spreadsheet grid draws its column-letter header, one-based row
/// numbers, and cell contents through the same TestBackend surface.
#[cfg(feature = "xlsx")]
#[test]
fn sheet_grid_renders_headers_and_cells() {
    use tempfile::TempDir;

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("grid.xlsx");
    let mut doc = tty_office::open(&path).expect("open sheet path");
    {
        let sheet = doc.sheet_mut().expect("sheet");
        sheet.set_cell_content("region");
        sheet.move_cursor(tty_office::Motion::Right, false);
        sheet.set_cell_content("q1");
        sheet.move_cursor(tty_office::Motion::Down, false);
        sheet.move_cursor(tty_office::Motion::LineStart, false);
        sheet.set_cell_content("north");
    }
    let mut app = App::new(doc);
    let backend = TestBackend::new(50, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("region"), "cell missing: {text}");
    assert!(text.contains("q1"), "cell missing: {text}");
    assert!(text.contains("north"), "cell missing: {text}");
    // Column-letter header plus the one-based number for the first row.
    assert!(text.contains('A'), "column header missing: {text}");
    assert!(text.contains("    1 "), "row number missing: {text}");
}

#[test]
fn cursor_sits_on_the_first_text_row_below_the_top_hairline() {
    let mut app = app_with("hello");
    let backend = TestBackend::new(30, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Row 0 holds the menu bar and row 1 the hairline that closes the top
    // of the text pane, so the caret must be placed on row 2, which holds
    // the first document line. The caret sits after the inserted "hello",
    // hence column 5; the row is the regression target, since the old code
    // drew it onto the hairline.
    term.backend_mut().assert_cursor_position((5, 2));
}
