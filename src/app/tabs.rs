//! Tab strip: several documents share one session, with the active
//! document held in `App::doc` and the rest parked in order around it.
//!
//! The full tab list is `background[..active]`, then `doc`, then
//! `background[active..]`; parking the active document at its slot turns
//! the background into the previous full list, so switching is one insert
//! plus one remove with no index adjustment.

use crate::editor::Editor;
use crate::text::TextDocument;
use crate::Document;

use super::App;

impl App {
    /// Number of open tabs, including the active document.
    pub fn tab_count(&self) -> usize {
        self.background.len() + 1
    }

    /// Display index of the active tab.
    pub fn active_tab(&self) -> usize {
        self.active
    }

    /// The document shown at display index `i`, or `None` when out of range.
    fn tab_doc(&self, i: usize) -> Option<&Document> {
        if i == self.active {
            Some(&self.doc)
        } else if i < self.active {
            self.background.get(i)
        } else {
            self.background.get(i - 1)
        }
    }

    /// Title for tab `i`: the file name with a `*` marker while dirty.
    pub fn tab_title(&self, i: usize) -> Option<String> {
        let doc = self.tab_doc(i)?;
        let mut title = doc.display_name();
        if doc.is_dirty() {
            title.push('*');
        }
        Some(title)
    }

    /// Whether any open tab holds unsaved changes.
    pub fn any_dirty(&self) -> bool {
        if self.doc.is_dirty() {
            return true;
        }
        self.background.iter().any(|d| d.is_dirty())
    }

    /// Switch to tab `i`, parking the active document at its slot.
    /// Out-of-range targets and the current tab are ignored.
    pub fn switch_tab(&mut self, target: usize) {
        if target >= self.tab_count() || target == self.active {
            return;
        }
        let parked = std::mem::replace(&mut self.doc, Document::Text(TextDocument::new()));
        self.background.insert(self.active, parked);
        self.doc = self.background.remove(target);
        self.active = target;
    }

    /// Move to the next tab, wrapping past the last one.
    pub fn next_tab(&mut self) {
        let count = self.tab_count();
        if count > 1 {
            self.switch_tab((self.active + 1) % count);
        }
    }

    /// Move to the previous tab, wrapping past the first one.
    pub fn prev_tab(&mut self) {
        let count = self.tab_count();
        if count > 1 {
            self.switch_tab((self.active + count - 1) % count);
        }
    }

    /// Open `doc` as a new tab and activate it.
    pub fn new_tab(&mut self, doc: Document) {
        let parked = std::mem::replace(&mut self.doc, doc);
        self.background.insert(self.active, parked);
        self.active = self.background.len();
    }

    /// Close the active tab. A dirty tab refuses with a message, and the
    /// last tab falls back to quitting so the session never idles empty.
    pub fn close_tab(&mut self) {
        if self.doc.is_dirty() {
            self.message = "Unsaved changes; save or discard first".to_string();
            return;
        }
        if self.tab_count() == 1 {
            self.should_quit = true;
            return;
        }
        if self.active == self.background.len() {
            self.doc = self
                .background
                .pop()
                .expect("tab count above proves a parked tab exists");
            self.active -= 1;
        } else {
            self.doc = self.background.remove(self.active);
        }
        let name = self.doc.display_name();
        self.message = format!("Closed tab; now {name}");
    }
}
