//! Chord to action lookup with merged defaults.

use std::collections::HashMap;
use std::fs;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::action::{action_from_name, action_name, describe, Action};
use super::bindings::default_bindings;
use super::chord::Chord;
use super::config::Config;

/// Chord to action lookup with merged defaults.
#[derive(Debug, Clone)]
pub struct Keymap {
    map: HashMap<Chord, Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    /// Build the default map.
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for (chord, action) in default_bindings() {
            map.insert(chord, action);
        }
        Self { map }
    }

    /// Default map with `~/.config/tty-office/config.toml` overrides applied.
    pub fn load_user() -> Self {
        if let Some(path) = super::config::user_config_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&text) {
                    return Self::from_config(cfg);
                }
            }
        }
        Self::new()
    }

    /// Map with a parsed user config applied. Unknown action names and
    /// unparseable chords skip entry by entry, so one typo cannot sink
    /// the rest of the file.
    pub fn from_config(cfg: Config) -> Self {
        let mut km = Self::new();
        for (action_name, chord_spec) in cfg.keys {
            if let (Some(action), Some(chord)) =
                (action_from_name(&action_name), Chord::parse(&chord_spec))
            {
                // Remove any prior chord bound to this action so a
                // rebind does not leave a ghost shortcut.
                km.map.retain(|_, a| a != &action);
                km.map.insert(chord, action);
            }
        }
        km
    }

    /// Resolve a key event to an action, or [`Action::Noop`].
    pub fn resolve(&self, key: &KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
            || matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Backspace
                    | KeyCode::Delete
                    | KeyCode::Enter
                    | KeyCode::Esc
                    | KeyCode::Tab
            )
        {
            let mut best: Option<(u8, Action)> = None;
            for (chord, action) in &self.map {
                // Two chords can both match one event when they differ only
                // by SHIFT (`ctrl+s` versus `ctrl+shift+s`), while the map
                // iteration order is arbitrary; the closest modifier match
                // wins so resolution stays deterministic across instances.
                let score = chord.match_score(key);
                if score > 0 && best.as_ref().is_none_or(|(top, _)| score > *top) {
                    best = Some((score, action.clone()));
                }
            }
            if let Some((_, action)) = best {
                return action;
            }
        }
        // Plain printable character falls through to insert.
        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
            if let KeyCode::Char(c) = key.code {
                if c == '\t' {
                    return Action::Insert('\t');
                }
                if !c.is_control() {
                    return Action::Insert(c);
                }
            }
        }
        Action::Noop
    }

    /// Human-readable listing for the help screen.
    pub fn help_lines(&self) -> Vec<(String, String)> {
        self.help_rows()
            .into_iter()
            .map(|(chord, _, blurb)| (chord, blurb))
            .collect()
    }

    /// Help-screen rows as (chord, config name, description), sorted by
    /// chord, so the overlay shows what the user actually set.
    pub fn help_rows(&self) -> Vec<(String, &'static str, String)> {
        use super::chord::format_chord;
        let mut rows: Vec<(String, &'static str, String)> = self
            .map
            .iter()
            .filter_map(|(chord, action)| {
                action_name(action).map(|name| (format_chord(chord), name, describe(action)))
            })
            .collect();
        rows.sort();
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebind_replaces_the_default_chord() {
        let mut keys = HashMap::new();
        keys.insert("exit".to_string(), "ctrl+q".to_string());
        let cfg = Config {
            keys,
            ..Default::default()
        };
        let map = Keymap::from_config(cfg);
        assert_eq!(
            map.resolve(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Action::Exit
        );
        assert_ne!(
            map.resolve(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Action::Exit
        );
    }

    #[test]
    fn every_bound_action_names_its_config_key() {
        for action in Keymap::new().map.values() {
            if matches!(action, Action::Insert(_) | Action::PromptChar(_)) {
                continue;
            }
            assert!(
                action_name(action).is_some(),
                "bindable action without a config name: {action:?}"
            );
        }
    }
}
