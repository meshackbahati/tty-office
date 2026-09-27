//! `SheetDocument`: package model plus a grid projection of sheet 0.
//!
//! Callers drive this type through [`crate::Editor`] and the grid accessors
//! on this type. The underlying `umya_spreadsheet::Workbook` and
//! `spreadsheet_ods::WorkBook` values are never exposed, which keeps
//! third-party types out of the public semver surface.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::cell::{Cell, SheetFormat};
use super::edit::{
    a1_ref, move_grid, project_tsv, scroll_offsets, select_all_cells, used_range, SheetEdit,
};
use super::formula::{new_mirror, push_grid_to_mirror, reevaluate_all};
use super::package::{
    load_package, new_package, package_matches, sync_grid_into_package, write_package, Package,
};
use crate::editor::{Cursor, Editor, Motion};
use crate::error::DocumentError;

/// Spreadsheet document: package model plus a grid projection of sheet 0.
pub struct SheetDocument {
    pub(crate) format: SheetFormat,
    pub(crate) path: Option<PathBuf>,
    pub(crate) package: Package,
    /// Sparse sheet-0 projection: zero-based `(row, col)` keys.
    pub(crate) cells: HashMap<(usize, usize), Cell>,
    pub(crate) cursor_row: usize,
    pub(crate) cursor_col: usize,
    /// Selection anchor, or `None` when collapsed.
    pub(crate) anchor: Option<(usize, usize)>,
    /// First visible row.
    pub(crate) rowoff: usize,
    /// First visible column.
    pub(crate) coloff: usize,
    pub(crate) dirty: bool,
    /// Undo units; each unit is one or more cell replacements applied in order.
    pub(crate) undo: Vec<Vec<SheetEdit>>,
    pub(crate) redo: Vec<Vec<SheetEdit>>,
    /// Evaluation mirror in Excel dialect; sheet [`super::formula::MIRROR_SHEET`].
    pub(crate) mirror: formualizer::Workbook,
}

impl fmt::Debug for SheetDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SheetDocument")
            .field("format", &self.format)
            .field("path", &self.path)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl SheetDocument {
    /// Create an empty spreadsheet that will be saved as `format`.
    pub fn new(format: SheetFormat) -> Self {
        let package = new_package(format);
        let mirror = new_mirror();
        let mut doc = Self {
            format,
            path: None,
            package,
            cells: HashMap::new(),
            cursor_row: 0,
            cursor_col: 0,
            anchor: None,
            rowoff: 0,
            coloff: 0,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            mirror,
        };
        doc.rebuild_mirror();
        doc
    }

    /// Open an `.xlsx`, `.ods`, or `.xls` file from disk.
    pub fn open(path: &Path) -> Result<Self, DocumentError> {
        let format = SheetFormat::from_path(path).ok_or_else(|| {
            DocumentError::UnsupportedFormat(
                path.extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
        })?;
        let (package, cells) = load_package(path, format)?;
        let mut mirror = new_mirror();
        push_grid_to_mirror(&mut mirror, &cells);
        let mut doc = Self {
            format,
            path: Some(path.to_path_buf()),
            package,
            cells,
            cursor_row: 0,
            cursor_col: 0,
            anchor: None,
            rowoff: 0,
            coloff: 0,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            mirror,
        };
        reevaluate_all(&mut doc.mirror, &mut doc.cells);
        Ok(doc)
    }

    /// Adopt `path` as the save destination without reading or writing it.
    pub fn adopt_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
        if let Some(fmt) = SheetFormat::from_path(path) {
            self.format = fmt;
            // Natives were produced for the previous package dialect; drop
            // them so a later save cannot write the wrong syntax.
            if !matches!(
                (&self.package, fmt),
                (Package::Xlsx(_), SheetFormat::Xlsx) | (Package::Ods(_), SheetFormat::Ods)
            ) {
                for cell in self.cells.values_mut() {
                    cell.native_formula = None;
                }
            }
        }
    }

    /// Path this document was opened from, when known.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Package format used for the next save.
    pub fn format(&self) -> SheetFormat {
        self.format
    }

    /// Display name for the status bar.
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            None => "[no name]".to_string(),
        }
    }

    /// Zero-based `(row, col)` of the cursor head.
    pub fn cursor_cell(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Normalized rectangular selection as `(top_left, bottom_right)`, or
    /// `None` when the selection is collapsed.
    pub fn selection_rect(&self) -> Option<((usize, usize), (usize, usize))> {
        let (ar, ac) = self.anchor?;
        let (hr, hc) = (self.cursor_row, self.cursor_col);
        let r0 = ar.min(hr);
        let r1 = ar.max(hr);
        let c0 = ac.min(hc);
        let c1 = ac.max(hc);
        Some(((r0, c0), (r1, c1)))
    }

    /// Column letters of the cursor column, e.g. `B`, for messages.
    pub fn cursor_col_label(&self) -> String {
        super::cell::col_letters(self.cursor_col)
    }

    /// Select an entire row, keeping the cursor column as the active cell.
    pub fn select_row(&mut self, row: usize) {
        self.anchor = Some((row, 0));
        self.cursor_row = row;
    }

    /// Select an entire column, keeping the cursor row as the active cell.
    pub fn select_col(&mut self, col: usize) {
        self.anchor = Some((0, col));
        self.cursor_col = col;
    }

    /// Place the caret on a cell for mouse clicks, clearing any selection.
    pub fn click_cell(&mut self, row: usize, col: usize) {
        self.cursor_row = row;
        self.cursor_col = col;
        self.anchor = None;
    }

    /// Extend the selection from the click origin to a cell while dragging.
    pub fn select_to(&mut self, origin: (usize, usize), to: (usize, usize)) {
        self.anchor = Some(origin);
        self.cursor_row = to.0;
        self.cursor_col = to.1;
    }

    /// Scroll the grid vertically without moving the caret.
    pub fn scroll_rows(&mut self, delta: i32) {
        self.rowoff = (self.rowoff as i32 + delta).max(0) as usize;
    }

    /// Read-only access to the grid for tests and the renderer.
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(&(row, col))
    }

    /// Inclusive used-range maximum `(max_row, max_col)`, or `(0, 0)` when
    /// the grid is empty.
    pub fn used_range(&self) -> (usize, usize) {
        used_range(&self.cells)
    }

    /// Number of cells holding a formula, for headless summaries.
    pub fn formula_count(&self) -> usize {
        self.cells.values().filter(|c| c.formula.is_some()).count()
    }

    /// First visible row index.
    pub fn rowoff(&self) -> usize {
        self.rowoff
    }

    /// First visible column index.
    pub fn coloff(&self) -> usize {
        self.coloff
    }

    /// Clamp scroll offsets so the cursor stays inside a viewport of the
    /// given character size.
    pub fn scroll_to_cursor(&mut self, view_h: usize, view_w: usize) {
        scroll_offsets(
            (self.cursor_row, self.cursor_col),
            &mut self.rowoff,
            &mut self.coloff,
            view_h,
            view_w,
        );
    }

    /// A1-style reference for `(row, col)`, e.g. `(0, 0)` is `A1`.
    pub fn cell_ref(row: usize, col: usize) -> String {
        a1_ref(row, col)
    }

    pub(crate) fn save_to(&mut self, target: &Path) -> Result<(), DocumentError> {
        let target_format = SheetFormat::from_path(target).ok_or_else(|| DocumentError::Save {
            path: target.to_path_buf(),
            message: "destination extension must be .xlsx, .ods, or .xls".to_string(),
        })?;
        if target_format == SheetFormat::Xls {
            return Err(DocumentError::Save {
                path: target.to_path_buf(),
                message: "binary .xls cannot be written; save as .xlsx or .ods".to_string(),
            });
        }
        if target_format == SheetFormat::Csv {
            // CSV writes straight from the grid; no package takes part,
            // and formulas travel as their `=` text.
            let text = super::csv::grid_to_csv(&self.cells);
            std::fs::write(target, text).map_err(|err| DocumentError::Save {
                path: target.to_path_buf(),
                message: err.to_string(),
            })?;
            self.path = Some(target.to_path_buf());
            self.format = SheetFormat::Csv;
            self.dirty = false;
            return Ok(());
        }

        let rebuilding =
            target_format != self.format || !package_matches(&self.package, target_format);
        if rebuilding {
            super::edit::prepare_rebuild(self, target_format);
        }

        sync_grid_into_package(&mut self.package, &self.cells, self.format)?;
        write_package(&mut self.package, target, self.format)?;

        self.path = Some(target.to_path_buf());
        self.dirty = false;
        Ok(())
    }
}

impl Default for SheetDocument {
    fn default() -> Self {
        Self::new(SheetFormat::Xlsx)
    }
}

impl Editor for SheetDocument {
    fn cursor(&self) -> Cursor {
        Cursor::Cell {
            row: self.cursor_row,
            col: self.cursor_col,
        }
    }

    fn selection(&self) -> Option<(Cursor, Cursor)> {
        let ((r0, c0), (r1, c1)) = self.selection_rect()?;
        Some((
            Cursor::Cell { row: r0, col: c0 },
            Cursor::Cell { row: r1, col: c1 },
        ))
    }

    fn select_all(&mut self) {
        select_all_cells(self);
    }

    fn clear_selection(&mut self) {
        self.anchor = None;
    }

    fn move_cursor(&mut self, motion: Motion, extend: bool) {
        move_grid(self, motion, extend);
    }

    fn insert_char(&mut self, c: char) {
        self.insert_char_into_cell(c);
    }

    fn insert_str(&mut self, s: &str) {
        self.set_cell_str(s);
    }

    fn delete_back(&mut self) {
        self.clear_cells();
    }

    fn delete_forward(&mut self) {
        self.clear_cells();
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), DocumentError> {
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => self.path.clone().ok_or_else(|| DocumentError::Save {
                path: PathBuf::from("[no name]"),
                message: "no path; use save_as".to_string(),
            })?,
        };
        self.save_to(&target)
    }

    fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(unit) => {
                self.apply_unit(&unit, false);
                self.redo.push(unit);
                true
            }
            None => false,
        }
    }

    fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(unit) => {
                self.apply_unit(&unit, true);
                self.undo.push(unit);
                true
            }
            None => false,
        }
    }

    fn text_projection(&self) -> String {
        let (max_row, max_col) = self.used_range();
        project_tsv(&self.cells, max_row, max_col)
    }
}
