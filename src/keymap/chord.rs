//! Parsed chords: key code plus modifiers, with matching that tolerates
//! the SHIFT reports terminals differ on.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Parsed chord: key code plus modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    /// Key code.
    pub code: KeyCode,
    /// Required modifiers (CONTROL, ALT, SHIFT as applicable).
    pub mods: KeyModifiers,
}

impl Chord {
    /// Create a chord.
    pub fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    /// Parse strings like `ctrl+x`, `alt+b`, `shift+left`, `ctrl+\\`.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut mods = KeyModifiers::NONE;
        let mut key_part = spec;
        loop {
            let lower = key_part.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("ctrl+") {
                mods |= KeyModifiers::CONTROL;
                key_part = &key_part[key_part.len() - rest.len()..];
            } else if let Some(rest) = lower.strip_prefix("alt+") {
                mods |= KeyModifiers::ALT;
                key_part = &key_part[key_part.len() - rest.len()..];
            } else if let Some(rest) = lower.strip_prefix("shift+") {
                mods |= KeyModifiers::SHIFT;
                key_part = &key_part[key_part.len() - rest.len()..];
            } else {
                break;
            }
        }
        let code = match key_part {
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "enter" | "return" => KeyCode::Enter,
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "esc" | "escape" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            fkey if (fkey.starts_with('f') || fkey.starts_with('F')) && fkey.len() > 1 => {
                let n: u8 = fkey[1..].parse().ok()?;
                if !(1..=24).contains(&n) {
                    return None;
                }
                KeyCode::F(n)
            }
            single if single.chars().count() == 1 => {
                let c = single.chars().next()?;
                KeyCode::Char(c)
            }
            _ => return None,
        };
        // Shifted printable characters arrive as uppercase with SHIFT set on
        // some terminals; normalize plain letters so matching stays simple.
        Some(Self::new(code, mods))
    }

    /// Whether this chord matches a key event, ignoring key release/repeat
    /// kind differences handled by the caller.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        if key.code != self.code {
            // Uppercase char events match lowercase chords without SHIFT.
            if let (KeyCode::Char(ours), KeyCode::Char(theirs)) = (self.code, key.code) {
                if ours.is_lowercase()
                    && theirs.eq_ignore_ascii_case(&ours)
                    && key.modifiers.difference(KeyModifiers::SHIFT)
                        == self.mods.difference(KeyModifiers::SHIFT)
                {
                    return true;
                }
            }
            return false;
        }
        // Printable characters may report SHIFT implicitly for uppercase; only
        // the non-SHIFT bits need to agree there. Navigation keys require an
        // exact modifier match so Shift+Right cannot resolve to plain Right.
        if matches!(key.code, KeyCode::Char(_)) {
            let required = self.mods.difference(KeyModifiers::SHIFT);
            let present = key.modifiers.difference(KeyModifiers::SHIFT);
            return required == present;
        }
        key.modifiers == self.mods
    }

    /// Specificity of this chord for an event: 3 for a full agreement, 2
    /// when exactly one of the code and the modifiers agrees exactly
    /// (uppercase event against a lowercase chord, or SHIFT held against
    /// a chord that omits it), 1 for the doubly tolerant agreement, and
    /// 0 for no match. Scoring lets `resolve` prefer `ctrl+shift+s` over
    /// `ctrl+s` when SHIFT is actually held, the reverse when it is not,
    /// and `ctrl+shift+r` over the `ctrl+r` read-file chord on terminals
    /// that report the uppercase event, instead of whichever entry the
    /// map visits first.
    pub(crate) fn match_score(&self, key: &KeyEvent) -> u8 {
        if !self.matches(key) {
            return 0;
        }
        let code_exact = key.code == self.code;
        let mods_exact = self.mods == key.modifiers;
        match (code_exact, mods_exact) {
            (true, true) => 3,
            (true, false) | (false, true) => 2,
            (false, false) => 1,
        }
    }
}

/// Format a chord for display, e.g. `Ctrl+X`.
pub fn format_chord(chord: &Chord) -> String {
    let mut s = String::new();
    if chord.mods.contains(KeyModifiers::CONTROL) {
        s.push_str("Ctrl+");
    }
    if chord.mods.contains(KeyModifiers::ALT) {
        s.push_str("Alt+");
    }
    if chord.mods.contains(KeyModifiers::SHIFT) {
        s.push_str("Shift+");
    }
    match chord.code {
        KeyCode::Char(c) => {
            if c == '\\' {
                s.push('\\');
            } else {
                s.extend(c.to_uppercase());
            }
        }
        KeyCode::Left => s.push_str("Left"),
        KeyCode::Right => s.push_str("Right"),
        KeyCode::Up => s.push_str("Up"),
        KeyCode::Down => s.push_str("Down"),
        KeyCode::Home => s.push_str("Home"),
        KeyCode::End => s.push_str("End"),
        KeyCode::PageUp => s.push_str("PageUp"),
        KeyCode::PageDown => s.push_str("PageDown"),
        KeyCode::Enter => s.push_str("Enter"),
        KeyCode::Esc => s.push_str("Esc"),
        KeyCode::Backspace => s.push_str("Backspace"),
        KeyCode::Delete => s.push_str("Delete"),
        KeyCode::Tab => s.push_str("Tab"),
        KeyCode::F(n) => {
            s.push('F');
            s.push_str(&n.to_string());
        }
        other => s.push_str(&format!("{other:?}")),
    }
    s
}
