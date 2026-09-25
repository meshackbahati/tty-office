//! Key resolution guarantees through the public API.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tty_office::{Action, Keymap};

#[test]
fn shift_variants_resolve_to_the_closest_modifier_match() {
    // The bindings live in a HashMap, so each fresh Keymap may visit its
    // chords in a different order; resolving across many instances proves
    // the closest modifier match wins regardless of order.
    for _ in 0..50 {
        let map = Keymap::new();
        let save = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(map.resolve(&save), Action::Save);
        let shifted = KeyEvent::new(
            KeyCode::Char('S'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(map.resolve(&shifted), Action::SaveAs);
    }
}
