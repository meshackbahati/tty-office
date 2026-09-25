//! Export paths for spreadsheet grids (HTML table and Markdown pipe table).

#![cfg(feature = "xlsx")]

use std::path::Path;

use tty_office::{export, Document, Editor, SheetDocument, SheetFormat};

fn sheet_doc() -> Document {
    let mut s = SheetDocument::new(SheetFormat::Xlsx);
    s.set_cell_content("name");
    s.move_cursor(tty_office::Motion::Right, false);
    s.set_cell_content("qty");
    s.move_cursor(tty_office::Motion::Down, false);
    s.move_cursor(tty_office::Motion::LineStart, false);
    s.set_cell_content("apple");
    s.move_cursor(tty_office::Motion::Right, false);
    s.set_cell_content("3");
    Document::Sheet(Box::new(s))
}

#[test]
fn export_sheet_markdown_is_pipe_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.md");
    let mut doc = sheet_doc();
    export(&mut doc, &path).expect("markdown export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert!(on_disk.contains("| name | qty |"), "got:\n{on_disk}");
    assert!(on_disk.contains("| --- | --- |"));
    assert!(on_disk.contains("| apple | 3 |"));
    let _ = Path::new(".");
}

#[test]
fn export_sheet_html_is_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("out.html");
    let mut doc = sheet_doc();
    export(&mut doc, &path).expect("html export");
    let on_disk = std::fs::read_to_string(&path).expect("read");
    assert!(on_disk.contains("<table>"));
    assert!(on_disk.contains("<td>name</td>"));
    assert!(on_disk.contains("<td>3</td>"));
}
