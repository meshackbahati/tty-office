//! Tab strip above the text pane, drawn once a second tab exists.
//!
//! Titles come from the session in display order; the active tab renders
//! in reverse video and dirty tabs carry the `*` marker the title
//! already includes. Each title is truncated so one long file name
//! cannot push the rest of the strip off the row.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::App;

/// Longest title drawn for one tab before truncation.
const TITLE_LIMIT: usize = 20;

/// Draw the tab strip; the caller reserves the row only when the session
/// holds more than one tab.
pub(super) fn draw_tab_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let width = area.width as usize;
    let mut spans = Vec::new();
    let mut used = 0usize;
    for i in 0..app.tab_count() {
        let title = app.tab_title(i).unwrap_or_default();
        let short: String = title.chars().take(TITLE_LIMIT).collect();
        let divider = if i == 0 { "" } else { "│" };
        let cell = format!("{divider} {short} ");
        if used + cell.width() + 4 > width {
            spans.push(Span::raw(format!(" +{}", app.tab_count() - i)));
            break;
        }
        let mut style = Style::default();
        if i == app.active_tab() {
            style = style.add_modifier(Modifier::REVERSED);
        }
        used += cell.width();
        spans.push(Span::styled(cell, style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
