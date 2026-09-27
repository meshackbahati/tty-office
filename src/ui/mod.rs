//! Ratatui rendering: editor viewport, status bar, message and prompt line.
//!
//! Layout is content-first: the text pane takes the full frame minus two rows,
//! one hairline status bar and one message row. Colors stay monochrome with a
//! single accent for the status filename and selection highlight. The text
//! pane marks page boundaries with a hairline rule carrying the page number,
//! using the same [`crate::page::PageLayout`] the PDF exporter chunks on.

mod menu;
#[cfg(feature = "xlsx")]
mod sheet;
mod sidebar;
mod status;
mod tabs;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::app::sidebar::{MIN_WIDTH_FOR_SIDEBAR, SIDEBAR_WIDTH};
use crate::app::{App, Mode, PromptKind, ViewRects};
use crate::page::PageLayout;
use crate::Theme;

/// Style for selected ranges: reverse video for contrast without adding
/// a palette color, so every theme keeps readable selections.
fn selected_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// Style for misspelled words: the theme error color plus underline.
fn misspelled_style(error: Color) -> Style {
    Style::default()
        .fg(error)
        .add_modifier(Modifier::UNDERLINED)
}

/// Per-character decoration for one rendered line.
struct LineDecor<'a> {
    selection: Option<(usize, usize)>,
    misspelled: &'a [(usize, usize)],
    theme: Theme,
}

/// Draw one frame.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.height < 4 {
        return;
    }
    // The menu bar always takes the first row; the tab strip takes a row
    // of its own only once a second tab exists, and a single document
    // keeps that height for text. The sidebar takes a fixed column when
    // enabled and the frame is wide enough to spare it.
    let show_tabs = app.tab_count() > 1;
    if show_tabs && area.height < 5 {
        return;
    }
    let show_side = app.sidebar && area.width >= MIN_WIDTH_FOR_SIDEBAR;
    let mut constraints = vec![Constraint::Length(1)];
    if show_tabs {
        constraints.push(Constraint::Length(1));
    }
    constraints.extend([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let chunks = Layout::vertical(constraints).split(area);
    // Row 0 is the menu bar; the body starts below it and any tab row.
    let base = 1;
    let tab_y = if show_tabs {
        tabs::draw_tab_bar(frame, app, chunks[base]);
        Some(chunks[base].y)
    } else {
        None
    };
    let body = chunks[base + if show_tabs { 1 } else { 0 }];
    let (status, message) = (
        chunks[base + if show_tabs { 1 } else { 0 } + 1],
        chunks[base + if show_tabs { 1 } else { 0 } + 2],
    );
    let (side, text) = if show_side {
        let cols =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(1)]).split(body);
        (Some(cols[0]), cols[1])
    } else {
        (None, body)
    };

    menu::draw_menu_bar(frame, app, chunks[0]);

    let (text_h, text_w) = (text.height as usize, text.width as usize);
    app.set_viewport(text_h, text_w);
    app.view = ViewRects {
        text_x: text.x,
        text_y: text.y,
        text_h: text.height,
        side_x: side.map(|r| r.x).unwrap_or(0),
        side_w: side.map(|r| r.width).unwrap_or(0),
        tab_y,
    };
    if let Some(side) = side {
        sidebar::draw_sidebar(frame, app, side);
    }

    draw_text(frame, app, text);
    status::draw_status(frame, app, status);
    draw_message(frame, app, message);
    menu::draw_dropdown(frame, app);

    if app.mode == Mode::Help {
        draw_help(frame, app, text);
    }
}

fn draw_text(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    #[cfg(feature = "xlsx")]
    if matches!(app.doc, crate::Document::Sheet(_)) {
        sheet::draw_sheet(frame, app, area);
        return;
    }
    // Text and Rich share a line-oriented surface; Phase 4 adds a grid pane
    // for Sheet rather than forcing cells through this renderer.
    let (selection, mut rowoff, coloff, cursor_line, cursor_col, line_count) = {
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
    #[cfg(feature = "proof")]
    let misspelled: Vec<(usize, usize)> = app.misspelled_ranges().to_vec();
    #[cfg(not(feature = "proof"))]
    let misspelled: Vec<(usize, usize)> = Vec::new();
    let theme = app.theme();
    let decor = LineDecor {
        selection,
        misspelled: &misspelled,
        theme,
    };
    let height = area.height as usize;
    let width = area.width as usize;
    // Text occupies the pane between the page side borders.
    let content_w = width.saturating_sub(2);
    let layout = app.page_layout();

    // Break rules above page starts consume viewport rows of their own, so
    // pull the window down until the caret and its rules fit. The stored
    // scroll offset is left untouched: scrolling stays measured in text lines
    // and each frame derives the same display position deterministically.
    // Zoom levels add blank rows after every text line, so each line
    // occupies `step` display rows and the caret math counts them.
    let step = 1 + app.zoom_step();
    while rowoff < cursor_line
        && (cursor_line - rowoff) * step + layout.breaks_between(rowoff, cursor_line) >= height
    {
        rowoff += 1;
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);
    let mut cur = rowoff;
    // Whether the rule above `cur` has already been pushed for this line;
    // without it the same boundary would repaint on every remaining row.
    let mut rule_drawn = false;
    while lines.len() < height {
        if !rule_drawn && cur < line_count && cur > rowoff && layout.is_page_start(cur) {
            lines.push(page_rule(&layout, cur, width, theme));
            rule_drawn = true;
            continue;
        }
        if cur >= line_count {
            lines.push(blank_frame(width, theme));
            continue;
        }
        let Some((raw, line_start)) = (|| {
            let surface = app.doc.prose_surface()?;
            Some((surface.line_text(cur), surface.line_char_start(cur)))
        })() else {
            // The prose surface was checked before the loop; if it disappears
            // mid-frame the pane simply stops drawing rather than panicking.
            break;
        };
        lines.push(frame_line(
            render_line(
                &raw,
                line_start,
                coloff,
                content_w,
                &decor,
                cur == cursor_line,
                cursor_col,
            ),
            content_w,
            theme,
        ));
        for _ in 1..step {
            if lines.len() >= height {
                break;
            }
            lines.push(blank_frame(width, theme));
        }
        cur += 1;
        rule_drawn = false;
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.dim)),
    );
    frame.render_widget(paragraph, area);

    // Place the terminal cursor on the caret when it is inside the viewport,
    // counting the rule rows the caret's page boundary has inserted above it
    // plus the blank rows the zoom level adds after every text line.
    // The pane's top hairline holds the first screen row, so document
    // row `view_row` renders one row below it. Without the offset the
    // terminal cursor sits on the line above the one being typed, and
    // the final row would spill onto the status bar.
    let view_row = (cursor_line - rowoff) * step + layout.breaks_between(rowoff, cursor_line);
    if view_row + 1 < height {
        let disp = cursor_col.saturating_sub(coloff);
        // The left page border holds the first content column, so the
        // caret renders one cell to the right of the pane edge.
        if disp < content_w {
            let x = area.x + 1 + disp as u16;
            let y = area.y + 1 + view_row as u16;
            frame.set_cursor_position((x, y));
        }
    }
}

/// Hairline rule marking the boundary above `line`, carrying its page number.
/// The joints tie into the page side borders drawn around text rows.
fn page_rule(layout: &PageLayout, line: usize, width: usize, theme: Theme) -> Line<'static> {
    let label = format!(" page {} ", layout.page_of(line));
    let inner = width.saturating_sub(2);
    let fill = inner.saturating_sub(label.chars().count());
    let left = fill / 2;
    let right = fill - left;
    let text = format!("├{}{}{}┤", "─".repeat(left), label, "─".repeat(right));
    Line::styled(text, Style::default().fg(theme.dim))
}

/// Wrap a rendered text line in the page side borders, padding short
/// lines so the right border always lands on the pane edge.
fn frame_line<'a>(mut line: Line<'a>, content_w: usize, theme: Theme) -> Line<'a> {
    let pad = content_w.saturating_sub(line.width());
    let side = Style::default().fg(theme.dim);
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    spans.push(Span::styled("│", side));
    spans.append(&mut line.spans);
    spans.push(Span::styled(format!("{}│", " ".repeat(pad)), side));
    Line::from(spans)
}

/// Blank framed row for viewport space past the document end and for
/// the blank rows zoom levels insert between lines.
fn blank_frame(width: usize, theme: Theme) -> Line<'static> {
    let text = format!("│{}│", " ".repeat(width.saturating_sub(2)));
    Line::styled(text, Style::default().fg(theme.dim))
}

fn render_line<'a>(
    raw: &str,
    line_start: usize,
    coloff: usize,
    width: usize,
    decor: &LineDecor<'_>,
    is_cursor_line: bool,
    _cursor_col: usize,
) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut disp = 0usize;
    let mut char_idx = line_start;
    let mut buf = String::new();
    let mut buf_style = Style::default();

    let style_for = |idx: usize| -> Style {
        let selected = decor.selection.is_some_and(|(a, b)| idx >= a && idx < b);
        if selected {
            selected_style()
        } else if decor.misspelled.iter().any(|&(a, b)| idx >= a && idx < b) {
            misspelled_style(decor.theme.error)
        } else {
            Style::default()
        }
    };

    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'a>>| {
        if buf.is_empty() {
            return;
        }
        spans.push(Span::styled(std::mem::take(buf), style));
    };

    for ch in raw.chars() {
        if ch == '\t' {
            // One rope character expands to up to four visual spaces; all
            // of them share the tab's character index for decoration.
            let tab_w = 4 - (disp % 4);
            let style = style_for(char_idx);
            for _ in 0..tab_w {
                if style != buf_style && !buf.is_empty() {
                    flush(&mut buf, buf_style, &mut spans);
                }
                buf_style = style;
                if disp >= coloff && disp < coloff + width {
                    buf.push(' ');
                }
                disp += 1;
            }
            char_idx += 1;
            continue;
        }
        let w = ch.width().unwrap_or(0);
        if disp + w > coloff + width && disp >= coloff {
            break;
        }
        let style = style_for(char_idx);
        if style != buf_style && !buf.is_empty() {
            flush(&mut buf, buf_style, &mut spans);
            buf_style = style;
        }
        if buf.is_empty() {
            buf_style = style;
        }
        if disp >= coloff && disp + w <= coloff + width {
            buf.push(ch);
        } else if disp >= coloff {
            // Wide character straddling the right edge: pad with spaces.
            for _ in 0..(coloff + width - disp) {
                buf.push(' ');
            }
            flush(&mut buf, buf_style, &mut spans);
            break;
        }
        disp += w;
        char_idx += 1;
        let _ = is_cursor_line;
    }
    flush(&mut buf, buf_style, &mut spans);
    Line::from(spans)
}

fn draw_message(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = app.theme();
    let content = if let Some(p) = app.prompt_display() {
        Span::styled(format!(" {p}"), Style::default().fg(theme.accent))
    } else if !app.message.is_empty() {
        Span::raw(format!(" {}", app.message))
    } else {
        Span::styled(
            " Ctrl+G help · Ctrl+S save · Ctrl+X exit",
            Style::default().fg(theme.dim),
        )
    };
    frame.render_widget(Paragraph::new(Line::from(content)), area);
    // The caret follows the prompt text, except while a cell edit shows
    // it inside the grid instead.
    if let Mode::Prompt(kind) = &app.mode {
        if !matches!(kind, PromptKind::CellEdit) {
            use unicode_width::UnicodeWidthStr;
            let prefix: String = app.prompt_buf.chars().take(app.prompt_cursor).collect();
            let at = 1 + app.prompt_label.width() + prefix.width();
            let x = (area.x + at as u16).min(area.x + area.width.saturating_sub(1));
            frame.set_cursor_position((x, area.y));
        }
    }
}

fn draw_help(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let theme = app.theme();
    let inner_h = area.height.saturating_sub(2) as usize;
    // The table renders the live keymap, so rebound shortcuts show what
    // the user actually set rather than the packaged defaults; the name
    // column names the config key that rebinds each row.
    let mut lines: Vec<Line<'static>> = app
        .keymap
        .help_rows()
        .into_iter()
        .map(|(chord, name, blurb)| {
            Line::from(vec![
                Span::styled(format!("{chord:<14}"), Style::default().fg(theme.accent)),
                Span::styled(format!("{name:<22}"), Style::default().fg(theme.dim)),
                Span::raw(blurb),
            ])
        })
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Prompts".to_string(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(
        "  Enter confirms · Esc cancels · Backspace deletes a character",
    ));
    lines.push(Line::from(
        "  Menus: Alt+F/E/V/H or F10 · arrows move · Enter runs · Esc closes",
    ));
    lines.push(Line::from(
        "  Rebind anything above in ~/.config/tty-office/config.toml as name = \"chord\"",
    ));
    lines.push(Line::from(
        "  Quit: Ctrl+X prompts on dirty tabs · Ctrl+S saves · Ctrl+X again discards",
    ));
    let lines: Vec<Line<'static>> = lines.into_iter().take(inner_h).collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — Esc or Ctrl+G to close ")
        .border_style(Style::default().fg(theme.accent));
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
