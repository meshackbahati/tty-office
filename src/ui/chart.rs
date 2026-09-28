//! Chart overlay: the selected numbers as a line graph over the grid.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Block, Chart, Clear, Dataset, GraphType};
use ratatui::Frame;

use crate::app::App;

/// Draw the chart overlay; no stored chart means no overlay.
#[cfg(feature = "xlsx")]
pub(super) fn draw_chart(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(stored) = &app.chart else {
        return;
    };
    let theme = app.theme();
    let points = &stored.points;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for (_, y) in points {
        ymin = ymin.min(*y);
        ymax = ymax.max(*y);
    }
    if (ymax - ymin).abs() < f64::EPSILON {
        ymin -= 1.0;
        ymax += 1.0;
    } else {
        let pad = (ymax - ymin) * 0.1;
        ymin -= pad;
        ymax += pad;
    }
    let xmax = points.len().saturating_sub(1).max(1) as f64;
    let dataset = Dataset::default()
        .name(stored.title.clone())
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.accent))
        .data(points);
    let widget = Chart::new(vec![dataset])
        .block(
            Block::bordered()
                .title(format!(" Chart {} (Esc closes) ", stored.title))
                .border_style(Style::default().fg(theme.accent)),
        )
        .x_axis(Axis::default().bounds([0.0, xmax]))
        .y_axis(Axis::default().bounds([ymin, ymax]));
    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}
