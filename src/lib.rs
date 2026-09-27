//! Pure TTY Document Suite.
//!
//! The library exposes the document model, the [`Editor`] trait, and format
//! detection. The binary in `main.rs` wires Ratatui and crossterm on top.
//!
//! ```
//! use tty_office::{open, Editor, Document};
//! use std::path::Path;
//!
//! let path = Path::new("/tmp/tty-office-lib-doc-test.txt");
//! std::fs::remove_file(path).ok();
//! let mut doc = open(path).expect("new file opens");
//! doc.insert_str("hello");
//! assert!(doc.is_dirty());
//! doc.save(None).expect("save writes the adopted path");
//! assert!(!doc.is_dirty());
//! std::fs::remove_file(path).ok();
//! ```

mod app;
mod document;
mod editor;
mod error;
mod headless;
mod history;
mod io;
mod keymap;
mod page;
mod print;
#[cfg(feature = "proof")]
mod proof;
#[cfg(feature = "docx")]
mod rich;
#[cfg(feature = "xlsx")]
mod sheet;
mod text;
mod theme;
mod ui;

pub use app::{open_optional, App, Mode, PromptKind};
pub use document::{open, Document};
pub use editor::{Cursor, Editor, Motion};
pub use error::DocumentError;
pub use headless::{cat_text, convert_files, info_text};
pub use history::History;
pub use keymap::{action_name, format_chord, Action, Chord, Config, Keymap};
pub use page::PageLayout;
pub use print::{export, ExportFormat};
#[cfg(feature = "proof")]
pub use proof::{Misspelling, ProofEngine};
#[cfg(feature = "docx")]
pub use rich::{RichDocument, RichFormat};
#[cfg(feature = "xlsx")]
pub use sheet::{Cell, CellValue, SheetDocument, SheetFormat};
pub use text::TextDocument;
pub use theme::Theme;
pub use ui::draw;

/// Full help document embedded from `docs/KEYMAP.md`.
///
/// Embedding keeps `Ctrl+G` correct after `cargo install` without hunting for
/// a data directory next to the binary.
pub fn help_text() -> &'static str {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/KEYMAP.md"))
}
