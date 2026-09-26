//! Spreadsheet grid pane: bordered cells with header rules and live
//! cell-edit echo.
//!
//! Every column carries its left border line and every row its rule
//! below, so the joints mark each cell; the renderer, the scroll maths,
//! and the mouse handler share the stride this layout defines.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Mode, PromptKind};

use super::selected_style;

#[cfg(feature = "xlsx")]
pub(super) fn draw_sheet(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    use crate::sheet::{CELL_STRIDE, CELL_WIDTH, COL_HEADER_H, ROW_GUTTER};

    let (height, width) = (area.height as usize, area.width as usize);
    if height < COL_HEADER_H + 1 || width <= ROW_GUTTER {
        frame.render_widget(Paragraph::new(""), area);
        return;
    }

    // Snapshot grid geometry so the borrow of the sheet ends before render.
    let snapshot = {
        let Some(sheet) = app.doc.sheet_mut() else {
            frame.render_widget(Paragraph::new(""), area);
            return;
        };
        sheet.scroll_to_cursor(height, width);
        let (cursor_row, cursor_col) = sheet.cursor_cell();
        let selection = sheet.selection_rect();
        let rowoff = sheet.rowoff();
        let coloff = sheet.coloff();
        let visible_rows = height.saturating_sub(COL_HEADER_H).div_ceil(2);
        let visible_cols = width.saturating_sub(ROW_GUTTER).div_ceil(CELL_STRIDE);
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_rows);
        for r in rowoff..rowoff + visible_rows {
            let mut cols: Vec<String> = Vec::with_capacity(visible_cols);
            for c in coloff..coloff + visible_cols {
                cols.push(
                    sheet
                        .cell(r, c)
                        .map(|cell| cell.value.display())
                        .unwrap_or_default(),
                );
            }
            rows.push(cols);
        }
        (
            cursor_row,
            cursor_col,
            selection,
            rowoff,
            coloff,
            rows,
            visible_cols,
        )
    };
    let (cursor_row, cursor_col, selection, rowoff, coloff, mut rows, visible_cols) = snapshot;
    // While the cell prompt is open, the typed text echoes inside the
    // cursor cell itself rather than only on the prompt line below.
    if matches!(app.mode, Mode::Prompt(PromptKind::CellEdit))
        && cursor_row >= rowoff
        && cursor_col >= coloff
    {
        let (vr, vc) = (cursor_row - rowoff, cursor_col - coloff);
        if vr < rows.len() && vc < visible_cols {
            rows[vr][vc] = app.prompt_buf.clone();
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);

    // Column-letter header row with a rule below it.
    {
        let mut header = String::with_capacity(ROW_GUTTER + visible_cols * CELL_STRIDE);
        header.push_str(&" ".repeat(ROW_GUTTER));
        for c in coloff..coloff + visible_cols {
            let name = crate::sheet::SheetDocument::cell_ref(0, c);
            // cell_ref includes the row number; strip digits for the header.
            let letters: String = name
                .chars()
                .take_while(|ch| ch.is_ascii_alphabetic())
                .collect();
            let label = format!("{letters:width$}", width = CELL_WIDTH);
            header.push('│');
            header.push_str(&label[..CELL_WIDTH.min(label.len())]);
        }
        lines.push(Line::from(header));
        lines.push(grid_separator(visible_cols));
    }

    for (i, row_vals) in rows.iter().enumerate() {
        let r = rowoff + i;
        let mut spans: Vec<Span<'static>> = Vec::new();
        let row_num = format!("{:>5} ", r + 1);
        spans.push(Span::styled(row_num, Style::default().fg(Color::DarkGray)));
        for (j, val) in row_vals.iter().enumerate() {
            let c = coloff + j;
            let selected = selection
                .is_some_and(|((r0, c0), (r1, c1))| r >= r0 && r <= r1 && c >= c0 && c <= c1);
            let is_cursor = r == cursor_row && c == cursor_col;
            let text = format!("{val:width$}", width = CELL_WIDTH);
            let text = text.chars().take(CELL_WIDTH).collect::<String>();
            let style = if selected {
                selected_style()
            } else if is_cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            spans.push(Span::raw("│"));
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
        lines.push(grid_separator(visible_cols));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);
}

/// Horizontal grid rule: gutter dashes plus one joint per column.
#[cfg(feature = "xlsx")]
fn grid_separator(visible_cols: usize) -> Line<'static> {
    let mut rule = "─".repeat(crate::sheet::ROW_GUTTER);
    for _ in 0..visible_cols {
        rule.push('┼');
        rule.push_str(&"─".repeat(crate::sheet::CELL_WIDTH));
    }
    Line::styled(rule, Style::default().fg(Color::DarkGray))
}
