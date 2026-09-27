//! Run styles: bold, italic, and underline read from the package model
//! render distinctly, and page jumps move by page starts.

#![cfg(feature = "docx")]

use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;
use tempfile::TempDir;
use tty_office::{draw, App, Document};

fn styled_path(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("styles.docx");
    let mut package = rdocx::Document::new();
    let mut paragraph = package.add_paragraph("plain ");
    paragraph.add_run("bold").bold(true);
    let mut second = package.add_paragraph("tail ");
    second.add_run("ital").italic(true);
    package.save(&path).expect("save fixture");
    path
}

fn run_styles(app: &App, line: usize) -> Vec<(usize, usize, bool, bool, bool)> {
    match &app.doc {
        Document::Rich(rich) => rich
            .run_styles(line)
            .into_iter()
            .map(|s| (s.start, s.end, s.flags.0, s.flags.1, s.flags.2))
            .collect(),
        _ => panic!("rich expected"),
    }
}

#[test]
fn styled_runs_detect_with_flags() {
    let dir = TempDir::new().expect("temp dir");
    let path = styled_path(&dir);
    let app = App::new(tty_office::open(&path).expect("open"));
    // "plain " is six characters; "bold" runs 6..10.
    assert_eq!(run_styles(&app, 0), vec![(6, 10, true, false, false)]);
    // Second paragraph: "tail " then italic "ital" at 5..9.
    assert_eq!(run_styles(&app, 1), vec![(5, 9, false, true, false)]);
}

#[test]
fn styled_runs_render_with_modifiers() {
    let dir = TempDir::new().expect("temp dir");
    let path = styled_path(&dir);
    let mut app = App::new(tty_office::open(&path).expect("open"));
    let backend = TestBackend::new(40, 8);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|frame| draw(frame, &mut app)).expect("draw");
    // Menu row 0, hairline row 1, first text row 2, past the page
    // border: column 7 sits inside the bold run.
    let bold = term.backend().buffer()[(7, 2)].style();
    assert!(
        bold.add_modifier.contains(Modifier::BOLD),
        "bold run is not bold"
    );
    let plain = term.backend().buffer()[(1, 2)].style();
    assert!(
        !plain.add_modifier.contains(Modifier::BOLD),
        "plain run should not be bold"
    );
}
