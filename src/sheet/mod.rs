//! Spreadsheet documents behind the shared editing surface.
//!
//! The package model (xlsx, ods, or a read-only xls marker) is the source of
//! truth for structure and serialization. Interactive editing runs on a sparse
//! `cells` grid projection of sheet 0 so cursor, selection, and undo stay
//! independent of the backend. On save the projection is written back into
//! sheet 0; other sheets are left untouched. Formula evaluation uses a
//! `formualizer` mirror workbook in Excel dialect: package formulas are
//! converted on load, results are cached in the grid, and edited formulas are
//! converted back into the package dialect on save.

mod cell;
mod document;
mod edit;
mod formula;
mod package;

pub use cell::{Cell, CellValue, SheetFormat};
pub use document::SheetDocument;

/// Maximum number of undo units retained; older units are dropped.
const HISTORY_CAP: usize = 1000;
/// Rows stepped by PageUp and PageDown when the UI has not overridden them.
const PAGE_ROWS: usize = 40;
/// Display width of one grid column, used for horizontal scroll maths.
pub(crate) const CELL_WIDTH: usize = 12;
/// Display width of one grid column including its left border line.
pub(crate) const CELL_STRIDE: usize = CELL_WIDTH + 1;
/// Width of the row-number gutter in the grid pane.
pub(crate) const ROW_GUTTER: usize = 6;
/// Height of the column-letter header row in the grid pane.
pub(crate) const COL_HEADER_H: usize = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Editor, Motion};

    #[test]
    fn cell_edit_roundtrip() {
        let mut doc = SheetDocument::new(SheetFormat::Xlsx);
        doc.set_cell_content("=1+2");
        assert_eq!(doc.cell_edit_content(), "=1+2");
        assert_eq!(
            doc.cell(0, 0).map(|c| c.value.clone()),
            Some(CellValue::Number(3.0)),
            "formula result is cached in the grid"
        );
    }

    #[test]
    fn undo_restores_previous_cell() {
        let mut doc = SheetDocument::new(SheetFormat::Xlsx);
        doc.set_cell_content("one");
        doc.set_cell_content("two");
        assert!(doc.undo());
        assert_eq!(
            doc.cell(0, 0).map(|c| c.value.display()),
            Some("one".to_string())
        );
        assert!(doc.redo());
        assert_eq!(
            doc.cell(0, 0).map(|c| c.value.display()),
            Some("two".to_string())
        );
    }

    #[test]
    fn text_projection_is_tsv() {
        let mut doc = SheetDocument::new(SheetFormat::Xlsx);
        doc.set_cell_content("a");
        doc.cursor_col = 1;
        doc.set_cell_content("b");
        doc.cursor_row = 1;
        doc.cursor_col = 0;
        doc.set_cell_content("c");
        assert_eq!(doc.text_projection(), "a\tb\nc\t");
    }

    #[test]
    fn motion_end_reaches_used_range() {
        let mut doc = SheetDocument::new(SheetFormat::Xlsx);
        doc.set_cell_content("x");
        doc.move_cursor(Motion::Down, false);
        doc.move_cursor(Motion::Right, false);
        doc.set_cell_content("y");
        doc.move_cursor(Motion::BufferStart, false);
        doc.move_cursor(Motion::BufferEnd, false);
        assert_eq!(doc.cursor_cell(), (1, 1));
    }
}
