# tty-office
Pure TTY Document Suite

> Experimental: tty-office is under active development toward a
> terminal-native document suite covering most of everyday word
> processing and spreadsheets. Until the first stable release,
> formats, key bindings, and behaviour may change; keep backups of
> important documents.

Word documents (DOCX, ODT), spreadsheets (XLSX, ODS, XLS, CSV),
plain text, and Markdown, with PDF viewing and export, in one
terminal session. Tabs hold several documents at once; a menu bar,
sidebar, and mouse support sit beside the Nano-style keyboard
contract. Headless subcommands (`cat`, `info`, `convert`) script
the same open and save paths without the interface.

## Run

```sh
cargo install --path .
tty-office notes.odt
```

Open `tty-office` without arguments for an untitled word document.
Press Ctrl+G for the full key list, F10 for the menu bar, and Ctrl+B
for the sidebar. Headless use never touches the terminal:

```sh
tty-office cat report.docx
tty-office info budget.xlsx
tty-office convert budget.xlsx budget.ods
```

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
one clipboard, one undo model per document, and one configuration.
A budget sheet and its supporting memo sit in adjacent tabs; either
can be converted headlessly in a pipeline.

## Layout

- `src/`: the suite; `text/`, `rich/`, and `sheet/` own the three
  document surfaces, while `app/` and `ui/` own state and rendering.
- `docs/KEYMAP.md`: every binding, and the help overlay embeds it.
- `docs/FORMATS.md`: format support and dictionary setup.
- `docs/FORMULAS.md`: tested spreadsheet functions and limits.
- `docs/PERF.md`: performance and memory budgets with measurements.
- `tests/fixtures/`: the round-trip corpus, rebuilt with
  `cargo run --example make_fixtures`.

## Status

Spell checking underlines against system Hunspell dictionaries,
find and replace previews matches before applying them, and release
builds carry performance budgets for rendering and large files.
Images inside documents and tracked change histories are not
implemented yet; both are planned before stable.
