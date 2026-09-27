//! Word wrap: greedy line breaking for word-processor surfaces.
//!
//! Display widths come from `unicode-width`, so wide characters occupy
//! two columns exactly as the renderer draws them. Breaking prefers
//! spaces; words longer than the width break mid-word rather than
//! overflowing, and empty lines yield exactly one segment.

use unicode_width::UnicodeWidthChar;

/// One wrapped piece of a document line: char offsets within the line
/// plus the display width of the piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapSegment {
    /// Char offset of the first character, relative to the line start.
    pub start: usize,
    /// Char offset one past the last character, relative to line start.
    pub end: usize,
    /// Display width of the piece in columns.
    pub width: usize,
}

/// Split `line` (without its newline) into display-width pieces of at
/// most `width` columns. Degenerate widths yield one segment per
/// character so callers never divide by zero.
pub fn wrap_str(line: &str, width: usize) -> Vec<WrapSegment> {
    let width = width.max(1);
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return vec![WrapSegment {
            start: 0,
            end: 0,
            width: 0,
        }];
    }
    let mut segments = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        // Greedy fill: farthest break at or before the width edge.
        let mut used = 0usize;
        let mut end = start;
        let mut last_space: Option<usize> = None;
        while end < chars.len() {
            let w = chars[end].width().unwrap_or(0);
            if used + w > width {
                break;
            }
            if chars[end] == ' ' {
                last_space = Some(end);
            }
            used += w;
            end += 1;
        }
        if end == chars.len() {
            segments.push(segment(&chars, start, end));
            break;
        }
        match last_space {
            // Break after the space so the next piece starts clean;
            // the space itself belongs to neither piece visually, but
            // keeping it in the first keeps offsets contiguous.
            Some(at) if at > start => {
                segments.push(segment(&chars, start, at + 1));
                start = at + 1;
            }
            _ => {
                // No space on this row: break a long word mid-word.
                // A single wide character still advances one piece.
                let cut = if end == start { start + 1 } else { end };
                segments.push(segment(&chars, start, cut));
                start = cut;
            }
        }
    }
    segments
}

/// One segment over `chars[start..end]` with its measured width.
fn segment(chars: &[char], start: usize, end: usize) -> WrapSegment {
    let width = chars[start..end]
        .iter()
        .map(|c| c.width().unwrap_or(0))
        .sum();
    WrapSegment { start, end, width }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lines_stay_whole() {
        assert_eq!(
            wrap_str("hello", 20),
            vec![WrapSegment {
                start: 0,
                end: 5,
                width: 5
            }]
        );
    }

    #[test]
    fn breaks_prefer_spaces() {
        let segments = wrap_str("alpha beta gamma", 10);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, 0);
        // The greedy fill takes "alpha " (the break space belongs to
        // the first piece so offsets stay contiguous); "beta gamma"
        // starts clean at 6.
        assert_eq!((segments[0].end, segments[1].start), (6, 6));
    }

    #[test]
    fn long_words_break_mid_word() {
        let segments = wrap_str("abcdefghij", 4);
        assert_eq!(segments.len(), 3);
        assert_eq!((segments[0].start, segments[0].end), (0, 4));
        assert_eq!((segments[2].start, segments[2].end), (8, 10));
    }

    #[test]
    fn empty_lines_yield_one_segment() {
        assert_eq!(
            wrap_str("", 10),
            vec![WrapSegment {
                start: 0,
                end: 0,
                width: 0
            }]
        );
    }

    #[test]
    fn wide_characters_count_two_columns() {
        let segments = wrap_str("ab\u{6f22}cd", 4);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].width, 4);
    }
}
