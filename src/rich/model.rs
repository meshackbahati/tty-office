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
