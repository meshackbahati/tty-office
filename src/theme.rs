//! Display themes: named palettes applied across every chrome surface.
//!
//! The terminal owns the default foreground and background, so themes
//! only override the accents the suite paints (filenames, borders, the
//! status wash, errors). Every preset stays legible on both dark and
//! light terminals because none of them forces a base color.

use ratatui::style::Color;

/// Accent palette for one named theme; cheap to copy into renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Display name used by the config file and the cycle message.
    pub name: &'static str,
    /// Filenames, prompt labels, and other highlights.
    pub accent: Color,
    /// Borders, rules, gutters, and other structural chrome.
    pub dim: Color,
    /// Status bar wash.
    pub status_bg: Color,
    /// Misspellings and other error marks.
    pub error: Color,
}

impl Theme {
    /// Today's monochrome look: the default for missing config.
    pub const MONO: Self = Self {
        name: "Mono",
        accent: Color::Cyan,
        dim: Color::DarkGray,
        status_bg: Color::Black,
        error: Color::Red,
    };
    /// Cool blues for the chrome.
    pub const OCEAN: Self = Self {
        name: "Ocean",
        accent: Color::LightBlue,
        dim: Color::Blue,
        status_bg: Color::Black,
        error: Color::LightRed,
    };
    /// Warm ambers for the chrome.
    pub const EMBER: Self = Self {
        name: "Ember",
        accent: Color::Yellow,
        dim: Color::Gray,
        status_bg: Color::Black,
        error: Color::LightRed,
    };
    /// Greens for the chrome.
    pub const FOREST: Self = Self {
        name: "Forest",
        accent: Color::LightGreen,
        dim: Color::DarkGray,
        status_bg: Color::Black,
        error: Color::Red,
    };

    /// Presets in cycle order.
    pub const ALL: [Self; 4] = [Self::MONO, Self::OCEAN, Self::EMBER, Self::FOREST];

    /// Look up a preset by name, ignoring case; unknown names miss so the
    /// caller falls back to the default instead of failing to start.
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Next preset in cycle order, wrapping past the last one.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_names_resolve_case_insensitively() {
        assert_eq!(Theme::by_name("ocean"), Some(Theme::OCEAN));
        assert_eq!(Theme::by_name("EMBER"), Some(Theme::EMBER));
    }

    #[test]
    fn unknown_names_miss_for_the_default_fallback() {
        assert_eq!(Theme::by_name("neon"), None);
    }

    #[test]
    fn cycling_wraps_past_the_last_preset() {
        let mut theme = Theme::MONO;
        for _ in 0..Theme::ALL.len() {
            theme = theme.next();
        }
        assert_eq!(theme, Theme::MONO);
    }
}
