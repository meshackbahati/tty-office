//! CSV spreadsheets: parse on open, quote on save, and formulas that
//! travel as `=` text and re-evaluate after the round trip.

#![cfg(feature = "xlsx")]

use tempfile::TempDir;
use tty_office::{open, Document, Editor, Motion, SheetDocument, SheetFormat};

fn fresh_sheet() -> SheetDocument {
    SheetDocument::new(SheetFormat::Xlsx)
}

fn put(sheet: &mut SheetDocument, row: usize, col: usize, input: &str) {
    sheet.move_cursor(Motion::BufferStart, false);
    for _ in 0..row {
        sheet.move_cursor(Motion::Down, false);
    }
    for _ in 0..col {
        sheet.move_cursor(Motion::Right, false);
    }
    sheet.set_cell_content(input);
}

fn value(sheet: &SheetDocument, row: usize, col: usize) -> String {
    sheet
        .cell(row, col)
        .map(|c| c.value.display())
        .unwrap_or_default()
}

#[test]
fn csv_roundtrip_preserves_values_and_formulas() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("sheet.csv");
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 0, "region");
    put(&mut sheet, 0, 1, "120");
    put(&mut sheet, 1, 1, "=B1*2");
    let mut doc = Document::Sheet(Box::new(sheet));
    doc.save_as(&path).expect("save csv");
    let again = open(&path).expect("reopen csv");
    let Document::Sheet(sheet) = again else {
        panic!("csv must open as a sheet");
    };
    assert_eq!(value(&sheet, 0, 0), "region");
    assert_eq!(value(&sheet, 0, 1), "120");
    assert_eq!(value(&sheet, 1, 1), "240");
}

#[test]
fn csv_quoting_survives_commas_and_quotes() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("quoted.csv");
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 0, "has, comma");
    put(&mut sheet, 0, 1, "says \"hi\"");
    let mut doc = Document::Sheet(Box::new(sheet));
    doc.save_as(&path).expect("save csv");
    let raw = std::fs::read_to_string(&path).expect("read raw");
    assert!(raw.contains("\"has, comma\""), "quoting missing: {raw}");
    let again = open(&path).expect("reopen csv");
    let Document::Sheet(sheet) = again else {
        panic!("csv must open as a sheet");
    };
    assert_eq!(value(&sheet, 0, 0), "has, comma");
    assert_eq!(value(&sheet, 0, 1), "says \"hi\"");
}

#[test]
fn csv_opens_hand_written_files() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("hand.csv");
    std::fs::write(&path, "a,b\r\n1,2\n").expect("write fixture");
    let doc = open(&path).expect("open csv");
    let Document::Sheet(sheet) = doc else {
        panic!("csv must open as a sheet");
    };
    assert_eq!(value(&sheet, 0, 0), "a");
    assert_eq!(value(&sheet, 1, 1), "2");
}

#[test]
fn empty_csv_opens_an_empty_sheet() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("empty.csv");
    std::fs::write(&path, "").expect("write fixture");
    let doc = open(&path).expect("open csv");
    assert!(matches!(doc, Document::Sheet(_)));
    assert!(!doc.is_dirty());
}
