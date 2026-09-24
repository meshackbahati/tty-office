//! Spreadsheet cell model: format, values, and CellEdit parsing.
//!
//! The cell is the unit of grid editing: a cached value plus an optional
//! Excel-canonical formula. Native package formula text is retained until
//! the user edits the cell so an untouched formula round-trips byte-stable.

use std::path::Path;

use super::formula::ods_formula_to_excel;

/// On-disk package kind for a spreadsheet document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetFormat {
    /// Office Open XML spreadsheet (`.xlsx`), read and written.
    Xlsx,
    /// OpenDocument spreadsheet (`.ods`), read and written.
    Ods,
    /// Legacy binary workbook (`.xls`), read-only; save refuses with a
    /// message directing the user to xlsx or ods.
    Xls,
}

impl SheetFormat {
    /// Format implied by a path extension, if any.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "xlsx" => Some(Self::Xlsx),
            "ods" => Some(Self::Ods),
            "xls" => Some(Self::Xls),
            _ => None,
        }
    }
}

/// Value held by a grid cell.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// No content; the key should normally be absent from the grid.
    Empty,
    /// Text content.
    Text(String),
    /// Numeric content.
    Number(f64),
    /// Boolean content.
    Bool(bool),
    /// Evaluation or package error, rendered like `#DIV/0!`.
    Error(String),
}

impl CellValue {
    /// Display string used by the grid pane and text projection.
    pub fn display(&self) -> String {
        match self {
            CellValue::Empty => String::new(),
            CellValue::Text(s) => s.clone(),
            CellValue::Number(n) => format_number(*n),
            CellValue::Bool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
            CellValue::Error(e) => e.clone(),
        }
    }

    /// Whether this value has no user-visible content.
    pub fn is_empty(&self) -> bool {
        match self {
            CellValue::Empty => true,
            CellValue::Text(s) => s.is_empty(),
            CellValue::Number(_) | CellValue::Bool(_) | CellValue::Error(_) => false,
        }
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// One grid cell: cached value plus optional formula.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Cached display value; for formula cells this is the last evaluation.
    pub value: CellValue,
    /// Excel-canonical formula including the leading `=`, when this cell is a
    /// formula.
    pub formula: Option<String>,
    /// Byte-stable package-original formula text, retained until the user
    /// edits this cell so an untouched formula round-trips exactly. Dialect
    /// matches the package this document was loaded from.
    pub native_formula: Option<String>,
}

impl Cell {
    /// Build a non-formula cell holding `value`.
    pub(crate) fn from_value(value: CellValue) -> Self {
        Self {
            value,
            formula: None,
            native_formula: None,
        }
    }

    /// Build a formula cell with the given Excel-canonical formula and an
    /// empty cached value pending evaluation.
    pub(crate) fn from_formula(formula: String, native: Option<String>) -> Self {
        Self {
            value: CellValue::Empty,
            formula: Some(formula),
            native_formula: native,
        }
    }

    /// Whether the cell holds no content at all.
    pub(crate) fn is_vacant(&self) -> bool {
        self.formula.is_none() && self.value.is_empty()
    }

    /// Edit prefill: the formula when present, otherwise the value text.
    pub(crate) fn edit_content(&self) -> String {
        match &self.formula {
            Some(f) => f.clone(),
            None => self.value.display(),
        }
    }
}

/// Parse CellEdit input into an optional cell; empty input clears the cell.
pub(crate) fn parse_cell_input(input: &str) -> Option<Cell> {
    if input.is_empty() {
        return None;
    }
    if let Some(rest) = input.strip_prefix('=') {
        if rest.is_empty() {
            return None;
        }
        return Some(Cell::from_formula(format!("={rest}"), None));
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        return Some(Cell::from_value(CellValue::Number(n)));
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Some(Cell::from_value(CellValue::Bool(true)));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Some(Cell::from_value(CellValue::Bool(false)));
    }
    Some(Cell::from_value(CellValue::Text(input.to_string())))
}

/// Column index to Excel letters: `0` → `A`, `26` → `AA`.
pub(crate) fn col_letters(mut col: usize) -> String {
    let mut out = String::new();
    loop {
        let rem = col % 26;
        out.insert(0, (b'A' + rem as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    out
}

/// Adopt an ODS cell value and formula into the shared cell model.
pub(crate) fn cell_from_ods(value: super::CellValue, native: Option<String>) -> Option<Cell> {
    let cell = match native {
        Some(native) => {
            let canonical = ods_formula_to_excel(&native);
            let mut c = Cell::from_formula(canonical, Some(native));
            c.value = value;
            c
        }
        None => Cell::from_value(value),
    };
    if cell.is_vacant() {
        None
    } else {
        Some(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_letters_sequences() {
        assert_eq!(col_letters(0), "A");
        assert_eq!(col_letters(25), "Z");
        assert_eq!(col_letters(26), "AA");
        assert_eq!(col_letters(27), "AB");
    }

    #[test]
    fn parse_cell_input_variants() {
        assert_eq!(parse_cell_input(""), None);
        assert_eq!(
            parse_cell_input("=A1+1").map(|c| c.formula),
            Some(Some("=A1+1".to_string()))
        );
        assert_eq!(
            parse_cell_input("42").map(|c| c.value),
            Some(CellValue::Number(42.0))
        );
        assert_eq!(
            parse_cell_input("hi").map(|c| c.value),
            Some(CellValue::Text("hi".to_string()))
        );
        assert_eq!(
            parse_cell_input("TRUE").map(|c| c.value),
            Some(CellValue::Bool(true))
        );
    }
}
