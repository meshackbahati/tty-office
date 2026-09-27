//! User config file: overrides that merge with the default bindings.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// File format for user config overrides.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Map from action name to chord spec, e.g. `exit = "ctrl+q"`.
    #[serde(default)]
    pub keys: HashMap<String, String>,
    /// Logical lines per page; absent or zero keeps the default page model.
    #[serde(default)]
    pub page_lines: Option<usize>,
    /// Theme preset name, e.g. `theme = "ocean"`; unknown names fall back
    /// to the monochrome default rather than failing to start.
    #[serde(default)]
    pub theme: Option<String>,
}

impl Config {
    /// Read `~/.config/tty-office/config.toml`, falling back to the default
    /// configuration when the file is missing or malformed, because a broken
    /// user config must not prevent the editor from starting.
    pub fn load_user() -> Self {
        if let Some(path) = user_config_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&text) {
                    return cfg;
                }
            }
        }
        Self::default()
    }
}

pub(crate) fn user_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/tty-office/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_page_lines_alongside_keys() {
        let cfg: Config =
            toml::from_str("page_lines = 42\n[keys]\nexit = \"ctrl+q\"").expect("valid config");
        assert_eq!(cfg.page_lines, Some(42));
        assert_eq!(cfg.keys.get("exit").map(String::as_str), Some("ctrl+q"));
    }

    #[test]
    fn config_defaults_when_page_lines_absent() {
        let cfg: Config = toml::from_str("[keys]\nexit = \"ctrl+q\"").expect("valid config");
        assert_eq!(cfg.page_lines, None);
    }
}
