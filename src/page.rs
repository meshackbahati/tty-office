//! Page geometry shared by the viewport and the PDF exporter.
//!
//! The terminal has no paper size, so a page is modelled as a fixed number of
//! logical lines: the same count the printpdf exporter chunks on, with the
//! result that a break drawn in the viewport is a break printed in the PDF.
//! The editor does not soft-wrap lines, which makes this mapping exact rather
//! than approximate.

/// Page model for prose documents.
///
/// A layout is a plain value with no failure mode: [`PageLayout::new`] clamps
/// degenerate input instead of returning an error, because page setup must
/// never prevent a document from opening.
///
/// ```
/// use tty_office::PageLayout;
///
/// let layout = PageLayout::new(12);
/// assert_eq!(layout.page_of(0), 1);
/// assert_eq!(layout.page_of(12), 2);
/// assert_eq!(layout.page_count(13), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLayout {
    /// Lines per page; always at least one so pagination never divides by zero.
    lines_per_page: usize,
}

impl Default for PageLayout {
    /// The default layout: [`PageLayout::DEFAULT_LINES`] lines per page, which
    /// is also the chunk height the PDF exporter uses.
    fn default() -> Self {
        Self::new(Self::DEFAULT_LINES)
    }
}

impl PageLayout {
    /// Logical lines in one page. Screen and print both derive from this
    /// number, which is why on-screen breaks match printed ones.
    pub const DEFAULT_LINES: usize = 50;

    /// Build a layout, clamping `lines_per_page` to at least one.
    pub fn new(lines_per_page: usize) -> Self {
        Self {
            lines_per_page: lines_per_page.max(1),
        }
    }

    /// Lines per page in this layout.
    pub fn lines_per_page(&self) -> usize {
        self.lines_per_page
    }

    /// One-based page number containing zero-based `line`.
    pub fn page_of(&self, line: usize) -> usize {
        line / self.lines_per_page + 1
    }

    /// Total pages for `line_count` logical lines; an empty document still
    /// counts as one page, since it would print as one blank sheet.
    pub fn page_count(&self, line_count: usize) -> usize {
        line_count.div_ceil(self.lines_per_page).max(1)
    }

    /// Whether a page-break rule is drawn above zero-based `line`. Line zero
    /// never gets a rule because the top of the document is not a break.
    pub fn is_page_start(&self, line: usize) -> bool {
        line > 0 && line.is_multiple_of(self.lines_per_page)
    }

    /// Number of break rules strictly after `from` and up to and including
    /// `to`, which is exactly how many rows the rules consume between two
    /// text lines in the viewport.
    pub fn breaks_between(&self, from: usize, to: usize) -> usize {
        to / self.lines_per_page - from / self.lines_per_page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_of_is_one_based_at_boundaries() {
        let l = PageLayout::new(50);
        assert_eq!(l.page_of(0), 1);
        assert_eq!(l.page_of(49), 1);
        assert_eq!(l.page_of(50), 2);
        assert_eq!(l.page_of(99), 2);
        assert_eq!(l.page_of(100), 3);
    }

    #[test]
    fn page_count_is_at_least_one() {
        let l = PageLayout::new(50);
        assert_eq!(l.page_count(0), 1);
        assert_eq!(l.page_count(1), 1);
        assert_eq!(l.page_count(50), 1);
        assert_eq!(l.page_count(51), 2);
        assert_eq!(l.page_count(100), 2);
        assert_eq!(l.page_count(101), 3);
    }

    #[test]
    fn new_clamps_degenerate_input() {
        assert_eq!(PageLayout::new(0).lines_per_page(), 1);
        assert_eq!(
            PageLayout::default().lines_per_page(),
            PageLayout::DEFAULT_LINES
        );
    }

    #[test]
    fn no_rule_above_the_first_line() {
        let l = PageLayout::new(10);
        assert!(!l.is_page_start(0));
        assert!(l.is_page_start(10));
        assert!(!l.is_page_start(11));
        assert!(l.is_page_start(20));
    }

    #[test]
    fn breaks_between_counts_interior_boundaries() {
        let l = PageLayout::new(50);
        // The rule above line 50 lies after line 0 and up to line 50.
        assert_eq!(l.breaks_between(0, 50), 1);
        // Once the window starts at the boundary itself the rule has scrolled
        // off with the previous page, so nothing is consumed.
        assert_eq!(l.breaks_between(50, 50), 0);
        assert_eq!(l.breaks_between(0, 49), 0);
        assert_eq!(l.breaks_between(40, 60), 1);
        assert_eq!(l.breaks_between(0, 150), 3);
    }
}
