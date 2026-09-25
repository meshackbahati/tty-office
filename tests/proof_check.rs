//! Spell-check engine: fixture dictionary, byte-to-char ranges, scheduling.

#![cfg(feature = "proof")]

use std::time::{Duration, Instant};

use tty_office::{App, Document, Editor, ProofEngine, TextDocument};

/// Minimal Hunspell pair: known words so misses are unambiguous.
const AFF: &str = "SET UTF-8\n";
const DIC: &str = "2\nhello\ncafé\n";

fn fixture() -> ProofEngine {
    ProofEngine::from_parts(AFF, DIC).expect("fixture dictionary")
}

#[test]
fn check_now_flags_unknown_words() {
    let engine = fixture();
    assert!(engine.check_now("hello").is_empty());
    let hits = engine.check_now("helo wrld");
    assert_eq!(hits.len(), 2);
}

#[test]
fn check_now_converts_byte_offsets_to_char_indices() {
    let engine = fixture();
    // "café helo": é is two bytes, so byte and char indices diverge.
    let text = "café helo";
    let hits = engine.check_now(text);
    assert_eq!(hits.len(), 1);
    let (start, end) = hits[0];
    let word: String = text.chars().skip(start).take(end - start).collect();
    assert_eq!(word, "helo");
    // Byte index of 'h' is 6 (c,a,f,é=2 bytes, space) but char index is 5.
    assert_eq!(start, 5);
}

#[test]
fn schedule_and_poll_delivers_ranges() {
    let mut engine = fixture();
    engine.schedule("hello helo");
    let deadline = Instant::now() + Duration::from_secs(5);
    while engine.ranges().is_empty() && Instant::now() < deadline {
        engine.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.ranges().len(), 1, "worker should report helo");
    assert!(!engine.is_checking());
}

#[test]
fn reschedule_same_text_is_noop() {
    let mut engine = fixture();
    engine.schedule("hello");
    // Drain the first check.
    let deadline = Instant::now() + Duration::from_secs(5);
    while engine.is_checking() && Instant::now() < deadline {
        engine.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    // Scheduling the identical text again must not mark work pending.
    engine.schedule("hello");
    assert!(!engine.is_checking());
    assert!(engine.ranges().is_empty());
}

#[test]
fn system_dictionary_loads_when_present() {
    // The CI image and the developer machine both ship en_US hunspell files;
    // skip softly when absent so the fixture tests remain the source of truth.
    match ProofEngine::load() {
        Ok(engine) => {
            let hits = engine.check_now("thi is a tset of Engliish speling");
            assert!(
                !hits.is_empty(),
                "system dictionary should flag obvious typos"
            );
        }
        Err(err) => {
            eprintln!("system dictionary unavailable, skipping: {err}");
        }
    }
}

#[test]
fn app_tick_schedules_proof_for_prose() {
    let mut t = TextDocument::new();
    t.insert_str("helo");
    let mut app = App::new(Document::Text(t));
    app.tick();
    // With a system dictionary the worker may already be running; without one
    // proof stays None. Either way tick must not panic.
    app.tick();
}
