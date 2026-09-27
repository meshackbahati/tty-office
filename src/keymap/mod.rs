//! Keybinding map: Nano contract plus word-processor shortcuts.
//!
//! Defaults match `plan.txt`. A user file at
//! `~/.config/tty-office/config.toml` may override individual bindings; the
//! override merges with defaults rather than replacing the whole table.

mod action;
mod bindings;
mod chord;
mod config;
mod map;

pub use action::{action_name, describe, Action};
pub use chord::{format_chord, Chord};
pub use config::Config;
pub use map::Keymap;
