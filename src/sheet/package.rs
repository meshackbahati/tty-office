//! Package load and save: umya (xlsx), spreadsheet-ods, calamine (xls).
//!
//! The package model is the source of truth for structure. On load, sheet 0
//! is projected into the sparse grid; on save the grid is written back into
//! sheet 0 while other sheets are left untouched. `xls` is read-only: the
//! grid is the sole storage after load.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::cell::{cell_from_ods, Cell, CellValue, SheetFormat};
use super::formula::excel_formula_to_ods;
use crate::error::DocumentError;

/// Sparse grid keyed by `(row, col)` zero-based coordinates.
type CellGrid = HashMap<(usize, usize), Cell>;

/// Package backend. Kept private so umya and spreadsheet-ods types never
/// appear in the public API.
pub(crate) enum Package {
    Xlsx(umya_spreadsheet::Workbook),
    Ods(spreadsheet_ods::WorkBook),
    /// Read-only binary marker; the grid is the sole storage after load.
    Xls,
}

pub(crate) fn new_package(format: SheetFormat) -> Package {
    match format {
        SheetFormat::Xlsx => Package::Xlsx(umya_spreadsheet::new_file()),
        SheetFormat::Ods => {
            let mut book = spreadsheet_ods::WorkBook::new_empty();
            book.push_sheet(spreadsheet_ods::Sheet::new("Sheet1"));
            Package::Ods(book)
        }
        SheetFormat::Xls => Package::Xls,
    }
}

pub(crate) fn package_matches(package: &Package, format: SheetFormat) -> bool {
    matches!(
        (package, format),
        (Package::Xlsx(_), SheetFormat::Xlsx)
            | (Package::Ods(_), SheetFormat::Ods)
            | (Package::Xls, SheetFormat::Xls)
    )
}

pub(crate) fn load_package(
    path: &Path,
    format: SheetFormat,
) -> Result<(Package, CellGrid), DocumentError> {
    match format {
        SheetFormat::Xlsx => {
            let book = umya_spreadsheet::reader::xlsx::read(path)
                .map_err(|err| DocumentError::Parse(err.to_string()))?;
            let cells = project_umya_sheet0(&book);
            Ok((Package::Xlsx(book), cells))
        }
        SheetFormat::Ods => {
            let book = spreadsheet_ods::read_ods(path)
                .map_err(|err| DocumentError::Parse(err.to_string()))?;
            let cells = project_ods_sheet0(&book);
            Ok((Package::Ods(book), cells))
        }
        SheetFormat::Xls => {
            let cells = load_xls_grid(path)?;
            Ok((Package::Xls, cells))
        }
    }
}

fn project_umya_sheet0(book: &umya_spreadsheet::Workbook) -> CellGrid {
    let mut cells = HashMap::new();
    let Ok(sheet) = book.sheet(0) else {
        return cells;
    };
    for (&(row1, col1), cell) in sheet.collection_to_hashmap() {
        // Umya map keys are one-based `(row, col)`.
        let key = ((row1 - 1) as usize, (col1 - 1) as usize);
        let value = umya_cell_value(cell);
        let cell = if cell.is_formula() {
            let raw = cell.formula();
            let canonical = if raw.starts_with('=') {
                raw.to_string()
            } else {
                format!("={raw}")
            };
            // Native stores the package form (no leading `=`).
            let native = raw.trim_start_matches('=').to_string();
            let mut c = Cell::from_formula(canonical, Some(native));
            c.value = value;
            c
        } else {
            Cell::from_value(value)
        };
        if !cell.is_vacant() {
            cells.insert(key, cell);
        }
    }
    cells
}

fn umya_cell_value(cell: &umya_spreadsheet::Cell) -> CellValue {
    use umya_spreadsheet::CellRawValue;
    match cell.raw_value() {
        CellRawValue::Empty => CellValue::Empty,
        CellRawValue::String(s) => CellValue::Text(s.to_string()),
        CellRawValue::RichText(rt) => CellValue::Text(rt.text().into_owned()),
        CellRawValue::Lazy(s) => CellValue::Text(s.to_string()),
        CellRawValue::Numeric(n) => CellValue::Number(*n),
        CellRawValue::Bool(b) => CellValue::Bool(*b),
        CellRawValue::Error(e) => CellValue::Error(e.to_string()),
    }
}

fn project_ods_sheet0(book: &spreadsheet_ods::WorkBook) -> CellGrid {
    use spreadsheet_ods::Value;
    let mut cells = HashMap::new();
    if book.num_sheets() == 0 {
        return cells;
    }
    let sheet = book.sheet(0);
    for ((row, col), content) in sheet.iter() {
        let value = match content.value() {
            Value::Empty => CellValue::Empty,
            Value::Boolean(b) => CellValue::Bool(*b),
            Value::Number(n) => CellValue::Number(*n),
            Value::Percentage(p) => CellValue::Number(*p),
            Value::Currency(c, _) => CellValue::Number(*c),
            Value::Text(s) => CellValue::Text(s.clone()),
            Value::TextXml(_) => CellValue::Text(String::new()),
            Value::DateTime(dt) => CellValue::Text(dt.to_string()),
            Value::TimeDuration(d) => CellValue::Text(d.to_string()),
        };
        if let Some(cell) = cell_from_ods(value, content.formula().cloned()) {
            cells.insert((row as usize, col as usize), cell);
        }
    }
    cells
}

fn load_xls_grid(path: &Path) -> Result<CellGrid, DocumentError> {
    use calamine::{open_workbook, Data, Reader, Xls};

    let mut book: Xls<_> =
        open_workbook::<Xls<_>, _>(path).map_err(|err| DocumentError::Parse(err.to_string()))?;
    let name = book
        .sheet_names()
        .into_iter()
        .next()
        .ok_or_else(|| DocumentError::Parse("workbook has no sheets".to_string()))?;

    let mut cells = CellGrid::new();
    let range = book
        .worksheet_range(&name)
        .map_err(|err| DocumentError::Parse(err.to_string()))?;
    let start = range.start().unwrap_or((0, 0));
    for (dr, dc, data) in range.cells() {
        let row = start.0 as usize + dr;
        let col = start.1 as usize + dc;
        let value = match data {
            Data::Empty => continue,
            Data::Int(i) => CellValue::Number(*i as f64),
            Data::Float(f) => CellValue::Number(*f),
            Data::String(s) => CellValue::Text(s.clone()),
            Data::Bool(b) => CellValue::Bool(*b),
            Data::DateTime(dt) => CellValue::Text(dt.to_string()),
            Data::DateTimeIso(s) => CellValue::Text(s.clone()),
            Data::DurationIso(s) => CellValue::Text(s.clone()),
            Data::Error(e) => CellValue::Error(e.to_string()),
        };
        cells.insert((row, col), Cell::from_value(value));
    }

    let formulas = book
        .worksheet_formula(&name)
        .map_err(|err| DocumentError::Parse(err.to_string()))?;
    let fstart = formulas.start().unwrap_or((0, 0));
    for (dr, dc, formula) in formulas.cells() {
        if formula.is_empty() {
            continue;
        }
        let row = fstart.0 as usize + dr;
        let col = fstart.1 as usize + dc;
        let canonical = if formula.starts_with('=') {
            formula.clone()
        } else {
            format!("={formula}")
        };
        let entry = cells.entry((row, col)).or_insert_with(|| Cell {
            value: CellValue::Empty,
            formula: None,
            native_formula: None,
        });
        entry.formula = Some(canonical);
        entry.native_formula = Some(formula.trim_start_matches('=').to_string());
    }
    Ok(cells)
}

pub(crate) fn sync_grid_into_package(
    package: &mut Package,
    cells: &CellGrid,
    format: SheetFormat,
) -> Result<(), DocumentError> {
    match (package, format) {
        (Package::Xlsx(book), SheetFormat::Xlsx) => sync_umya(book, cells),
        (Package::Ods(book), SheetFormat::Ods) => sync_ods(book, cells),
        (Package::Xls, _) => Err(DocumentError::Save {
            path: PathBuf::from("[xls]"),
            message: "binary .xls cannot be written; save as .xlsx or .ods".to_string(),
        }),
        _ => Err(DocumentError::Save {
            path: PathBuf::from("[package]"),
            message: "internal package format mismatch".to_string(),
        }),
    }
}

fn sync_umya(book: &mut umya_spreadsheet::Workbook, cells: &CellGrid) -> Result<(), DocumentError> {
    let sheet = book.sheet_mut(0).map_err(|err| DocumentError::Save {
        path: PathBuf::from("[xlsx]"),
        message: err.to_string(),
    })?;
    // Blank package cells that the grid no longer holds so deletes survive.
    let stale: Vec<(u32, u32)> = sheet
        .collection_to_hashmap()
        .keys()
        .copied()
        .filter(|&(row1, col1)| {
            let key = ((row1 - 1) as usize, (col1 - 1) as usize);
            !cells.contains_key(&key)
        })
        .collect();
    for (row1, col1) in stale {
        let cell = sheet.cell_mut((col1, row1));
        // set_value on CellValue clears any formula held by the cell.
        cell.cell_value_mut().set_value("");
    }
    for (&(row, col), cell) in cells {
        if cell.is_vacant() {
            let umya = sheet.cell_mut(((col + 1) as u32, (row + 1) as u32));
            umya.cell_value_mut().set_value("");
            continue;
        }
        let umya = sheet.cell_mut(((col + 1) as u32, (row + 1) as u32));
        if let Some(formula) = &cell.formula {
            let text = match &cell.native_formula {
                Some(native) => native.clone(),
                None => formula.trim_start_matches('=').to_string(),
            };
            umya.set_formula(text);
            match &cell.value {
                CellValue::Number(n) => {
                    umya.set_formula_result_number(*n);
                }
                CellValue::Text(s) => {
                    umya.set_formula_result_string(s);
                }
                CellValue::Bool(b) => {
                    umya.set_formula_result_bool(*b);
                }
                CellValue::Error(e) => {
                    if let Ok(err) = e.parse::<umya_spreadsheet::CellErrorType>() {
                        umya.set_formula_result_error(err);
                    } else {
                        umya.set_formula_result_string(e);
                    }
                }
                CellValue::Empty => {
                    umya.set_formula_result_blank();
                }
            }
            continue;
        }
        match &cell.value {
            CellValue::Empty => {
                umya.set_value("");
            }
            CellValue::Text(s) => {
                umya.set_value_string(s);
            }
            CellValue::Number(n) => {
                umya.set_value_number(*n);
            }
            CellValue::Bool(b) => {
                umya.set_value_bool(*b);
            }
            CellValue::Error(e) => {
                umya.set_error(e);
            }
        }
    }
    Ok(())
}

fn sync_ods(book: &mut spreadsheet_ods::WorkBook, cells: &CellGrid) -> Result<(), DocumentError> {
    use spreadsheet_ods::Value;
    if book.num_sheets() == 0 {
        book.push_sheet(spreadsheet_ods::Sheet::new("Sheet1"));
    }
    let sheet = book.sheet_mut(0);
    let stale: Vec<(u32, u32)> = sheet
        .iter()
        .map(|((r, c), _)| (r, c))
        .filter(|&(r, c)| !cells.contains_key(&(r as usize, c as usize)))
        .collect();
    for (r, c) in stale {
        sheet.clear_formula(r, c);
        sheet.set_value(r, c, Value::Empty);
    }
    for (&(row, col), cell) in cells {
        let (r, c) = (row as u32, col as u32);
        if cell.is_vacant() {
            sheet.clear_formula(r, c);
            sheet.set_value(r, c, Value::Empty);
            continue;
        }
        if let Some(formula) = &cell.formula {
            let text = match &cell.native_formula {
                Some(native) => native.clone(),
                None => excel_formula_to_ods(formula),
            };
            sheet.set_formula(r, c, text);
            // ODS stores the last displayed value alongside the formula.
            let value = match &cell.value {
                CellValue::Empty => Value::Empty,
                CellValue::Text(s) => Value::Text(s.clone()),
                CellValue::Number(n) => Value::Number(*n),
                CellValue::Bool(b) => Value::Boolean(*b),
                CellValue::Error(e) => Value::Text(e.clone()),
            };
            sheet.set_value(r, c, value);
            continue;
        }
        sheet.clear_formula(r, c);
        let value = match &cell.value {
            CellValue::Empty => Value::Empty,
            CellValue::Text(s) => Value::Text(s.clone()),
            CellValue::Number(n) => Value::Number(*n),
            CellValue::Bool(b) => Value::Boolean(*b),
            CellValue::Error(e) => Value::Text(e.clone()),
        };
        sheet.set_value(r, c, value);
    }
    Ok(())
}

pub(crate) fn write_package(
    package: &mut Package,
    target: &Path,
    format: SheetFormat,
) -> Result<(), DocumentError> {
    match (package, format) {
        (Package::Xlsx(book), SheetFormat::Xlsx) => {
            umya_spreadsheet::writer::xlsx::write(book, target).map_err(|err| DocumentError::Save {
                path: target.to_path_buf(),
                message: err.to_string(),
            })
        }
        (Package::Ods(book), SheetFormat::Ods) => write_ods_atomic(target, book),
        _ => Err(DocumentError::Save {
            path: target.to_path_buf(),
            message: "internal package format mismatch".to_string(),
        }),
    }
}

/// Write an ODS workbook through a same-directory temp file so a crash
/// mid-write cannot truncate the destination; umya already renames atomically.
fn write_ods_atomic(
    target: &Path,
    book: &mut spreadsheet_ods::WorkBook,
) -> Result<(), DocumentError> {
    let dir = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let file_name = target.file_name().ok_or_else(|| DocumentError::Save {
        path: target.to_path_buf(),
        message: "path has no file name".to_string(),
    })?;
    let tmp_path = dir.join(format!(".{}.tty-office.tmp", file_name.to_string_lossy()));
    spreadsheet_ods::write_ods(book, &tmp_path).map_err(|err| DocumentError::Save {
        path: target.to_path_buf(),
        message: err.to_string(),
    })?;
    fs::rename(&tmp_path, target).map_err(|err| DocumentError::Save {
        path: target.to_path_buf(),
        message: err.to_string(),
    })
}
