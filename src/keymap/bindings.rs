//! Default chord table: the Nano contract plus word-processor shortcuts.
//!
//! The table lives apart from the matcher so neither file approaches the
//! size where navigation suffers; prompts stay unbound because the
//! application layer handles them.

use super::{Action, Chord};

/// Default chord to action table implementing the Nano contract.
///
/// Prompt-local chords (`Confirm`, `Cancel`, `PromptBackspace`) are handled by
/// the application layer rather than this table, so they are not bound here.
pub(crate) fn default_bindings() -> Vec<(Chord, Action)> {
    use crate::editor::Motion;
    use crossterm::event::KeyCode as K;
    use crossterm::event::KeyModifiers;
    let m = KeyModifiers::CONTROL;
    let a = KeyModifiers::ALT;
    let s = KeyModifiers::SHIFT;
    let n = KeyModifiers::NONE;
    vec![
        (Chord::new(K::Char('x'), m), Action::Exit),
        (Chord::new(K::Char('o'), m), Action::Open),
        (Chord::new(K::Char('n'), m), Action::New),
        (Chord::new(K::Char('n'), m | s), Action::NewText),
        (Chord::new(K::Char('e'), m | s), Action::NewSheet),
        (Chord::new(K::Char('b'), m), Action::ToggleSidebar),
        (Chord::new(K::Char('='), m), Action::ZoomIn),
        (Chord::new(K::Char('+'), m), Action::ZoomIn),
        (Chord::new(K::Char('-'), m), Action::ZoomOut),
        (Chord::new(K::Char('0'), m), Action::ZoomReset),
        (Chord::new(K::Char('t'), a), Action::CycleTheme),
        (Chord::new(K::Char('d'), m), Action::FillDown),
        (Chord::new(K::Char('1'), m), Action::ApplyHeading(1)),
        (Chord::new(K::Char('2'), m), Action::ApplyHeading(2)),
        (Chord::new(K::Char('3'), m), Action::ApplyHeading(3)),
        (Chord::new(K::Char('4'), m), Action::ApplyHeading(4)),
        (Chord::new(K::Char('5'), m), Action::ApplyHeading(5)),
        (Chord::new(K::Char('6'), m), Action::ApplyHeading(6)),
        (Chord::new(K::Char('7'), m), Action::ApplyHeading(7)),
        (Chord::new(K::Char('8'), m), Action::ApplyHeading(8)),
        (Chord::new(K::Char('9'), m), Action::ApplyHeading(9)),
        (Chord::new(K::Enter, m), Action::OpenLink),
        (Chord::new(K::PageUp, a), Action::PrevPage),
        (Chord::new(K::PageDown, a), Action::NextPage),
        (Chord::new(K::F(6), n), Action::FocusNext),
        (Chord::new(K::Char('r'), a), Action::SelectRow),
        (Chord::new(K::Char('c'), a), Action::SelectCol), // Uppercase without a SHIFT flag: terminals that report Shift
        // with Ctrl deliver the shifted fill chord, while legacy ones
        // deliver plain Ctrl+R, which keeps opening the read-file
        // prompt. Resolution scoring keeps the two deterministic.
        (Chord::new(K::Char('R'), m), Action::FillRight),
        (Chord::new(K::F(4), m), Action::CloseTab),
        (Chord::new(K::PageDown, m), Action::NextTab),
        (Chord::new(K::PageUp, m), Action::PrevTab),
        (Chord::new(K::Right, a), Action::NextTab),
        (Chord::new(K::Left, a), Action::PrevTab),
        (Chord::new(K::Char('s'), m), Action::Save),
        (Chord::new(K::Char('s'), m | s), Action::SaveAs),
        (Chord::new(K::Char('r'), m), Action::ReadFile),
        (Chord::new(K::Char('w'), m), Action::Find),
        (Chord::new(K::Char('\\'), m), Action::Replace),
        (Chord::new(K::Char('f'), m), Action::Find),
        (Chord::new(K::Char('k'), m), Action::CutLine),
        (Chord::new(K::Char('u'), m), Action::Uncut),
        (Chord::new(K::Char('c'), m), Action::ShowPosition),
        (Chord::new(K::Char('g'), m), Action::Help),
        (Chord::new(K::Char('z'), m), Action::Undo),
        (Chord::new(K::Char('y'), m), Action::Redo),
        (Chord::new(K::Char('a'), m), Action::SelectAll),
        (Chord::new(K::Char('b'), a), Action::ToggleBold),
        (Chord::new(K::Char('i'), a), Action::ToggleItalic),
        (Chord::new(K::Char('p'), m), Action::Export),
        (Chord::new(K::F(1), n), Action::Help),
        (Chord::new(K::Enter, n), Action::InsertNewline),
        (Chord::new(K::Tab, n), Action::Insert('\t')),
        (Chord::new(K::Backspace, n), Action::Backspace),
        (Chord::new(K::Delete, n), Action::DeleteForward),
        (Chord::new(K::Left, n), Action::Move(Motion::Left)),
        (Chord::new(K::Right, n), Action::Move(Motion::Right)),
        (Chord::new(K::Up, n), Action::Move(Motion::Up)),
        (Chord::new(K::Down, n), Action::Move(Motion::Down)),
        (Chord::new(K::Home, n), Action::Move(Motion::LineStart)),
        (Chord::new(K::End, n), Action::Move(Motion::LineEnd)),
        (Chord::new(K::PageUp, n), Action::Move(Motion::PageUp)),
        (Chord::new(K::PageDown, n), Action::Move(Motion::PageDown)),
        (Chord::new(K::Left, s), Action::Extend(Motion::Left)),
        (Chord::new(K::Right, s), Action::Extend(Motion::Right)),
        (Chord::new(K::Up, s), Action::Extend(Motion::Up)),
        (Chord::new(K::Down, s), Action::Extend(Motion::Down)),
        (Chord::new(K::Home, s), Action::Extend(Motion::LineStart)),
        (Chord::new(K::End, s), Action::Extend(Motion::LineEnd)),
        (Chord::new(K::Home, m), Action::Move(Motion::BufferStart)),
        (Chord::new(K::End, m), Action::Move(Motion::BufferEnd)),
        (Chord::new(K::Left, m), Action::Move(Motion::WordLeft)),
        (Chord::new(K::Right, m), Action::Move(Motion::WordRight)),
        (Chord::new(K::Left, m | s), Action::Extend(Motion::WordLeft)),
        (
            Chord::new(K::Right, m | s),
            Action::Extend(Motion::WordRight),
        ),
    ]
}
