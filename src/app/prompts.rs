//! Prompt handling: input line, completion, find and replace.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::Editor;
#[cfg(feature = "xlsx")]
use crate::editor::Motion;
use crate::io::load_rope;

use super::{App, Mode, PromptKind};

impl App {
    pub(crate) fn handle_prompt(&mut self, kind: PromptKind, key: KeyEvent) {
        // The replace preview is a confirmation stop rather than a text
        // field: only Enter and Esc act, so the staged spans and the shown
        // summary cannot drift apart by editing the prompt buffer.
        if kind == PromptKind::ReplaceConfirm {
            match key.code {
                KeyCode::Enter => self.commit_replace(),
                KeyCode::Esc => {
                    self.pending_replace.clear();
                    self.mode = Mode::Normal;
                    self.prompt_buf.clear();
                    self.message = "Replace cancelled".to_string();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.prompt_buf.clear();
                self.pending_replace.clear();
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

    pub(crate) fn complete_prompt(&mut self, kind: PromptKind, value: String) {
        match kind {
            PromptKind::SaveAs => {
                if value.is_empty() {
                    self.message = "Save As cancelled".to_string();
                    return;
                }
                // Bare names gain the document extension, so `budget`
                // saves as `budget.xlsx` for a spreadsheet rather than
                // failing on the missing suffix.
                let value = if Path::new(&value).extension().is_none() {
                    format!("{value}.{}", self.doc.default_extension())
                } else {
                    value
                };
                self.do_save(Some(Path::new(&value)));
            }
            PromptKind::OpenFile => {
                if value.is_empty() {
                    self.message = "Open cancelled".to_string();
                    return;
                }
                match crate::open(Path::new(&value)) {
                    Ok(doc) => {
                        self.new_tab(doc);
                        let name = self.doc.display_name();
                        self.message = format!("Opened {name}");
                    }
                    Err(err) => self.message = format!("Open failed: {err}"),
                }
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
                self.preview_replace();
            }
            #[cfg(feature = "xlsx")]
            PromptKind::CellEdit => {
                if let Some(sheet) = self.doc.sheet_mut() {
                    sheet.set_cell_content(&value);
                }
                self.scroll_to_cursor();
                // Move one row down after commit so repeated entry flows.
                self.doc.move_cursor(Motion::Down, false);
                self.scroll_to_cursor();
                self.message.clear();
            }
            PromptKind::Export => {
                if value.is_empty() {
                    self.message = "Export cancelled".to_string();
                    return;
                }
                match crate::print::export(&mut self.doc, Path::new(&value)) {
                    Ok(()) => self.message = format!("Exported {value}"),
                    Err(err) => self.message = format!("Export failed: {err}"),
                }
            }
            // The confirmation stop is answered inside `handle_prompt` before
            // completion runs, so nothing reaches this arm.
            PromptKind::ReplaceConfirm => {}
        }
    }

    pub(crate) fn find_next(&mut self, needle: &str) {
        let Some(surface) = self.doc.prose_surface() else {
            self.message = format!("Not found: {needle}");
            return;
        };
        let from = surface.cursor_char().saturating_add(1);
        match surface.find(needle, from) {
            Some((start, end)) => {
                surface.set_cursor_range(start, end);
                surface.scroll_to_cursor(self.view_h, self.view_w);
                self.message = format!("Found {needle}");
            }
            None => self.message = format!("Not found: {needle}"),
        }
    }

    /// Collect matches without touching the document and stage them behind a
    /// confirmation prompt, so the scale of the change is visible before any
    /// character moves.
    pub(crate) fn preview_replace(&mut self) {
        let needle = self.last_find.clone();
        let replacement = self.replace_with.clone();
        if needle.is_empty() {
            self.message = "Replace cancelled".to_string();
            return;
        }
        let Some(surface) = self.doc.prose_surface() else {
            self.message = "Replace is unavailable for this document type".to_string();
            return;
        };
        // Collect non-overlapping matches first so a replacement that
        // itself contains the needle cannot re-match and spin.
        let mut spans = Vec::new();
        let mut cursor = 0usize;
        while let Some((a, b)) = surface.find_no_wrap(&needle, cursor) {
            spans.push((a, b));
            cursor = b.max(a + 1);
        }
        if spans.is_empty() {
            self.message = format!("No matches for '{needle}'");
            return;
        }
        self.pending_replace = spans;
        self.mode = Mode::Prompt(PromptKind::ReplaceConfirm);
        self.prompt_label = "Replace preview: ".to_string();
        self.prompt_buf = format!(
            "{} occurrence(s): '{needle}' -> '{replacement}'",
            self.pending_replace.len()
        );
        self.message = "Enter to apply, Esc to cancel".to_string();
    }

    /// Apply the spans staged by the preview when the user confirms.
    pub(crate) fn commit_replace(&mut self) {
        let spans = std::mem::take(&mut self.pending_replace);
        let replacement = self.replace_with.clone();
        let count = spans.len();
        self.mode = Mode::Normal;
        self.prompt_buf.clear();
        let Some(surface) = self.doc.prose_surface() else {
            self.message = "Replace is unavailable for this document type".to_string();
            return;
        };
        // Replace from the end so earlier character indices stay valid.
        for (a, b) in spans.into_iter().rev() {
            surface.replace_range(a, b, &replacement);
        }
        self.message = format!("Replaced {count} occurrence(s)");
    }
}
