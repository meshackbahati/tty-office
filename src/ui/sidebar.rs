//! Sidebar rendering: section headers, entries, and the tab list.
//!
//! Rows come from the model in order; the visible slice is the leading
//! run that fits the body height, so a long tab list truncates instead
//! of spilling onto the status bar.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::sidebar::{rows, RowKind};
use crate::app::App;

/// Draw the sidebar into its reserved column with a right hairline.
pub(super) fn draw_sidebar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = app.theme();
    let width = area.width as usize;
    let focused = app.focus == crate::app::Focus::Sidebar;
    let lines: Vec<Line<'static>> = rows(app)
        .into_iter()
        .take(area.height as usize)
        .enumerate()
        .map(|(idx, row)| {
            let mut text = format!(" {}", row.label);
            while text.width() > width.saturating_sub(1) {
                text.pop();
            }
            let highlight = focused && idx == app.side_sel;
            let style = match row.kind {
                RowKind::Header => Style::default().fg(theme.dim),
                RowKind::Tab(i) if i == app.active_tab() => {
                    let mut style = Style::default().add_modifier(Modifier::REVERSED);
                    if highlight {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    style
                }
                _ if highlight => Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
                _ => Style::default(),
            };
            Line::from(Span::styled(text, style))
        })
        .collect();
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(theme.dim));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
