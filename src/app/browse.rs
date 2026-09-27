//! File browser: a pure-TTY directory picker for opening files.
//!
//! The browser is modal chrome over the body area: type to filter,
//! arrows to move, Enter to open or descend, Backspace to filter back
//! or ascend, Esc to leave. Mouse clicks select, and clicking the
//! selected row activates it.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Mode};

/// One directory entry with its kind for rendering and activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowseEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Browser session: directory, entries, selection, and filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowseState {
    pub dir: PathBuf,
    pub entries: Vec<BrowseEntry>,
    pub selected: usize,
    pub filter: String,
}

impl BrowseState {
    /// Read `dir`, listing directories before files, each alphabetical.
    /// Unreadable directories yield an empty listing rather than an error
    /// the caller must thread through the event loop. The directory
    /// canonicalizes first so ascending always has a parent to reach;
    /// without that, relative paths bottomed out at an empty string and
    /// the browser visibly stuck.
    pub fn open_dir(dir: &Path) -> Self {
        let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let mut entries = Vec::new();
        // A parent row keeps mouse users moving upward too; the filter
        // below applies to it like any other entry.
        if let Some(parent) = dir.parent() {
            entries.push(BrowseEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }
        if let Ok(read) = std::fs::read_dir(&dir) {
            for entry in read.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                if name == "." || name == ".." {
                    continue;
                }
                entries.push(BrowseEntry {
                    name,
                    path,
                    is_dir: entry.file_type().map(|t| t.is_dir()).unwrap_or(false),
                });
            }
        }
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Self {
            dir: dir.to_path_buf(),
            entries,
            selected: 0,
            filter: String::new(),
        }
    }

    /// Entries matching the type-to-filter substring, case-insensitively.
    pub fn visible(&self) -> Vec<&BrowseEntry> {
        if self.filter.is_empty() {
            return self.entries.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&needle))
            .collect()
    }

    /// Move the selection, clamping to the visible list.
    pub fn move_sel(&mut self, delta: i32) {
        let n = self.visible().len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        let next = self.selected as i32 + delta;
        self.selected = next.clamp(0, n as i32 - 1) as usize;
    }

    /// First visible row for a body height of `height` rows, keeping the
    /// selection on screen. Shared by the renderer and the mouse handler
    /// so clicks resolve identically.
    pub fn offset(selected: usize, len: usize, height: usize) -> usize {
        if len <= height || height == 0 {
            return 0;
        }
        let half = height / 2;
        (selected.saturating_sub(half)).min(len - height)
    }
}

impl App {
    /// Open the browser at the current file's directory, else the working
    /// directory, and hand it the keyboard.
    pub(crate) fn enter_browse(&mut self) {
        let dir = self
            .doc
            .path()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        self.browse = Some(BrowseState::open_dir(&dir));
        self.open_menu = None;
        self.mode = Mode::Browse;
    }

    /// Drive the browser; every key is consumed while it is open.
    pub(crate) fn handle_browse(&mut self, key: KeyEvent) {
        if self.browse.is_none() {
            self.enter_browse();
        }
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.browse = None;
            }
            KeyCode::Up if key.modifiers.is_empty() => {
                if let Some(b) = self.browse.as_mut() {
                    b.move_sel(-1);
                }
            }
            KeyCode::Down if key.modifiers.is_empty() => {
                if let Some(b) = self.browse.as_mut() {
                    b.move_sel(1);
                }
            }
            KeyCode::Home if key.modifiers.is_empty() => {
                if let Some(b) = self.browse.as_mut() {
                    b.selected = 0;
                }
            }
            KeyCode::End if key.modifiers.is_empty() => {
                if let Some(b) = self.browse.as_mut() {
                    b.selected = b.visible().len().saturating_sub(1);
                }
            }
            KeyCode::Enter => self.browse_activate(),
            KeyCode::Backspace => {
                let pop = self
                    .browse
                    .as_ref()
                    .map(|b| !b.filter.is_empty())
                    .unwrap_or(false);
                if pop {
                    if let Some(b) = self.browse.as_mut() {
                        b.filter.pop();
                        b.selected = 0;
                    }
                } else if let Some(parent) = self
                    .browse
                    .as_ref()
                    .and_then(|b| b.dir.parent().map(Path::to_path_buf))
                {
                    self.browse = Some(BrowseState::open_dir(&parent));
                }
            }
            KeyCode::Char(c)
                if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                    && !c.is_control() =>
            {
                if let Some(b) = self.browse.as_mut() {
                    b.filter.push(c);
                    b.selected = 0;
                }
            }
            _ => {}
        }
    }

    /// Open the selected file as a tab, or descend into the directory.
    pub(crate) fn browse_activate(&mut self) {
        let target = self.browse.as_ref().and_then(|b| {
            b.visible()
                .get(b.selected)
                .map(|e| (e.path.clone(), e.is_dir))
        });
        let Some((path, is_dir)) = target else {
            return;
        };
        if is_dir {
            self.browse = Some(BrowseState::open_dir(&path));
        } else {
            self.mode = Mode::Normal;
            self.browse = None;
            self.open_path_as_tab(&path);
        }
    }

    /// Open `path` as a new tab with a message, shared by the Open
    /// prompt and the browser.
    pub(crate) fn open_path_as_tab(&mut self, path: &Path) {
        match crate::open(path) {
            Ok(doc) => {
                self.new_tab(doc);
                let name = self.doc.display_name();
                self.message = format!("Opened {name}");
            }
            Err(err) => self.message = format!("Open failed: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(names: &[(&str, bool)]) -> BrowseState {
        BrowseState {
            dir: PathBuf::from("/tmp"),
            entries: names
                .iter()
                .map(|(name, is_dir)| BrowseEntry {
                    name: name.to_string(),
                    path: PathBuf::from("/tmp").join(name),
                    is_dir: *is_dir,
                })
                .collect(),
            selected: 0,
            filter: String::new(),
        }
    }

    #[test]
    fn selection_clamps_to_the_visible_list() {
        let mut b = state(&[("a", false), ("b", false)]);
        b.move_sel(5);
        assert_eq!(b.selected, 1);
        b.move_sel(-5);
        assert_eq!(b.selected, 0);
    }

    #[test]
    fn filter_narrows_case_insensitively() {
        let mut b = state(&[("Cargo.toml", false), ("src", true)]);
        b.filter = "TOML".to_string();
        let visible = b.visible();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "Cargo.toml");
    }

    #[test]
    fn offset_keeps_the_selection_on_screen() {
        assert_eq!(BrowseState::offset(0, 20, 5), 0);
        assert_eq!(BrowseState::offset(19, 20, 5), 15);
        assert_eq!(BrowseState::offset(10, 20, 5), 8);
        assert_eq!(BrowseState::offset(2, 3, 5), 0);
    }
}
