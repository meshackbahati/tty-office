//! Tab strip: several documents share one session, switched by shortcut.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, Mode, PromptKind, TextDocument};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::ALT)
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

#[test]
fn ctrl_n_opens_a_second_tab_and_activates_it() {
    let mut app = plain_app();
    app.handle_key(ctrl('n'));
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.active_tab(), 1);
    assert!(app.message.starts_with("New "), "message: {}", app.message);
}

#[test]
fn tabs_keep_separate_buffers() {
    let mut app = plain_app();
    app.doc.insert_str("first");
    app.handle_key(ctrl('n'));
    app.doc.insert_str("second");
    app.handle_key(key(KeyCode::Left, KeyModifiers::ALT));
    assert_eq!(app.active_tab(), 0);
    assert_eq!(app.doc.text_projection(), "first");
    app.handle_key(key(KeyCode::Right, KeyModifiers::ALT));
    assert_eq!(app.active_tab(), 1);
    assert_eq!(app.doc.text_projection(), "second");
}

#[test]
fn alt_digit_jumps_to_numbered_tab() {
    let mut app = plain_app();
    app.handle_key(ctrl('n'));
    app.handle_key(ctrl('n'));
    assert_eq!(app.tab_count(), 3);
    app.handle_key(alt('1'));
    assert_eq!(app.active_tab(), 0);
    app.handle_key(alt('3'));
    assert_eq!(app.active_tab(), 2);
    // Out-of-range digits are ignored rather than wrapping or panicking.
    app.handle_key(alt('9'));
    assert_eq!(app.active_tab(), 2);
}

#[test]
fn next_tab_wraps_past_the_last_tab() {
    let mut app = plain_app();
    app.handle_key(ctrl('n'));
    assert_eq!(app.active_tab(), 1);
    app.handle_key(key(KeyCode::Right, KeyModifiers::ALT));
    assert_eq!(app.active_tab(), 0);
    app.handle_key(key(KeyCode::PageDown, KeyModifiers::CONTROL));
    assert_eq!(app.active_tab(), 1);
}

#[test]
fn close_tab_drops_the_active_clean_tab() {
    let mut app = plain_app();
    app.handle_key(ctrl('n'));
    app.handle_key(key(KeyCode::F(4), KeyModifiers::CONTROL));
    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.active_tab(), 0);
    assert!(!app.should_quit);
}

#[test]
fn close_tab_refuses_a_dirty_tab() {
    let mut app = plain_app();
    app.doc.insert_str("keep me");
    app.handle_key(key(KeyCode::F(4), KeyModifiers::CONTROL));
    assert_eq!(app.tab_count(), 1);
    assert!(!app.should_quit);
    assert!(app.message.contains("Unsaved"), "message: {}", app.message);
}

#[test]
fn closing_the_last_clean_tab_quits() {
    let mut app = plain_app();
    app.handle_key(key(KeyCode::F(4), KeyModifiers::CONTROL));
    assert!(app.should_quit);
}

#[test]
fn open_prompt_accept_opens_a_new_tab() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::OpenFile));
    for c in "tab-probe.txt".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.active_tab(), 1);
    assert!(
        app.message.starts_with("Opened "),
        "message: {}",
        app.message
    );
}

#[test]
fn exit_prompts_when_a_background_tab_is_dirty() {
    let mut app = plain_app();
    app.doc.insert_str("unsaved");
    app.handle_key(ctrl('n'));
    app.handle_key(ctrl('x'));
    assert_eq!(app.mode, Mode::ConfirmQuit);
    app.handle_key(ctrl('x'));
    assert!(app.should_quit);
}

#[test]
fn tab_bar_renders_all_titles_once_a_second_tab_exists() {
    let mut app = plain_app();
    app.handle_key(ctrl('o'));
    for c in "tab-probe.txt".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    let backend = TestBackend::new(60, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    let text = buffer_text(&mut term);
    assert!(text.contains("tab-probe"), "tab missing: {text}");
    assert!(text.contains("[no name]"), "first tab missing: {text}");
}
