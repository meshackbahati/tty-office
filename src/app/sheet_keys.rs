//! Spreadsheet-specific normal-mode key handling.

use crossterm::event::KeyEvent;

use crate::editor::{Editor, Motion};
use crate::keymap::Action;

use super::{App, Mode, PromptKind};

/// Points and title for the chart overlay.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChartData {
    /// Title naming the charted range, e.g. `B1:B5`.
    pub title: String,
    /// Points with x as the position in the range.
    pub points: Vec<(f64, f64)>,
}

impl App {
    pub(crate) fn is_sheet(&self) -> bool {
        matches!(self.doc, crate::Document::Sheet(_))
    }

    /// Close the chart overlay on Escape; other keys stay put.
    pub(crate) fn handle_chart(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            self.chart = None;
        }
    }

    /// Chart the selected numbers as a line graph overlay, or report
    /// when the selection holds none. A collapsed selection charts the
    /// cursor column over the used range.
    pub(crate) fn chart_selection(&mut self) {
        use crate::sheet::CellValue;
        let Some(sheet) = self.doc.sheet_mut() else {
            self.message = "Charts need a spreadsheet".to_string();
            return;
        };
        let ((r0, c0), (r1, c1)) = match sheet.selection_rect() {
            Some(((r0, c0), (r1, c1))) if r0 != r1 || c0 != c1 => ((r0, c0), (r1, c1)),
            _ => {
                let cc = sheet.cursor_cell().1;
                let (max_row, _) = sheet.used_range();
                ((0, cc), (max_row, cc))
            }
        };
        // Column-wise series: first numeric column wins, x follows rows.
        let mut col = c0;
        while col <= c1 {
            let series: Vec<(f64, f64)> = (r0..=r1)
                .filter_map(|r| match sheet.cell(r, col).map(|c| &c.value) {
                    Some(CellValue::Number(n)) => Some(*n),
                    _ => None,
                })
                .enumerate()
                .map(|(i, n)| (i as f64, n))
                .collect();
            if series.len() > 1 {
                let top = crate::sheet::SheetDocument::cell_ref(r0, col);
                let bottom = crate::sheet::SheetDocument::cell_ref(r1, col);
                self.chart = Some(ChartData {
                    title: format!("{top}:{bottom}"),
                    points: series,
                });
                self.mode = Mode::Chart;
                return;
            }
            col += 1;
        }
        self.message = "No numbers in selection".to_string();
    }

    /// Sheet-specific normal-mode keys. Returns true when handled.
    pub(crate) fn handle_sheet_normal(&mut self, action: Action, key: KeyEvent) -> bool {
        match action {
            Action::InsertNewline => {
                let (prefill, label) = match self.doc.sheet_mut() {
                    Some(sheet) => {
                        let (row, col) = sheet.cursor_cell();
                        (
                            sheet.cell_edit_content(),
                            format!("{}: ", crate::sheet::SheetDocument::cell_ref(row, col)),
                        )
                    }
                    None => (String::new(), "Cell: ".to_string()),
                };
                self.open_prompt(PromptKind::CellEdit, &label, prefill);
                true
            }
            Action::Insert('\t') => {
                self.apply_motion(Motion::Right, false);
                true
            }
            Action::Insert(c) if !c.is_control() => {
                self.open_prompt(PromptKind::CellEdit, "Cell: ", c.to_string());
                true
            }
            Action::Backspace | Action::DeleteForward => {
                self.doc.delete_back();
                true
            }
            Action::Uncut => {
                let buf = self.cutbuffer.clone();
                self.doc.insert_str(&buf);
                true
            }
            _ => {
                let _ = key;
                false
            }
        }
    }
}
