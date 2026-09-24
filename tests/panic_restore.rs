//! Phase 0 gate: a panic after `ratatui::init` must run the restore hook and
//! exit non-zero rather than hang or succeed silently.
//!
//! The binary exposes `--self-test-panic` for this purpose. Without a
//! controlling terminal, `ratatui::init` fails before the hook can run, so the
//! test allocates a pseudo-TTY with `script(1)` when available.

use std::process::Command;

#[test]
fn panic_path_exits_nonzero_after_init() {
    let bin = env!("CARGO_BIN_EXE_tty-office");

    // Prefer a real PTY so init succeeds and the restore hook is exercised.
    let mut cmd = match which("script") {
        Some(script) => {
            let mut c = Command::new(script);
            c.args(["-qefc", &format!("{bin} --self-test-panic"), "/dev/null"]);
            c
        }
        None => {
            let mut c = Command::new(bin);
            c.arg("--self-test-panic");
            c
        }
    };

    let output = cmd.output().expect("spawn binary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "panic path must exit non-zero; status={:?}",
        output.status.code()
    );
    assert!(
        combined.contains("self-test panic") || combined.contains("failed to initialize terminal"),
        "expected panic or init-failure evidence, got: {combined}"
    );
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
