//! Grid editing: cell edits, selection, undo units, and Editor motion.
//!
//! Edits are applied to the sparse projection first, then pushed into the
//! evaluation mirror, then all formula cells are re-evaluated so dependent
//! results stay consistent. Each user-visible change is one undo unit.

use std::collections::HashMap;

use super::cell::{col_letters, parse_cell_input, Cell, SheetFormat};
use super::document::SheetDocument;
use super::formula::{push_grid_to_mirror, reevaluate_formulas, sync_cell_to_mirror};
use super::PAGE_ROWS;

/// One reversible cell replacement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SheetEdit {
    row: usize,
    col: usize,
    before: Option<Cell>,
    after: Option<Cell>,
}

impl SheetDocument {
    /// Edit prefill for the cell under the cursor: formula or value text.
    pub fn cell_edit_content(&self) -> String {
        self.cells
            .get(&(self.cursor_row, self.cursor_col))
            .map(Cell::edit_content)
            .unwrap_or_default()
    }

    /// Commit one CellEdit prompt: parse `input`, replace the cell under the
    /// cursor as a single undo unit, refresh the evaluation mirror, and
    /// re-evaluate dependent formulas.
    pub fn set_cell_content(&mut self, input: &str) {
        let trimmed = input.trim_end_matches('\n');
        let after = parse_cell_input(trimmed);
        let (row, col) = (self.cursor_row, self.cursor_col);
        let before = self.cells.get(&(row, col)).cloned();
        if before == after {
            return;
        }
        match &after {
            Some(cell) => {
                self.cells.insert((row, col), cell.clone());
            }
            None => {
                self.cells.remove(&(row, col));
            }
        }
        self.push_unit(vec![SheetEdit {
            row,
            col,
            before,
            after: after.clone(),
        }]);
        self.dirty = true;
        sync_cell_to_mirror(&mut self.mirror, row, col, after.as_ref());
        reevaluate_formulas(&mut self.mirror, &mut self.cells);
    }

    /// Clear the cell under the cursor (or every cell in the selection) as a
    /// single undo unit.
    pub fn clear_cells(&mut self) {
        let targets: Vec<(usize, usize)> = match self.selection_rect() {
            Some(((r0, c0), (r1, c1))) => {
                let mut out = Vec::new();
                for r in r0..=r1 {
                    for c in c0..=c1 {
                        if self.cells.contains_key(&(r, c)) {
                            out.push((r, c));
                        }
                    }
                }
                out
            }
            None => {
                let pos = (self.cursor_row, self.cursor_col);
                if self.cells.contains_key(&pos) {
                    vec![pos]
                } else {
                    Vec::new()
                }
            }
        };
        if targets.is_empty() {
            return;
        }
        let mut unit = Vec::with_capacity(targets.len());
        for (row, col) in targets {
            let before = self.cells.remove(&(row, col));
            if before.is_none() {
                continue;
            }
            sync_cell_to_mirror(&mut self.mirror, row, col, None);
            unit.push(SheetEdit {
                row,
                col,
                before,
                after: None,
            });
        }
        if unit.is_empty() {
            return;
        }
        self.push_unit(unit);
        self.dirty = true;
        reevaluate_formulas(&mut self.mirror, &mut self.cells);
    }

    /// Append `c` to the cell under the cursor as one undo unit. The UI
    /// normally routes typing through the CellEdit prompt; this exists so
    /// [`crate::Editor::insert_char`] has a coherent meaning on grids.
    pub fn insert_char_into_cell(&mut self, c: char) {
        let mut content = self.cell_edit_content();
        content.push(c);
        self.set_cell_content(&content);
    }

    /// Replace the cell under the cursor with `s` as one undo unit.
    pub fn set_cell_str(&mut self, s: &str) {
        self.set_cell_content(s);
    }

    pub(crate) fn push_unit(&mut self, unit: Vec<SheetEdit>) {
        if unit.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(unit);
        if self.undo.len() > super::HISTORY_CAP {
            let excess = self.undo.len() - super::HISTORY_CAP;
            self.undo.drain(0..excess);
        }
    }

    pub(crate) fn apply_unit(&mut self, unit: &[SheetEdit], forward: bool) {
        for edit in unit {
            let cell = if forward {
                edit.after.clone()
            } else {
                edit.before.clone()
            };
            match &cell {
                Some(c) => {
                    self.cells.insert((edit.row, edit.col), c.clone());
                }
                None => {
                    self.cells.remove(&(edit.row, edit.col));
                }
            }
            sync_cell_to_mirror(&mut self.mirror, edit.row, edit.col, cell.as_ref());
        }
        self.dirty = true;
        reevaluate_formulas(&mut self.mirror, &mut self.cells);
    }

    pub(crate) fn rebuild_mirror(&mut self) {
        self.mirror = super::formula::new_mirror();
        push_grid_to_mirror(&mut self.mirror, &self.cells);
        reevaluate_formulas(&mut self.mirror, &mut self.cells);
    }
}

/// Apply a directional motion to the grid cursor.
pub(crate) fn move_grid(doc: &mut SheetDocument, motion: crate::editor::Motion, extend: bool) {
    use crate::editor::Motion;
    if extend {
        if doc.anchor.is_none() {
            doc.anchor = Some((doc.cursor_row, doc.cursor_col));
        }
    } else {
        // Collapsing a non-empty selection in the direction of travel is
        // handled per-motion below for Left and Right; otherwise clear.
        if !matches!(motion, Motion::Left | Motion::Right) {
            doc.anchor = None;
        }
    }
    match motion {
        Motion::Left => {
            if !extend {
                if let Some(((r0, c0), _)) = doc.selection_rect() {
                    if (r0, c0) != (doc.cursor_row, doc.cursor_col) {
                        doc.cursor_row = r0;
                        doc.cursor_col = c0;
                        doc.anchor = None;
                        return;
                    }
                }
            }
            if doc.cursor_col > 0 {
                doc.cursor_col -= 1;
            }
        }
        Motion::Right => {
            if !extend {
                if let Some((_, (r1, c1))) = doc.selection_rect() {
                    if (r1, c1) != (doc.cursor_row, doc.cursor_col) {
                        doc.cursor_row = r1;
                        doc.cursor_col = c1;
                        doc.anchor = None;
                        return;
                    }
                }
            }
            doc.cursor_col += 1;
        }
        Motion::Up => {
            doc.cursor_row = doc.cursor_row.saturating_sub(1);
        }
        Motion::Down => {
            doc.cursor_row += 1;
        }
        Motion::LineStart => {
            doc.cursor_col = 0;
        }
        Motion::LineEnd => {
            let row = doc.cursor_row;
            let mut last = 0usize;
            for (&(r, c), cell) in &doc.cells {
                if r == row && !cell.is_vacant() {
                    last = last.max(c);
                }
            }
            doc.cursor_col = last;
        }
        Motion::PageUp => {
            doc.cursor_row = doc.cursor_row.saturating_sub(PAGE_ROWS);
        }
        Motion::PageDown => {
            doc.cursor_row += PAGE_ROWS;
        }
        Motion::BufferStart => {
            doc.cursor_row = 0;
            doc.cursor_col = 0;
        }
        Motion::BufferEnd => {
            let (max_row, max_col) = doc.used_range();
            doc.cursor_row = max_row;
            doc.cursor_col = max_col;
        }
    }
}

/// Build the TSV text projection of the used range.
pub(crate) fn project_tsv(
    cells: &HashMap<(usize, usize), Cell>,
    max_row: usize,
    max_col: usize,
) -> String {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..=max_row {
        let mut cols: Vec<String> = Vec::with_capacity(max_col + 1);
        for c in 0..=max_col {
            cols.push(
                cells
                    .get(&(r, c))
                    .map(|cell| cell.value.display())
                    .unwrap_or_default(),
            );
        }
        rows.push(cols.join("\t"));
    }
    // An empty grid projects to an empty string rather than one blank line.
    if rows.len() == 1 && rows[0].is_empty() && cells.is_empty() {
        return String::new();
    }
    rows.join("\n")
}

/// Clamp scroll offsets for a viewport of `view_h` × `view_w` characters.
pub(crate) fn scroll_offsets(
    cursor: (usize, usize),
    rowoff: &mut usize,
    coloff: &mut usize,
    view_h: usize,
    view_w: usize,
) {
    if view_h == 0 || view_w == 0 {
        return;
    }
    // One row is the column-letter header; one gutter is the row numbers.
    let visible_rows = view_h.saturating_sub(super::COL_HEADER_H).max(1);
    let visible_cols = view_w
        .saturating_sub(super::ROW_GUTTER)
        .div_ceil(super::CELL_WIDTH)
        .max(1);
    if cursor.0 < *rowoff {
        *rowoff = cursor.0;
    } else if cursor.0 >= *rowoff + visible_rows {
        *rowoff = cursor.0 + 1 - visible_rows;
    }
    if cursor.1 < *coloff {
        *coloff = cursor.1;
    } else if cursor.1 >= *coloff + visible_cols {
        *coloff = cursor.1 + 1 - visible_cols;
    }
}

/// Inclusive used-range maximum over non-vacant cells.
pub(crate) fn used_range(cells: &HashMap<(usize, usize), Cell>) -> (usize, usize) {
    let mut max_row = 0usize;
    let mut max_col = 0usize;
    for (&(r, c), cell) in cells {
        if cell.is_vacant() {
            continue;
        }
        max_row = max_row.max(r);
        max_col = max_col.max(c);
    }
    (max_row, max_col)
}

/// A1-style reference for `(row, col)`, e.g. `(0, 0)` is `A1`.
pub(crate) fn a1_ref(row: usize, col: usize) -> String {
    format!("{}{}", col_letters(col), row + 1)
}

/// Select the used range from (0,0) through the last used cell.
pub(crate) fn select_all_cells(doc: &mut SheetDocument) {
    let (max_row, max_col) = used_range(&doc.cells);
    doc.anchor = Some((0, 0));
    doc.cursor_row = max_row;
    doc.cursor_col = max_col;
}

/// Rebuild the package for `format` and drop natives produced for another
/// dialect, used when Save As changes the on-disk format.
pub(crate) fn prepare_rebuild(doc: &mut SheetDocument, target_format: SheetFormat) {
    doc.package = super::package::new_package(target_format);
    doc.format = target_format;
    for cell in doc.cells.values_mut() {
        cell.native_formula = None;
    }
}
