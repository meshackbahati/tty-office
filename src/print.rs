//! Export dispatcher: PDF, HTML, and Markdown writers.
//!
//! HTML and Markdown need no third-party crates and are always compiled. PDF
//! is split by document kind: rich packages use `rdocx` (requires `docx`);
//! plain text and sheets use `printpdf` (requires `pdf`). Format is inferred
//! from the destination path extension, mirroring save-as.

use std::path::Path;

use crate::document::Document;
use crate::editor::Editor;
use crate::error::DocumentError;
#[cfg(feature = "pdf")]
use crate::io::save_bytes_atomic;
use crate::io::save_str_atomic;

/// Export target format, selected by path extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Portable Document Format (`.pdf`).
    Pdf,
    /// HTML document (`.html`, `.htm`).
    Html,
    /// Markdown document (`.md`, `.markdown`).
    Markdown,
}

impl ExportFormat {
    /// Format implied by a path extension, if any.
    ///
    /// ```
    /// use std::path::Path;
    /// use tty_office::ExportFormat;
    /// assert_eq!(
    ///     ExportFormat::from_path(Path::new("out.pdf")),
    ///     Some(ExportFormat::Pdf)
    /// );
    /// assert_eq!(ExportFormat::from_path(Path::new("out.bin")), None);
    /// ```
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "pdf" => Some(Self::Pdf),
            "html" | "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

/// Write `doc` to `path`, inferring format from the extension.
///
/// The document path is not adopted; export never marks the buffer clean.
pub fn export(doc: &mut Document, path: &Path) -> Result<(), DocumentError> {
    let format = ExportFormat::from_path(path).ok_or_else(|| {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        DocumentError::UnsupportedFormat(format!("export to {ext}"))
    })?;
    match doc {
        Document::Text(t) => export_text(t, path, format),
        #[cfg(feature = "docx")]
        Document::Rich(r) => export_rich(r, path, format),
        #[cfg(feature = "xlsx")]
        Document::Sheet(s) => export_sheet(s, path, format),
    }
}

#[cfg(feature = "docx")]
fn export_rich(
    rich: &mut crate::rich::RichDocument,
    path: &Path,
    format: ExportFormat,
) -> Result<(), DocumentError> {
    rich.prepare_export()?;
    match format {
        ExportFormat::Pdf => rich.export_pdf(path),
        ExportFormat::Html => save_str_atomic(path, &rich.export_html()),
        ExportFormat::Markdown => save_str_atomic(path, &rich.export_markdown()),
    }
}

fn export_text(
    text: &crate::text::TextDocument,
    path: &Path,
    format: ExportFormat,
) -> Result<(), DocumentError> {
    let body = text.text_projection();
    match format {
        ExportFormat::Html => {
            let html = wrap_html(&text.display_name(), &escape_html(&body), true);
            save_str_atomic(path, &html)
        }
        ExportFormat::Markdown => save_str_atomic(path, &body),
        ExportFormat::Pdf => export_text_pdf(path, &body),
    }
}

#[cfg(feature = "xlsx")]
fn export_sheet(
    sheet: &mut crate::sheet::SheetDocument,
    path: &Path,
    format: ExportFormat,
) -> Result<(), DocumentError> {
    let tsv = sheet.text_projection();
    match format {
        ExportFormat::Html => {
            let html = tsv_to_html(&tsv);
            let page = wrap_html(&sheet.display_name(), &html, false);
            save_str_atomic(path, &page)
        }
        ExportFormat::Markdown => save_str_atomic(path, &tsv_to_markdown(&tsv)),
        ExportFormat::Pdf => export_text_pdf(path, &tsv.replace('\t', "    ")),
    }
}

/// Minimal HTML shell around an escaped body or table fragment.
fn wrap_html(title: &str, inner: &str, preformatted: bool) -> String {
    let body = if preformatted {
        format!("<pre>{inner}</pre>")
    } else {
        inner.to_string()
    };
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n</head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

/// Escape the five XML metacharacters so projection text cannot break markup.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Convert a TSV projection into a single HTML table.
#[cfg(feature = "xlsx")]
fn tsv_to_html(tsv: &str) -> String {
    let mut out = String::from("<table>\n");
    for line in tsv.lines() {
        out.push_str("<tr>");
        for cell in line.split('\t') {
            out.push_str("<td>");
            out.push_str(&escape_html(cell));
            out.push_str("</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>");
    out
}

/// Convert a TSV projection into a GitHub-flavoured pipe table.
#[cfg(feature = "xlsx")]
fn tsv_to_markdown(tsv: &str) -> String {
    let mut lines = tsv.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let cols = first.split('\t').count().max(1);
    let mut out = String::new();
    out.push('|');
    for _ in 0..cols {
        out.push_str(" |");
    }
    out.push_str("\n|");
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
    for line in std::iter::once(first).chain(lines) {
        out.push('|');
        for cell in line.split('\t') {
            out.push(' ');
            out.push_str(&cell.replace('|', "\\|"));
            out.push_str(" |");
        }
        out.push('\n');
    }
    out
}

/// Paginate plain text into a single-font PDF via `printpdf`.
#[cfg(feature = "pdf")]
fn export_text_pdf(path: &Path, body: &str) -> Result<(), DocumentError> {
    use printpdf::*;

    const LEFT_MM: f32 = 20.0;
    const TOP_MM: f32 = 277.0;
    const FOOT_MM: f32 = 12.0;

    // Chunk on the same page model the viewport draws rules from, so a page
    // break shown in the editor is the break that lands in the PDF.
    let layout = crate::page::PageLayout::default();
    let lines: Vec<String> = body.lines().map(sanitize_latin1).collect();
    let chunks: Vec<&[String]> = if lines.is_empty() {
        vec![&[]]
    } else {
        lines.chunks(layout.lines_per_page()).collect()
    };
    let total = chunks.len();

    let mut pages = Vec::with_capacity(chunks.len());
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut ops = vec![
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(LEFT_MM), Mm(TOP_MM)),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(11.0),
            },
            Op::SetLineHeight { lh: Pt(14.0) },
            Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    icc_profile: None,
                }),
            },
        ];
        for (i, line) in chunk.iter().enumerate() {
            if i > 0 {
                ops.push(Op::AddLineBreak);
            }
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(line.clone())],
            });
        }
        ops.push(Op::EndTextSection);
        // Footer: the page number sits at the foot of every sheet in the same
        // left margin as the body, since printpdf offers no centred text op
        // without measuring glyph widths first.
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(LEFT_MM), Mm(FOOT_MM)),
        });
        ops.push(Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(9.0),
        });
        ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.45,
                g: 0.45,
                b: 0.45,
                icc_profile: None,
            }),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(footer_text(idx + 1, total))],
        });
        ops.push(Op::EndTextSection);
        pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    }

    let mut base = PdfDocument::new("tty-office export");
    let doc = base.with_pages(pages);
    let mut warnings = Vec::new();
    // printpdf::save returns the serialized document directly; warnings
    // are advisory and discarded here because they do not fail the write.
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    save_bytes_atomic(path, &bytes)
}

/// Footer line printed at the foot of each exported page.
#[cfg(feature = "pdf")]
fn footer_text(page: usize, total: usize) -> String {
    format!("page {page} / {total}")
}

/// Without the `pdf` crate, plain-text PDF export fails with a clear message.
#[cfg(not(feature = "pdf"))]
fn export_text_pdf(path: &Path, _body: &str) -> Result<(), DocumentError> {
    Err(DocumentError::Save {
        path: path.to_path_buf(),
        message: "PDF export for plain text requires the `pdf` cargo feature".to_string(),
    })
}

/// Map non-Latin-1 characters to `?` so builtin Helvetica can render them.
#[cfg(feature = "pdf")]
fn sanitize_latin1(line: &str) -> String {
    line.chars()
        .map(|c| if (c as u32) < 256 { c } else { '?' })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "pdf")]
    #[test]
    fn footer_labels_page_of_total() {
        assert_eq!(super::footer_text(2, 7), "page 2 / 7");
        assert_eq!(super::footer_text(1, 1), "page 1 / 1");
    }
}
