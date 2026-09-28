//! Ratatui rendering: editor viewport, status bar, message and prompt line.
//!
//! Layout is content-first: the text pane takes the full frame minus two rows,
//! one hairline status bar and one message row. Colors stay monochrome with a
//! single accent for the status filename and selection highlight. The text
//! pane marks page boundaries with a hairline rule carrying the page number,
//! using the same [`crate::page::PageLayout`] the PDF exporter chunks on.

mod browse;
#[cfg(feature = "xlsx")]
mod chart;
mod menu;
mod prose;
#[cfg(feature = "xlsx")]
mod sheet;
mod sidebar;
mod status;
mod tabs;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::sidebar::{MIN_WIDTH_FOR_SIDEBAR, SIDEBAR_WIDTH};
#[cfg(feature = "xlsx")]
use crate::app::PromptKind;
use crate::app::{App, Mode, ViewRects};

/// Style for selected ranges: reverse video for contrast without adding
/// a palette color, so every theme keeps readable selections.
fn selected_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
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
        text_w: text.width,
        text_h: text.height,
        side_x: side.map(|r| r.x).unwrap_or(0),
        side_w: side.map(|r| r.width).unwrap_or(0),
        tab_y,
    };
    if let Some(side) = side {
        sidebar::draw_sidebar(frame, app, side);
    }

    prose::draw_text(frame, app, text);
    status::draw_status(frame, app, status);
    draw_message(frame, app, message);
    menu::draw_dropdown(frame, app);
    if app.mode == Mode::Browse {
        browse::draw_browse(frame, app, body);
    }
    #[cfg(feature = "xlsx")]
    if app.mode == Mode::Chart {
        chart::draw_chart(frame, app, text);
    }

    if app.mode == Mode::Help {
        draw_help(frame, app, text);
    }
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
        #[cfg(feature = "xlsx")]
        let grid_edit = matches!(kind, PromptKind::CellEdit);
        #[cfg(not(feature = "xlsx"))]
        let grid_edit = {
            let _ = kind;
            false
        };
        if !grid_edit {
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
