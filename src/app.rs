//! Application shell: mode machine, keymap dispatch, prompts, cutbuffer.
//!
//! The shell owns the open [`Document`] and translates resolved [`Action`]s
//! into document edits or mode changes. It never talks to the terminal
//! directly, which keeps the event loop and the tests separable.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::{Editor, Motion};
use crate::error::DocumentError;
use crate::io::load_rope;
use crate::keymap::{Action, Keymap};
use crate::text::TextDocument;
use crate::Document;

/// UI mode driving how key events are interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Default editing.
    Normal,
    /// Confirm discard of unsaved changes before exit.
    ConfirmQuit,
    /// Line prompt for save-as, read-file, find, or replace.
    Prompt(PromptKind),
    /// Full-screen help overlay.
    Help,
}

/// Which prompt is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKind {
    /// Destination path for save-as.
    SaveAs,
    /// Path of a file to insert at the cursor.
    ReadFile,
    /// Search needle.
    Find,
    /// First phase of replace: the needle.
    ReplaceFind,
    /// Second phase of replace: the replacement text.
    ReplaceText,
}

/// Application state driven by the event loop.
pub struct App {
    /// Open document.
    pub doc: Document,
    /// Resolved key bindings.
    pub keymap: Keymap,
    /// Transient status line content.
    pub message: String,
    /// Whether the event loop should stop.
    pub should_quit: bool,
    /// Current interaction mode.
    pub mode: Mode,
    /// Active prompt buffer.
    pub prompt_buf: String,
    /// Label shown to the left of the prompt buffer.
    pub prompt_label: String,
    /// Cutbuffer for Nano-style cut and paste.
    pub cutbuffer: String,
    /// Last find needle, reused by repeated find.
    pub last_find: String,
    /// Replacement text staged between replace phases.
    pub replace_with: String,
    /// Viewport height from the last frame, for page motion.
    pub view_h: usize,
    /// Viewport width from the last frame, for horizontal scroll.
    pub view_w: usize,
}

impl App {
    /// Wrap an open document with default keymap and empty prompts.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            keymap: Keymap::load_user(),
            message: String::new(),
            should_quit: false,
            mode: Mode::Normal,
            prompt_buf: String::new(),
            prompt_label: String::new(),
            cutbuffer: String::new(),
            last_find: String::new(),
            replace_with: String::new(),
            view_h: 24,
            view_w: 80,
        }
    }

    /// Update viewport metrics reported by the UI after each frame.
    pub fn set_viewport(&mut self, h: usize, w: usize) {
        self.view_h = h.max(1);
        self.view_w = w.max(1);
        match &mut self.doc {
            Document::Text(t) => t.scroll_to_cursor(self.view_h, self.view_w),
        }
    }

    /// Handle one key press according to the current mode.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != crossterm::event::KeyEventKind::Press {
            return;
        }
        match self.mode.clone() {
            Mode::Normal => self.handle_normal(key),
            Mode::ConfirmQuit => self.handle_confirm_quit(key),
            Mode::Prompt(kind) => self.handle_prompt(kind, key),
            Mode::Help => self.handle_help(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let action = self.keymap.resolve(&key);
        match action {
            Action::Insert(c) => self.doc.insert_char(c),
            Action::InsertNewline => self.doc.insert_char('\n'),
            Action::Backspace => self.doc.delete_back(),
            Action::DeleteForward => self.doc.delete_forward(),
            Action::Move(m) => self.apply_motion(m, false),
            Action::Extend(m) => self.apply_motion(m, true),
            Action::Exit => {
                if self.doc.is_dirty() {
                    self.mode = Mode::ConfirmQuit;
                    self.message =
                        "Unsaved changes. Ctrl+S save, Ctrl+X discard, any other key cancels"
                            .to_string();
                } else {
                    self.should_quit = true;
                }
            }
            Action::Save => self.do_save(None),
            Action::SaveAs => {
                self.mode = Mode::Prompt(PromptKind::SaveAs);
                self.prompt_label = "Save As: ".to_string();
                self.prompt_buf.clear();
            }
            Action::ReadFile => {
                self.mode = Mode::Prompt(PromptKind::ReadFile);
                self.prompt_label = "Insert File: ".to_string();
                self.prompt_buf.clear();
            }
            Action::Find => {
                self.mode = Mode::Prompt(PromptKind::Find);
                self.prompt_label = "Where Is: ".to_string();
                self.prompt_buf = self.last_find.clone();
            }
            Action::Replace => {
                self.mode = Mode::Prompt(PromptKind::ReplaceFind);
                self.prompt_label = "Replace: ".to_string();
                self.prompt_buf.clear();
            }
            Action::CutLine => self.cut_line(),
            Action::Uncut => self.doc.insert_str(&self.cutbuffer.clone()),
            Action::ShowPosition => {
                self.message = self.position_message();
            }
            Action::Help => {
                self.mode = Mode::Help;
            }
            Action::Undo => {
                if !self.doc.undo() {
                    self.message = "Already at oldest change".to_string();
                } else {
                    self.message.clear();
                }
            }
            Action::Redo => {
                if !self.doc.redo() {
                    self.message = "Already at newest change".to_string();
                } else {
                    self.message.clear();
                }
            }
            Action::SelectAll => self.doc.select_all(),
            Action::ToggleBold => self.wrap_selection("**"),
            Action::ToggleItalic => self.wrap_selection("*"),
            Action::Export => {
                self.message = "Export arrives with Phase 5 (PDF, HTML, Markdown)".to_string();
            }
            Action::Confirm | Action::Cancel | Action::PromptChar(_) | Action::PromptBackspace => {
                self.message.clear();
            }
            Action::Noop => {}
        }
    }

    fn handle_confirm_quit(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.do_save(None);
                if !self.doc.is_dirty() {
                    self.should_quit = true;
                } else {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.should_quit = true;
            }
            _ => {
                self.mode = Mode::Normal;
                self.message.clear();
            }
        }
    }

    fn handle_prompt(&mut self, kind: PromptKind, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.prompt_buf.clear();
                self.message.clear();
            }
            KeyCode::Enter => {
                let value = std::mem::take(&mut self.prompt_buf);
                self.mode = Mode::Normal;
                self.complete_prompt(kind, value);
            }
            KeyCode::Backspace => {
                self.prompt_buf.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prompt_buf.push(c);
            }
            _ => {}
        }
    }

    fn handle_help(&mut self, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('g') | KeyCode::Char('q')
        ) {
            self.mode = Mode::Normal;
            self.message.clear();
        }
    }

    fn complete_prompt(&mut self, kind: PromptKind, value: String) {
        match kind {
            PromptKind::SaveAs => {
                if value.is_empty() {
                    self.message = "Save As cancelled".to_string();
                    return;
                }
                self.do_save(Some(Path::new(&value)));
            }
            PromptKind::ReadFile => {
                if value.is_empty() {
                    self.message = "Insert cancelled".to_string();
                    return;
                }
                match load_rope(Path::new(&value)) {
                    Ok(rope) => {
                        let text = rope.to_string();
                        self.doc.insert_str(&text);
                        self.message = format!("Inserted {value}");
                    }
                    Err(err) => self.message = format!("Insert failed: {err}"),
                }
            }
            PromptKind::Find => {
                if value.is_empty() {
                    self.message = "Find cancelled".to_string();
                    return;
                }
                self.last_find = value.clone();
                self.find_next(&value);
            }
            PromptKind::ReplaceFind => {
                if value.is_empty() {
                    self.message = "Replace cancelled".to_string();
                    return;
                }
                self.last_find = value;
                self.mode = Mode::Prompt(PromptKind::ReplaceText);
                self.prompt_label = "Replace with: ".to_string();
                self.prompt_buf.clear();
            }
            PromptKind::ReplaceText => {
                self.replace_with = value;
                self.replace_all();
            }
        }
    }

    fn apply_motion(&mut self, motion: Motion, extend: bool) {
        let page = self.view_h;
        match motion {
            Motion::PageUp => {
                for _ in 0..page {
                    self.doc.move_cursor(Motion::Up, extend);
                }
            }
            Motion::PageDown => {
                for _ in 0..page {
                    self.doc.move_cursor(Motion::Down, extend);
                }
            }
            other => self.doc.move_cursor(other, extend),
        }
        match &mut self.doc {
            Document::Text(t) => t.scroll_to_cursor(self.view_h, self.view_w),
        }
    }

    fn do_save(&mut self, path: Option<&Path>) {
        let result = match path {
            Some(p) => self.doc.save_as(p),
            None => self.doc.save(None),
        };
        match result {
            Ok(()) => {
                let name = self.doc.display_name();
                self.message = format!("Wrote {name}");
            }
            Err(DocumentError::Save { path, message }) => {
                self.message = format!("Save failed for {}: {message}", path.display());
            }
            Err(err) => self.message = format!("Save failed: {err}"),
        }
    }

    fn cut_line(&mut self) {
        match &mut self.doc {
            Document::Text(t) => {
                let mut buf = String::new();
                t.cut_current_line(&mut buf);
                self.cutbuffer = buf;
                self.message = "Cut line".to_string();
            }
        }
    }

    fn wrap_selection(&mut self, marker: &str) {
        match &mut self.doc {
            Document::Text(t) => {
                if let Some((a, b)) = t.selection_range() {
                    if a < b {
                        let inner = t.rope().slice(a..b).to_string();
                        let replacement = format!("{marker}{inner}{marker}");
                        t.replace_range(a, b, &replacement);
                        self.message.clear();
                        return;
                    }
                }
                // No selection: tell the user rather than guessing a word.
                self.message = format!("Select text to apply {marker}");
            }
        }
    }

    fn find_next(&mut self, needle: &str) {
        match &mut self.doc {
            Document::Text(t) => {
                let from = t.cursor_char().saturating_add(1);
                match t.find(needle, from) {
                    Some((start, end)) => {
                        t.set_cursor_range(start, end);
                        t.scroll_to_cursor(self.view_h, self.view_w);
                        self.message = format!("Found {needle}");
                    }
                    None => self.message = format!("Not found: {needle}"),
                }
            }
        }
    }

    fn replace_all(&mut self) {
        match &mut self.doc {
            Document::Text(t) => {
                let needle = self.last_find.clone();
                let replacement = self.replace_with.clone();
                if needle.is_empty() {
                    self.message = "Replace cancelled".to_string();
                    return;
                }
                // Collect non-overlapping matches first so a replacement that
                // itself contains the needle cannot re-match and spin.
                let mut spans = Vec::new();
                let mut cursor = 0usize;
                while let Some((a, b)) = t.find_no_wrap(&needle, cursor) {
                    spans.push((a, b));
                    cursor = b.max(a + 1);
                }
                let count = spans.len();
                // Replace from the end so earlier character indices stay valid.
                for (a, b) in spans.into_iter().rev() {
                    t.replace_range(a, b, &replacement);
                }
                self.message = format!("Replaced {count} occurrence(s)");
            }
        }
    }

    fn position_message(&mut self) -> String {
        match &mut self.doc {
            Document::Text(t) => {
                let line = t.cursor_line() + 1;
                let col = t.cursor_display_col() + 1;
                let words = t.word_count();
                let dirty = if t.is_dirty() { "modified" } else { "saved" };
                format!("Ln {line}, Col {col}, Words {words}, {dirty}")
            }
        }
    }

    /// Prompt label plus buffer for the status line.
    pub fn prompt_display(&self) -> Option<String> {
        match self.mode {
            Mode::Prompt(_) => Some(format!("{}{}", self.prompt_label, self.prompt_buf)),
            Mode::ConfirmQuit => {
                Some("[unsaved] Ctrl+S save · Ctrl+X discard · other cancel".into())
            }
            _ => None,
        }
    }
}

/// Open `path`, or an empty document when no path is given.
pub fn open_optional(path: Option<&PathBuf>) -> Result<Document, DocumentError> {
    match path {
        Some(p) => crate::open(p),
        None => Ok(Document::Text(TextDocument::new())),
    }
}
