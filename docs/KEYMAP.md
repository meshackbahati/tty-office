# tty-office key bindings

Defaults match the Nano contract plus word-processor shortcuts. User
overrides live in `~/.config/tty-office/config.toml` as an action name mapped
to a chord, for example `exit = "ctrl+q"`. Overrides merge with the defaults
in this file; they do not replace the whole table.

## Nano set

| Chord | Action |
|-------|--------|
| Ctrl+X | Exit. Prompts when the document has unsaved changes. |
| Ctrl+O | Save to the current path. |
| Ctrl+S | Quick save to the current path. |
| Ctrl+R | Insert a file at the cursor. |
| Ctrl+W | Where Is: find the next match. |
| Ctrl+F | Find the next match. |
| Ctrl+\\ | Replace: find, then replace all matches. |
| Ctrl+K | Cut the current line into the cutbuffer. |
| Ctrl+U | Uncut: paste the cutbuffer at the cursor. |
| Ctrl+C | Show line, column, word count, and modified state. |
| Ctrl+G | Get help. Esc or Ctrl+G closes the help overlay. |

## Word-processor set

| Chord | Action |
|-------|--------|
| Ctrl+Z | Undo the last edit unit. |
| Ctrl+Y | Redo the last undone unit. |
| Ctrl+A | Select the entire document. |
| Alt+B | Toggle Markdown bold (`**`) around the selection. |
| Alt+I | Toggle Markdown italic (`*`) around the selection. |
| Ctrl+P | Export. Full export paths arrive with Phase 5. |

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

## Quit confirmation

When the document is dirty, Ctrl+X opens a confirmation. Ctrl+S saves and
exits when the save succeeds. Ctrl+X discards and exits. Any other key
cancels and returns to editing.
