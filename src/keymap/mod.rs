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
    /// Step to the next display theme.
    CycleTheme,
    /// Fill downward from the cell above or the selection top row,
    /// shifting relative formula references per row.
    FillDown,
    /// Fill rightward from the cell to the left or the selection left
    /// column, shifting relative formula references per column.
    FillRight,
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
    fn match_score(&self, key: &KeyEvent) -> u8 {
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

/// File format for user config overrides.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Map from action name to chord spec, e.g. `exit = "ctrl+x"`.
    #[serde(default)]
    pub keys: HashMap<String, String>,
    /// Logical lines per page; absent or zero keeps the default page model.
    #[serde(default)]
    pub page_lines: Option<usize>,
    /// Theme preset name, e.g. `theme = "ocean"`; unknown names fall back
    /// to the monochrome default rather than failing to start.
    #[serde(default)]
    pub theme: Option<String>,
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
        if let Some(path) = user_config_path() {
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
        Action::CycleTheme => "Cycle theme".into(),
        Action::FillDown => "Fill down".into(),
        Action::FillRight => "Fill right".into(),
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

/// Config-file name for an action, when chords can express it.
/// Character insertions and prompt characters are structural typing
/// rather than shortcuts, so they carry no name.
pub fn action_name(action: &Action) -> Option<&'static str> {
    use crate::editor::Motion;
    use Action::*;
    let motion_suffix = |prefix: &str, m: &Motion| -> &'static str {
        match (prefix, m) {
            ("move", Motion::Left) => "move_left",
            ("move", Motion::Right) => "move_right",
            ("move", Motion::Up) => "move_up",
            ("move", Motion::Down) => "move_down",
            ("move", Motion::LineStart) => "move_home",
            ("move", Motion::LineEnd) => "move_end",
            ("move", Motion::PageUp) => "move_page_up",
            ("move", Motion::PageDown) => "move_page_down",
            ("move", Motion::BufferStart) => "move_buffer_start",
            ("move", Motion::BufferEnd) => "move_buffer_end",
            (_, Motion::Left) => "extend_left",
            (_, Motion::Right) => "extend_right",
            (_, Motion::Up) => "extend_up",
            (_, Motion::Down) => "extend_down",
            (_, Motion::LineStart) => "extend_home",
            (_, Motion::LineEnd) => "extend_end",
            (_, Motion::PageUp) => "extend_page_up",
            (_, Motion::PageDown) => "extend_page_down",
            (_, Motion::BufferStart) => "extend_buffer_start",
            (_, Motion::BufferEnd) => "extend_buffer_end",
        }
    };
    Some(match action {
        Exit => "exit",
        New => "new",
        NewText => "new_text",
        NewSheet => "new_sheet",
        Open => "open",
        NextTab => "tab_next",
        PrevTab => "tab_prev",
        CloseTab => "close_tab",
        ToggleSidebar => "toggle_sidebar",
        ZoomIn => "zoom_in",
        ZoomOut => "zoom_out",
        ZoomReset => "zoom_reset",
        CycleTheme => "cycle_theme",
        FillDown => "fill_down",
        FillRight => "fill_right",
        Save => "save",
        SaveAs => "save_as",
        ReadFile => "read_file",
        Find => "find",
        Replace => "replace",
        CutLine => "cut_line",
        Uncut => "uncut",
        ShowPosition => "show_position",
        Help => "help",
        Undo => "undo",
        Redo => "redo",
        SelectAll => "select_all",
        ToggleBold => "toggle_bold",
        ToggleItalic => "toggle_italic",
        Export => "export",
        Insert(_) => return None,
        InsertNewline => "insert_newline",
        Backspace => "backspace",
        DeleteForward => "delete_forward",
        Move(m) => motion_suffix("move", m),
        Extend(m) => motion_suffix("extend", m),
        Confirm => "confirm",
        Cancel => "cancel",
        PromptChar(_) => return None,
        PromptBackspace => "prompt_backspace",
        Noop => "noop",
    })
}

fn action_from_name(name: &str) -> Option<Action> {
    use crate::editor::Motion;
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
        "cycle_theme" => CycleTheme,
        "fill_down" => FillDown,
        "fill_right" => FillRight,
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
        "insert_newline" => InsertNewline,
        "backspace" => Backspace,
        "delete_forward" => DeleteForward,
        "move_left" => Move(Motion::Left),
        "move_right" => Move(Motion::Right),
        "move_up" => Move(Motion::Up),
        "move_down" => Move(Motion::Down),
        "move_home" => Move(Motion::LineStart),
        "move_end" => Move(Motion::LineEnd),
        "move_page_up" => Move(Motion::PageUp),
        "move_page_down" => Move(Motion::PageDown),
        "move_buffer_start" => Move(Motion::BufferStart),
        "move_buffer_end" => Move(Motion::BufferEnd),
        "extend_left" => Extend(Motion::Left),
        "extend_right" => Extend(Motion::Right),
        "extend_up" => Extend(Motion::Up),
        "extend_down" => Extend(Motion::Down),
        "extend_home" => Extend(Motion::LineStart),
        "extend_end" => Extend(Motion::LineEnd),
        "extend_page_up" => Extend(Motion::PageUp),
        "extend_page_down" => Extend(Motion::PageDown),
        "extend_buffer_start" => Extend(Motion::BufferStart),
        "extend_buffer_end" => Extend(Motion::BufferEnd),
        "confirm" => Confirm,
        "cancel" => Cancel,
        "prompt_backspace" => PromptBackspace,
        "noop" => Noop,
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
