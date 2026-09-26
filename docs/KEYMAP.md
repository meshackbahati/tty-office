# tty-office key bindings

Defaults match the Nano contract plus word-processor shortcuts. User
overrides live in `~/.config/tty-office/config.toml` as an action name mapped
to a chord, for example `exit = "ctrl+q"`. Overrides merge with the defaults
in this file; they do not replace the whole table.

## Nano set

One deliberate departure: Nano uses Ctrl+O to write out, but tty-office
follows the graphical convention and opens files with Ctrl+O, while
Ctrl+S saves. Muscle memory from VS Code transfers directly.

| Chord | Action |
|-------|--------|
| Ctrl+X | Exit. Prompts when any tab has unsaved changes. |
| Ctrl+O | Open a file in a new tab. |
| Ctrl+N | New tab with an untitled word document. |
| Ctrl+S | Save; folds into Save As for a pathless document. |
| Ctrl+Shift+S | Save As. |
| Ctrl+Shift+N | New tab with an untitled text file. |
| Ctrl+Shift+E | New tab with an untitled spreadsheet. |
| Ctrl+R | Insert a file at the cursor. |
| Ctrl+W | Where Is: find the next match. |
| Ctrl+F | Find the next match. |
| Ctrl+\\ | Replace: find, then replace all matches. |
| Ctrl+K | Cut the current line into the cutbuffer. |
| Ctrl+U | Uncut: paste the cutbuffer at the cursor. |
| Ctrl+C | Show line, column, word count, and modified state. |
| Ctrl+G | Get help. Esc or Ctrl+G closes the help overlay. |
| Ctrl+B | Show or hide the sidebar. |

## Word-processor set

| Chord | Action |
|-------|--------|
| Ctrl+Z | Undo the last edit unit. |
| Ctrl+Y | Redo the last undone unit. |
| Ctrl+A | Select the entire document. |
| Alt+B | Toggle Markdown bold (`**`) around the selection. |
| Alt+I | Toggle Markdown italic (`*`) around the selection. |
| Ctrl+P | Export to PDF, HTML, or Markdown. Format follows the path extension. |

## Motion and selection

| Chord | Action |
|-------|--------|
| Left, Right, Up, Down | Move the cursor. |
| Shift + arrows | Extend the selection. |
| Home | Start of line. |
| End | End of line. |
| Ctrl+Home | Start of document. |
| Ctrl+End | End of document. |
| PageUp, PageDown | Move one viewport height. |
| Enter | Insert a newline. |
| Tab | Insert a tab. |
| Backspace | Delete the selection or the character before the cursor. |
| Delete | Delete the selection or the character after the cursor. |

## Prompt keys

| Chord | Action |
|-------|--------|
| Enter | Confirm the prompt. |
| Esc | Cancel the prompt. |
| Backspace | Delete the last prompt character. |
| Printable keys | Append to the prompt buffer. |

## Spreadsheet keys

When the open document is a spreadsheet, normal-mode keys are interpreted as
cell operations rather than character insertion.

| Chord | Action |
|-------|--------|
| Enter | Open the cell edit prompt for the cursor cell. |
| Tab | Move the cursor one column to the right. |
| Printable keys | Start the cell edit prompt with that character. |
| Backspace, Delete | Clear the selection or delete toward the cursor. |
| Ctrl+U | Uncut: paste the cutbuffer into the cell edit. |
| Ctrl+C | Show the cell reference (for example `Cell A1`). |

Inside the cell edit prompt, Enter commits the value and moves down one row;
an empty commit clears the cell. Esc cancels without changing the cell.

## Quit confirmation

When any tab is dirty, Ctrl+X opens a confirmation. Ctrl+S saves the
visible tab and exits once every tab is clean. Ctrl+X discards all
tabs and exits. Any other key cancels and returns to editing.
Closing the last clean tab with Ctrl+F4 quits the same way.

## Tabs

Several documents share one session; the strip above the text pane
appears once a second tab exists. New and Open add tabs, so nothing
is ever replaced, and each tab keeps its own cursor, scroll, and
dirty state.

| Chord | Action |
|-------|--------|
| Alt+Left, Alt+Right | Previous and next tab, wrapping around. |
| Ctrl+PageUp, Ctrl+PageDown | Previous and next tab. |
| Alt+1 to Alt+9 | Jump to the numbered tab. |
| Ctrl+F4 | Close the active tab. Refuses while dirty. |

## Menu bar

The bar always occupies the first row. Alt+letter pulls down a menu
and F10 opens File; while a menu is open, every key is consumed, so
typing can never leak into the document.

| Chord | Action |
|-------|--------|
| Alt+F, Alt+E, Alt+V, Alt+H | Open the File, Edit, View, and Help menus. |
| F10 | Open the File menu. |
| Up, Down | Move the highlight. |
| Left, Right | Switch to the neighbouring menu. |
| Enter | Run the highlighted item. |
| Esc or any other key | Dismiss the menu. |
| Letter | Switch to the menu whose name starts with it. |

## Sidebar

The sidebar lists file creation entries for every format, common
commands, and the open tabs. Ctrl+B toggles it, and terminals
narrower than 60 columns hide it to protect the text pane. Every
entry mirrors a menu or key action; clicking a tab jumps to it.

## Mouse

Clicking places the caret in text and on cells in grids, dragging
extends the selection, and the wheel scrolls without moving the
caret. The menu bar, dropdown items, sidebar rows, and tab cells
are all clickable; a click outside an open dropdown dismisses it
and still lands. Mouse reporting turns on with the session and is
released on exit and on crashes.

## Zoom

Zoom adjusts line rhythm, which is the part of text size an
application can control inside a terminal; glyph size belongs to
the terminal emulator, whose own font shortcut enlarges text
further. The status bar shows the percentage while zoomed.

| Chord | Action |
|-------|--------|
| Ctrl+=, Ctrl++ | Zoom in, up to 200%. |
| Ctrl+- | Zoom out. |
| Ctrl+0 | Reset zoom to 100%. |

## Themes

Four chrome palettes ship with the suite: Mono, Ocean, Ember, and
Forest. Alt+T steps through them for the session, and
`theme = "ocean"` in `~/.config/tty-office/config.toml` sets the
default; unknown names fall back to Mono. The sidebar names the
current theme, and its row cycles as well.
