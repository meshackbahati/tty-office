//! File browser overlay: directory listing with filter and selection.
//!
//! The overlay covers the body area modally; geometry mirrors the
//! selection math in the model so clicks resolve identically.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::browse::BrowseState;
use crate::app::App;

/// Draw the browser over the body area; no session means no overlay.
pub(super) fn draw_browse(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(state) = &app.browse else {
        return;
    };
    let theme = app.theme();
    let content_h = area.height.saturating_sub(2) as usize;
    let visible = state.visible();
    let off = BrowseState::offset(state.selected, visible.len(), content_h);
    let mut lines: Vec<Line<'static>> = visible
        .iter()
        .enumerate()
        .skip(off)
        .take(content_h)
        .map(|(i, entry)| {
            let marker = if entry.is_dir { "/" } else { "" };
            let mut style = Style::default();
            if i == state.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::from(Span::styled(format!(" {}{}", entry.name, marker), style))
        })
        .collect();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " (no matches)".to_string(),
            Style::default().fg(theme.dim),
        )));
    }
    let title = if state.filter.is_empty() {
        format!(" {} ", state.dir.display())
    } else {
        format!(" {} [{}] ", state.dir.display(), state.filter)
    };
    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(theme.accent));
    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
