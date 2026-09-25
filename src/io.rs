//! Atomic file load and save helpers.
//!
//! Saves write a temporary file in the destination directory and rename it
//! into place, so a crash mid-write cannot truncate the user's document.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::error::DocumentError;

/// Read a text file into a rope.
///
/// The reader is buffered; Ropey documents roughly ten percent overhead above
/// the raw file size when loading, which is acceptable for the sizes this
/// editor targets.
pub fn load_rope(path: &Path) -> Result<Rope, DocumentError> {
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    Rope::from_reader(&mut reader).map_err(DocumentError::Io)
}

/// Shared atomic write: temp file in the same directory, then rename.
fn atomic_write<F>(path: &Path, write: F) -> Result<(), DocumentError>
where
    F: FnOnce(&mut BufWriter<fs::File>) -> std::io::Result<()>,
{
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let file_name = path.file_name().ok_or_else(|| DocumentError::Save {
        path: path.to_path_buf(),
        message: "path has no file name".to_string(),
    })?;
    let tmp_name = format!(".{}.tty-office.tmp", file_name.to_string_lossy());
    let tmp_path = dir.join(tmp_name);

    let write_result = (|| -> std::io::Result<()> {
        let file = fs::File::create(&tmp_path)?;
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer.flush()?;
        writer.into_inner()?.sync_all()
    })();

    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(DocumentError::Save {
            path: path.to_path_buf(),
            message: err.to_string(),
        });
    }

    fs::rename(&tmp_path, path).map_err(|err| DocumentError::Save {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

/// Write a rope to `path` atomically: temp file in the same directory, then
/// rename.
pub fn save_rope_atomic(path: &Path, rope: &Rope) -> Result<(), DocumentError> {
    atomic_write(path, |w| rope.write_to(w))
}

/// Write a UTF-8 string to `path` atomically.
pub fn save_str_atomic(path: &Path, contents: &str) -> Result<(), DocumentError> {
    atomic_write(path, |w| w.write_all(contents.as_bytes()))
}

/// Write raw bytes to `path` atomically.
#[cfg_attr(not(feature = "pdf"), allow(dead_code))]
pub fn save_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), DocumentError> {
    atomic_write(path, |w| w.write_all(bytes))
}
