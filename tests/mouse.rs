//! Mouse: click-to-caret, drag selection, wheel scroll, and chrome hits.
//!
//! Every test draws first so the view geometry the handler reads is the
//! one the renderer stored; coordinates below assume an 80-column frame
//! with the sidebar visible, the menu on row 0, and no tab strip.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Cursor, Document, Editor, TextDocument};

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn down(column: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), column, row)
}

fn drag_to(column: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Drag(MouseButton::Left), column, row)
}

fn app_with(text: &str) -> App {
    let mut doc = TextDocument::new();
    doc.insert_str(text);
    App::new(Document::Text(doc))
}

fn drawn(app: &mut App) {
    let backend = TestBackend::new(80, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
}

fn cursor_char(app: &mut App) -> usize {
    app.doc.prose_surface().expect("surface").cursor_char()
}

#[test]
fn click_places_the_caret() {
    let mut app = app_with("hello world");
    drawn(&mut app);
    // Sidebar takes columns 0..22 and the page border column 22, so text
    // column 6 is frame column 29; row 2 is the first document line below
    // the menu and the hairline.
    app.handle_mouse(down(29, 2));
    assert_eq!(cursor_char(&mut app), 6);
}

#[test]
fn drag_selects_a_range() {
    let mut app = app_with("hello world");
    drawn(&mut app);
    app.handle_mouse(down(29, 2));
    app.handle_mouse(drag_to(34, 2));
    app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 33, 2));
    assert_eq!(
        app.doc.selection(),
        Some((Cursor::Char(6), Cursor::Char(11)))
    );
}

#[test]
fn wheel_scrolls_without_moving_the_caret() {
    let text = (0..100)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = app_with(&text);
    drawn(&mut app);
    // Insertion leaves the caret at the end, so home it and redraw to
    // reset the viewport before measuring the wheel step.
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    drawn(&mut app);
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 30, 4));
    let rowoff = app.doc.prose_surface().expect("surface").rowoff();
    assert_eq!(rowoff, 3);
    assert_eq!(cursor_char(&mut app), 0);
}

#[test]
fn menu_click_opens_and_activates() {
    let mut app = app_with("body");
    drawn(&mut app);
    // Column 2 of row 0 sits on the File label; row 2 of the dropdown
    // holds its first item, New tab.
    app.handle_mouse(down(2, 0));
    app.handle_mouse(down(2, 2));
    assert_eq!(app.tab_count(), 2);
}

#[test]
fn tab_click_switches_tabs() {
    let mut app = app_with("first");
    drawn(&mut app);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    drawn(&mut app);
    assert_eq!(app.active_tab(), 1);
    // The tab strip takes row 1; column 5 sits inside the first cell.
    app.handle_mouse(down(5, 1));
    assert_eq!(app.active_tab(), 0);
    assert_eq!(app.doc.text_projection(), "first");
}

#[test]
fn sidebar_click_creates_a_text_file() {
    let mut app = app_with("body");
    drawn(&mut app);
    // Sidebar rows start at the body top (row 1); row 2 is New text file.
    app.handle_mouse(down(5, 2));
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.doc, Document::Text(_)));
}

#[test]
fn click_outside_an_open_menu_dismisses_and_lands() {
    let mut app = app_with("hello world");
    drawn(&mut app);
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT));
    // Column 40 sits past the File dropdown, so the click dismisses the
    // menu and lands in the text, clamped to the end of the line.
    app.handle_mouse(down(40, 2));
    assert_eq!(cursor_char(&mut app), 11);
}

#[test]
#[cfg(feature = "xlsx")]
fn sheet_click_and_drag_select_cells() {
    use tty_office::{SheetDocument, SheetFormat};
    let doc = Document::Sheet(Box::new(SheetDocument::new(SheetFormat::Xlsx)));
    let mut app = App::new(doc);
    drawn(&mut app);
    // Row 1 is the column-letter header with its rule on row 2, so row
    // 3 is grid row 0; column 29 is grid column 0 past the sidebar, the
    // gutter, and the first border line.
    app.handle_mouse(down(29, 3));
    app.handle_mouse(drag_to(42, 5));
    let rect = match &app.doc {
        Document::Sheet(sheet) => sheet.selection_rect(),
        _ => None,
    };
    assert_eq!(rect, Some(((0, 0), (1, 1))));
}
