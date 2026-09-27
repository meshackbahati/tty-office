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
        // F6 toggles section focus everywhere in normal mode.
        if key.code == KeyCode::F(6) && key.modifiers.is_empty() {
            self.cycle_focus();
            return;
        }
        // A dropdown menu owns the keyboard while open; every key either
        // navigates it or dismisses it, so typing can never leak through.
        if self.open_menu.is_some() {
            self.handle_menu_key(key);
            return;
        }
        // A focused sidebar owns the navigation keys; anything else
        // returns focus to the text and still reaches the document, so
        // no keystroke is ever swallowed.
        if self.focus == super::Focus::Sidebar {
            match key.code {
                KeyCode::Up if key.modifiers.is_empty() => {
                    self.side_move(-1);
                    return;
                }
                KeyCode::Down if key.modifiers.is_empty() => {
                    self.side_move(1);
                    return;
                }
                KeyCode::Home if key.modifiers.is_empty() => {
                    self.side_home();
                    return;
                }
                KeyCode::End if key.modifiers.is_empty() => {
                    self.side_end();
                    return;
                }
                KeyCode::Enter if key.modifiers.is_empty() => {
                    self.sidebar_activate();
                    return;
                }
                KeyCode::Esc => {
                    self.focus = super::Focus::Text;
                    return;
                }
                _ => {
                    self.focus = super::Focus::Text;
                }
            }
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
                self.enter_browse();
                self.message = "Enter opens · type filters · Esc closes".to_string();
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
            Action::FocusNext => self.cycle_focus(),
            Action::FillDown => self.sheet_fill(true),
            Action::FillRight => self.sheet_fill(false),
            Action::OpenLink => self.open_link(),
            Action::SelectRow => self.sheet_select_axis(true),
            Action::SelectCol => self.sheet_select_axis(false),
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
                    self.open_prompt(
                        PromptKind::SaveAs,
                        "Save As: ",
                        default_save_name(&self.doc),
                    );
                    self.message = "Enter a file name for this document".to_string();
                } else {
                    self.do_save(None);
                }
            }
            Action::SaveAs => {
                // Prefill with the current full path so renaming is one
                // edit away and the directory is preserved; pathless
                // documents fall back to the format suggestion. The
                // prompt stays a typed line below the status bar: no
                // file manager ever appears.
                let prefill = self
                    .doc
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| default_save_name(&self.doc));
                self.open_prompt(PromptKind::SaveAs, "Save As: ", prefill);
            }
            Action::ReadFile => {
                self.open_prompt(PromptKind::ReadFile, "Insert File: ", String::new());
            }
            Action::Find => {
                let needle = self.last_find.clone();
                self.open_prompt(PromptKind::Find, "Where Is: ", needle);
            }
            Action::Replace => {
                self.open_prompt(PromptKind::ReplaceFind, "Replace: ", String::new());
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
            Action::ApplyHeading(level) => self.apply_heading(level),
            Action::Export => {
                let suggestion = self.export_suggestion();
                self.open_prompt(PromptKind::Export, "Export to: ", suggestion);
            }
            Action::Confirm | Action::Cancel | Action::PromptChar(_) | Action::PromptBackspace => {
                self.message.clear();
            }
            Action::Noop => {}
        }
    }

    /// Open the hyperlink under the cursor with the system handler.
    fn open_link(&mut self) {
        let target = (|| {
            let surface = self.doc.prose_surface()?;
            let line = surface.cursor_line();
            let at = surface.cursor_char();
            let base = surface.line_char_start(line);
            self.doc
                .link_spans(line)
                .into_iter()
                .find(|s| at >= base + s.0 && at < base + s.1)
                .and_then(|s| s.2)
        })();
        match target {
            Some(target) => match open_url(&target) {
                Ok(()) => {
                    self.message = format!("Opened {target}");
                }
                Err(err) => {
                    self.message = err;
                }
            },
            None => {
                self.message = "No link under cursor".to_string();
            }
        }
    }

    /// Apply a heading level to the cursor paragraph in word documents.
    /// Without the word backend the action reports instead of matching
    /// a variant that does not exist.
    #[cfg(feature = "docx")]
    fn apply_heading(&mut self, level: u8) {
        match &mut self.doc {
            crate::Document::Rich(rich) => match rich.apply_heading(level) {
                Ok(()) => {
                    self.message = format!("Heading {level}");
                }
                Err(err) => {
                    self.message = err;
                }
            },
            _ => {
                self.message = "Headings need a word document".to_string();
            }
        }
    }

    /// Apply a heading level to the cursor paragraph in word documents.
    /// Without the word backend the action reports instead of matching
    /// a variant that does not exist.
    #[cfg(not(feature = "docx"))]
    fn apply_heading(&mut self, _level: u8) {
        self.message = "Headings need a word document".to_string();
    }

    /// Fill from the leading cell or edge for `FillDown` (`down`) and
    /// `FillRight`, reporting the outcome on the message line. Without
    /// the spreadsheet backend the action reports instead of matching
    /// a variant that does not exist.
    #[cfg(feature = "xlsx")]
    fn sheet_fill(&mut self, down: bool) {
        let filled = match &mut self.doc {
            crate::Document::Sheet(sheet) => {
                if down {
                    sheet.fill_down()
                } else {
                    sheet.fill_right()
                }
            }
            _ => {
                self.message = "Fill needs a spreadsheet".to_string();
                return;
            }
        };
        if filled == 0 {
            self.message = if down {
                "Nothing to fill from the cell above".to_string()
            } else {
                "Nothing to fill from the cell to the left".to_string()
            };
        } else {
            let plural = if filled == 1 { "" } else { "s" };
            self.message = format!("Filled {filled} cell{plural}");
        }
    }

    /// Select the cursor row (`row`) or column in spreadsheets,
    /// reporting the outcome on the message line.
    #[cfg(feature = "xlsx")]
    fn sheet_select_axis(&mut self, row: bool) {
        match &mut self.doc {
            crate::Document::Sheet(sheet) => {
                if row {
                    let r = sheet.cursor_cell().0;
                    sheet.select_row(r);
                    self.message = format!("Selected row {}", r + 1);
                } else {
                    sheet.select_col(sheet.cursor_cell().1);
                    let letters = sheet.cursor_col_label();
                    self.message = format!("Selected column {letters}");
                }
            }
            _ => {
                self.message = "Row and column selection need a spreadsheet".to_string();
            }
        }
    }

    /// Fill from the leading cell or edge for `FillDown` (`down`) and
    /// `FillRight`, reporting the outcome on the message line.
    #[cfg(not(feature = "xlsx"))]
    fn sheet_fill(&mut self, _down: bool) {
        self.message = "Fill needs a spreadsheet".to_string();
    }

    /// Select the cursor row (`row`) or column in spreadsheets,
    /// reporting the outcome on the message line.
    #[cfg(not(feature = "xlsx"))]
    fn sheet_select_axis(&mut self, _row: bool) {
        self.message = "Row and column selection need a spreadsheet".to_string();
    }
}

/// Hand `target` to the system URL handler without blocking the loop.
/// Only web and mail targets go out; anything else reports instead of
/// guessing what the desktop would do with it.
pub(crate) fn open_url(target: &str) -> Result<(), String> {
    let lower = target.to_lowercase();
    if !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:"))
    {
        return Err(format!("Unsupported link target: {target}"));
    }
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("cmd");
        command.args(["/c", "start", "", target]);
        return command
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("Could not open link: {err}"));
    } else {
        std::process::Command::new("xdg-open")
    };
    command
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Could not open link: {err}"))
}
