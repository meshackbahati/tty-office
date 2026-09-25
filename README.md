# tty-office
Pure TTY Document Suite

> Experimental: tty-office is under active development toward a
> terminal-native document suite covering most of everyday word
> processing and spreadsheets. Until the first stable release,
> formats, key bindings, and behaviour may change; keep backups of
> important documents.

Word documents (ODT, DOCX), spreadsheets (XLSX, ODS, read-only XLS),
plain text, and Markdown, with PDF export, in one terminal session.
Tabs hold several documents at once; a menu bar, sidebar, and mouse
support sit beside the Nano-style keyboard contract.

## Run

```sh
cargo install --path .
tty-office notes.odt
```

Open `tty-office` without arguments for an untitled word document.
Press Ctrl+G for the full key list, F10 for the menu bar, and Ctrl+B
for the sidebar.

## Layout

- `src/`: the suite; `text/`, `rich/`, and `sheet/` own the three
  document surfaces, while `app/` and `ui/` own state and rendering.
- `docs/KEYMAP.md`: every binding, and the help overlay embeds it.
- `docs/FORMATS.md`: format support and dictionary setup.
- `tests/fixtures/`: the round-trip corpus, rebuilt with
  `cargo run --example make_fixtures`.

## Status

Spell checking underlines against system Hunspell dictionaries,
find and replace previews matches before applying them, and release
builds carry performance budgets for rendering and large files.
Images inside documents and tracked change histories are not
implemented yet; both are planned before stable.
