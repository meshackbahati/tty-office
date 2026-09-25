//! Integration tests for TextDocument editing, history, and persistence.

use std::path::Path;

use tempfile::NamedTempFile;
use tty_office::{open, Editor, Motion, TextDocument};

#[test]
fn insert_and_projection() {
    let mut doc = TextDocument::new();
    doc.insert_str("hello world");
    assert_eq!(doc.text_projection(), "hello world");
    assert!(doc.is_dirty());
}

#[test]
fn backspace_removes_previous_char() {
    let mut doc = TextDocument::new();
    doc.insert_str("ab");
    doc.delete_back();
    assert_eq!(doc.text_projection(), "a");
}

#[test]
fn delete_forward_removes_next_char() {
    let mut doc = TextDocument::new();
    doc.insert_str("ab");
    doc.move_cursor(Motion::BufferStart, false);
    doc.delete_forward();
    assert_eq!(doc.text_projection(), "b");
}

#[test]
fn select_all_then_backspace_clears_document() {
    let mut doc = TextDocument::new();
    doc.insert_str("to be wiped");
    doc.select_all();
    assert!(doc.selection().is_some());
    doc.delete_back();
    assert_eq!(doc.text_projection(), "");
    assert!(doc.selection().is_none());
}

#[test]
fn shift_motion_extends_selection() {
    let mut doc = TextDocument::new();
    doc.insert_str("abcd");
    doc.move_cursor(Motion::BufferStart, false);
    doc.move_cursor(Motion::Right, true);
    doc.move_cursor(Motion::Right, true);
    let sel = doc.selection().expect("selection active");
    assert_eq!(
        sel,
        (tty_office::Cursor::Char(0), tty_office::Cursor::Char(2))
    );
}

#[test]
fn undo_restores_deleted_text_and_redo_reapplies() {
    let mut doc = TextDocument::new();
    doc.insert_str("xyz");
    doc.delete_back();
    assert_eq!(doc.text_projection(), "xy");
    assert!(doc.undo());
    assert_eq!(doc.text_projection(), "xyz");
    assert!(doc.redo());
    assert_eq!(doc.text_projection(), "xy");
}

#[test]
fn typing_coalesces_into_one_undo_unit() {
    let mut doc = TextDocument::new();
    for c in "cat".chars() {
        doc.insert_char(c);
    }
    // One coalesced unit: a single undo removes the whole run.
    assert!(doc.undo());
    assert_eq!(doc.text_projection(), "");
    assert!(!doc.undo());
}

#[test]
fn motion_line_end_and_line_start() {
    let mut doc = TextDocument::new();
    doc.insert_str("one\ntwo");
    doc.move_cursor(Motion::BufferStart, false);
    doc.move_cursor(Motion::LineEnd, false);
    assert_eq!(doc.cursor(), tty_office::Cursor::Char(3));
    doc.move_cursor(Motion::Down, false);
    doc.move_cursor(Motion::LineStart, false);
    assert_eq!(doc.cursor(), tty_office::Cursor::Char(4));
}

#[test]
fn vertical_motion_preserves_goal_column() {
    let mut doc = TextDocument::new();
    doc.insert_str("abcdef\nxy\nabcdef");
    doc.move_cursor(Motion::BufferStart, false);
    for _ in 0..3 {
        doc.move_cursor(Motion::Right, false);
    }
    // Goal column is 3. Line "xy" is only two characters, so the cursor
    // clamps to the end of that short line: start of line 1 is char 7,
    // plus offset 2 equals char 9.
    doc.move_cursor(Motion::Down, false);
    assert_eq!(doc.cursor(), tty_office::Cursor::Char(9));
    // One more down lands on the third line and restores the goal column:
    // start of line 2 is char 10, plus offset 3 equals char 13.
    doc.move_cursor(Motion::Down, false);
    assert_eq!(doc.cursor(), tty_office::Cursor::Char(13));
}

#[test]
fn save_and_reopen_roundtrip() {
    let tmp = NamedTempFile::new().expect("temp file");
    let path = tmp.path().to_path_buf();
    // NamedTempFile creates the file; write via editor save path.
    let mut doc = match open(&path) {
        Ok(tty_office::Document::Text(t)) => t,
        _ => panic!("expected text document"),
    };
    doc.insert_str("round trip\nsecond line");
    doc.save(None).expect("save");
    assert!(!doc.is_dirty());

    let again = TextDocument::open(&path).expect("reopen");
    assert_eq!(again.text_projection(), "round trip\nsecond line");
}

#[test]
fn open_missing_txt_adopts_path_and_starts_clean() {
    let path = std::env::temp_dir().join("tty-office-open-missing-test.txt");
    let _ = std::fs::remove_file(&path);
    let doc = open(&path).expect("open new");
    assert!(!doc.is_dirty());
    assert_eq!(doc.path().map(Path::to_path_buf), Some(path.clone()));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unsupported_extension_is_rejected() {
    let path = Path::new("/tmp/tty-office-test.bin");
    let err = open(path).expect_err("binary rejected");
    assert!(matches!(
        err,
        tty_office::DocumentError::UnsupportedFormat(_)
    ));
}

#[test]
fn cut_line_moves_text_to_cutbuffer_via_replace() {
    let mut doc = TextDocument::new();
    doc.insert_str("keep\ncut me\nafter");
    // insert_str leaves the cursor at the end; move to line 1 first.
    doc.move_cursor(Motion::BufferStart, false);
    doc.move_cursor(Motion::Down, false);
    doc.move_cursor(Motion::LineStart, false);
    let mut cut = String::new();
    doc.cut_current_line(&mut cut);
    assert_eq!(cut, "cut me\n");
    assert_eq!(doc.text_projection(), "keep\nafter");
}

#[test]
fn find_returns_first_match() {
    let mut doc = TextDocument::new();
    doc.insert_str("alpha beta alpha");
    let hit = doc.find("alpha", 0).expect("match");
    assert_eq!(hit, (0, 5));
    let hit2 = doc.find("alpha", 1).expect("second match");
    assert_eq!(hit2, (11, 16));
}

#[test]
fn find_handles_multibyte_needles_past_ascii_offset() {
    let mut doc = TextDocument::new();
    // Character index of the second "é" is not the same as its byte index.
    doc.insert_str("éé target");
    let hit = doc.find("target", 0).expect("match");
    // "éé " is three characters before "target".
    assert_eq!(hit.0, 3);
    let hit2 = doc.find("é", 1).expect("second e-acute");
    assert_eq!(hit2, (1, 2));
    // Searching past the last match wraps once and returns the first.
    let hit3 = doc.find("é", 4).expect("wrap to first");
    assert_eq!(hit3, (0, 1));
}

#[test]
fn find_no_wrap_does_not_loop_to_start() {
    let mut doc = TextDocument::new();
    doc.insert_str("abc abc");
    assert!(doc.find_no_wrap("abc", 0).is_some());
    assert!(doc.find_no_wrap("abc", 4).is_some());
    assert!(doc.find_no_wrap("abc", 7).is_none());
}

/// Regression: vertical motion over a zero-length buffer indexed
/// character zero of the single empty line and panicked inside ropey.
#[test]
fn vertical_motion_on_empty_document_does_not_panic() {
    let mut doc = TextDocument::new();
    doc.move_cursor(Motion::Down, false);
    doc.move_cursor(Motion::Up, false);
    doc.move_cursor(Motion::Down, true);
    assert_eq!(doc.text_projection(), "");
    assert_eq!(doc.cursor(), tty_office::Cursor::Char(0));
}
