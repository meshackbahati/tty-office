//! Recalculation cost when an edit dirties a workbook with a running
//! total over a thousand rows, the Phase 7 recalc bench.

#[cfg(feature = "xlsx")]
use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(feature = "xlsx")]
use tempfile::TempDir;
#[cfg(feature = "xlsx")]
use tty_office::{open, Editor, Motion};

#[cfg(feature = "xlsx")]
fn measure_reevaluation(c: &mut Criterion) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("recalc.xlsx");
    let mut doc = open(&path).expect("open workbook");
    {
        let sheet = doc.sheet_mut().expect("sheet");
        sheet.move_cursor(Motion::BufferStart, false);
        for row in 0..1_000 {
            if row > 0 {
                sheet.move_cursor(Motion::Down, false);
            }
            sheet.set_cell_content(&row.to_string());
        }
        sheet.move_cursor(Motion::BufferStart, false);
        sheet.move_cursor(Motion::Right, false);
        sheet.set_cell_content("=SUM(A1:A1000)");
        sheet.move_cursor(Motion::Right, false);
    }

    // Every cell edit re-evaluates the workbook, so toggling a value the
    // sum does not depend on still exercises the full pass.
    let mut flip = false;
    c.bench_function("edit_triggers_1k_row_reevaluation", |b| {
        b.iter(|| {
            flip = !flip;
            let sheet = doc.sheet_mut().expect("sheet");
            sheet.move_cursor(Motion::BufferStart, false);
            sheet.move_cursor(Motion::Down, false);
            sheet.move_cursor(Motion::Right, false);
            sheet.move_cursor(Motion::Right, false);
            sheet.set_cell_content(if flip { "1" } else { "2" });
            criterion::black_box(flip);
        })
    });
}

#[cfg(feature = "xlsx")]
criterion_group!(recalc_group, measure_reevaluation);
#[cfg(feature = "xlsx")]
criterion_main!(recalc_group);

#[cfg(not(feature = "xlsx"))]
fn main() {
    // A zero exit keeps a bare `cargo bench` green: there is simply
    // nothing to measure without a workbook backend.
    eprintln!("the recalc bench requires --features xlsx; skipping");
}
