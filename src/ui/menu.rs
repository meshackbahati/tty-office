//! Menu bar and dropdown rendering.
//!
//! The bar always occupies the first frame row; an open dropdown paints
//! over the text pane afterwards so its rows stay legible. Geometry comes
//! from the model so clicks and rendering agree by construction.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::menu::{dropdown, items, LABELS};
use crate::app::App;

/// Draw the bar itself into the reserved first row.
pub(super) fn draw_menu_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (i, label) in LABELS.iter().enumerate() {
        let mut style = Style::default();
        if app.open_menu == Some(i) {
            style = style.add_modifier(Modifier::REVERSED);
        }
        spans.push(Span::styled((*label).to_string(), style));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Paint the open dropdown over whatever the text pane drew.
pub(super) fn draw_dropdown(frame: &mut Frame<'_>, app: &App) {
    let Some(menu) = app.open_menu else {
        return;
    };
    let list = items(menu);
    let frame_w = frame.area().width;
    let (mut x, mut w) = dropdown(menu);
    x = x.min(frame_w.saturating_sub(3));
    w = w.min(frame_w.saturating_sub(x)).max(6);
    let rect = Rect::new(x, 1, w, list.len() as u16 + 2);
    let inner_w = (w.saturating_sub(2)) as usize;
    let rows: Vec<Line<'static>> = list
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let pad = inner_w.saturating_sub(it.label.len() + it.shortcut.len() + 2);
            let row = format!(" {}{:pad$}{} ", it.label, "", it.shortcut, pad = pad);
            let mut style = Style::default();
            if i == app.menu_item {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::from(Span::styled(row, style))
        })
        .collect();
    let block = Block::bordered().title(LABELS[menu.min(LABELS.len() - 1)]);
    frame.render_widget(Paragraph::new(rows).block(block), rect);
}
