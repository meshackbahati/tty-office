//! Application shell: mode machine, keymap dispatch, prompts, cutbuffer.
//!
//! The shell owns the open [`Document`] and translates resolved [`Action`]s
//! into document edits or mode changes. It never talks to the terminal
//! directly, which keeps the event loop and the tests separable.

pub(crate) mod menu;
mod mouse;
mod normal;
mod open;
mod prompts;
#[cfg(feature = "xlsx")]
mod sheet_keys;
pub(crate) mod sidebar;
pub(crate) mod tabs;

pub use open::open_optional;

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::{Editor, Motion};
use crate::error::DocumentError;
use crate::keymap::Keymap;
use crate::page::PageLayout;
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

/// Which completion path a line prompt should take on Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PromptKind {
    /// Path prompt for writing the current document.
    SaveAs,
    /// Path prompt opening a document, which replaces the buffer.
    OpenFile,
    /// Path prompt inserting a text file at the cursor.
    ReadFile,
    /// Search needle.
    Find,
    /// Search phase of replace: asks for the replacement next.
    ReplaceFind,
    /// Replacement text phase of replace.
    ReplaceText,
    /// Spreadsheet cell entry, commit on Enter.
    #[cfg(feature = "xlsx")]
    CellEdit,
    /// Path prompt for export; format follows the extension.
    Export,
    /// Confirmation stop after a replace preview; Enter applies the staged
    /// matches and Esc discards them. Not a text field.
    ReplaceConfirm,
}

/// Absolute frame geometry the mouse handler needs, refreshed on every
/// draw. Plain offsets keep the application shell free of renderer types.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ViewRects {
    /// Text pane origin and height.
    pub text_x: u16,
    pub text_y: u16,
    pub text_h: u16,
    /// Sidebar origin and width; zero width means hidden.
    pub side_x: u16,
    pub side_w: u16,
    /// Tab strip row, when a second tab reserves one.
    pub tab_y: Option<u16>,
}

/// Highest zoom level; each level adds one blank row per text line.
pub(crate) const MAX_ZOOM: u8 = 4;

/// Application state driven by the event loop.
pub struct App {
    /// Active document; inactive tabs live in `background`.
    pub doc: Document,
    /// Inactive tabs in display order, with the active document held
    /// separately in `doc` at logical index `active`.
    background: Vec<Document>,
    /// Display index of the active document among all tabs.
    active: usize,
    /// Open dropdown menu by bar index, when the menu owns the keyboard.
    pub(crate) open_menu: Option<usize>,
    /// Highlighted item within the open dropdown menu.
    pub(crate) menu_item: usize,
    /// Whether the sidebar is drawn; narrow terminals hide it regardless.
    pub(crate) sidebar: bool,
    /// Frame geometry from the last draw, for the mouse handler.
    pub(crate) view: ViewRects,
    /// Drag origin for mouse selection, cleared on release.
    drag: Option<mouse::DragOrigin>,
    /// Display zoom: extra blank rows per text line, up to MAX_ZOOM.
    zoom: u8,
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
    /// Spell-check worker; `None` when the `proof` feature is off or no
    /// system dictionary was found at startup.
    #[cfg(feature = "proof")]
    proof: Option<crate::proof::ProofEngine>,
    /// Surface revision last handed to the proof engine (`None` = never).
    #[cfg(feature = "proof")]
    proof_rev: Option<u64>,
    /// Page geometry for prose documents, from `page_lines` in the user
    /// config or the shared default when the key is absent.
    page_layout: PageLayout,
    /// Match spans staged by the replace preview, applied only when the
    /// confirmation prompt is accepted and cleared on any cancel path.
    pending_replace: Vec<(usize, usize)>,
}

impl App {
    /// Wrap an open document with default keymap and empty prompts.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            background: Vec::new(),
            active: 0,
            open_menu: None,
            menu_item: 0,
            sidebar: true,
            view: ViewRects::default(),
            drag: None,
            zoom: 0,
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
            #[cfg(feature = "proof")]
            proof: crate::proof::ProofEngine::load().ok(),
            #[cfg(feature = "proof")]
            proof_rev: None,
            page_layout: PageLayout::new(
                crate::keymap::Config::load_user()
                    .page_lines
                    .unwrap_or(PageLayout::DEFAULT_LINES),
            ),
            pending_replace: Vec::new(),
        }
    }

    /// Page geometry the viewport and the status bar render with.
    pub fn page_layout(&self) -> PageLayout {
        self.page_layout
    }

    /// Extra blank rows drawn per text line; zero is the 100% level.
    pub(crate) fn zoom_step(&self) -> usize {
        self.zoom as usize
    }

    /// Zoom as a percentage for the status bar: 100 plus 25 per level.
    pub fn zoom_percent(&self) -> u16 {
        100 + self.zoom as u16 * 25
    }

    /// Update viewport metrics reported by the UI after each frame.
    pub fn set_viewport(&mut self, h: usize, w: usize) {
        self.view_h = h.max(1);
        self.view_w = w.max(1);
        self.scroll_to_cursor();
    }

    pub(crate) fn scroll_to_cursor(&mut self) {
        let (h, w) = (self.view_h, self.view_w);
        match &mut self.doc {
            Document::Text(t) => t.scroll_to_cursor(h, w),
            #[cfg(feature = "docx")]
            Document::Rich(r) => r.scroll_to_cursor(h, w),
            #[cfg(feature = "xlsx")]
            Document::Sheet(s) => s.scroll_to_cursor(h, w),
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

    fn handle_confirm_quit(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.do_save(None);
                // Quitting needs every tab clean, not just the visible
                // one, or background edits would be lost silently.
                if !self.any_dirty() {
                    self.should_quit = true;
                } else {
                    self.mode = Mode::Normal;
                    self.message = "Other tabs still have unsaved changes".to_string();
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

    fn handle_help(&mut self, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('g') | KeyCode::Char('q')
        ) {
            self.mode = Mode::Normal;
            self.message.clear();
        }
    }

    pub(crate) fn apply_motion(&mut self, motion: Motion, extend: bool) {
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
        self.scroll_to_cursor();
    }

    pub(crate) fn do_save(&mut self, path: Option<&Path>) {
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
        let Some(surface) = self.doc.prose_surface() else {
            self.message = "Cut line is unavailable for this document type".to_string();
            return;
        };
        let mut buf = String::new();
        surface.cut_current_line(&mut buf);
        self.cutbuffer = buf;
        self.message = "Cut line".to_string();
    }

    fn wrap_selection(&mut self, marker: &str) {
        let Some(surface) = self.doc.prose_surface() else {
            self.message = format!("Select text to apply {marker}");
            return;
        };
        if let Some((a, b)) = surface.selection_range() {
            if a < b {
                let inner = surface.rope().slice(a..b).to_string();
                let replacement = format!("{marker}{inner}{marker}");
                surface.replace_range(a, b, &replacement);
                self.message.clear();
                return;
            }
        }
        // No selection: tell the user rather than guessing a word.
        self.message = format!("Select text to apply {marker}");
    }

    fn position_message(&mut self) -> String {
        #[cfg(feature = "xlsx")]
        if let Document::Sheet(sheet) = &self.doc {
            let (row, col) = sheet.cursor_cell();
            let dirty = if sheet.is_dirty() {
                "modified"
            } else {
                "saved"
            };
            return format!(
                "Cell {}, {}",
                crate::sheet::SheetDocument::cell_ref(row, col),
                dirty
            );
        }
        let Some(surface) = self.doc.prose_surface() else {
            return "position is unavailable for this document type".to_string();
        };
        let line = surface.cursor_line() + 1;
        let col = surface.cursor_display_col() + 1;
        let words = surface.word_count();
        let dirty = if surface.is_dirty() {
            "modified"
        } else {
            "saved"
        };
        format!("Ln {line}, Col {col}, Words {words}, {dirty}")
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

    /// Default export destination shown in the prompt.
    fn export_suggestion(&self) -> String {
        match self.doc.path() {
            Some(p) => {
                let mut out = p.to_path_buf();
                out.set_extension("pdf");
                out.to_string_lossy().into_owned()
            }
            None => "export.pdf".to_string(),
        }
    }

    /// Drain proof results and schedule a re-check when prose changed.
    ///
    /// Called once per event-loop tick; sheets are not proofed.
    pub fn tick(&mut self) {
        #[cfg(feature = "proof")]
        {
            let (rev, text) = match &self.doc {
                Document::Text(t) => (Some(t.revision()), Some(t.text_projection())),
                #[cfg(feature = "docx")]
                Document::Rich(r) => (Some(r.revision()), Some(r.text_projection())),
                #[cfg(feature = "xlsx")]
                Document::Sheet(_) => (None, None),
            };
            if let (Some(engine), Some(rev), Some(text)) = (self.proof.as_mut(), rev, text) {
                if self.proof_rev != Some(rev) {
                    self.proof_rev = Some(rev);
                    engine.schedule(&text);
                }
                engine.poll();
            }
        }
    }

    /// Misspelled character ranges for the current prose surface.
    ///
    /// Sheets never schedule a check, so their range list stays empty.
    #[cfg(feature = "proof")]
    pub fn misspelled_ranges(&self) -> &[crate::proof::Misspelling] {
        self.proof.as_ref().map(|p| p.ranges()).unwrap_or_default()
    }

    /// Number of misspellings currently underlined in the status bar.
    #[cfg(feature = "proof")]
    pub fn misspelling_count(&self) -> usize {
        self.misspelled_ranges().len()
    }
}
