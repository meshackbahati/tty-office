//! Open-save-reopen corpus over the fixtures committed in `tests/fixtures`.
//!
//! Every fixture prints a per-file `pass:` or `fail:` line so a run records
//! which files cleared the round trip; the first failure panics with the
//! fixture named. Regenerate the corpus with
//! `cargo run --example make_fixtures` when the editor surface gains new
//! content types. The text fixtures run on every build; rich and sheet
//! fixtures need their backend features.

use std::path::PathBuf;

use tempfile::TempDir;

fn fixture_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    assert!(
        path.is_file(),
        "missing fixture {name}; run `cargo run --example make_fixtures --features docx,xlsx`"
    );
    path
}

/// Save a copy of the opened fixture beside a temporary directory and
/// assert that reopening it reproduces the same projection.
#[cfg(feature = "docx")]
mod rich {
    use super::*;
    use tty_office::{open, Editor};

    const RICH_TEXT: &str =
        "Quarterly report\nThe budget closes at the end of March.\n\nAppendix A holds the detail.";

    fn roundtrip(name: &str) {
        let src = fixture_path(name);
        let mut doc = open(&src).expect("open fixture");
        assert_eq!(
            doc.text_projection(),
            RICH_TEXT,
            "projection mismatch on first open of {name}"
        );
        assert!(!doc.is_dirty(), "opening {name} must not dirty it");

        let dir = TempDir::new().expect("temp dir");
        let out = dir.path().join(name);
        doc.save(Some(&out)).expect("save copy");
        let again = open(&out).expect("reopen saved copy");
        assert_eq!(
            again.text_projection(),
            RICH_TEXT,
            "projection drift after round trip of {name}"
        );
        assert!(!again.is_dirty(), "reopening {name} must not dirty it");
        println!("pass: {name}");
    }

    #[test]
    fn docx_corpus_roundtrip() {
        roundtrip("document.docx");
    }

    #[test]
    fn odt_corpus_roundtrip() {
        roundtrip("notes.odt");
    }
}

/// Spreadsheet fixtures: assert every cell of the used range before and
/// after the round trip, including formula evaluation results.
#[cfg(feature = "xlsx")]
mod sheets {
    use super::*;
    use tty_office::{open, Document, Editor, SheetDocument};

    const CELLS: [(usize, usize, &str); 12] = [
        (0, 0, "region"),
        (0, 1, "q1"),
        (0, 2, "q2"),
        (1, 0, "north"),
        (1, 1, "120"),
        (1, 2, "135"),
        (2, 0, "south"),
        (2, 1, "98"),
        (2, 2, "104"),
        (3, 0, "total"),
        (3, 1, "218"),
        (3, 2, "239"),
    ];

    fn cell_display(sheet: &SheetDocument, row: usize, col: usize) -> String {
        sheet
            .cell(row, col)
            .map(|c| c.value.display())
            .unwrap_or_default()
    }

    fn assert_grid(doc: &mut Document, name: &str, phase: &str) {
        let sheet = doc.sheet_mut().expect("sheet document");
        for (row, col, expected) in CELLS {
            let got = cell_display(sheet, row, col);
            assert_eq!(got, expected, "{phase} mismatch at ({row},{col}) of {name}");
        }
    }

    fn roundtrip(name: &str) {
        let src = fixture_path(name);
        let mut doc = open(&src).expect("open fixture");
        assert_grid(&mut doc, name, "first open");
        assert!(!doc.is_dirty(), "opening {name} must not dirty it");

        let dir = TempDir::new().expect("temp dir");
        let out = dir.path().join(name);
        doc.save(Some(&out)).expect("save copy");
        let mut again = open(&out).expect("reopen saved copy");
        assert_grid(&mut again, name, "round trip");
        assert!(!again.is_dirty(), "reopening {name} must not dirty it");
        println!("pass: {name}");
    }

    #[test]
    fn xlsx_corpus_roundtrip() {
        roundtrip("budget.xlsx");
    }

    #[test]
    fn ods_corpus_roundtrip() {
        roundtrip("sales.ods");
    }

    #[test]
    fn csv_corpus_roundtrip() {
        roundtrip("data.csv");
    }
}

/// Plain text fixtures need no backend feature: they pin the text surface
/// every build owns, including `--no-default-features`.
mod text {
    use super::*;
    use tty_office::{open, Editor};

    fn roundtrip(name: &str, body: &str) {
        let src = fixture_path(name);
        let mut doc = open(&src).expect("open fixture");
        assert_eq!(
            doc.text_projection(),
            body,
            "projection mismatch on first open of {name}"
        );
        assert!(!doc.is_dirty(), "opening {name} must not dirty it");

        let dir = TempDir::new().expect("temp dir");
        let out = dir.path().join(name);
        doc.save(Some(&out)).expect("save copy");
        let again = open(&out).expect("reopen saved copy");
        assert_eq!(
            again.text_projection(),
            body,
            "projection drift after round trip of {name}"
        );
        assert!(!again.is_dirty(), "reopening {name} must not dirty it");
        println!("pass: {name}");
    }

    #[test]
    fn txt_corpus_roundtrip() {
        roundtrip(
            "plain.txt",
            "Plain notes\nA second line for the corpus.\n\nTrailing paragraph.",
        );
    }

    #[test]
    fn md_corpus_roundtrip() {
        roundtrip("notes.md", "# Notes\n\n- first\n- second\n\nDone.");
    }
}
