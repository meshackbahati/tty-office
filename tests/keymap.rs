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

#[test]
fn shifted_fill_chord_beats_read_file_deterministically() {
    use tty_office::Action::*;
    // Ctrl+Shift+R arrives as uppercase R on reporting terminals, while
    // legacy ones deliver plain Ctrl+R; every combination must resolve
    // the same way on every map instance.
    let cases = [
        (KeyCode::Char('r'), KeyModifiers::CONTROL, ReadFile),
        (KeyCode::Char('R'), KeyModifiers::CONTROL, FillRight),
        (
            KeyCode::Char('R'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            FillRight,
        ),
        (
            KeyCode::Char('r'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ReadFile,
        ),
    ];
    for _ in 0..50 {
        let map = Keymap::new();
        for (code, mods, expected) in &cases {
            assert_eq!(map.resolve(&KeyEvent::new(*code, *mods)), *expected);
        }
    }
}

#[test]
fn every_config_name_rebinds() {
    use std::collections::HashMap;
    use tty_office::Config;

    // Each name below must survive a config round trip: parsed from the
    // name, installed on an unusual chord, and resolved back.
    let names = [
        ("exit", tty_office::Action::Exit),
        ("new", tty_office::Action::New),
        ("new_text", tty_office::Action::NewText),
        ("new_sheet", tty_office::Action::NewSheet),
        ("open", tty_office::Action::Open),
        ("tab_next", tty_office::Action::NextTab),
        ("tab_prev", tty_office::Action::PrevTab),
        ("close_tab", tty_office::Action::CloseTab),
        ("toggle_sidebar", tty_office::Action::ToggleSidebar),
        ("zoom_in", tty_office::Action::ZoomIn),
        ("zoom_out", tty_office::Action::ZoomOut),
        ("zoom_reset", tty_office::Action::ZoomReset),
        ("cycle_theme", tty_office::Action::CycleTheme),
        ("fill_down", tty_office::Action::FillDown),
        ("fill_right", tty_office::Action::FillRight),
        ("save", tty_office::Action::Save),
        ("save_as", tty_office::Action::SaveAs),
        ("read_file", tty_office::Action::ReadFile),
        ("find", tty_office::Action::Find),
        ("replace", tty_office::Action::Replace),
        ("cut_line", tty_office::Action::CutLine),
        ("uncut", tty_office::Action::Uncut),
        ("show_position", tty_office::Action::ShowPosition),
        ("help", tty_office::Action::Help),
        ("undo", tty_office::Action::Undo),
        ("redo", tty_office::Action::Redo),
        ("select_all", tty_office::Action::SelectAll),
        ("toggle_bold", tty_office::Action::ToggleBold),
        ("toggle_italic", tty_office::Action::ToggleItalic),
        ("export", tty_office::Action::Export),
        ("confirm", tty_office::Action::Confirm),
        ("cancel", tty_office::Action::Cancel),
        ("prompt_backspace", tty_office::Action::PromptBackspace),
        ("noop", tty_office::Action::Noop),
        ("insert_newline", tty_office::Action::InsertNewline),
        ("backspace", tty_office::Action::Backspace),
        ("delete_forward", tty_office::Action::DeleteForward),
    ];
    for (name, expected) in names {
        let cfg = Config {
            keys: HashMap::from([(name.to_string(), "ctrl+f12".to_string())]),
            ..Default::default()
        };
        let map = Keymap::from_config(cfg);
        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::CONTROL);
        assert_eq!(map.resolve(&key), expected, "name {name} did not rebind");
    }
}

#[test]
fn motion_names_rebind() {
    use std::collections::HashMap;
    use tty_office::{Config, Motion};

    let names = [
        ("move_left", Motion::Left, false),
        ("move_end", Motion::LineEnd, false),
        ("move_page_down", Motion::PageDown, false),
        ("move_buffer_start", Motion::BufferStart, false),
        ("extend_right", Motion::Right, true),
        ("extend_buffer_end", Motion::BufferEnd, true),
    ];
    for (name, motion, extend) in names {
        let cfg = Config {
            keys: HashMap::from([(name.to_string(), "ctrl+f12".to_string())]),
            ..Default::default()
        };
        let map = Keymap::from_config(cfg);
        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::CONTROL);
        let expected = if extend {
            tty_office::Action::Extend(motion)
        } else {
            tty_office::Action::Move(motion)
        };
        assert_eq!(map.resolve(&key), expected, "name {name} did not rebind");
    }
}

#[test]
fn rebind_replaces_the_default_chord_and_tolerates_bad_entries() {
    use tty_office::Config;

    let cfg: Config = toml::from_str(
        "[keys]\nexit = \"ctrl+q\"\nbogus_action = \"ctrl+z\"\nexit_typo = \"ctrl+\"\n",
    )
    .expect("valid config");
    let map = Keymap::from_config(cfg);
    assert_eq!(
        map.resolve(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
        tty_office::Action::Exit
    );
    // The default chord is gone: no ghost shortcut survives the rebind.
    assert_ne!(
        map.resolve(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        tty_office::Action::Exit
    );
}

#[test]
fn help_rows_name_the_config_key() {
    let rows = Keymap::new().help_rows();
    assert!(
        rows.iter()
            .any(|(chord, name, _)| chord == "Ctrl+Shift+S" && *name == "save_as"),
        "save_as row missing"
    );
    assert!(
        rows.iter().any(|(_, name, _)| *name == "move_left"),
        "motion rows missing"
    );
}
