//! Package-model helpers shared by open, save, and export paths.
//!
//! These free functions operate on `rdocx::Document` directly so that the
//! `RichDocument` impl block stays within the workspace file-size limit.

use std::path::Path;

use super::RichFormat;

/// Load a DOCX or ODT package into an `rdocx` model.
pub(super) fn load_model(
    path: &Path,
    format: RichFormat,
) -> Result<rdocx::Document, crate::error::DocumentError> {
    match format {
        RichFormat::Docx => rdocx::Document::open(path)
            .map_err(|err| crate::error::DocumentError::Parse(err.to_string())),
        RichFormat::Odt => rdocx::Document::open_odt(path)
            .map_err(|err| crate::error::DocumentError::Parse(err.to_string()))
            .map(|result| result.document),
    }
}

/// Plain-text projection of body paragraphs, one line per paragraph.
///
/// Table cell text is intentionally excluded: the TTY editor addresses body
/// paragraphs as lines, and interleaving tab-separated table rows would make
/// cursor motion ambiguous. Tables remain in the package model untouched.
pub(super) fn project_body_paragraphs(model: &rdocx::Document) -> String {
    let paragraphs = model.paragraphs();
    if paragraphs.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, p) in paragraphs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&p.text());
    }
    out
}

/// Heading level (1-9) of body paragraph `index`, from its style id.
/// Anything unstylized or exotic reads as no heading.
pub(super) fn paragraph_heading(model: &rdocx::Document, index: usize) -> Option<u8> {
    let style = model.paragraph(index)?.style_id()?.to_string();
    let level: u8 = style.strip_prefix("Heading")?.parse().ok()?;
    (1..=9).contains(&level).then_some(level)
}

/// Set body paragraph `index` to Heading `level`, creating nothing: the
/// caller syncs the surface first so fresh documents style correctly.
pub(super) fn set_paragraph_heading(
    model: &mut rdocx::Document,
    index: usize,
    level: u8,
) -> Result<(), String> {
    if !(1..=9).contains(&level) {
        return Err(format!("heading level must be 1-9, got {level}"));
    }
    let Some(mut paragraph) = model.paragraph_mut(index) else {
        return Err(format!("body paragraph {index} is out of range"));
    };
    paragraph.set_style(&format!("Heading{level}"));
    Ok(())
}

/// One hyperlink run within a paragraph: character range in the
/// paragraph text plus its resolved target, if external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    /// Char offset of the first character in the paragraph text.
    pub start: usize,
    /// Char offset one past the last character.
    pub end: usize,
    /// Resolved URL, or `None` for internal anchors and missing targets.
    pub target: Option<String>,
}

/// Hyperlink runs of body paragraph `index` with paragraph-relative
/// character ranges matching the projected surface lines. When the
/// walked text does not add up to the paragraph text, the spans come
/// back empty rather than misaligned.
pub(super) fn paragraph_links(model: &rdocx::Document, index: usize) -> Vec<LinkSpan> {
    let Some(paragraph) = model.paragraph(index) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for item in paragraph.items() {
        match item {
            rdocx::ParagraphItemRef::Run(run) => {
                offset += run.text().chars().count();
            }
            rdocx::ParagraphItemRef::Hyperlink(link) => {
                let len = link.text().chars().count();
                let target = link
                    .relationship_id()
                    .and_then(|rel| model.hyperlink_url(rel));
                spans.push(LinkSpan {
                    start: offset,
                    end: offset + len,
                    target,
                });
                offset += len;
            }
            _ => {}
        }
    }
    if offset != paragraph.text().chars().count() {
        return Vec::new();
    }
    spans
}

/// One styled run within a paragraph: character range plus flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStyle {
    /// Char offset of the first character in the paragraph text.
    pub start: usize,
    /// Char offset one past the last character.
    pub end: usize,
    /// Bold, italic, and underline flags in that order.
    pub flags: (bool, bool, bool),
}

/// Styled runs of body paragraph `index` with paragraph-relative
/// character ranges. Direct runs only; hyperlink contents keep link
/// styling. Like links, mismatched totals come back empty.
pub(super) fn paragraph_run_styles(model: &rdocx::Document, index: usize) -> Vec<RunStyle> {
    let Some(paragraph) = model.paragraph(index) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for item in paragraph.items() {
        match item {
            rdocx::ParagraphItemRef::Run(run) => {
                let len = run.text().chars().count();
                if run.is_bold() || run.is_italic() || run.underline_code_value().is_some() {
                    spans.push(RunStyle {
                        start: offset,
                        end: offset + len,
                        flags: (
                            run.is_bold(),
                            run.is_italic(),
                            run.underline_code_value().is_some(),
                        ),
                    });
                }
                offset += len;
            }
            rdocx::ParagraphItemRef::Hyperlink(link) => {
                offset += link.text().chars().count();
            }
            _ => {}
        }
    }
    if offset != paragraph.text().chars().count() {
        return Vec::new();
    }
    spans
}

/// Rewrite body paragraph `index` so its plain text equals `text`.
///
/// Empty or text-only paragraphs are updated in place through the public run
/// API. Paragraphs that contain non-text run content are left unchanged and
/// returned as an error so a save never silently drops drawings or fields.
pub(super) fn set_paragraph_text(
    model: &mut rdocx::Document,
    index: usize,
    text: &str,
) -> Result<(), String> {
    let (current, run_count) = match model.paragraph(index) {
        Some(p) => (p.text(), p.run_count()),
        None => return Ok(()),
    };
    if current == text {
        return Ok(());
    }
    // Reject paragraphs whose items are not plain runs: rewriting them would
    // destroy hyperlinks, equations, or drawings. The immutable view exposes
    // `items()`; the mutable view used below does not.
    if run_count > 0 {
        if let Some(p) = model.paragraph(index) {
            for item in p.items() {
                let safe = match item {
                    rdocx::ParagraphItemRef::Run(run) => !run_has_nontext(run),
                    _ => false,
                };
                if !safe {
                    return Err(format!(
                        "body paragraph {index} holds non-text content that cannot be rewritten in place"
                    ));
                }
            }
        }
    }
    let mut paragraph = model
        .paragraph_mut(index)
        .ok_or_else(|| format!("body paragraph {index} is out of range"))?;
    let run_count = paragraph.run_count();
    if run_count == 0 {
        if !text.is_empty() {
            paragraph.add_run(text);
        }
        return Ok(());
    }
    for i in 0..run_count {
        let Some(mut run) = paragraph.run_mut(i) else {
            return Err(format!(
                "body paragraph {index} run {i} is not directly mutable"
            ));
        };
        if i == 0 {
            run.set_text(text);
        } else {
            run.set_text("");
        }
    }
    Ok(())
}

fn run_has_nontext(run: rdocx::RunRef<'_>) -> bool {
    for item in run.items() {
        match item {
            rdocx::RunItemRef::Text(_) | rdocx::RunItemRef::Tab => {}
            _ => return true,
        }
    }
    false
}
