//! Spreadsheet-specific normal-mode key handling.

use crossterm::event::KeyEvent;

use crate::editor::{Editor, Motion};
use crate::keymap::Action;

use super::{App, Mode, PromptKind};

impl App {
    pub(crate) fn is_sheet(&self) -> bool {
        matches!(self.doc, crate::Document::Sheet(_))
    }

    /// Sheet-specific normal-mode keys. Returns true when handled.
    pub(crate) fn handle_sheet_normal(&mut self, action: Action, key: KeyEvent) -> bool {
        match action {
            Action::InsertNewline => {
                let (prefill, label) = match self.doc.sheet_mut() {
                    Some(sheet) => {
                        let (row, col) = sheet.cursor_cell();
                        (
                            sheet.cell_edit_content(),
                            format!("{}: ", crate::sheet::SheetDocument::cell_ref(row, col)),
                        )
                    }
                    None => (String::new(), "Cell: ".to_string()),
                };
                self.mode = Mode::Prompt(PromptKind::CellEdit);
                self.prompt_label = label;
                self.prompt_buf = prefill;
                true
            }
            Action::Insert('\t') => {
                self.apply_motion(Motion::Right, false);
                true
            }
            Action::Insert(c) if !c.is_control() => {
                self.mode = Mode::Prompt(PromptKind::CellEdit);
                self.prompt_label = "Cell: ".to_string();
                self.prompt_buf = c.to_string();
                true
            }
            Action::Backspace | Action::DeleteForward => {
                self.doc.delete_back();
                true
            }
            Action::Uncut => {
                let buf = self.cutbuffer.clone();
                self.doc.insert_str(&buf);
                true
            }
            _ => {
                let _ = key;
                false
            }
        }
    }
}
