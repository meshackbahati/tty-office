//! Normal-mode dispatch: key events become resolved [`Action`]s, which
//! [`App::perform`] carries out. Menus, the sidebar, and the mouse share
//! `perform` so every surface triggers identical behavior.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::Editor;
use crate::keymap::Action;

use super::open::{default_save_name, open_optional};
use super::{App, Mode, PromptKind};

impl App {
    pub(crate) fn handle_normal(&mut self, key: KeyEvent) {
        // A dropdown menu owns the keyboard while open; every key either
        // navigates it or dismisses it, so typing can never leak through.
        if self.open_menu.is_some() {
            self.handle_menu_key(key);
            return;
        }
        // Alt+letter and F10 pull down a menu without touching the keymap.
        if let Some(menu) = Self::menu_hotkey(&key) {
            self.open_menu = Some(menu);
            self.menu_item = 0;
            return;
        }
        // Alt+digit jumps straight to a tab; digits carry no other
        // binding, so they never reach the keymap.
        if key.modifiers == KeyModifiers::ALT {
            if let KeyCode::Char(c @ '1'..='9') = key.code {
                let n = (c as usize) - ('0' as usize);
                if n <= self.tab_count() {
                    self.switch_tab(n - 1);
                }
                return;
            }
        }
        let action = self.keymap.resolve(&key);
        #[cfg(feature = "xlsx")]
        if self.is_sheet() && self.handle_sheet_normal(action.clone(), key) {
            return;
        }
        self.perform(action);
    }

    /// Carry out one resolved action; shared by keys, menus, and mouse.
    pub(crate) fn perform(&mut self, action: Action) {
        match action {
            Action::Insert(c) => self.doc.insert_char(c),
            Action::InsertNewline => self.doc.insert_char('\n'),
            Action::Backspace => self.doc.delete_back(),
            Action::DeleteForward => self.doc.delete_forward(),
            Action::Move(m) => self.apply_motion(m, false),
            Action::Extend(m) => self.apply_motion(m, true),
            Action::Exit => {
                if self.any_dirty() {
                    self.mode = Mode::ConfirmQuit;
                    self.message =
                        "Unsaved changes. Ctrl+S save, Ctrl+X discard, any other key cancels"
                            .to_string();
                } else {
                    self.should_quit = true;
                }
            }
            Action::New => {
                // New buffers open beside the current one, so no dirty
                // guard is needed: nothing is ever replaced.
                match open_optional(None) {
                    Ok(doc) => {
                        self.new_tab(doc);
                        let name = self.doc.display_name();
                        self.message = format!("New {name}");
                    }
                    Err(err) => self.message = format!("New failed: {err}"),
                }
            }
            Action::Open => {
                self.mode = Mode::Prompt(PromptKind::OpenFile);
                self.prompt_label = "Open: ".to_string();
                self.prompt_buf.clear();
                self.message = "Enter a file to open in a new tab".to_string();
            }
            Action::NextTab => self.next_tab(),
            Action::PrevTab => self.prev_tab(),
            Action::CloseTab => self.close_tab(),
            Action::ToggleSidebar => {
                self.sidebar = !self.sidebar;
            }
            Action::ZoomIn => {
                self.zoom = self.zoom.saturating_add(1).min(super::MAX_ZOOM);
            }
            Action::ZoomOut => {
                self.zoom = self.zoom.saturating_sub(1);
            }
            Action::ZoomReset => {
                self.zoom = 0;
            }
            Action::CycleTheme => {
                self.theme = self.theme.next();
                let name = self.theme.name;
                self.message = format!("Theme: {name}");
            }
            Action::NewText => {
                self.new_tab(super::sidebar::new_text_doc());
                let name = self.doc.display_name();
                self.message = format!("New {name}");
            }
            Action::NewSheet => {
                #[cfg(feature = "xlsx")]
                {
                    self.new_tab(super::sidebar::new_sheet_doc());
                    let name = self.doc.display_name();
                    self.message = format!("New {name}");
                }
                #[cfg(not(feature = "xlsx"))]
                {
                    self.message = "Spreadsheet support needs the xlsx feature".to_string();
                }
            }
            Action::Save => {
                if self.doc.path().is_none() {
                    // Without a path there is nothing to write to; the
                    // file name is the only missing input, so the save
                    // folds into Save As the way graphical editors do.
                    self.mode = Mode::Prompt(PromptKind::SaveAs);
                    self.prompt_label = "Save As: ".to_string();
                    self.prompt_buf = default_save_name(&self.doc);
                    self.message = "Enter a file name for this document".to_string();
                } else {
                    self.do_save(None);
                }
            }
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
                self.mode = Mode::Prompt(PromptKind::Export);
                self.prompt_label = "Export to: ".to_string();
                self.prompt_buf = self.export_suggestion();
            }
            Action::Confirm | Action::Cancel | Action::PromptChar(_) | Action::PromptBackspace => {
                self.message.clear();
            }
            Action::Noop => {}
        }
    }
}
