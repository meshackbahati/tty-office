//! Prose pane: line-oriented rendering shared by text and rich documents.
//!
//! Plain text scrolls horizontally past the edge; word documents wrap at
//! the content width, and every wrapped piece behaves like a display row
//! for scrolling, caret placement, and clicks. Page rules and zoom rows
//! consume viewport rows of their own in both modes.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::app::App;
use crate::page::PageLayout;
use crate::Theme;

use super::selected_style;

/// Snapshot of everything one prose frame derives from.
struct ProseView {
    selection: Option<(usize, usize)>,
    rowoff: usize,
    coloff: usize,
    cursor_line: usize,
    cursor_col: usize,
    line_count: usize,
    misspelled: Vec<(usize, usize)>,
    height: usize,
    width: usize,
    content_w: usize,
    step: usize,
    layout: PageLayout,
    theme: Theme,
    wrapped: bool,
}

/// Style for misspelled words: the theme error color plus underline.
fn misspelled_style(error: Color) -> Style {
    Style::default()
        .fg(error)
        .add_modifier(Modifier::UNDERLINED)
}

/// Per-character decoration for one rendered line.
#[derive(Clone)]
struct LineDecor<'a> {
    selection: Option<(usize, usize)>,
    misspelled: &'a [(usize, usize)],
    theme: Theme,
    heading: Option<u8>,
    links: Vec<(usize, usize)>,
}

/// Draw the text pane, dispatching wrapped and legacy renderers.
pub(super) fn draw_text(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    #[cfg(feature = "xlsx")]
    if matches!(app.doc, crate::Document::Sheet(_)) {
        super::sheet::draw_sheet(frame, app, area);
        return;
    }
    // Text and Rich share a line-oriented surface; Phase 4 adds a grid pane
    // for Sheet rather than forcing cells through this renderer.
    let Some(mut view) = snapshot(app, area) else {
        frame.render_widget(Paragraph::new(""), area);
        return;
    };
    let (lines, view_row) = if view.wrapped {
        wrapped_lines(app, &mut view)
    } else {
        legacy_lines(app, &mut view)
    };

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(view.theme.dim)),
    );
    frame.render_widget(paragraph, area);

    // Place the terminal cursor on the caret when it is inside the viewport,
    // counting the rule rows the caret's page boundary has inserted above it
    // plus the blank rows the zoom level adds after every text line.
    // The pane's top hairline holds the first screen row, so document
    // row `view_row` renders one row below it. Without the offset the
    // terminal cursor sits on the line above the one being typed, and
    // the final row would spill onto the status bar.
    if view_row + 1 < view.height {
        let disp = view.cursor_col.saturating_sub(view.coloff);
        // The left page border holds the first content column, so the
        // caret renders one cell to the right of the pane edge.
        if disp < view.content_w {
            let x = area.x + 1 + disp as u16;
            let y = area.y + 1 + view_row as u16;
            frame.set_cursor_position((x, y));
        }
    }
}

/// Read everything one frame needs, ending the surface borrow.
fn snapshot(app: &mut App, area: Rect) -> Option<ProseView> {
    let surface = app.doc.prose_surface()?;
    let view = ProseView {
        selection: surface.selection_range(),
        rowoff: surface.rowoff(),
        coloff: surface.coloff(),
        cursor_line: surface.cursor_line(),
        cursor_col: surface.cursor_display_col(),
        line_count: surface.line_count(),
        wrapped: surface.wrap_enabled(),
        misspelled: Vec::new(),
        height: area.height as usize,
        width: area.width as usize,
        content_w: (area.width as usize).saturating_sub(2),
        step: 0,
        layout: app.page_layout(),
        theme: app.theme(),
    };
    #[cfg(feature = "proof")]
    let misspelled: Vec<(usize, usize)> = app.misspelled_ranges().to_vec();
    #[cfg(not(feature = "proof"))]
    let misspelled: Vec<(usize, usize)> = Vec::new();
    let step = 1 + app.zoom_step();
    Some(ProseView {
        misspelled,
        step,
        ..view
    })
}

/// Display rows one line occupies in the wrapped renderer.
fn segs_of(app: &mut App, line: usize, content_w: usize) -> usize {
    app.doc
        .prose_surface()
        .map(|s| s.wrap_segments(line, content_w).len().max(1))
        .unwrap_or(1)
}

/// Legacy rendering for unwrapped surfaces, one display row per line.
fn legacy_lines(app: &mut App, view: &mut ProseView) -> (Vec<Line<'static>>, usize) {
    let height = view.height;
    let width = view.width;
    let content_w = view.content_w;
    let step = view.step;
    let layout = view.layout;
    let theme = view.theme;
    let mut decor = LineDecor {
        selection: view.selection,
        misspelled: &view.misspelled,
        theme,
        heading: None,
        links: Vec::new(),
    };
    // Break rules above page starts consume viewport rows of their own, so
    // pull the window down until the caret and its rules fit. The stored
    // scroll offset is left untouched: scrolling stays measured in text lines
    // and each frame derives the same display position deterministically.
    // Zoom levels add blank rows after every text line, so each line
    // occupies `step` display rows and the caret math counts them.
    while view.rowoff < view.cursor_line
        && (view.cursor_line - view.rowoff) * step
            + layout.breaks_between(view.rowoff, view.cursor_line)
            >= height
    {
        view.rowoff += 1;
    }
    let rowoff = view.rowoff;

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);
    let mut cur = rowoff;
    // Whether the rule above `cur` has already been pushed for this line;
    // without it the same boundary would repaint on every remaining row.
    let mut rule_drawn = false;
    while lines.len() < height {
        if !rule_drawn && cur < view.line_count && cur > rowoff && layout.is_page_start(cur) {
            lines.push(page_rule(&layout, cur, width, theme));
            rule_drawn = true;
            continue;
        }
        if cur >= view.line_count {
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
        decor.heading = app.doc.heading_at(cur);
        decor.links = app
            .doc
            .link_spans(cur)
            .iter()
            .map(|s| (s.start, s.end))
            .collect();
        lines.push(frame_line(
            render_line(
                &raw,
                line_start,
                view.coloff,
                content_w,
                &decor,
                cur == view.cursor_line,
                view.cursor_col,
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

    let view_row =
        (view.cursor_line - rowoff) * step + layout.breaks_between(rowoff, view.cursor_line);
    (lines, view_row)
}

/// Wrapped rendering for word documents: every piece behaves like a
/// display row for scrolling, caret placement, and clicks.
fn wrapped_lines(app: &mut App, view: &mut ProseView) -> (Vec<Line<'static>>, usize) {
    let height = view.height;
    let width = view.width;
    let content_w = view.content_w;
    let step = view.step;
    let layout = view.layout;
    let theme = view.theme;
    let mut decor = LineDecor {
        selection: view.selection,
        misspelled: &view.misspelled,
        theme,
        heading: None,
        links: Vec::new(),
    };
    // The caret piece is shared by the pull-down and the row math.
    let cursor_seg = match app.doc.prose_surface() {
        Some(s) => s.cursor_segment(content_w),
        None => 0,
    };
    // Pull the window down until the caret, its pieces, and its rules fit.
    while view.rowoff < view.cursor_line {
        let mut span = layout.breaks_between(view.rowoff, view.cursor_line);
        span += (view.rowoff..view.cursor_line)
            .map(|l| segs_of(app, l, content_w) * step)
            .sum::<usize>();
        span += cursor_seg * step;
        if span < height {
            break;
        }
        view.rowoff += 1;
    }
    let rowoff = view.rowoff;

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(height);
    let mut cur = rowoff;
    let mut rule_drawn = false;
    while lines.len() < height {
        if !rule_drawn && cur < view.line_count && cur > rowoff && layout.is_page_start(cur) {
            lines.push(page_rule(&layout, cur, width, theme));
            rule_drawn = true;
            continue;
        }
        if cur >= view.line_count {
            lines.push(blank_frame(width, theme));
            continue;
        }
        let Some((pieces, raw, line_start)) = (|| {
            let surface = app.doc.prose_surface()?;
            let pieces = surface.wrap_segments(cur, content_w);
            let raw = surface.line_text(cur);
            let start = surface.line_char_start(cur);
            Some((pieces, raw, start))
        })() else {
            break;
        };
        for piece in &pieces {
            let chunk: String = raw
                .chars()
                .skip(piece.start)
                .take(piece.end - piece.start)
                .collect();
            decor.heading = app.doc.heading_at(cur);
            decor.links = app
                .doc
                .link_spans(cur)
                .iter()
                .map(|s| (s.start, s.end))
                .collect();
            lines.push(frame_line(
                render_line(
                    &chunk,
                    line_start + piece.start,
                    0,
                    content_w,
                    &decor,
                    cur == view.cursor_line,
                    view.cursor_col,
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
            if lines.len() >= height {
                break;
            }
        }
        cur += 1;
        rule_drawn = false;
    }

    let mut view_row = layout.breaks_between(rowoff, view.cursor_line);
    view_row += (rowoff..view.cursor_line)
        .map(|l| segs_of(app, l, content_w) * step)
        .sum::<usize>();
    view_row += cursor_seg * step;
    (lines, view_row)
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
        } else if decor.links.iter().any(|&(a, b)| idx >= a && idx < b) {
            Style::default()
                .fg(decor.theme.accent)
                .add_modifier(Modifier::UNDERLINED)
        } else if let Some(level) = decor.heading {
            // Headings read bold; the top two levels take the accent.
            let mut style = Style::default().add_modifier(Modifier::BOLD);
            if level <= 2 {
                style = style.fg(decor.theme.accent);
            }
            style
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
        }
        disp += w;
        char_idx += 1;
        let _ = is_cursor_line;
    }
    flush(&mut buf, buf_style, &mut spans);
    Line::from(spans)
}
