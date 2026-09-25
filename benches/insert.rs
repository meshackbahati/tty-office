//! Insert throughput on the rope editor, one of the Phase 7 benches.

use criterion::{criterion_group, criterion_main, Criterion};
use tty_office::{Editor, TextDocument};

fn bench_insert(c: &mut Criterion) {
    c.bench_function("insert_10k_chars_into_empty", |b| {
        b.iter(|| {
            let mut doc = TextDocument::new();
            for _ in 0..10_000 {
                doc.insert_char('x');
            }
            criterion::black_box(doc.text_projection().len())
        })
    });

    let mut seeded = TextDocument::new();
    seeded.insert_str(&"lorem ipsum dolor sit amet consectetur\n".repeat(20_000));
    c.bench_function("insert_1k_chars_into_700k_buffer", |b| {
        b.iter(|| {
            for _ in 0..1_000 {
                seeded.insert_char('y');
            }
            criterion::black_box(seeded.text_projection().len())
        })
    });
}

criterion_group!(benches, bench_insert);
criterion_main!(benches);
