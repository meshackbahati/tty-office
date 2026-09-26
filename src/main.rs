//! tty-office binary: CLI, terminal setup, event loop.
//!
//! `ratatui::init` installs a panic hook that restores the terminal, so a
//! panic leaves the alternate screen rather than stranding the user.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use ratatui::DefaultTerminal;
use tty_office::{cat_text, convert_files, draw, info_text, open_optional, App};

/// Pure TTY Document Suite — word documents, spreadsheets, text, and PDF
/// export, with a plain-text fallback behind `--no-default-features`.
#[derive(Parser, Debug)]
#[command(name = "tty-office", version, about)]
struct Cli {
    /// File to open. A path that does not exist yet starts an empty buffer
    /// that saves to that path.
    path: Option<PathBuf>,

    /// Headless operation without the terminal interface.
    #[command(subcommand)]
    command: Option<Command>,

    /// Initialize the terminal and panic, for the restore-on-panic check.
    #[arg(long, hide = true)]
    self_test_panic: bool,
}

/// Scriptable document operations sharing the interface's open and save.
#[derive(Subcommand, Debug)]
enum Command {
    /// Print the document text to stdout.
    Cat {
        /// File to read.
        file: PathBuf,
    },
    /// Print document metadata as `key: value` lines.
    Info {
        /// File to inspect.
        file: PathBuf,
    },
    /// Convert between formats, inferring both from their extensions.
    Convert {
        /// File to read.
        input: PathBuf,
        /// File to write.
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        let output = match command {
            Command::Cat { file } => cat_text(&file)?,
            Command::Info { file } => info_text(&file)?,
            Command::Convert { input, output } => convert_files(&input, &output)?,
        };
        print!("{output}");
        return Ok(());
    }
    let terminal = ratatui::init();
    // Mouse reporting stays enabled across a panic unless the hook turns
    // it off first, so chain the disable ahead of ratatui's restore hook.
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        previous(info);
    }));
    if cli.self_test_panic {
        // The hook installed by ratatui::init must restore the terminal before
        // the process exits; the panic-restore test asserts on that path.
        panic!("tty-office self-test panic (expected)");
    }
    let doc = open_optional(cli.path.as_ref())?;
    let mut app = App::new(doc);
    let result = run(terminal, &mut app);
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        app.tick();
        terminal.draw(|frame| draw(frame, app))?;
        // Poll with a timeout so a resize or external signal is picked up
        // even when the user is not typing.
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(_, _) => {
                    // Next draw picks up the new size via draw's set_viewport.
                }
                _ => {}
            }
        }
    }
    Ok(())
}
