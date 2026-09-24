//! Ratatui rendering: editor viewport, status bar, message and prompt line.
//!
//! Layout is content-first: the text pane takes the full frame minus two rows,
//! one hairline status bar and one message row. Colors stay monochrome with a
//! single accent for the status filename and selection highlight.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::app::{App, Mode};
use crate::editor::Editor;

/// Accent used for the filename and active hints.
const ACCENT: Color = Color::Cyan;
/// Style for selected ranges: reverse video for contrast without adding a
/// second palette color.
fn selected_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// Draw one frame.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.height < 3 {
        return;
    }
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let (text_h, text_w) = (chunks[0].height as usize, chunks[0].width as usize);
    app.set_viewport(text_h, text_w);

    draw_text(frame, app, chunks[0]);
    draw_status(frame, app, chunks[1]);
    draw_message(frame, app, chunks[2]);

    if app.mode == Mode::Help {
        draw_help(frame, app, chunks[0]);
    }
}

fn draw_text(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    // Text and Rich share a line-oriented surface; Phase 4 adds a grid pane
    // for Sheet rather than forcing cells through this renderer.
    let (selection, rowoff, coloff, cursor_line, cursor_col, line_count) = {
        let surface = match app.doc.prose_surface() {
            Some(s) => s,
            None => {
                frame.render_widget(Paragraph::new(""), area);
                return;
            }
        };
        (
            surface.selection_range(),
            surface.rowoff(),
            surface.coloff(),
            surface.cursor_line(),
            surface.cursor_display_col(),
            surface.line_count(),
        )
    };
    let height = area.height as usize;
    let width = area.width as usize;

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);
    for row in 0..height {
        let line_idx = rowoff + row;
        if line_idx >= line_count {
            lines.push(Line::from(""));
            continue;
        }
        let (raw, line_start) = {
            let surface = app
                .doc
                .prose_surface()
                .expect("prose surface checked above");
            (
                surface.line_text(line_idx),
                surface.line_char_start(line_idx),
            )
        };
        lines.push(render_line(
            &raw,
            line_start,
            coloff,
            width,
            selection,
            line_idx == cursor_line,
            cursor_col,
        ));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(paragraph, area);

    // Place the terminal cursor on the caret when it is inside the viewport.
    let view_row = cursor_line.saturating_sub(rowoff);
    if view_row < height {
        let disp = cursor_col.saturating_sub(coloff);
        if disp < width {
            let x = area.x + disp as u16;
            let y = area.y + view_row as u16;
            frame.set_cursor_position((x, y));
        }
    }
}

fn render_line<'a>(
    raw: &str,
    line_start: usize,
    coloff: usize,
    width: usize,
    selection: Option<(usize, usize)>,
    is_cursor_line: bool,
    _cursor_col: usize,
) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut disp = 0usize;
    let mut char_idx = line_start;
    let mut buf = String::new();
    let mut buf_selected = false;

    let mut flush = |buf: &mut String, selected: bool, spans: &mut Vec<Span<'a>>| {
        if buf.is_empty() {
            return;
        }
        let style = if selected {
            selected_style()
        } else {
            Style::default()
        };
        spans.push(Span::styled(std::mem::take(buf), style));
        let _ = selected;
    };

    for ch in raw.chars() {
        if ch == '\t' {
            let tab_w = 4 - (disp % 4);
            for _ in 0..tab_w {
                push_char(
                    &mut buf,
                    ' ',
                    &mut disp,
                    coloff,
                    width,
                    &mut buf_selected,
                    selection,
                    char_idx,
                    &mut spans,
                    &mut flush,
                );
                char_idx += 1;
            }
            continue;
        }
        let w = ch.width().unwrap_or(0);
        if disp + w > coloff + width && disp >= coloff {
            break;
        }
        let selected = selection.is_some_and(|(a, b)| char_idx >= a && char_idx < b);
        if selected != buf_selected && !buf.is_empty() {
            flush(&mut buf, buf_selected, &mut spans);
            buf_selected = selected;
        }
        if buf.is_empty() {
            buf_selected = selected;
        }
        if disp >= coloff && disp + w <= coloff + width {
            buf.push(ch);
        } else if disp >= coloff {
            // Wide character straddling the right edge: pad with spaces.
            for _ in 0..(coloff + width - disp) {
                buf.push(' ');
            }
            flush(&mut buf, buf_selected, &mut spans);
            break;
        }
        disp += w;
        char_idx += 1;
        let _ = is_cursor_line;
    }
    flush(&mut buf, buf_selected, &mut spans);
    Line::from(spans)
}

#[allow(clippy::too_many_arguments)]
fn push_char<'a>(
    buf: &mut String,
    ch: char,
    disp: &mut usize,
    coloff: usize,
    width: usize,
    buf_selected: &mut bool,
    selection: Option<(usize, usize)>,
    char_idx: usize,
    spans: &mut Vec<Span<'a>>,
    flush: &mut dyn FnMut(&mut String, bool, &mut Vec<Span<'a>>),
) {
    let selected = selection.is_some_and(|(a, b)| char_idx >= a && char_idx < b);
    if *buf_selected != selected && !buf.is_empty() {
        flush(buf, *buf_selected, spans);
    }
    *buf_selected = selected;
    if *disp >= coloff && *disp < coloff + width {
        buf.push(ch);
    }
    *disp += 1;
}

fn draw_status(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let name = app.doc.display_name();
    let dirty = if app.doc.is_dirty() { " [+]" } else { "" };
    let (line, col, words) = match app.doc.prose_surface() {
        Some(t) => (
            t.cursor_line() + 1,
            t.cursor_display_col() + 1,
            t.word_count(),
        ),
        None => (1, 1, 0),
    };
    let mode = match app.mode {
        Mode::Help => "HELP",
        Mode::ConfirmQuit => "EXIT?",
        Mode::Prompt(_) => "PROMPT",
        Mode::Normal => "EDIT",
    };
    let left = Line::from(vec![
        Span::styled(
            format!(" {name}{dirty}"),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::raw(format!("Ln {line}, Col {col}")),
        Span::raw(" │ "),
        Span::raw(format!("{words} words")),
    ]);
    let right = Line::from(Span::styled(
        format!("{mode} "),
        Style::default().fg(Color::DarkGray),
    ));
    let bar = ratatui::widgets::Paragraph::new(left).style(Style::default().bg(Color::Black));
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
            Paragraph::new(right).style(Style::default().bg(Color::Black)),
            right_area,
        );
    }
}

fn draw_message(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let content = if let Some(p) = app.prompt_display() {
        Span::styled(format!(" {p}"), Style::default().fg(ACCENT))
    } else if !app.message.is_empty() {
        Span::raw(format!(" {}", app.message))
    } else {
        Span::styled(
            " Ctrl+G help · Ctrl+S save · Ctrl+X exit",
            Style::default().fg(Color::DarkGray),
        )
    };
    frame.render_widget(Paragraph::new(Line::from(content)), area);
}

fn draw_help(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let inner_h = area.height.saturating_sub(2) as usize;
    // The help body is the packaged KEYMAP document so prose and bindings
    // stay in one place rather than diverging from the keymap table.
    let body = crate::help_text();
    let lines: Vec<Line<'static>> = body
        .lines()
        .take(inner_h)
        .map(|line| {
            if let Some(rest) = line.strip_prefix("| ") {
                // Soften markdown table rows so the popup stays readable.
                if rest.contains("---") && rest.chars().all(|c| matches!(c, '|' | '-' | ' ')) {
                    return Line::from("");
                }
            }
            if line.starts_with("# ") {
                return Line::styled(
                    line.trim_start_matches("# ").to_string(),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                );
            }
            if line.starts_with("## ") {
                return Line::styled(
                    line.trim_start_matches("## ").to_string(),
                    Style::default().fg(ACCENT),
                );
            }
            Line::from(line.to_string())
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — Esc or Ctrl+G to close ")
        .border_style(Style::default().fg(ACCENT));
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    let popup = centered_rect(80, 90, area);
    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(para, popup);
    let _ = app;
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
