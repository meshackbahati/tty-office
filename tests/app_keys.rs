//! Headless key handling: every Nano and word-processor chord from
//! `plan.txt` exercised through `App::handle_key` without a terminal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tty_office::{App, Document, Editor, Mode, PromptKind, TextDocument};

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

#[test]
fn typing_inserts_characters() {
    let mut app = plain_app();
    for c in ['h', 'i'] {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.doc.text_projection(), "hi");
}

#[test]
fn ctrl_z_undoes_and_ctrl_y_redoes() {
    let mut app = plain_app();
    app.handle_key(key(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key(ctrl('z'));
    assert_eq!(app.doc.text_projection(), "");
    app.handle_key(ctrl('y'));
    assert_eq!(app.doc.text_projection(), "a");
}

#[test]
fn ctrl_a_selects_all_and_backspace_clears() {
    let mut app = plain_app();
    app.doc.insert_str("wipe me");
    app.handle_key(ctrl('a'));
    assert!(app.doc.selection().is_some());
    app.handle_key(key(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.doc.text_projection(), "");
}

#[test]
fn ctrl_x_on_clean_document_quits() {
    let mut app = plain_app();
    app.handle_key(ctrl('x'));
    assert!(app.should_quit);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn ctrl_x_dirty_opens_confirm_then_esc_cancels() {
    let mut app = plain_app();
    app.doc.insert_str("dirty");
    app.handle_key(ctrl('x'));
    assert_eq!(app.mode, Mode::ConfirmQuit);
    assert!(!app.should_quit);
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
    assert!(!app.should_quit);
}

#[test]
fn ctrl_x_dirty_discard_with_ctrl_x() {
    let mut app = plain_app();
    app.doc.insert_str("dirty");
    app.handle_key(ctrl('x'));
    assert_eq!(app.mode, Mode::ConfirmQuit);
    app.handle_key(ctrl('x'));
    assert!(app.should_quit);
}

#[test]
fn ctrl_w_opens_find_prompt_and_finds_match() {
    let mut app = plain_app();
    app.doc.insert_str("alpha beta alpha");
    app.handle_key(ctrl('w'));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::Find));
    for c in "alpha".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.doc.cursor(), tty_office::Cursor::Char(5));
    assert_eq!(app.message, "Found alpha");
}

#[test]
fn ctrl_f_is_find_as_well() {
    let mut app = plain_app();
    app.handle_key(ctrl('f'));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::Find));
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn ctrl_backslash_replace_all() {
    let mut app = plain_app();
    app.doc.insert_str("foo bar foo");
    app.handle_key(ctrl('\\'));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::ReplaceFind));
    for c in "foo".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::ReplaceText));
    for c in "qux".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    // The preview stages the matches and asks for confirmation before any
    // character moves.
    assert_eq!(app.mode, Mode::Prompt(PromptKind::ReplaceConfirm));
    assert_eq!(app.message, "Enter to apply, Esc to cancel");
    assert!(app.prompt_buf.starts_with("2 occurrence(s)"));
    assert_eq!(app.doc.text_projection(), "foo bar foo");
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.doc.text_projection(), "qux bar qux");
    assert_eq!(app.message, "Replaced 2 occurrence(s)");
}

#[test]
fn replace_preview_esc_cancels_without_changes() {
    let mut app = plain_app();
    app.doc.insert_str("foo bar foo");
    app.handle_key(ctrl('\\'));
    for c in "foo".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    for c in "qux".chars() {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::ReplaceConfirm));
    // Typing during the preview is ignored so the summary stays truthful.
    app.handle_key(key(KeyCode::Char('z'), KeyModifiers::NONE));
    assert!(app.prompt_buf.starts_with("2 occurrence(s)"));
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.doc.text_projection(), "foo bar foo");
    assert_eq!(app.message, "Replace cancelled");
}

#[test]
fn ctrl_k_cuts_line_and_ctrl_u_uncuts() {
    let mut app = plain_app();
    app.doc.insert_str("keep\ncut\nafter");
    app.doc.move_cursor(tty_office::Motion::BufferStart, false);
    app.doc.move_cursor(tty_office::Motion::Down, false);
    app.handle_key(ctrl('k'));
    assert_eq!(app.cutbuffer, "cut\n");
    assert_eq!(app.doc.text_projection(), "keep\nafter");
    app.handle_key(ctrl('u'));
    assert_eq!(app.doc.text_projection(), "keep\ncut\nafter");
}

#[test]
fn ctrl_c_reports_position() {
    let mut app = plain_app();
    app.doc.insert_str("line one");
    app.handle_key(ctrl('c'));
    assert!(app.message.contains("Ln 1"));
    assert!(app.message.contains("Words 2"));
    assert!(app.message.contains("modified"));
}

#[test]
fn ctrl_g_opens_help_and_esc_closes() {
    let mut app = plain_app();
    app.handle_key(ctrl('g'));
    assert_eq!(app.mode, Mode::Help);
    app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn alt_b_wraps_selection_in_bold_markers() {
    let mut app = plain_app();
    app.doc.insert_str("word");
    app.handle_key(ctrl('a'));
    app.handle_key(alt('b'));
    assert_eq!(app.doc.text_projection(), "**word**");
}

#[test]
fn alt_i_wraps_selection_in_italic_markers() {
    let mut app = plain_app();
    app.doc.insert_str("word");
    app.handle_key(ctrl('a'));
    app.handle_key(alt('i'));
    assert_eq!(app.doc.text_projection(), "*word*");
}

#[test]
fn shift_right_extends_selection() {
    let mut app = plain_app();
    app.doc.insert_str("abcd");
    app.doc.move_cursor(tty_office::Motion::BufferStart, false);
    app.handle_key(key(
        KeyCode::Right,
        KeyModifiers::SHIFT | KeyModifiers::NONE,
    ));
    app.handle_key(key(
        KeyCode::Right,
        KeyModifiers::SHIFT | KeyModifiers::NONE,
    ));
    let sel = app.doc.selection().expect("selection");
    assert_eq!(
        sel,
        (tty_office::Cursor::Char(0), tty_office::Cursor::Char(2))
    );
}

#[test]
fn ctrl_s_saves_and_reports_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("save-via-key.txt");
    let mut app = App::new(tty_office::open(&path).expect("open"));
    app.doc.insert_str("saved");
    app.handle_key(ctrl('s'));
    assert!(!app.doc.is_dirty());
    assert!(app.message.starts_with("Wrote "));
    let on_disk = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(on_disk, "saved");
}

#[test]
fn ctrl_p_opens_export_prompt() {
    let mut app = plain_app();
    app.handle_key(ctrl('p'));
    assert_eq!(app.mode, Mode::Prompt(PromptKind::Export));
    assert_eq!(app.prompt_label, "Export to: ");
    assert_eq!(app.prompt_buf, "export.pdf");
}

#[test]
fn help_text_embeds_keymap_document() {
    let body = tty_office::help_text();
    assert!(body.contains("Ctrl+X"));
    assert!(body.contains("Where Is"));
}
