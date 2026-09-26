//! Headless operation through the library surface: cat, info, and
//! convert share the interface's open and save paths.

use std::path::PathBuf;

use tempfile::TempDir;
use tty_office::{cat_text, convert_files, info_text};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn cat_prints_the_text_projection() {
    let body = cat_text(&fixture("plain.txt")).expect("cat");
    assert_eq!(
        body,
        "Plain notes\nA second line for the corpus.\n\nTrailing paragraph."
    );
}

#[test]
fn info_reports_text_metadata() {
    let info = info_text(&fixture("plain.txt")).expect("info");
    assert!(info.contains("kind: text"), "info: {info}");
    assert!(info.contains("lines: 4"), "info: {info}");
    assert!(info.contains("bytes: "), "info: {info}");
}

#[test]
fn convert_copies_text_between_extensions() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("copy.md");
    let message = convert_files(&fixture("plain.txt"), &out).expect("convert");
    assert!(message.contains("Converted"), "message: {message}");
    let back = cat_text(&out).expect("read back");
    assert_eq!(
        back,
        "Plain notes\nA second line for the corpus.\n\nTrailing paragraph."
    );
}

#[test]
#[cfg(feature = "xlsx")]
fn info_reports_sheet_metadata() {
    let info = info_text(&fixture("budget.xlsx")).expect("info");
    assert!(info.contains("kind: spreadsheet (xlsx)"), "info: {info}");
    assert!(info.contains("formulas: 2"), "info: {info}");
}

#[test]
#[cfg(feature = "xlsx")]
fn convert_rewrites_a_workbook_format() {
    let dir = TempDir::new().expect("temp dir");
    let out = dir.path().join("copy.ods");
    convert_files(&fixture("budget.xlsx"), &out).expect("convert");
    let info = info_text(&out).expect("info converted");
    assert!(info.contains("kind: spreadsheet (ods)"), "info: {info}");
    assert!(info.contains("formulas: 2"), "info: {info}");
}
