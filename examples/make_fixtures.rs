//! Regenerate the binary corpus under `tests/fixtures`.
//!
//! Run with `cargo run --example make_fixtures --features docx,xlsx` from
//! the workspace root. The committed fixtures pin bytes on disk so the
//! corpus tests exercise the read path against stable inputs rather than
//! only round-tripping whatever the current writer happens to produce.

#[cfg(all(feature = "docx", feature = "xlsx"))]
fn main() -> anyhow::Result<()> {
    use std::path::Path;

    use tty_office::{export, open, Editor, Motion, SheetDocument};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::create_dir_all(&root)?;

    // Rich documents: one heading, a body sentence, a blank separator
    // line, and an appendix line, which exercise paragraph splitting.
    let rich_text =
        "Quarterly report\nThe budget closes at the end of March.\n\nAppendix A holds the detail.";
    for name in ["document.docx", "notes.odt"] {
        let path = root.join(name);
        let _ = std::fs::remove_file(&path);
        let mut doc = open(&path)?;
        doc.insert_str(rich_text);
        doc.save(None)?;
    }

    // Plain text and Markdown travel through the text surface, which every
    // build owns; Markdown keeps its markers verbatim since the editor
    // stores source text rather than rendered output.
    for (name, body) in [
        (
            "plain.txt",
            "Plain notes\nA second line for the corpus.\n\nTrailing paragraph.",
        ),
        ("notes.md", "# Notes\n\n- first\n- second\n\nDone."),
    ] {
        let path = root.join(name);
        let _ = std::fs::remove_file(&path);
        let mut doc = open(&path)?;
        doc.insert_str(body);
        doc.save(None)?;
    }

    // Workbooks: text and numeric cells plus sum formulas that must
    // re-evaluate after every open-save cycle.
    let grid: [(usize, usize, &str); 12] = [
        (0, 0, "region"),
        (0, 1, "q1"),
        (0, 2, "q2"),
        (1, 0, "north"),
        (1, 1, "120"),
        (1, 2, "135"),
        (2, 0, "south"),
        (2, 1, "98"),
        (2, 2, "104"),
        (3, 0, "total"),
        (3, 1, "=B2+B3"),
        (3, 2, "=C2+C3"),
    ];
    for name in ["budget.xlsx", "sales.ods", "data.csv"] {
        let path = root.join(name);
        let _ = std::fs::remove_file(&path);
        let mut doc = open(&path)?;
        let sheet: &mut SheetDocument = doc.sheet_mut().expect("sheet path");
        for (row, col, input) in grid {
            // There is no direct cursor setter on the public surface, so
            // walk from the origin; the corpus is small enough that this
            // stays trivial.
            sheet.move_cursor(Motion::BufferStart, false);
            for _ in 0..row {
                sheet.move_cursor(Motion::Down, false);
            }
            for _ in 0..col {
                sheet.move_cursor(Motion::Right, false);
            }
            sheet.set_cell_content(input);
        }
        doc.save(None)?;
    }
    println!("fixtures written to {}", root.display());
    // A one-page PDF for the viewer, exported from the plain fixture.
    #[cfg(feature = "pdf")]
    {
        let mut doc = open(&root.join("plain.txt"))?;
        export(&mut doc, &root.join("sample.pdf"))?;
        println!("sample.pdf written");
    }
    // A two-page PDF: sixty lines paginate on the fifty-line default.
    #[cfg(feature = "pdf")]
    {
        let long: String = (1..=60)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let long_path = root.join("long.txt");
        std::fs::write(&long_path, &long)?;
        let mut doc = open(&long_path)?;
        export(&mut doc, &root.join("sample2.pdf"))?;
        std::fs::remove_file(&long_path)?;
        println!("sample2.pdf written");
    }
    Ok(())
}

#[cfg(not(all(feature = "docx", feature = "xlsx")))]
fn main() {
    eprintln!("make_fixtures requires --features docx,xlsx");
    std::process::exit(1);
}
