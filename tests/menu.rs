//! Menu bar: Alt mnemonics and F10 pull down dropdowns whose items run
//! the same actions as the keymap.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, Mode, PromptKind, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn alt(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::ALT)
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
    let backend = TestBackend::new(60, 12);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, app)).expect("draw");
    buffer_text(&mut term)
}

#[test]
fn menu_bar_lists_menus_and_alt_f_opens_file_dropdown() {
    let mut app = plain_app();
    let idle = drawn(&mut app);
    assert!(idle.contains("File"), "bar missing: {idle}");
    assert!(idle.contains("Help"), "bar missing: {idle}");
    app.handle_key(alt('f'));
    let open = drawn(&mut app);
    assert!(open.contains("New tab"), "dropdown missing: {open}");
    assert!(open.contains("Close tab"), "dropdown missing: {open}");
    assert!(open.contains("Quit"), "dropdown missing: {open}");
}

#[test]
fn f10_opens_the_file_menu() {
    let mut app = plain_app();
    app.handle_key(key(KeyCode::F(10), KeyModifiers::NONE));
    let open = drawn(&mut app);
    assert!(open.contains("Quit"), "dropdown missing: {open}");
}

#[test]
fn typing_a_mnemonic_switches_menus() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Char('e'), KeyModifiers::NONE));
    let open = drawn(&mut app);
    assert!(open.contains("Cut line"), "edit menu missing: {open}");
}

#[test]
fn menu_arrows_activate_close_tab() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    for _ in 0..4 {
        app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    // File item 4 is Close tab; the only tab is clean, so it quits.
    assert!(app.should_quit);
}

#[test]
fn menu_save_on_pathless_document_prompts_for_name() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    for _ in 0..5 {
        app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    // File item 5 is Save, which folds into Save As without a path.
    assert_eq!(app.mode, Mode::Prompt(PromptKind::SaveAs));
    assert_eq!(app.prompt_buf, "untitled.txt");
}

#[test]
fn menu_enter_on_new_tab_opens_a_tab() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
}

#[test]
fn menu_swallows_keys_and_esc_restores_typing() {
    let mut app = plain_app();
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Char('x'), KeyModifiers::NONE));
    // Plain x is no mnemonic, so the menu dismisses without inserting.
    assert_eq!(app.doc.text_projection(), "");
    app.handle_key(alt('f'));
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    app.handle_key(key(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.doc.text_projection(), "x");
}

#[test]
fn menu_save_as_prefills_the_current_path() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    for c in "READM".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    // File item 6 is Save As; the adopted path prefills the prompt so
    // renaming is one edit away.
    app.handle_key(alt('f'));
    for _ in 0..6 {
        app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::SaveAs));
    assert!(
        app.prompt_buf.ends_with("README.md"),
        "prefill missing: {}",
        app.prompt_buf
    );
}
