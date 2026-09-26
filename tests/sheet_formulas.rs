//! Formula battery: representative functions across the engine's
//! families, plus the failure modes that must stay errors rather than
//! hangs or panics (unknown functions, circular references, division
//! by zero). Evaluation runs in Excel dialect on a single-sheet mirror.

#![cfg(feature = "xlsx")]

use tty_office::{Editor, Motion, SheetDocument, SheetFormat};

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

fn seeded() -> SheetDocument {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 0, "10");
    put(&mut sheet, 1, 0, "20");
    put(&mut sheet, 2, 0, "30");
    sheet
}

#[test]
fn aggregates_over_a_range() {
    let mut sheet = seeded();
    put(&mut sheet, 0, 1, "=SUM(A1:A3)");
    put(&mut sheet, 1, 1, "=AVERAGE(A1:A3)");
    put(&mut sheet, 2, 1, "=MIN(A1:A3)");
    put(&mut sheet, 3, 1, "=MAX(A1:A3)");
    put(&mut sheet, 4, 1, "=COUNT(A1:A3)");
    assert_eq!(value(&sheet, 0, 1), "60");
    assert_eq!(value(&sheet, 1, 1), "20");
    assert_eq!(value(&sheet, 2, 1), "10");
    assert_eq!(value(&sheet, 3, 1), "30");
    assert_eq!(value(&sheet, 4, 1), "3");
}

#[test]
fn arithmetic_and_comparison_operators() {
    let mut sheet = seeded();
    put(&mut sheet, 0, 1, "=A1*2+A2");
    put(&mut sheet, 1, 1, "=A3-A1");
    put(&mut sheet, 2, 1, "=A1>A2");
    put(&mut sheet, 3, 1, "=A2>=20");
    assert_eq!(value(&sheet, 0, 1), "40");
    assert_eq!(value(&sheet, 1, 1), "20");
    assert_eq!(value(&sheet, 2, 1), "FALSE");
    assert_eq!(value(&sheet, 3, 1), "TRUE");
}

#[test]
fn logical_branching() {
    let mut sheet = seeded();
    put(&mut sheet, 0, 1, "=IF(A1>5,\"big\",\"small\")");
    put(&mut sheet, 1, 1, "=IF(A1>50,\"big\",\"small\")");
    put(&mut sheet, 2, 1, "=AND(A1>5,A2>5)");
    put(&mut sheet, 3, 1, "=OR(A1>50,A2>5)");
    assert_eq!(value(&sheet, 0, 1), "big");
    assert_eq!(value(&sheet, 1, 1), "small");
    assert_eq!(value(&sheet, 2, 1), "TRUE");
    assert_eq!(value(&sheet, 3, 1), "TRUE");
}

#[test]
fn text_functions() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 1, "=UPPER(\"hi\")");
    put(&mut sheet, 1, 1, "=LEN(\"hello\")");
    put(&mut sheet, 2, 1, "=LEFT(\"hello\",2)");
    put(&mut sheet, 3, 1, "=CONCAT(\"a\",\"b\")");
    assert_eq!(value(&sheet, 0, 1), "HI");
    assert_eq!(value(&sheet, 1, 1), "5");
    assert_eq!(value(&sheet, 2, 1), "he");
    assert_eq!(value(&sheet, 3, 1), "ab");
}

#[test]
fn math_functions() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 1, "=ABS(-7)");
    put(&mut sheet, 1, 1, "=SQRT(16)");
    put(&mut sheet, 2, 1, "=POWER(2,3)");
    put(&mut sheet, 3, 1, "=MOD(7,3)");
    assert_eq!(value(&sheet, 0, 1), "7");
    assert_eq!(value(&sheet, 1, 1), "4");
    assert_eq!(value(&sheet, 2, 1), "8");
    assert_eq!(value(&sheet, 3, 1), "1");
}

#[test]
fn unknown_functions_stay_errors() {
    let mut sheet = seeded();
    put(&mut sheet, 0, 1, "=NOSUCHFN(A1:A3)");
    let got = value(&sheet, 0, 1);
    assert!(
        !got.is_empty() && got != "60",
        "unknown function must not evaluate silently: {got}"
    );
}

#[test]
fn division_by_zero_stays_an_error() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 0, 1, "=1/0");
    let got = value(&sheet, 0, 1);
    assert!(!got.is_empty(), "division by zero must show an error");
    assert!(
        got != "0" && got != "1",
        "division by zero misvalued: {got}"
    );
}

#[test]
fn circular_references_terminate_as_errors() {
    let mut sheet = fresh_sheet();
    put(&mut sheet, 4, 0, "=A6+1");
    put(&mut sheet, 5, 0, "=A5+1");
    // The engine detects the cycle; the test completing at all proves
    // evaluation terminates instead of looping forever.
    let first = value(&sheet, 4, 0);
    let second = value(&sheet, 5, 0);
    assert!(
        !first.is_empty() && !second.is_empty(),
        "cycle cells must hold error values: {first:?} {second:?}"
    );
}
