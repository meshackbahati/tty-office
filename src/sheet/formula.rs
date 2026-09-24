//! Formula dialect conversion and the formualizer evaluation mirror.
//!
//! Package formulas arrive in either Excel canonical form (`=A1+1`) or ODS
//! form (`of:=[.A1]+1`). Interactive evaluation always runs in Excel dialect
//! against a single-sheet mirror workbook; conversion back into the package
//! dialect happens only at save time.

use std::collections::HashMap;

use super::cell::{Cell, CellValue};

/// Fixed mirror sheet name inside the `formualizer` evaluation workbook.
pub(crate) const MIRROR_SHEET: &str = "Sheet";

/// Create an empty evaluation mirror with [`MIRROR_SHEET`] present.
pub(crate) fn new_mirror() -> formualizer::Workbook {
    let mut workbook = formualizer::Workbook::new();
    if !workbook.has_sheet(MIRROR_SHEET) {
        let _ = workbook.add_sheet(MIRROR_SHEET);
    }
    workbook
}

/// Load every non-vacant grid cell into the mirror.
pub(crate) fn push_grid_to_mirror(
    mirror: &mut formualizer::Workbook,
    cells: &HashMap<(usize, usize), Cell>,
) {
    for (&(row, col), cell) in cells {
        let r = row as u32 + 1;
        let c = col as u32 + 1;
        if let Some(formula) = &cell.formula {
            let _ = mirror.set_formula(MIRROR_SHEET, r, c, formula);
        } else if !cell.value.is_empty() {
            let literal = cell_value_to_literal(&cell.value);
            let _ = mirror.set_value(MIRROR_SHEET, r, c, literal);
        }
    }
}

/// Write one grid cell (or its absence) into the mirror.
pub(crate) fn sync_cell_to_mirror(
    mirror: &mut formualizer::Workbook,
    row: usize,
    col: usize,
    cell: Option<&Cell>,
) {
    let r = row as u32 + 1;
    let c = col as u32 + 1;
    match cell {
        Some(cell) => {
            if let Some(formula) = &cell.formula {
                let _ = mirror.set_formula(MIRROR_SHEET, r, c, formula);
            } else {
                let literal = cell_value_to_literal(&cell.value);
                let _ = mirror.set_value(MIRROR_SHEET, r, c, literal);
            }
        }
        None => {
            let _ = mirror.set_value(MIRROR_SHEET, r, c, formualizer::LiteralValue::Empty);
        }
    }
}

/// Re-evaluate every formula cell and cache the result in the grid.
pub(crate) fn reevaluate_formulas(
    mirror: &mut formualizer::Workbook,
    cells: &mut HashMap<(usize, usize), Cell>,
) {
    let formula_cells: Vec<(usize, usize)> = cells
        .iter()
        .filter(|(_, c)| c.formula.is_some())
        .map(|(&k, _)| k)
        .collect();
    for (row, col) in formula_cells {
        let result = mirror.evaluate_cell(MIRROR_SHEET, row as u32 + 1, col as u32 + 1);
        if let Some(cell) = cells.get_mut(&(row, col)) {
            match result {
                Ok(value) => cell.value = literal_to_cell_value(&value),
                Err(err) => cell.value = CellValue::Error(err.to_string()),
            }
        }
    }
}

/// Convert an ODS formula (`of:=[.A1]+1`) to Excel canonical (`=A1+1`).
///
/// Bracketed local references and the list separator are rewritten; function
/// names and other tokens pass through unchanged, which is an accepted
/// limitation for dialects that rename functions.
pub(crate) fn ods_formula_to_excel(ods: &str) -> String {
    let body = ods.strip_prefix("of:=").unwrap_or(ods);
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len() + 1);
    out.push('=');
    let mut i = 0usize;
    let mut in_str = false;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            in_str = !in_str;
            out.push(ch);
            i += 1;
            continue;
        }
        if !in_str && ch == '[' && chars.get(i + 1) == Some(&'.') {
            if let Some(offset) = chars[i..].iter().position(|&c| c == ']') {
                let inner: String = chars[i + 2..i + offset].iter().collect();
                out.push_str(&inner.replace(":.", ":"));
                i += offset + 1;
                continue;
            }
        }
        if !in_str && ch == ';' {
            out.push(',');
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

/// Convert an Excel canonical formula (`=A1+1`) to ODS (`of:=[.A1]+1`).
pub(crate) fn excel_formula_to_ods(excel: &str) -> String {
    let body = excel.strip_prefix('=').unwrap_or(excel);
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len() + 5);
    out.push_str("of:=");
    let mut i = 0usize;
    let mut in_str = false;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            in_str = !in_str;
            out.push(ch);
            i += 1;
            continue;
        }
        if !in_str && ch == ',' {
            out.push(';');
            i += 1;
            continue;
        }
        if !in_str && (ch == '$' || ch.is_ascii_alphabetic()) {
            if let Some(len) = match_excel_ref(&chars, i) {
                // A following `(` marks a function name such as LOG10, not a cell.
                if chars.get(i + len) != Some(&'(') {
                    // A colon followed by another reference forms an ODS range
                    // (`[.A1:.A10]`), which wraps both endpoints in one bracket.
                    let after = i + len;
                    if chars.get(after) == Some(&':') {
                        let rhs_start = after + 1;
                        if let Some(rhs_len) = match_excel_ref(&chars, rhs_start) {
                            out.push_str("[.");
                            out.extend(&chars[i..after]);
                            out.push_str(":.");
                            out.extend(&chars[rhs_start..rhs_start + rhs_len]);
                            out.push(']');
                            i = rhs_start + rhs_len;
                            continue;
                        }
                    }
                    out.push_str("[.");
                    out.extend(&chars[i..i + len]);
                    out.push(']');
                    i += len;
                    continue;
                }
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Match an Excel A1 reference starting at `start`; returns its length.
fn match_excel_ref(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    if chars.get(i) == Some(&'$') {
        i += 1;
    }
    let col_start = i;
    while i < chars.len() && chars[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == col_start {
        return None;
    }
    if chars.get(i) == Some(&'$') {
        i += 1;
    }
    let row_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == row_start {
        return None;
    }
    if let Some(&c) = chars.get(i) {
        if c.is_ascii_alphanumeric() {
            return None;
        }
    }
    Some(i - start)
}

pub(crate) fn cell_value_to_literal(value: &CellValue) -> formualizer::LiteralValue {
    match value {
        CellValue::Empty => formualizer::LiteralValue::Empty,
        CellValue::Text(s) => formualizer::LiteralValue::Text(s.clone()),
        CellValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                formualizer::LiteralValue::Int(*n as i64)
            } else {
                formualizer::LiteralValue::Number(*n)
            }
        }
        CellValue::Bool(b) => formualizer::LiteralValue::Boolean(*b),
        CellValue::Error(e) => formualizer::LiteralValue::Text(e.clone()),
    }
}

pub(crate) fn literal_to_cell_value(value: &formualizer::LiteralValue) -> CellValue {
    use formualizer::LiteralValue;
    match value {
        LiteralValue::Int(i) => CellValue::Number(*i as f64),
        LiteralValue::Number(n) => CellValue::Number(*n),
        LiteralValue::Text(s) => CellValue::Text(s.clone()),
        LiteralValue::Boolean(b) => CellValue::Bool(*b),
        LiteralValue::Empty | LiteralValue::Pending => CellValue::Empty,
        LiteralValue::Error(e) => CellValue::Error(e.to_string()),
        LiteralValue::Array(rows) => {
            let first = rows.first().and_then(|r| r.first());
            match first {
                Some(inner) => literal_to_cell_value(inner),
                None => CellValue::Empty,
            }
        }
        LiteralValue::Date(d) => CellValue::Text(d.to_string()),
        LiteralValue::DateTime(dt) => CellValue::Text(dt.to_string()),
        LiteralValue::Time(t) => CellValue::Text(t.to_string()),
        LiteralValue::Duration(d) => CellValue::Text(d.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ods_to_excel_basic() {
        assert_eq!(ods_formula_to_excel("of:=[.A1]+1"), "=A1+1");
        assert_eq!(ods_formula_to_excel("of:=[.$A$1]+1"), "=$A$1+1");
        assert_eq!(ods_formula_to_excel("of:=SUM([.A1:.A10])"), "=SUM(A1:A10)");
        assert_eq!(ods_formula_to_excel("of:=IF([.A1]>1;2;3)"), "=IF(A1>1,2,3)");
    }

    #[test]
    fn excel_to_ods_basic() {
        assert_eq!(excel_formula_to_ods("=A1+1"), "of:=[.A1]+1");
        assert_eq!(excel_formula_to_ods("=$A$1+1"), "of:=[.$A$1]+1");
        assert_eq!(excel_formula_to_ods("=SUM(A1:A10)"), "of:=SUM([.A1:.A10])");
        assert_eq!(excel_formula_to_ods("=IF(A1>1,2,3)"), "of:=IF([.A1]>1;2;3)");
        // Function names that look like references stay unwrapped.
        assert_eq!(excel_formula_to_ods("=LOG10(100)"), "of:=LOG10(100)");
    }

    #[test]
    fn formula_roundtrip_preserves_refs() {
        let original = "=SUM(A1:B2,C3)*1.5";
        let ods = excel_formula_to_ods(original);
        assert_eq!(ods_formula_to_excel(&ods), original);
    }
}
