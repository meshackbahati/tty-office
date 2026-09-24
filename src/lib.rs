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
mod history;
mod io;
mod keymap;
mod text;
mod ui;

pub use app::{open_optional, App, Mode, PromptKind};
pub use document::{open, Document};
pub use editor::{Cursor, Editor, Motion};
pub use error::DocumentError;
pub use history::History;
pub use keymap::{format_chord, Action, Chord, Keymap};
pub use text::TextDocument;
pub use ui::draw;

/// Full help document embedded from `docs/KEYMAP.md`.
///
/// Embedding keeps `Ctrl+G` correct after `cargo install` without hunting for
/// a data directory next to the binary.
pub fn help_text() -> &'static str {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/KEYMAP.md"))
}
