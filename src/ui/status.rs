//! Status bar: filename, caret position, page, counts, and mode label.
//!
//! The bar is one hairline row with the document identity on the left and the
//! mode on the right; the page readout sits with the other counters so the
//! caret's page is visible at a glance while editing.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};
use crate::editor::Editor;

pub(super) fn draw_status(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme();
    let name = app.doc.display_name();
    let dirty = if app.doc.is_dirty() { " [+]" } else { "" };
    let layout = app.page_layout();
    let (line, col, words, page, pages) = match app.doc.prose_surface() {
        Some(t) => (
            t.cursor_line() + 1,
            t.cursor_display_col() + 1,
            t.word_count(),
            layout.page_of(t.cursor_line()),
            layout.page_count(t.line_count()),
        ),
        None => {
            #[cfg(feature = "xlsx")]
            if let crate::Document::Sheet(sheet) = &app.doc {
                let (r, c) = sheet.cursor_cell();
                let ref_str = crate::sheet::SheetDocument::cell_ref(r, c);
                let left = Line::from(vec![
                    Span::styled(
                        format!(" {name}{dirty}"),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" │ "),
                    Span::raw(format!("Cell {ref_str}")),
                ]);
                let mode = match app.mode {
                    Mode::Help => "HELP",
                    Mode::ConfirmQuit => "EXIT?",
                    Mode::Prompt(_) => "PROMPT",
                    Mode::Browse => "BROWSE",
                    Mode::Normal => "EDIT",
                };
                render_status_bar(frame, area, left, mode, theme);
                return;
            }
            (1, 1, 0, 1, 1)
        }
    };
    let mode = match app.mode {
        Mode::Help => "HELP",
        Mode::ConfirmQuit => "EXIT?",
        Mode::Prompt(_) => "PROMPT",
        Mode::Browse => "BROWSE",
        Mode::Normal => "EDIT",
    };
    let left_spans = {
        #[cfg_attr(not(feature = "proof"), allow(unused_mut))]
        let mut spans = vec![
            Span::styled(
                format!(" {name}{dirty}"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" │ "),
            Span::raw(format!("Ln {line}, Col {col}")),
            Span::raw(" │ "),
            Span::raw(format!("{words} words")),
            Span::raw(" │ "),
            Span::raw(format!("page {page}/{pages}")),
        ];
        if app.zoom_step() > 0 {
            spans.push(Span::raw(" │ "));
            spans.push(Span::raw(format!("{}%", app.zoom_percent())));
        }
        #[cfg(feature = "proof")]
        {
            let n = app.misspelling_count();
            if n > 0 {
                spans.push(Span::raw(" │ "));
                spans.push(Span::raw(format!("{n} misspellings")));
            }
        }
        spans
    };
    let left = Line::from(left_spans);
    render_status_bar(frame, area, left, mode, theme);
}

pub(super) fn render_status_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    left: Line<'static>,
    mode: &str,
    theme: crate::Theme,
) {
    let right = Line::from(Span::styled(
        format!("{mode} "),
        Style::default().fg(theme.dim),
    ));
    let bar = Paragraph::new(left).style(Style::default().bg(theme.status_bg));
    frame.render_widget(bar, area);
    let right_width = right.width() as u16;
    if area.width > right_width {
        let x = area.x + area.width - right_width;
        let right_area = Rect {
            x,
            y: area.y,
            width: right_width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(right).style(Style::default().bg(theme.status_bg)),
            right_area,
        );
    }
}
