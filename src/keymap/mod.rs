//! Keybinding map: Nano contract plus word-processor shortcuts.
//!
//! Defaults match `plan.txt`. A user file at
//! `~/.config/tty-office/config.toml` may override individual bindings; the
//! override merges with defaults rather than replacing the whole table.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

mod bindings;
use bindings::default_bindings;

/// Semantic action bound to one or more chords.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// Exit; prompt first when the document is dirty.
    Exit,
    /// Start a fresh untitled document.
    New,
    /// Start a fresh untitled text file.
    NewText,
    /// Start a fresh untitled spreadsheet.
    NewSheet,
    /// Prompt for a file to open in a new tab.
    Open,
    /// Activate the next tab, wrapping past the last one.
    NextTab,
    /// Activate the previous tab, wrapping past the first one.
    PrevTab,
    /// Close the active tab; dirty tabs refuse, the last tab quits.
    CloseTab,
    /// Show or hide the sidebar.
    ToggleSidebar,
    /// Add one blank row per text line, up to the maximum.
    ZoomIn,
    /// Remove one blank row per text line, down to none.
    ZoomOut,
    /// Return to one row per text line.
    ZoomReset,
    /// Write out to the current path.
    Save,
    /// Prompt for a destination path and save there.
    SaveAs,
    /// Prompt for a file and insert it at the cursor.
    ReadFile,
    /// Open the find prompt.
    Find,
    /// Open the replace prompt.
    Replace,
    /// Cut the current line into the cutbuffer.
    CutLine,
    /// Paste the cutbuffer at the cursor.
    Uncut,
    /// Show cursor position and document stats in the message bar.
    ShowPosition,
    /// Render the help screen.
    Help,
    /// Undo the last unit.
    Undo,
    /// Redo the last undone unit.
    Redo,
    /// Select the whole document.
    SelectAll,
    /// Wrap the selection or word in Markdown bold markers.
    ToggleBold,
    /// Wrap the selection or word in Markdown italic markers.
    ToggleItalic,
    /// Export to PDF, HTML, or Markdown; format follows the path extension.
    Export,
    /// Insert a literal character.
    Insert(char),
    /// Insert a newline.
    InsertNewline,
    /// Delete the selection or the character before the cursor.
    Backspace,
    /// Delete forward.
    DeleteForward,
    /// Move without extending the selection.
    Move(crate::editor::Motion),
    /// Move while extending the selection.
    Extend(crate::editor::Motion),
    /// Accept the current prompt or confirmation.
    Confirm,
    /// Cancel the current prompt or confirmation.
    Cancel,
    /// Literal character typed while a prompt is active.
    PromptChar(char),
    /// Backspace while a prompt is active.
    PromptBackspace,
    /// Unbound or not yet implemented.
    Noop,
}

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

    /// Specificity of this chord for an event: 2 for a full agreement, 1
    /// for the shift-tolerant printable agreement `matches` also accepts,
    /// 0 for no match. Scoring lets `resolve` prefer `ctrl+shift+s` over
    /// `ctrl+s` when SHIFT is actually held, and the reverse when it is
    /// not, instead of whichever entry the map visits first.
    fn match_score(&self, key: &KeyEvent) -> u8 {
        if !self.matches(key) {
            return 0;
        }
        if self.mods == key.modifiers {
            return 2;
        }
        1
    }
}

/// File format for user config overrides.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Map from action name to chord spec, e.g. `exit = "ctrl+x"`.
    #[serde(default)]
    pub keys: HashMap<String, String>,
    /// Logical lines per page; absent or zero keeps the default page model.
    #[serde(default)]
    pub page_lines: Option<usize>,
}

impl Config {
    /// Read `~/.config/tty-office/config.toml`, falling back to the default
    /// configuration when the file is missing or malformed, because a broken
    /// user config must not prevent the editor from starting.
    pub fn load_user() -> Self {
        if let Some(path) = user_config_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&text) {
                    return cfg;
                }
            }
        }
        Self::default()
    }
}

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
        let mut km = Self::new();
        if let Some(path) = user_config_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&text) {
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
                }
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
        let mut rows: Vec<(String, String)> = self
            .map
            .iter()
            .map(|(chord, action)| (format_chord(chord), describe(action)))
            .collect();
        rows.sort();
        rows
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

/// Short description of an action for the help screen.
pub fn describe(action: &Action) -> String {
    match action {
        Action::Exit => "Exit (prompt if modified)".into(),
        Action::New => "New document".into(),
        Action::NewText => "New text file".into(),
        Action::NewSheet => "New spreadsheet".into(),
        Action::Open => "Open file".into(),
        Action::NextTab => "Next tab".into(),
        Action::PrevTab => "Previous tab".into(),
        Action::CloseTab => "Close tab".into(),
        Action::ToggleSidebar => "Toggle sidebar".into(),
        Action::ZoomIn => "Zoom in".into(),
        Action::ZoomOut => "Zoom out".into(),
        Action::ZoomReset => "Reset zoom".into(),
        Action::Save => "Save file".into(),
        Action::SaveAs => "Save as".into(),
        Action::ReadFile => "Insert file at cursor".into(),
        Action::Find => "Find".into(),
        Action::Replace => "Replace".into(),
        Action::CutLine => "Cut line".into(),
        Action::Uncut => "Uncut (paste)".into(),
        Action::ShowPosition => "Show position".into(),
        Action::Help => "Get help".into(),
        Action::Undo => "Undo".into(),
        Action::Redo => "Redo".into(),
        Action::SelectAll => "Select all".into(),
        Action::ToggleBold => "Bold (**)".into(),
        Action::ToggleItalic => "Italic (*)".into(),
        Action::Export => "Export (PDF, HTML, Markdown)".into(),
        Action::Insert(_) => "Insert character".into(),
        Action::InsertNewline => "New line".into(),
        Action::Backspace => "Delete previous character".into(),
        Action::DeleteForward => "Delete next character".into(),
        Action::Move(m) => format!("Move {m:?}"),
        Action::Extend(m) => format!("Select {m:?}"),
        Action::Confirm => "Confirm".into(),
        Action::Cancel => "Cancel".into(),
        Action::PromptChar(_) => "Prompt character".into(),
        Action::PromptBackspace => "Prompt backspace".into(),
        Action::Noop => "No operation".into(),
    }
}

fn action_from_name(name: &str) -> Option<Action> {
    use Action::*;
    Some(match name {
        "exit" => Exit,
        "new" => New,
        "new_text" => NewText,
        "new_sheet" => NewSheet,
        "open" => Open,
        "tab_next" => NextTab,
        "tab_prev" => PrevTab,
        "close_tab" => CloseTab,
        "toggle_sidebar" => ToggleSidebar,
        "zoom_in" => ZoomIn,
        "zoom_out" => ZoomOut,
        "zoom_reset" => ZoomReset,
        "save" => Save,
        "save_as" => SaveAs,
        "read_file" => ReadFile,
        "find" => Find,
        "replace" => Replace,
        "cut_line" => CutLine,
        "uncut" => Uncut,
        "show_position" => ShowPosition,
        "help" => Help,
        "undo" => Undo,
        "redo" => Redo,
        "select_all" => SelectAll,
        "toggle_bold" => ToggleBold,
        "toggle_italic" => ToggleItalic,
        "export" => Export,
        "confirm" => Confirm,
        "cancel" => Cancel,
        _ => return None,
    })
}

fn user_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/tty-office/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_page_lines_alongside_keys() {
        let cfg: Config =
            toml::from_str("page_lines = 42\n[keys]\nexit = \"ctrl+q\"").expect("valid config");
        assert_eq!(cfg.page_lines, Some(42));
        assert_eq!(cfg.keys.get("exit").map(String::as_str), Some("ctrl+q"));
    }

    #[test]
    fn config_defaults_when_page_lines_absent() {
        let cfg: Config = toml::from_str("[keys]\nexit = \"ctrl+q\"").expect("valid config");
        assert_eq!(cfg.page_lines, None);
    }
}
