//! Mouse: click-to-caret, drag selection, wheel scroll, and chrome hits.
//!
//! Coordinates are absolute frame cells; the view geometry recorded by
//! the last draw translates them into document positions. Clicks land
//! only in Normal mode, while the wheel scrolls anywhere, and a click
//! outside an open dropdown dismisses the menu before landing.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use unicode_width::UnicodeWidthStr;

#[cfg(feature = "xlsx")]
use crate::sheet::{CELL_STRIDE, ROW_GUTTER};
use crate::Document;

use super::menu::{dropdown, hit_item, hit_label, items};
use super::sidebar::rows;
use super::{App, Mode};

/// Drag origin: a prose char offset or a sheet cell.
#[derive(Clone, Copy)]
pub(crate) enum DragOrigin {
    Prose(usize),
    #[cfg(feature = "xlsx")]
    Cell((usize, usize)),
}

/// Wheel step in lines per notch.
const WHEEL_STEP: i32 = 3;

impl App {
    /// Handle one mouse event from the event loop.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.mouse_down(mouse.column, mouse.row, mouse.modifiers);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.mouse_drag(mouse.column, mouse.row);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag = None;
            }
            MouseEventKind::ScrollDown => self.mouse_wheel(WHEEL_STEP),
            MouseEventKind::ScrollUp => self.mouse_wheel(-WHEEL_STEP),
            _ => {}
        }
    }

    /// Left press: menu bar, dropdown, browser overlay, sidebar, tab
    /// strip, then document. A control press over a hyperlink opens it
    /// without moving the caret.
    fn mouse_down(&mut self, x: u16, y: u16, mods: KeyModifiers) {
        if y == 0 {
            if self.mode == Mode::Normal {
                if let Some(i) = hit_label(x) {
                    self.toggle_menu(i);
                } else {
                    self.open_menu = None;
                }
            }
            return;
        }
        if self.open_menu.is_some() {
            if self.mode == Mode::Normal {
                if let Some(menu) = self.open_menu {
                    let (bx, bw) = dropdown(menu);
                    let n = items(menu).len() as u16;
                    if y >= 1 && y < 1 + n + 2 && x >= bx && x < bx + bw {
                        if let Some(item) = hit_item(menu, y) {
                            self.menu_item = item;
                            self.activate_menu_item();
                        }
                        return;
                    }
                }
            }
            // A click outside the dropdown dismisses it and still lands.
            self.open_menu = None;
        }
        let view = self.view;
        // The browser owns body clicks while open; the tab strip keeps
        // working above it and the menu row stays inert.
        if self.mode == Mode::Browse {
            if y > view.text_y {
                self.browse_click(x, y);
            }
            return;
        }
        // The tab strip spans the full width above the body, so it wins
        // over the sidebar columns it visually overlaps.
        if let Some(tab_y) = view.tab_y {
            if y == tab_y {
                if self.mode == Mode::Normal {
                    if let Some(i) = self.hit_tab(x) {
                        self.switch_tab(i);
                    }
                }
                return;
            }
        }
        if view.side_w > 0 && y >= view.text_y && x >= view.side_x && x < view.side_x + view.side_w
        {
            if self.mode == Mode::Normal {
                self.focus = super::Focus::Sidebar;
                let list = rows(self);
                let rel = (y - view.text_y) as usize;
                if let Some(row) = list.get(rel) {
                    self.activate_row(row);
                }
            }
            return;
        }
        if self.mode != Mode::Normal {
            return;
        }
        if mods.contains(KeyModifiers::CONTROL) {
            // Control clicks open links in place; anything else selects
            // nothing and leaves the caret where it was.
            if let Some(target) = self.link_at(x, y) {
                match super::normal::open_url(&target) {
                    Ok(()) => {
                        self.message = format!("Opened {target}");
                    }
                    Err(err) => {
                        self.message = err;
                    }
                }
            }
            return;
        }
        match &mut self.doc {
            Document::Text(_) => self.prose_press(x, y),
            #[cfg(feature = "docx")]
            Document::Rich(_) => self.prose_press(x, y),
            #[cfg(feature = "xlsx")]
            Document::Sheet(_) => self.sheet_press(x, y),
        }
    }

    /// Left drag: extend the selection from the press origin.
    fn mouse_drag(&mut self, x: u16, y: u16) {
        if self.mode != Mode::Normal || self.open_menu.is_some() {
            return;
        }
        match self.drag {
            Some(DragOrigin::Prose(origin)) => self.prose_move(x, y, origin),
            #[cfg(feature = "xlsx")]
            Some(DragOrigin::Cell(origin)) => self.sheet_move(x, y, origin),
            None => {}
        }
    }

    /// Wheel notch: scroll the view without moving the caret, or move
    /// the browser selection while the overlay is open.
    fn mouse_wheel(&mut self, delta: i32) {
        if self.mode == Mode::Browse {
            if let Some(b) = self.browse.as_mut() {
                b.move_sel(delta);
            }
            return;
        }
        if let Some(surface) = self.doc.prose_surface() {
            surface.scroll_lines(delta);
        }
        #[cfg(feature = "xlsx")]
        if self.doc.prose_surface().is_none() {
            if let Document::Sheet(sheet) = &mut self.doc {
                sheet.scroll_rows(delta);
            }
        }
    }

    /// Click inside the browser overlay: select the row, activating when
    /// the row is already selected. The overlay covers the body rows, so
    /// the text pane origin doubles as the body origin.
    fn browse_click(&mut self, x: u16, y: u16) {
        let _ = x;
        let view = self.view;
        let content_h = (view.text_h as usize).saturating_sub(2);
        let (selected, len) = match &self.browse {
            Some(b) => (b.selected, b.visible().len()),
            None => return,
        };
        let rel = (y - view.text_y - 1) as usize;
        let idx = super::browse::BrowseState::offset(selected, len, content_h) + rel;
        if idx >= len {
            return;
        }
        let already = self
            .browse
            .as_ref()
            .map(|b| b.selected == idx)
            .unwrap_or(false);
        if let Some(b) = self.browse.as_mut() {
            b.selected = idx;
        }
        if already {
            self.browse_activate();
        }
    }

    /// Tab index whose strip cell contains column `x`, if any.
    fn hit_tab(&self, x: u16) -> Option<usize> {
        let mut at = 0u16;
        for i in 0..self.tab_count() {
            let w = self.tab_cell(i).map(|c| c.width() as u16).unwrap_or(0);
            if x >= at && x < at + w {
                return Some(i);
            }
            at += w;
        }
        None
    }

    /// Document line shown at `r` display rows below the pane hairline,
    /// counting the page-rule rows the draw loop inserts and the blank
    /// rows the zoom level adds after every text line. Clicks on a rule
    /// row snap to the line below it.
    fn display_row_to_line(&mut self, r: usize) -> Option<(usize, usize)> {
        let step = 1 + self.zoom_step();
        let width = (self.view.text_w as usize).saturating_sub(2).max(1);
        let surface = self.doc.prose_surface()?;
        let (rowoff, line_count, wrapped) = (
            surface.rowoff(),
            surface.line_count(),
            surface.wrap_enabled(),
        );
        if line_count == 0 {
            return None;
        }
        let layout = self.page_layout();
        if !wrapped {
            let mut cur = rowoff;
            let mut rule_drawn = false;
            let mut i = 0usize;
            loop {
                if !rule_drawn && cur < line_count && cur > rowoff && layout.is_page_start(cur) {
                    if i == r {
                        return Some((cur, 0));
                    }
                    rule_drawn = true;
                    i += 1;
                    continue;
                }
                if i == r {
                    return Some((cur.min(line_count - 1), 0));
                }
                if cur + 1 >= line_count {
                    return Some((line_count - 1, 0));
                }
                cur += 1;
                rule_drawn = false;
                i += 1;
            }
        }
        let mut cur = rowoff;
        let mut rule_drawn = false;
        let mut i = 0usize;
        loop {
            if !rule_drawn && cur < line_count && cur > rowoff && layout.is_page_start(cur) {
                if i == r {
                    return Some((cur, 0));
                }
                rule_drawn = true;
                i += 1;
                continue;
            }
            if cur >= line_count {
                let last = line_count - 1;
                let pieces = self
                    .doc
                    .prose_surface()
                    .map(|s| s.wrap_segments(last, width).len())
                    .unwrap_or(1);
                return Some((last, pieces.saturating_sub(1)));
            }
            let pieces = self
                .doc
                .prose_surface()
                .map(|s| s.wrap_segments(cur, width).len().max(1))
                .unwrap_or(1);
            if r < i + pieces * step {
                return Some((cur, (r - i) / step));
            }
            i += pieces * step;
            cur += 1;
            rule_drawn = false;
        }
    }

    /// Line, wrapped piece, and piece-relative column for a press at
    /// absolute `(x, y)`, when the cell maps into the prose surface.
    fn prose_position(&mut self, x: u16, y: u16) -> Option<(usize, usize, usize)> {
        let view = self.view;
        // The left page border owns the first pane column; it selects
        // nothing, and content starts one cell to its right.
        if y <= view.text_y || x <= view.text_x {
            return None;
        }
        let r = (y - view.text_y - 1) as usize;
        let content_rows = (view.text_h as usize).saturating_sub(1);
        if r >= content_rows {
            return None;
        }
        let (line, seg) = self.display_row_to_line(r)?;
        let surface = self.doc.prose_surface()?;
        if surface.wrap_enabled() {
            // Every piece renders from pane column zero, so the click
            // column is already piece-relative.
            let col = (x - view.text_x - 1) as usize;
            Some((line, seg, col))
        } else {
            let col = (x - view.text_x - 1) as usize + surface.coloff();
            Some((line, 0, col))
        }
    }

    /// Press on prose: place the caret and remember the drag origin.
    fn prose_press(&mut self, x: u16, y: u16) {
        if let Some((line, seg, col)) = self.prose_position(x, y) {
            self.focus = super::Focus::Text;
            if let Some(surface) = self.doc.prose_surface() {
                let width = (self.view.text_w as usize).saturating_sub(2).max(1);
                if surface.wrap_enabled() {
                    surface.click_wrapped(line, width, seg, col, false);
                } else {
                    surface.click_at(line, col);
                }
                let at = surface.cursor_char();
                self.drag = Some(DragOrigin::Prose(at));
            }
            self.scroll_to_cursor();
        }
    }

    /// Drag on prose: extend the selection from the press origin.
    fn prose_move(&mut self, x: u16, y: u16, origin: usize) {
        if let Some((line, seg, col)) = self.prose_position(x, y) {
            if let Some(surface) = self.doc.prose_surface() {
                let width = (self.view.text_w as usize).saturating_sub(2).max(1);
                if surface.wrap_enabled() {
                    surface.click_wrapped(line, width, seg, col, true);
                } else {
                    surface.click_at(line, col);
                }
                let at = surface.cursor_char();
                if at == origin {
                    surface.set_cursor(at, false);
                } else {
                    surface.set_cursor_range(origin.min(at), origin.max(at));
                }
            }
        }
    }

    /// Link target under the absolute cell `(x, y)`, for control
    /// clicks. Wrapped pieces render from pane column zero, so the
    /// piece-relative column converts back to a line-relative offset.
    fn link_at(&mut self, x: u16, y: u16) -> Option<String> {
        let (line, seg, col) = self.prose_position(x, y)?;
        let width = (self.view.text_w as usize).saturating_sub(2).max(1);
        let surface = self.doc.prose_surface()?;
        let rel = if surface.wrap_enabled() {
            let at = surface.wrapped_char_at(line, width, seg, col)?;
            at - surface.line_char_start(line)
        } else {
            surface.char_off_for_display_col(line, col)
        };
        self.doc
            .link_spans(line)
            .into_iter()
            .find(|s| rel >= s.0 && rel < s.1)
            .and_then(|s| s.2)
    }

    /// Grid cell for a press at absolute `(x, y)`, when the cell maps
    /// into the data area below the column-letter header. Border, rule,
    /// and header rows select nothing rather than a neighbour cell.
    #[cfg(feature = "xlsx")]
    fn sheet_cell_at(&mut self, x: u16, y: u16) -> Option<(usize, usize)> {
        let view = self.view;
        if y < view.text_y {
            return None;
        }
        // Row 0 is the pane hairline, row 1 the column-letter header,
        // row 2 its rule; data rows pair with a rule below, so odd rows
        // past the rule are rules as well.
        let r = (y - view.text_y) as usize;
        if r < 3 || r.is_multiple_of(2) {
            return None;
        }
        let (rowoff, coloff) = match &self.doc {
            Document::Sheet(sheet) => (sheet.rowoff(), sheet.coloff()),
            _ => return None,
        };
        if x < view.text_x + ROW_GUTTER as u16 {
            return None;
        }
        let rel = (x - view.text_x - ROW_GUTTER as u16) as usize;
        if rel.is_multiple_of(CELL_STRIDE) {
            return None;
        }
        Some((rowoff + (r - 3) / 2, coloff + rel / CELL_STRIDE))
    }

    /// Press on a sheet: gutter and header clicks select whole rows
    /// and columns, data clicks place the cell caret.
    #[cfg(feature = "xlsx")]
    fn sheet_press(&mut self, x: u16, y: u16) {
        let view = self.view;
        if y == view.text_y + 1 && x >= view.text_x + ROW_GUTTER as u16 {
            // Column-letter header row past the gutter.
            let rel = (x - view.text_x - ROW_GUTTER as u16) as usize;
            if !rel.is_multiple_of(CELL_STRIDE) {
                if let Document::Sheet(sheet) = &mut self.doc {
                    let col = sheet.coloff() + rel / CELL_STRIDE;
                    sheet.select_col(col);
                }
            }
            return;
        }
        if x < view.text_x + ROW_GUTTER as u16 && y >= view.text_y + 3 {
            // Row-number gutter on data rows, skipping rule rows.
            let r = (y - view.text_y) as usize;
            if r >= 3 && !r.is_multiple_of(2) {
                if let Document::Sheet(sheet) = &mut self.doc {
                    sheet.select_row(sheet.rowoff() + (r - 3) / 2);
                }
            }
            return;
        }
        if let Some((r, c)) = self.sheet_cell_at(x, y) {
            self.focus = super::Focus::Text;
            if let Document::Sheet(sheet) = &mut self.doc {
                sheet.click_cell(r, c);
            }
            self.drag = Some(DragOrigin::Cell((r, c)));
        }
    }

    /// Drag on a sheet: extend the selection from the press origin.
    #[cfg(feature = "xlsx")]
    fn sheet_move(&mut self, x: u16, y: u16, origin: (usize, usize)) {
        if let Some((r, c)) = self.sheet_cell_at(x, y) {
            if let Document::Sheet(sheet) = &mut self.doc {
                sheet.select_to(origin, (r, c));
            }
        }
    }
}
