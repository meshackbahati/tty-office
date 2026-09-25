//! Viewport render cost on a 10k-line document, the Phase 7 sub-frame
//! budget measured through Ratatui's TestBackend so the diffing and
//! styling path matches the live terminal.

use criterion::{criterion_group, criterion_main, Criterion};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tty_office::{draw, App, Document, Editor, TextDocument};

fn bench_render(c: &mut Criterion) {
    let mut doc = TextDocument::new();
    let body: String = (0..10_000)
        .map(|i| format!("line {i} holds a sentence of the manuscript\n"))
        .collect();
    doc.insert_str(&body);
    let mut app = App::new(Document::Text(doc));
    let mut term = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");

    c.bench_function("render_steady_state_10k_lines", |b| {
        b.iter(|| {
            term.draw(|frame| draw(frame, &mut app)).expect("draw");
        })
    });

    // A frame that must repaint the whole pane, as after a resize.
    c.bench_function("render_full_repaint_10k_lines", |b| {
        b.iter(|| {
            term.backend_mut().resize(81, 25);
            term.draw(|frame| draw(frame, &mut app)).expect("draw");
        })
    });
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
