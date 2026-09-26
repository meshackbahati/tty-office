//! Comma-separated values: a small strict parser and writer.
//!
//! The grid is the sole storage for CSV, like read-only XLS; quoting
//! follows the common rule (commas, quotes, and line breaks quote the
//! field, inner quotes double). Formulas travel as their `=` text and
//! re-evaluate on open.

use std::collections::HashMap;

use super::cell::{parse_cell_input, Cell};
use super::edit::used_range;

/// Parse CSV text into rows of fields, handling quoted fields, doubled
/// quotes, commas inside quotes, and CRLF line endings.
pub(crate) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    field.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            field.push(ch);
            i += 1;
            continue;
        }
        match ch {
            '"' if field.is_empty() => {
                in_quotes = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
            }
            '\r' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
        i += 1;
    }
    if in_quotes || !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Quote one field when it needs quoting.
fn quote_field(field: &str) -> String {
    let needs = field.chars().any(|c| matches!(c, ',' | '"' | '\n' | '\r'))
        || field.starts_with(' ')
        || field.ends_with(' ');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Serialize grid rows into CSV text with a trailing newline.
pub(crate) fn write_csv(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out: String = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|f| quote_field(f))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

/// Project parsed rows into the sparse grid, skipping vacant fields so
/// blank cells stay absent rather than stored empties.
pub(crate) fn cells_from_rows(rows: Vec<Vec<String>>) -> HashMap<(usize, usize), Cell> {
    let mut cells = HashMap::new();
    for (r, row) in rows.into_iter().enumerate() {
        for (c, field) in row.into_iter().enumerate() {
            if let Some(cell) = parse_cell_input(&field) {
                cells.insert((r, c), cell);
            }
        }
    }
    cells
}

/// Serialize the grid to CSV text: formulas travel as their `=` text so
/// they survive the round trip, and other cells write their display.
pub(crate) fn grid_to_csv(cells: &HashMap<(usize, usize), Cell>) -> String {
    let (max_row, max_col) = used_range(cells);
    if cells.values().all(|c| c.is_vacant()) {
        return String::new();
    }
    let rows: Vec<Vec<String>> = (0..=max_row)
        .map(|r| {
            (0..=max_col)
                .map(|c| match cells.get(&(r, c)) {
                    Some(cell) => cell.formula.clone().unwrap_or_else(|| cell.value.display()),
                    None => String::new(),
                })
                .collect()
        })
        .collect();
    write_csv(&rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_rows() {
        assert_eq!(
            parse_csv("a,b,c\n1,2,3\n"),
            vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["1".to_string(), "2".to_string(), "3".to_string()],
            ]
        );
    }

    #[test]
    fn parses_quoted_commas_quotes_and_crlf() {
        assert_eq!(
            parse_csv("a,\"b,c\",\"d\"\"e\"\r\nx,,\n"),
            vec![
                vec!["a".to_string(), "b,c".to_string(), "d\"e".to_string()],
                vec!["x".to_string(), String::new(), String::new()],
            ]
        );
    }

    #[test]
    fn write_quotes_only_when_needed() {
        let rows = vec![vec!["plain".to_string(), "has, comma".to_string()]];
        assert_eq!(write_csv(&rows), "plain,\"has, comma\"\n");
    }

    #[test]
    fn write_read_round_trip() {
        let rows = vec![
            vec!["a,b".to_string(), "q\"q".to_string()],
            vec!["line\nbreak".to_string(), "tail ".to_string()],
        ];
        assert_eq!(parse_csv(&write_csv(&rows)), rows);
    }
}
