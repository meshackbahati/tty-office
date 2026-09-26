//! Menu bar model: labels, dropdown items, hit testing, keyboard driving.
//!
//! Geometry is pure so the renderer and the mouse handler agree without
//! sharing state: the bar always occupies row 0, each label starts where
//! the previous one ends, and each dropdown hangs below its own label.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keymap::Action;

use super::App;

/// Top-level menus in bar order.
pub(crate) const LABELS: [&str; 4] = ["File", "Edit", "View", "Help"];

/// One dropdown row: label, action, and the shortcut shown beside it.
pub(crate) struct MenuItem {
    pub(crate) label: &'static str,
    pub(crate) action: Action,
    pub(crate) shortcut: &'static str,
}

static FILE_ITEMS: [MenuItem; 9] = [
    MenuItem {
        label: "New tab",
        action: Action::New,
        shortcut: "Ctrl+N",
    },
    MenuItem {
        label: "New text file",
        action: Action::NewText,
        shortcut: "Ctrl+Shift+N",
    },
    MenuItem {
        label: "New spreadsheet",
        action: Action::NewSheet,
        shortcut: "Ctrl+Shift+E",
    },
    MenuItem {
        label: "Open…",
        action: Action::Open,
        shortcut: "Ctrl+O",
    },
    MenuItem {
        label: "Close tab",
        action: Action::CloseTab,
        shortcut: "Ctrl+F4",
    },
    MenuItem {
        label: "Save",
        action: Action::Save,
        shortcut: "Ctrl+S",
    },
    MenuItem {
        label: "Save As…",
        action: Action::SaveAs,
        shortcut: "Ctrl+Shift+S",
    },
    MenuItem {
        label: "Export…",
        action: Action::Export,
        shortcut: "Ctrl+P",
    },
    MenuItem {
        label: "Quit",
        action: Action::Exit,
        shortcut: "Ctrl+X",
    },
];

static EDIT_ITEMS: [MenuItem; 7] = [
    MenuItem {
        label: "Undo",
        action: Action::Undo,
        shortcut: "Ctrl+Z",
    },
    MenuItem {
        label: "Redo",
        action: Action::Redo,
        shortcut: "Ctrl+Y",
    },
    MenuItem {
        label: "Cut line",
        action: Action::CutLine,
        shortcut: "Ctrl+K",
    },
    MenuItem {
        label: "Paste",
        action: Action::Uncut,
        shortcut: "Ctrl+U",
    },
    MenuItem {
        label: "Select all",
        action: Action::SelectAll,
        shortcut: "Ctrl+A",
    },
    MenuItem {
        label: "Find…",
        action: Action::Find,
        shortcut: "Ctrl+W",
    },
    MenuItem {
        label: "Replace…",
        action: Action::Replace,
        shortcut: "Ctrl+\\",
    },
];

static VIEW_ITEMS: [MenuItem; 6] = [
    MenuItem {
        label: "Show position",
        action: Action::ShowPosition,
        shortcut: "Ctrl+C",
    },
    MenuItem {
        label: "Zoom in",
        action: Action::ZoomIn,
        shortcut: "Ctrl+=",
    },
    MenuItem {
        label: "Zoom out",
        action: Action::ZoomOut,
        shortcut: "Ctrl+-",
    },
    MenuItem {
        label: "Reset zoom",
        action: Action::ZoomReset,
        shortcut: "Ctrl+0",
    },
    MenuItem {
        label: "Cycle theme",
        action: Action::CycleTheme,
        shortcut: "Alt+T",
    },
    MenuItem {
        label: "Keyboard shortcuts",
        action: Action::Help,
        shortcut: "F1",
    },
];

static HELP_ITEMS: [MenuItem; 1] = [MenuItem {
    label: "Keyboard shortcuts",
    action: Action::Help,
    shortcut: "F1",
}];

/// Dropdown items for menu `idx`; anything past Help falls back to Help.
pub(crate) fn items(menu: usize) -> &'static [MenuItem] {
    match menu {
        0 => &FILE_ITEMS,
        1 => &EDIT_ITEMS,
        2 => &VIEW_ITEMS,
        _ => &HELP_ITEMS,
    }
}

/// Bar index whose label contains column `x`, if any.
pub(crate) fn hit_label(x: u16) -> Option<usize> {
    (0..LABELS.len()).find(|&i| {
        let (a, b) = label_span(i);
        x >= a && x < b
    })
}

/// Column span of menu `idx` on the bar, end exclusive. Labels carry one
/// leading space and are separated by two spaces.
pub(crate) fn label_span(idx: usize) -> (u16, u16) {
    let mut x = 1u16;
    for (i, label) in LABELS.iter().enumerate() {
        let w = label.len() as u16;
        if i == idx {
            return (x, x + w);
        }
        x += w + 2;
    }
    (x, x)
}

/// Dropdown box origin and width for menu `idx`: the box hangs below the
/// bar with a border, so items start at row 2 within columns `x..x+w`.
pub(crate) fn dropdown(idx: usize) -> (u16, u16) {
    let (x, _) = label_span(idx);
    let inner = items(idx)
        .iter()
        .map(|it| it.label.len() + 2 + it.shortcut.len())
        .max()
        .unwrap_or(10) as u16;
    (x.saturating_sub(1), inner + 2)
}

/// Item index at absolute row `y` inside an open menu, if the row holds one.
pub(crate) fn hit_item(menu: usize, y: u16) -> Option<usize> {
    let n = items(menu).len() as u16;
    y.checked_sub(2).filter(|&r| r < n).map(|r| r as usize)
}

impl App {
    /// Menu hotkey: Alt+mnemonic opens a menu, F10 opens File.
    pub(crate) fn menu_hotkey(key: &KeyEvent) -> Option<usize> {
        if key.code == KeyCode::F(10) {
            return Some(0);
        }
        if key.modifiers == KeyModifiers::ALT {
            if let KeyCode::Char(c) = key.code {
                return match c.to_ascii_lowercase() {
                    'f' => Some(0),
                    'e' => Some(1),
                    'v' => Some(2),
                    'h' => Some(3),
                    _ => None,
                };
            }
        }
        None
    }

    /// Drive the open menu; every key is consumed, and keys that do not
    /// navigate simply dismiss the menu instead of reaching the document.
    pub(crate) fn handle_menu_key(&mut self, key: KeyEvent) {
        let Some(menu) = self.open_menu else {
            return;
        };
        let count = items(menu).len();
        match key.code {
            KeyCode::Esc => self.open_menu = None,
            KeyCode::Up => self.menu_item = self.menu_item.saturating_sub(1),
            KeyCode::Down => self.menu_item = (self.menu_item + 1).min(count.saturating_sub(1)),
            KeyCode::Enter => self.activate_menu_item(),
            KeyCode::Left => {
                self.open_menu = Some((menu + LABELS.len() - 1) % LABELS.len());
                self.menu_item = 0;
            }
            KeyCode::Right => {
                self.open_menu = Some((menu + 1) % LABELS.len());
                self.menu_item = 0;
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.open_menu = None;
                } else if let Some(next) = Self::menu_hotkey(&key) {
                    self.open_menu = Some(next);
                    self.menu_item = 0;
                } else {
                    let lower = c.to_ascii_lowercase();
                    match LABELS
                        .iter()
                        .position(|l| l.to_ascii_lowercase().starts_with(lower))
                    {
                        Some(next) => {
                            self.open_menu = Some(next);
                            self.menu_item = 0;
                        }
                        None => self.open_menu = None,
                    }
                }
            }
            _ => self.open_menu = None,
        }
    }

    /// Toggle the dropdown for bar index `i` for mouse clicks.
    pub(crate) fn toggle_menu(&mut self, i: usize) {
        if self.open_menu == Some(i) {
            self.open_menu = None;
        } else {
            self.open_menu = Some(i);
            self.menu_item = 0;
        }
    }

    /// Run the highlighted item and dismiss the menu.
    pub(crate) fn activate_menu_item(&mut self) {
        if let Some(menu) = self.open_menu {
            if let Some(item) = items(menu).get(self.menu_item) {
                let action = item.action.clone();
                self.open_menu = None;
                self.perform(action);
                return;
            }
        }
        self.open_menu = None;
    }
}
