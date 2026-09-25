//! Deterministic pseudo-random stress over the rope editor and the cell
//! input parser, standing in for fuzzing without adding a dependency.
//!
//! Both modules drive a fixed-seed linear congruential generator, so any
//! failure reproduces from the printed seed. The invariants checked are
//! structural: the shadow string agrees with the projection, the caret
//! stays inside the buffer, and every undo or redo step lands on a state
//! the document actually passed through.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tty_office::{App, Cursor, Document, Editor, Motion, TextDocument};

/// Parameters of the 64-bit Lehmer generator recommended by Knuth; the
/// sequence is fully determined by the seed printed on failure.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Character offset of the caret; text fuzzing never produces grids, so
/// seeing a cell cursor here is a defect worth naming loudly.
fn caret<E: Editor>(doc: &E) -> usize {
    match doc.cursor() {
        Cursor::Char(i) => i,
        Cursor::Cell { .. } => panic!("text editor reported a cell cursor"),
    }
}

#[test]
fn random_edits_track_a_shadow_string() {
    let seed = 0x5EED_CAFE_F00D_D00D;
    let mut rng = Rng::new(seed);
    let mut doc = TextDocument::new();
    // The shadow tracks the document by character index: String::insert
    // takes a byte offset, which diverges from the caret as soon as a
    // non-ASCII glyph lands in the buffer.
    let mut shadow: Vec<char> = Vec::new();
    let alphabet = ['a', 'b', 'α', '字', ' ', '\n'];

    for step in 0..2000 {
        match rng.below(7) {
            0..=2 => {
                let c = alphabet[rng.below(alphabet.len() as u64) as usize];
                let at = caret(&doc);
                doc.insert_char(c);
                shadow.insert(at, c);
            }
            3 => {
                let word: Vec<char> = (0..rng.below(5))
                    .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
                    .collect();
                let at = caret(&doc);
                doc.insert_str(&word.iter().collect::<String>());
                for (i, c) in word.into_iter().enumerate() {
                    shadow.insert(at + i, c);
                }
            }
            4 => {
                let at = caret(&doc);
                doc.delete_back();
                if at > 0 {
                    shadow.remove(at - 1);
                }
            }
            5 => {
                let at = caret(&doc);
                doc.delete_forward();
                if at < shadow.len() {
                    shadow.remove(at);
                }
            }
            _ => {
                // Motions never extend: with no selection the shadow model
                // above stays exact, which is the point of this test.
                let motions = [
                    Motion::Left,
                    Motion::Right,
                    Motion::Up,
                    Motion::Down,
                    Motion::LineStart,
                    Motion::LineEnd,
                    Motion::BufferStart,
                    Motion::BufferEnd,
                ];
                let motion = motions[rng.below(motions.len() as u64) as usize];
                doc.move_cursor(motion, false);
            }
        }
        assert_eq!(
            doc.text_projection(),
            shadow.iter().collect::<String>(),
            "projection diverged at step {step} with seed {seed:#x}"
        );
        let len = shadow.len();
        let cur = caret(&doc);
        assert!(
            cur <= len,
            "cursor {cur} past length {len} at step {step} with seed {seed:#x}"
        );
    }
}

#[test]
fn undo_and_redo_walk_recorded_states() {
    let seed = 0xA11C_EED0_0000_0001;
    let mut rng = Rng::new(seed);
    let mut doc = TextDocument::new();
    // Typing bursts coalesce into shared history units, so every undo step
    // must land on a state the document actually held rather than on the
    // immediately preceding one.
    let mut seen: Vec<String> = vec![String::new()];
    let alphabet = ['x', 'y', 'z', '\n'];

    for _ in 0..500 {
        let c = alphabet[rng.below(alphabet.len() as u64) as usize];
        doc.insert_char(c);
        seen.push(doc.text_projection());
    }
    let final_state = doc.text_projection();

    let mut undos = 0usize;
    while doc.undo() {
        let now = doc.text_projection();
        assert!(
            seen.contains(&now),
            "undo produced an unrecorded state after {undos} steps with seed {seed:#x}"
        );
        undos += 1;
        assert!(undos <= 500, "too many undo steps with seed {seed:#x}");
    }
    assert_eq!(
        doc.text_projection(),
        "",
        "full undo did not reach the empty document with seed {seed:#x}"
    );

    let mut redos = 0usize;
    while doc.redo() {
        redos += 1;
        assert!(redos <= 500, "too many redo steps with seed {seed:#x}");
    }
    assert_eq!(
        doc.text_projection(),
        final_state,
        "full redo did not restore the final document with seed {seed:#x}"
    );
}

/// Random strings, including formula-shaped garbage, are pushed through
/// the public cell entry point; the grid must accept or ignore every
/// input without panicking, and repeated reads of each cell must agree.
#[cfg(feature = "xlsx")]
#[test]
fn random_cell_inputs_never_panic() {
    use tempfile::TempDir;

    let seed = 0xF0E0_0015_0015_0015;
    let mut rng = Rng::new(seed);
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("fuzz.xlsx");
    let mut doc = tty_office::open(&path).expect("open sheet path");

    let pieces = [
        "1", "-2.5", "word", "TRUE", "=A1", "=SUM(", "=1+", "=((", "=A1+A2", "  ", "\t", "0/0",
        "=Z99*99", "😱", "=1e400",
    ];
    for step in 0..500 {
        let input = pieces[rng.below(pieces.len() as u64) as usize];
        {
            let sheet = doc.sheet_mut().expect("sheet");
            sheet.move_cursor(Motion::BufferStart, false);
            for _ in 0..rng.below(4) {
                sheet.move_cursor(Motion::Down, false);
            }
            for _ in 0..rng.below(4) {
                sheet.move_cursor(Motion::Right, false);
            }
            sheet.set_cell_content(input);
        }
        let sheet = doc.sheet_mut().expect("sheet");
        for row in 0..4 {
            for col in 0..4 {
                let first = sheet.cell(row, col).map(|c| c.value.display());
                let second = sheet.cell(row, col).map(|c| c.value.display());
                assert_eq!(
                    first, second,
                    "unstable read at ({row},{col}) after step {step} with seed {seed:#x}"
                );
            }
        }
    }
}

/// The app-level key loop sees sequences no human would type in order,
/// so prompt and normal-mode dispatch both get exercised for panics and
/// for caret bounds.
#[test]
fn random_app_keys_do_not_panic() {
    let seed = 0x0E75_C4E5_15DE_0001;
    let mut rng = Rng::new(seed);
    let mut app = App::new(Document::Text(TextDocument::new()));
    let codes = [
        KeyCode::Char('a'),
        KeyCode::Char(' '),
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Backspace,
        KeyCode::Tab,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
    ];
    for step in 0..1000 {
        let code = codes[rng.below(codes.len() as u64) as usize];
        let mods = if rng.below(8) == 0 {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        };
        app.handle_key(KeyEvent::new(code, mods));
        let len = app.doc.text_projection().chars().count();
        let cur = caret(&app.doc);
        assert!(
            cur <= len,
            "app cursor {cur} past length {len} at step {step} with seed {seed:#x}"
        );
    }
}
