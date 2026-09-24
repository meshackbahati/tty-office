//! Search and word count over the rope.

use super::TextDocument;

impl TextDocument {
    /// Search forward from `from` for `needle`, wrapping once.
    ///
    /// `from` is a character index into the rope. Returns the match range as
    /// character indices.
    pub fn find(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let text = self.rope.to_string();
        // Convert the character offset to a byte offset: `str::find` works in
        // bytes, while callers pass rope character indices.
        let byte_start = text
            .char_indices()
            .nth(from)
            .map(|(b, _)| b)
            .unwrap_or(text.len());
        if let Some(rel) = text[byte_start..].find(needle) {
            let abs = byte_start + rel;
            let char_at = text[..abs].chars().count();
            return Some((char_at, char_at + needle.chars().count()));
        }
        if byte_start > 0 {
            if let Some(rel) = text[..byte_start].find(needle) {
                let char_at = text[..rel].chars().count();
                return Some((char_at, char_at + needle.chars().count()));
            }
        }
        None
    }

    /// Search forward from `from` without wrapping. Same return shape as
    /// [`TextDocument::find`].
    pub fn find_no_wrap(&self, needle: &str, from: usize) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let text = self.rope.to_string();
        let byte_start = text
            .char_indices()
            .nth(from)
            .map(|(b, _)| b)
            .unwrap_or(text.len());
        let rel = text[byte_start..].find(needle)?;
        let abs = byte_start + rel;
        let char_at = text[..abs].chars().count();
        Some((char_at, char_at + needle.chars().count()))
    }

    /// Full-document word count, cached until the next edit.
    pub fn word_count(&mut self) -> usize {
        if let Some(n) = self.word_count {
            return n;
        }
        let mut count = 0usize;
        let mut in_word = false;
        for ch in self.rope.to_string().chars() {
            if ch.is_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                count += 1;
            }
        }
        self.word_count = Some(count);
        count
    }
}
