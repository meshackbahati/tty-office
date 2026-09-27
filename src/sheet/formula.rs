//! Formula dialect conversion and the formualizer evaluation mirror.
//!
//! Package formulas arrive in either Excel canonical form (`=A1+1`) or ODS
//! form (`of:=[.A1]+1`). Interactive evaluation always runs in Excel dialect
//! against a single-sheet mirror workbook; conversion back into the package
//! dialect happens only at save time.

use std::collections::HashMap;
use std::collections::HashSet;

use super::cell::{col_letters, Cell, CellValue};

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

/// Re-evaluate every formula cell, used after a fresh load or mirror
/// rebuild where everything is new.
pub(crate) fn reevaluate_all(
    mirror: &mut formualizer::Workbook,
    cells: &mut HashMap<(usize, usize), Cell>,
) {
    let all: Vec<(usize, usize)> = cells
        .iter()
        .filter(|(_, c)| c.formula.is_some())
        .map(|(&k, _)| k)
        .collect();
    reevaluate_formulas(mirror, cells, &all);
}

/// Re-evaluate the formulas affected by edits to `changed` cells: the
/// changed formulas themselves plus their transitive dependents. A full
/// pass costs linear time per edit, which stalls typing on formula-heavy
/// sheets, while the affected set is usually a handful of cells. The
/// engine resolves references from the mirror, so evaluation order
/// within the set does not matter.
pub(crate) fn reevaluate_formulas(
    mirror: &mut formualizer::Workbook,
    cells: &mut HashMap<(usize, usize), Cell>,
    changed: &[(usize, usize)],
) {
    // The precedent map rebuilds per call so it can never drift from the
    // grid; scanning a few thousand formula texts costs microseconds
    // against the millisecond scale of evaluation itself.
    let mut precedents: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    let mut full = false;
    for (&at, cell) in cells.iter() {
        if let Some(formula) = &cell.formula {
            match formula_precedents(formula) {
                Some(deps) => {
                    precedents.insert(at, deps);
                }
                None => {
                    full = true;
                    break;
                }
            }
        }
    }
    let targets: Vec<(usize, usize)> = if full {
        cells
            .iter()
            .filter(|(_, c)| c.formula.is_some())
            .map(|(&k, _)| k)
            .collect()
    } else {
        let mut dependents: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
        for (f, deps) in &precedents {
            for d in deps {
                dependents.entry(*d).or_default().push(*f);
            }
        }
        let mut done: HashSet<(usize, usize)> = HashSet::new();
        let mut targets = Vec::new();
        let mut queue: Vec<(usize, usize)> = changed.to_vec();
        // Changed cells holding formulas evaluate first: their text is new.
        for c in changed {
            if precedents.contains_key(c) && done.insert(*c) {
                targets.push(*c);
            }
        }
        while let Some(cur) = queue.pop() {
            if let Some(nexts) = dependents.get(&cur) {
                for n in nexts {
                    if done.insert(*n) {
                        targets.push(*n);
                        queue.push(*n);
                    }
                }
            }
        }
        targets
    };
    for (row, col) in targets {
        let result = mirror.evaluate_cell(MIRROR_SHEET, row as u32 + 1, col as u32 + 1);
        if let Some(cell) = cells.get_mut(&(row, col)) {
            match result {
                Ok(value) => cell.value = literal_to_cell_value(&value),
                Err(err) => cell.value = CellValue::Error(err.to_string()),
            }
        }
    }
}

/// Shift relative references in a canonical formula by (`drow`, `dcol`)
/// for fill operations: C1 holding `=SUM(A1,B1)` filled one row down
/// becomes `=SUM(A2,B2)`. Absolute markers pin their axis, ranges shift
/// both endpoints, and strings, function names, foreign sheets, and
/// anything unparseable copy through verbatim rather than corrupting.
pub(crate) fn shift_formula_refs(formula: &str, drow: i32, dcol: i32) -> String {
    let (body, prefix) = match formula.strip_prefix('=') {
        Some(rest) => (rest, "="),
        None => (formula, ""),
    };
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len() + 8);
    out.push_str(prefix);
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            out.push(ch);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if ch == '\'' {
            // Quoted qualifier copies through; the reference after a
            // local qualifier still shifts.
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
            let local = chars[start..i.min(chars.len())]
                .iter()
                .collect::<String>()
                .eq_ignore_ascii_case(&format!("'{MIRROR_SHEET}'"));
            out.push_str(&chars[start..i.min(chars.len())].iter().collect::<String>());
            if chars.get(i) == Some(&'!') {
                out.push('!');
                i += 1;
            }
            if !local {
                // Foreign reference: copy the following token verbatim.
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '$' {
            let follows_number = i > 0 && (chars[i - 1].is_ascii_digit() || chars[i - 1] == '.');
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '.' | '$' | '_'))
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if follows_number || chars.get(i) == Some(&'(') || word == "TRUE" || word == "FALSE" {
                out.push_str(&word);
                continue;
            }
            if chars.get(i) == Some(&'!') {
                // Sheet qualifier: shift only after the local sheet.
                out.push_str(&word);
                out.push('!');
                i += 1;
                if word.eq_ignore_ascii_case(MIRROR_SHEET) {
                    continue;
                }
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                    out.push(chars[i]);
                    i += 1;
                }
                continue;
            }
            match shift_single_ref(&word, drow, dcol) {
                Some(shifted) => {
                    out.push_str(&shifted);
                    if chars.get(i) == Some(&':') {
                        out.push(':');
                        i += 1;
                        let second = i;
                        while i < chars.len()
                            && (chars[i].is_ascii_alphanumeric() || chars[i] == '$')
                        {
                            i += 1;
                        }
                        let word2: String = chars[second..i].iter().collect();
                        out.push_str(&shift_single_ref(&word2, drow, dcol).unwrap_or(word2));
                    }
                }
                None => out.push_str(&word),
            }
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Shift one `A1` reference, preserving absolute markers; returns `None`
/// when the token is not a plain reference.
fn shift_single_ref(word: &str, drow: i32, dcol: i32) -> Option<String> {
    let mut chars = word.chars().peekable();
    let col_abs = chars.peek() == Some(&'$');
    if col_abs {
        chars.next();
    }
    let mut letters = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            letters.push(c);
            chars.next();
        } else {
            break;
        }
    }
    let row_abs = chars.peek() == Some(&'$');
    if row_abs {
        chars.next();
    }
    let digits: String = chars.collect();
    if letters.is_empty() || letters.len() > 3 || digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col = 0usize;
    for c in letters.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize) + 1;
    }
    let row: usize = digits.parse().ok()?;
    let col = (col as i32 + if col_abs { 0 } else { dcol }).max(1) as usize;
    let row = (row as i32 + if row_abs { 0 } else { drow }).max(1) as usize;
    let mut out = String::new();
    if col_abs {
        out.push('$');
    }
    out.push_str(&col_letters(col - 1));
    if row_abs {
        out.push('$');
    }
    out.push_str(&row.to_string());
    Some(out)
}

/// Parse one `A1` reference into zero-based `(col, row)`, accepting
/// absolute markers. Returns `None` for anything that is not a plain
/// cell reference.
fn parse_a1(word: &str) -> Option<(usize, usize)> {
    let clean: String = word.chars().filter(|&c| c != '$').collect();
    let split = clean
        .char_indices()
        .find(|(_, c)| c.is_ascii_digit())
        .map(|(i, _)| i)?;
    let (letters, digits) = clean.split_at(split);
    if letters.is_empty() || letters.len() > 3 || digits.is_empty() || digits.len() > 7 {
        return None;
    }
    if !letters.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col = 0usize;
    for c in letters.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize) + 1;
    }
    let row: usize = digits.parse().ok()?;
    Some((col - 1, row - 1))
}

/// Cells a canonical formula reads, for dependent-only recalculation.
/// Returns `None` when the text defeats the scanner (structured
/// references, defined names, foreign sheets in ranges); the caller
/// then falls back to a full pass, so doubt costs time but never
/// correctness through a missed edge.
fn formula_precedents(formula: &str) -> Option<Vec<(usize, usize)>> {
    /// Ranges above this cell count fall back to a full pass rather
    /// than expanding whole columns into the dependent map.
    const RANGE_CAP: usize = 4096;
    let body = formula.strip_prefix('=').unwrap_or(formula);
    let chars: Vec<char> = body.chars().collect();
    let mut deps = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if ch == '\'' {
            // Quoted sheet name: only the local sheet keeps its
            // following reference live; anything else is static.
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            i += 1;
            if chars.get(i) != Some(&'!') {
                return None;
            }
            i += 1;
            if name.eq_ignore_ascii_case(MIRROR_SHEET) {
                continue;
            }
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                i += 1;
            }
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '$' {
            // Identifiers glued to a preceding digit are scientific
            // notation like 1E5, not references.
            let follows_number = i > 0 && (chars[i - 1].is_ascii_digit() || chars[i - 1] == '.');
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '.' | '$' | '_'))
            {
                i += 1;
            }
            if follows_number {
                continue;
            }
            let word: String = chars[start..i].iter().collect();
            if chars.get(i) == Some(&'!') {
                // Sheet-qualified reference: the local sheet keeps its
                // reference live, foreign sheets never change through
                // this grid, and anything unparseable bails out.
                if !word.eq_ignore_ascii_case(MIRROR_SHEET) {
                    i += 1;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                        i += 1;
                    }
                    continue;
                }
                i += 1;
                continue;
            }
            if chars.get(i) == Some(&'(') {
                continue;
            }
            if word == "TRUE" || word == "FALSE" {
                continue;
            }
            let (col, row) = parse_a1(&word)?;
            deps.push((row, col));
            if chars.get(i) == Some(&':') {
                i += 1;
                let second = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                    i += 1;
                }
                let word2: String = chars[second..i].iter().collect();
                let (col2, row2) = parse_a1(&word2)?;
                let (r0, r1) = (row.min(row2), row.max(row2));
                let (c0, c1) = (col.min(col2), col.max(col2));
                if (r1 - r0 + 1) * (c1 - c0 + 1) > RANGE_CAP {
                    return None;
                }
                for r in r0..=r1 {
                    for c in c0..=c1 {
                        if (r, c) != (row, col) {
                            deps.push((r, c));
                        }
                    }
                }
            }
            continue;
        }
        if matches!(ch, '[' | ']' | '{' | '}') {
            return None;
        }
        i += 1;
    }
    Some(deps)
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

    #[test]
    fn precedents_cover_refs_ranges_and_absolutes() {
        assert_eq!(formula_precedents("=A1"), Some(vec![(0, 0)]));
        assert_eq!(formula_precedents("=$B$2"), Some(vec![(1, 1)]));
        assert_eq!(
            formula_precedents("=SUM(A1:A3)"),
            Some(vec![(0, 0), (1, 0), (2, 0)])
        );
        assert_eq!(formula_precedents("=A1+B2"), Some(vec![(0, 0), (1, 1)]));
    }

    #[test]
    fn precedents_skip_strings_functions_and_foreign_sheets() {
        assert_eq!(
            formula_precedents("=IF(A1>5,\"A9\",\"small\")"),
            Some(vec![(0, 0)])
        );
        assert_eq!(formula_precedents("=1E5+A1"), Some(vec![(0, 0)]));
        assert_eq!(formula_precedents("=Other!A1+A2"), Some(vec![(1, 0)]));
        assert_eq!(formula_precedents("=TRUE"), Some(vec![]));
    }

    #[test]
    fn precedents_bail_to_full_on_exotic_forms() {
        assert_eq!(formula_precedents("=MyName+1"), None);
        assert_eq!(formula_precedents("=SUM(A:A)"), None);
        assert_eq!(formula_precedents("=Table1[Col]+1"), None);
    }

    #[test]
    fn shift_moves_relative_refs_per_row_and_column() {
        // The reported case: C1 holding =SUM(A1,B1) filled one row down.
        assert_eq!(shift_formula_refs("=SUM(A1,B1)", 1, 0), "=SUM(A2,B2)");
        assert_eq!(
            shift_formula_refs("=$A$1+$B2+C$3+D4", 1, 1),
            "=$A$1+$B3+D$3+E5"
        );
    }

    #[test]
    fn shift_copies_strings_foreign_sheets_and_names_verbatim() {
        assert_eq!(
            shift_formula_refs("=IF(A1>5,\"A9\",\"x\")", 2, 0),
            "=IF(A3>5,\"A9\",\"x\")"
        );
        assert_eq!(shift_formula_refs("=Other!A1+A2", 1, 0), "=Other!A1+A3");
        assert_eq!(shift_formula_refs("=1E5+A1", 1, 1), "=1E5+B2");
        assert_eq!(shift_formula_refs("A1+1", 1, 0), "A2+1");
    }

    #[test]
    fn recalc_propagates_through_chains() {
        let mut mirror = new_mirror();
        let mut cells: HashMap<(usize, usize), Cell> = HashMap::new();
        cells.insert((0, 0), Cell::from_value(CellValue::Number(1.0)));
        cells.insert((0, 1), Cell::from_formula("=A1*2".to_string(), None));
        cells.insert((0, 2), Cell::from_formula("=B1*2".to_string(), None));
        push_grid_to_mirror(&mut mirror, &cells);
        reevaluate_all(&mut mirror, &mut cells);
        assert_eq!(cells[&(0, 2)].value.display(), "4");

        cells.insert((0, 0), Cell::from_value(CellValue::Number(5.0)));
        sync_cell_to_mirror(&mut mirror, 0, 0, cells.get(&(0, 0)));
        reevaluate_formulas(&mut mirror, &mut cells, &[(0, 0)]);
        assert_eq!(cells[&(0, 1)].value.display(), "10");
        assert_eq!(cells[&(0, 2)].value.display(), "20");
    }
}
