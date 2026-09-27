# tty-office
Pure TTY Document Suite

> Experimental: tty-office is under active development toward a
> terminal-native document suite covering most of everyday word
> processing and spreadsheets. Until the first stable release,
> formats, key bindings, and behaviour may change; keep backups of
> important documents.

Word documents (DOCX, ODT), spreadsheets (XLSX, ODS, XLS, CSV),
plain text, and Markdown, with PDF viewing and export, in one
terminal session. Writer and calc share tabs, a menu bar, a sidebar,
a file browser, mouse support, and one configuration, beside a
Nano-style keyboard contract where every shortcut is rebindable.
Headless subcommands (`cat`, `info`, `convert`) script the same
open and save paths without the interface.

## Run

```sh
cargo install --path .
tty-office notes.docx
```

Open `tty-office` without arguments for an untitled word document.
Press `Ctrl+G` for the live key list, `F10` for the menu bar,
`Ctrl+B` for the sidebar, `Ctrl+O` for the file browser, and `F6`
to move keyboard focus between text and sidebar. Headless use never
touches the terminal:

```sh
tty-office cat report.docx
tty-office info budget.xlsx
tty-office convert budget.xlsx budget.ods
```

A Linux binary rides each GitHub release for machines without a
Rust toolchain.

## Writing

Documents wrap at the viewport width inside a page frame with side
borders, double-rule page breaks, and a page readout; `Alt+PageUp`
and `Alt+PageDown` jump between pages and zoom adjusts line rhythm.
Headings 1–9 apply from `Ctrl+1..9` or the Format menu and persist
in the package, while bold, italic, underline, and hyperlinks read
from the document and render distinctly. Links open with `Ctrl+Enter`
or `Ctrl+click`. Five themes include a white Paper page, and the
help overlay always shows the bindings the user actually set.

## Calculating

The grid draws full borders with joints, echoes typing inside the
cursor cell, and places a visible caret. Formulas cover aggregates,
logic, text, and math with dependent-only recalculation, so typing
stays responsive on large sheets. `Ctrl+D` fills down and
`Ctrl+Shift+R` fills right with relative reference shifting;
`Alt+R` and `Alt+C` select whole rows and columns, as do gutter and
header clicks. Word motion, drag selection, and the wheel work
throughout, and every grid action is rebindable.

## How it differs

Terminal tools cluster around single jobs. Spreadsheet calculators
such as sc-im edit grids with Vim bindings but handle no prose
(sc-im man page, Debian), data arrangers such as VisiData explore
and reshape tables but are not document editors (VisiData manual),
console word processors such as WordGrinder write prose in their
own format without spreadsheet support (WordGrinder introduction),
and docx viewers such as doxx read Word files without editing them
(doxx crate documentation). General terminal editors cover plain
text and code only.

tty-office is a suite instead: writer and calc share one session,
one cutbuffer, one undo model per document, and one configuration.
A budget sheet and its supporting memo sit in adjacent tabs; either
can be converted headlessly in a pipeline.

## Layout

- `src/`: the suite; `text/`, `rich/`, and `sheet/` own the three
  document surfaces, while `app/` and `ui/` own state, chrome, and
  rendering. `keymap/` splits actions, chords, config, and lookup.
- `docs/KEYMAP.md`: every binding and config name with a rebinding guide.
- `docs/FORMATS.md`: format support and dictionary setup.
- `docs/FORMULAS.md`: tested spreadsheet functions and limits.
- `docs/PERF.md`: performance and memory budgets with measurements.
- `tests/fixtures/`: the round-trip corpus, rebuilt with
  `cargo run --example make_fixtures`.

## Status

Spell checking underlines against system Hunspell dictionaries,
find and replace previews matches before applying them, and release
builds carry performance budgets for rendering and large files.
Multi-sheet workbooks, per-sheet charts, inline images, and tracked
change histories are planned; circular references and unknown
functions already evaluate to cell errors rather than hangs.
