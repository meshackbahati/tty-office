//! Sidebar: creation shortcuts, common commands, and the tab list.
//!
//! The sidebar is mouse-first chrome; every entry runs through `perform`
//! or the tab primitives, so keyboard users keep full parity through the
//! menu bar and the dedicated creation chords.

use crate::keymap::Action;
use crate::text::TextDocument;
use crate::Document;

use super::App;

/// Fixed sidebar width in columns.
pub(crate) const SIDEBAR_WIDTH: u16 = 22;
/// Terminals narrower than this hide the sidebar to protect the text pane.
pub(crate) const MIN_WIDTH_FOR_SIDEBAR: u16 = 60;

/// One sidebar row: a section header, a creation entry, a shared action,
/// or a tab jump.
pub(crate) struct SidebarRow {
    pub(crate) label: String,
    pub(crate) kind: RowKind,
}

pub(crate) enum RowKind {
    Header,
    NewText,
    #[cfg(feature = "xlsx")]
    NewSheet,
    Act(Action),
    Tab(usize),
}

/// Fresh empty text document for the creation entry.
pub(crate) fn new_text_doc() -> Document {
    Document::Text(TextDocument::new())
}

/// Fresh empty spreadsheet for the creation entry.
#[cfg(feature = "xlsx")]
pub(crate) fn new_sheet_doc() -> Document {
    Document::Sheet(Box::new(crate::SheetDocument::new(
        crate::SheetFormat::Xlsx,
    )))
}

/// Rows from top to bottom; deterministic so clicks resolve identically.
pub(crate) fn rows(app: &App) -> Vec<SidebarRow> {
    let files = [
        header("Files"),
        entry("New text file", RowKind::NewText),
        entry("New document", RowKind::Act(Action::New)),
    ];
    #[cfg(feature = "xlsx")]
    let sheet = [entry("New spreadsheet", RowKind::NewSheet)];
    #[cfg(not(feature = "xlsx"))]
    let sheet: [SidebarRow; 0] = [];
    let rest = [
        entry("Open…", RowKind::Act(Action::Open)),
        entry("Save", RowKind::Act(Action::Save)),
        entry("Export…", RowKind::Act(Action::Export)),
        header("Tools"),
        entry("Find…", RowKind::Act(Action::Find)),
        entry("Replace…", RowKind::Act(Action::Replace)),
        entry("Help", RowKind::Act(Action::Help)),
        header("View"),
        entry(
            &format!("Theme: {}", app.theme().name),
            RowKind::Act(Action::CycleTheme),
        ),
        header("Tabs"),
    ];
    let tabs = (0..app.tab_count()).map(|i| {
        let title = app.tab_title(i).unwrap_or_default();
        entry(&title, RowKind::Tab(i))
    });
    files
        .into_iter()
        .chain(sheet)
        .chain(rest)
        .chain(tabs)
        .collect()
}

fn header(label: &str) -> SidebarRow {
    SidebarRow {
        label: label.to_string(),
        kind: RowKind::Header,
    }
}

fn entry(label: &str, kind: RowKind) -> SidebarRow {
    SidebarRow {
        label: label.to_string(),
        kind,
    }
}

impl App {
    /// Run one sidebar row; headers are inert.
    pub(crate) fn activate_row(&mut self, row: &SidebarRow) {
        match &row.kind {
            RowKind::Header => {}
            RowKind::NewText => {
                self.new_tab(new_text_doc());
                let name = self.doc.display_name();
                self.message = format!("New {name}");
            }
            #[cfg(feature = "xlsx")]
            RowKind::NewSheet => {
                self.new_tab(new_sheet_doc());
                let name = self.doc.display_name();
                self.message = format!("New {name}");
            }
            RowKind::Act(action) => self.perform(action.clone()),
            RowKind::Tab(i) => self.switch_tab(*i),
        }
    }
}
