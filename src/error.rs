//! Errors surfaced by document operations.
//!
//! Library code returns [`DocumentError`]; the application binary converts to
//! `anyhow::Error` at the top level, which keeps the library edge explicit and
//! the application edge flexible.

use std::path::PathBuf;

/// Failures that can occur while opening, editing, or saving a document.
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    /// Underlying filesystem or stream failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The path extension has no registered loader.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// The file was recognized but could not be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// Serialization to the destination failed after a successful edit.
    #[error("save error to {path}: {message}")]
    Save {
        /// Destination that failed, when known.
        path: PathBuf,
        /// Human-readable cause.
        message: String,
    },

    /// A required prompt or confirmation was cancelled by the user.
    #[error("cancelled")]
    Cancelled,
}
