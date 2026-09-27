# tty-office format support

This document records which file formats the suite reads, writes, and
exports, and which behaviours are deliberately out of scope. It is the
authoritative support matrix: where this file and the code disagree, the
code is correct and this file has fallen behind.

## Opening and saving

| Format | Extension | Read | Save | Cargo feature | Backend |
|---|---|---|---|---|---|
| Plain text | `.txt` | yes | yes | none | ropey |
| Markdown | `.md`, `.markdown` | yes | yes | none | ropey |
| Word package | `.docx` | yes | yes | `docx` | rdocx |
| OpenDocument text | `.odt` | yes | yes | `docx` | rdocx |
| Excel workbook | `.xlsx` | yes | yes | `xlsx` | calamine plus umya-spreadsheet |
| Excel legacy | `.xls` | yes | no | `xlsx` | calamine (read-only by design) |
| OpenDocument sheet | `.ods` | yes | yes | `xlsx` | spreadsheet-ods |
| Comma-separated values | `.csv` | yes | yes | `xlsx` | built-in parser; the grid is the sole storage |
| Portable Document Format | `.pdf` | view as text | no | `pdf` | lopdf extraction |

Documents with no extension open as plain text. A new file that does not
yet exist on disk adopts the path implied by its extension and creates an
empty document of the matching kind, except `.pdf`, which cannot be
written by typing and starts untitled text that Save As will rename.

CSV quoting follows the common rule: fields holding commas, quotes,
or line breaks quote, and inner quotes double. Formulas travel as
their `=` text and re-evaluate on open. PDF viewing extracts page
text without layout; the buffer carries no path, so saving a viewed
PDF always passes through Save As instead of overwriting the file
with plain text.

Round-trip fidelity follows one rule throughout: content the editor does
not model is preserved rather than rewritten. Paragraphs of a rich
document that contain drawings, equations, or hyperlink runs are refused
for in-place editing and reported as an error, so a save can never
silently drop producer content. Workbook saves pass through the whole
package, which keeps charts, images, styles, and unparsed blobs intact.

## Export

| Target | Extension | Source documents | Cargo feature |
|---|---|---|---|
| PDF (plain text, sheets) | `.pdf` | text, markdown, sheets | `pdf` |
| PDF (rich documents) | `.pdf` | docx, odt | `docx` |
| HTML | `.html` | all | none |
| Markdown | `.md`, `.markdown` | all | none |

Export never adopts the destination path and never clears the dirty flag.
The export prompt (`Ctrl+P`) infers the format from the path extension
and rejects unknown extensions with `UnsupportedFormat`.

Plain-text PDF export paginates on the shared page model described
below, prints a page-number footer on every sheet, and substitutes `?`
for characters Helvetica cannot render. Rich PDF export routes through
the rdocx conversion chain.

## Page model

A page is a fixed number of logical lines, fifty by default, shared
identically by the viewport page rules, the status bar readout, and the
PDF exporter. The number is configurable as `page_lines` in
`~/.config/tty-office/config.toml`. Word documents wrap lines at the
viewport width, so long paragraphs occupy several display rows while
the page model keeps counting logical lines; plain text and Markdown
never wrap and scroll horizontally instead, which keeps their screen
mapping exact. Physical paper measurement is not attempted in the
terminal; exact print pagination lives in the PDF output.

## Proofing

Spell checking requires the `proof` feature and a Hunspell dictionary on
the system search path: `TTY_OFFICE_DICT` (a base path or an `.aff`
path whose sibling `.dic` is used), then `/usr/share/hunspell` and
`/usr/share/myspell/dicts` for the `en_US` locale. The document is
checked on a background worker thread; misspellings paint
red-underlined and a count appears in the status bar when it is
nonzero. Suggestions are not offered, because the upstream suggestion
API is marked work in progress.

## Explicitly out of scope

The following are permanent non-goals, accepted from the boundaries the
upstream crates document:

- VBA macros, ActiveX controls, OLE objects, and add-in execution.
- Binary `.doc` and Word 2003 XML documents.
- Presentation and drawing formats (Impress, Draw, PowerPoint).
- Spreadsheet chart and image editing; they survive round-trip but
  cannot be modified.
- Formula evaluation beyond what formualizer implements; the sheet
  editor reports unsupported functions rather than guessing.

## Planned but not yet implemented

- Inline images in the TTY viewport (Phase 6).
- Table of contents outline, headers and footers preview bar.
- Writer-side tables with shared calculation.
- Proofing suggestions behind a feature flag.
- PDF header lines above the body of each page.
