//! Phase 7 performance budgets from DEVELOPMENT_PLAN, run explicitly in
//! release mode with:
//!
//! `cargo test --release --test perf_budget -- --ignored --nocapture`
//!
//! The tests are ignored by default because the budgets are claims about
//! the optimized build, and because the hundred-megabyte load would
//! otherwise run on every development cycle.

#![cfg(target_os = "linux")]

use std::time::Instant;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, Motion, TextDocument};

/// Current resident set size in kilobytes as the kernel reports it.
fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// A 100 MB plain-text file must load well inside the Ropey overhead
/// budget: about 10 percent over the file, with allowance for the
/// harness itself and the transient read buffer.
#[test]
#[ignore]
fn hundred_megabyte_file_loads_within_budget() {
    use tempfile::TempDir;

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("large.txt");
    let line = "line of manuscript text padding the file to the target size\n";
    let body = line.repeat(1_800_000);
    assert!(body.len() >= 100_000_000, "fixture under 100 MB");
    std::fs::write(&path, &body).expect("write fixture");
    drop(body);

    let before_kb = rss_kb();
    let clock = Instant::now();
    let doc = tty_office::open(&path).expect("open large file");
    let elapsed = clock.elapsed();
    let after_kb = rss_kb();
    let delta_mb = (after_kb.saturating_sub(before_kb)) as f64 / 1024.0;
    println!(
        "load: {:?}, rss before {} MB, after {} MB, delta {:.1} MB",
        elapsed,
        before_kb / 1024,
        after_kb / 1024,
        delta_mb
    );
    assert_eq!(doc.path().map(|p| p.to_path_buf()), Some(path));
    assert!(
        delta_mb < 160.0,
        "load added {delta_mb:.1} MB RSS, budget is 160 MB"
    );
    assert!(
        after_kb / 1024 < 320,
        "absolute RSS {} MB exceeds 320 MB",
        after_kb / 1024
    );
    assert!(
        elapsed.as_secs() < 60,
        "load took {:?}, budget is 60 s",
        elapsed
    );
}

/// Steady-state keystrokes on a ten-thousand-line file must stay
/// microsecond-scale in the release build. The typing pattern alternates
/// insert and delete so line lengths stay representative of prose, since
/// display-column recomputation is linear in the length of the cursor's
/// own line; that long-line figure is measured and printed immediately
/// below without being held to the keystroke budget, because a ten
/// thousand character paragraph is a structurally different workload.
#[test]
#[ignore]
fn edits_stay_microsecond_scale() {
    let mut doc = TextDocument::new();
    doc.insert_str(&"sample line of text for the edit budget\n".repeat(10_000));
    let iterations = 10_000u128;
    let clock = Instant::now();
    for _ in 0..iterations {
        doc.insert_char('x');
        doc.delete_back();
    }
    let mean_ns = clock.elapsed().as_nanos() as f64 / iterations as f64;
    println!("mean keystroke (insert plus delete) in 10k-line buffer: {mean_ns:.0} ns");
    assert!(
        mean_ns < 50_000.0,
        "mean keystroke {mean_ns:.0} ns exceeds the 50 microsecond budget"
    );

    // Record the single-growing-line cost: each keystroke rescans the
    // whole line to rebuild the display column, so this is quadratic in
    // total. The bound only guards against a blow-up beyond that shape.
    let mut line = TextDocument::new();
    let long_iterations = 10_000u128;
    let clock = Instant::now();
    for _ in 0..long_iterations {
        line.insert_char('x');
    }
    let mean_ns = clock.elapsed().as_nanos() as f64 / long_iterations as f64;
    println!("mean insert extending one line to 10k chars: {mean_ns:.0} ns");
    assert!(
        mean_ns < 2_000_000.0,
        "mean long-line insert {mean_ns:.0} ns exceeds the 2 ms sanity bound"
    );
}

/// Rendering a ten-thousand-line document must average under one
/// sixtieth of a second, which is the sub-frame claim the plan makes.
#[test]
#[ignore]
fn render_stays_sub_frame_on_10k_lines() {
    let mut doc = TextDocument::new();
    let body: String = (0..10_000)
        .map(|i| format!("line {i} of the manuscript\n"))
        .collect();
    doc.insert_str(&body);
    let mut app = App::new(Document::Text(doc));
    let mut term = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    let frames = 60u128;
    let clock = Instant::now();
    for _ in 0..frames {
        app.doc.move_cursor(Motion::Down, false);
        term.draw(|frame| draw(frame, &mut app)).expect("draw");
    }
    let mean = clock.elapsed().as_nanos() as f64 / frames as f64 / 1_000_000.0;
    println!("mean frame over {frames} draws: {mean:.3} ms");
    assert!(
        mean < 16.6,
        "mean frame {mean:.3} ms exceeds the 16.6 ms sub-frame budget"
    );
}
