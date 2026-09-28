//! Semantic actions: every shortcut the suite documents carries a
//! variant, a config name, and a help description.

/// Semantic action bound to one or more chords.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// Exit; prompt first when the document is dirty.
    Exit,
    /// Start a fresh untitled document.
    New,
    /// Start a fresh untitled text file.
    NewText,
    /// Start a fresh untitled spreadsheet.
    NewSheet,
    /// Prompt for a file to open in a new tab.
    Open,
    /// Activate the next tab, wrapping past the last one.
    NextTab,
    /// Activate the previous tab, wrapping past the first one.
    PrevTab,
    /// Close the active tab; dirty tabs refuse, the last tab quits.
    CloseTab,
    /// Show or hide the sidebar.
    ToggleSidebar,
    /// Add one blank row per text line, up to the maximum.
    ZoomIn,
    /// Remove one blank row per text line, down to none.
    ZoomOut,
    /// Return to one row per text line.
    ZoomReset,
    /// Step to the next display theme.
    CycleTheme,
    /// Fill downward from the cell above or the selection top row,
    /// shifting relative formula references per row.
    FillDown,
    /// Fill rightward from the cell to the left or the selection left
    /// column, shifting relative formula references per column.
    FillRight,
    /// Write out to the current path.
    Save,
    /// Prompt for a destination path and save there.
    SaveAs,
    /// Prompt for a file and insert it at the cursor.
    ReadFile,
    /// Open the find prompt.
    Find,
    /// Open the replace prompt.
    Replace,
    /// Cut the current line into the cutbuffer.
    CutLine,
    /// Paste the cutbuffer at the cursor.
    Uncut,
    /// Show cursor position and document stats in the message bar.
    ShowPosition,
    /// Render the help screen.
    Help,
    /// Undo the last unit.
    Undo,
    /// Redo the last undone unit.
    Redo,
    /// Select the whole document.
    SelectAll,
    /// Wrap the selection or word in Markdown bold markers.
    ToggleBold,
    /// Wrap the selection or word in Markdown italic markers.
    ToggleItalic,
    /// Apply a heading level to the cursor paragraph in word documents.
    ApplyHeading(u8),
    /// Open the hyperlink under the cursor with the system handler.
    OpenLink,
    /// Jump to the previous page start in prose documents.
    PrevPage,
    /// Jump to the next page start in prose documents.
    NextPage,
    /// Cycle keyboard focus across menu bar, sidebar, and text.
    FocusNext,
    /// Select the entire cursor row in spreadsheets.
    SelectRow,
    /// Select the entire cursor column in spreadsheets.
    SelectCol,
    /// Activate the previous workbook sheet, wrapping around.
    PrevSheet,
    /// Activate the next workbook sheet, wrapping around.
    NextSheet,
    /// Export to PDF, HTML, or Markdown; format follows the path extension.
    Export,
    /// Insert a literal character.
    Insert(char),
    /// Insert a newline.
    InsertNewline,
    /// Delete the selection or the character before the cursor.
    Backspace,
    /// Delete forward.
    DeleteForward,
    /// Move without extending the selection.
    Move(crate::editor::Motion),
    /// Move while extending the selection.
    Extend(crate::editor::Motion),
    /// Accept the current prompt or confirmation.
    Confirm,
    /// Cancel the current prompt or confirmation.
    Cancel,
    /// Literal character typed while a prompt is active.
    PromptChar(char),
    /// Backspace while a prompt is active.
    PromptBackspace,
    /// Unbound or not yet implemented.
    Noop,
}

/// Short description of an action for the help screen.
pub fn describe(action: &Action) -> String {
    match action {
        Action::Exit => "Exit (prompt if modified)".into(),
        Action::New => "New document".into(),
        Action::NewText => "New text file".into(),
        Action::NewSheet => "New spreadsheet".into(),
        Action::Open => "Open file".into(),
        Action::NextTab => "Next tab".into(),
        Action::PrevTab => "Previous tab".into(),
        Action::CloseTab => "Close tab".into(),
        Action::ToggleSidebar => "Toggle sidebar".into(),
        Action::ZoomIn => "Zoom in".into(),
        Action::ZoomOut => "Zoom out".into(),
        Action::ZoomReset => "Reset zoom".into(),
        Action::CycleTheme => "Cycle theme".into(),
        Action::FillDown => "Fill down".into(),
        Action::FillRight => "Fill right".into(),
        Action::Save => "Save file".into(),
        Action::SaveAs => "Save as".into(),
        Action::ReadFile => "Insert file at cursor".into(),
        Action::Find => "Find".into(),
        Action::Replace => "Replace".into(),
        Action::CutLine => "Cut line".into(),
        Action::Uncut => "Uncut (paste)".into(),
        Action::ShowPosition => "Show position".into(),
        Action::Help => "Get help".into(),
        Action::Undo => "Undo".into(),
        Action::Redo => "Redo".into(),
        Action::SelectAll => "Select all".into(),
        Action::ToggleBold => "Bold (**)".into(),
        Action::ToggleItalic => "Italic (*)".into(),
        Action::ApplyHeading(level) => format!("Heading {level}"),
        Action::OpenLink => "Open link".into(),
        Action::PrevPage => "Previous page".into(),
        Action::NextPage => "Next page".into(),
        Action::FocusNext => "Focus next section".into(),
        Action::SelectRow => "Select row".into(),
        Action::SelectCol => "Select column".into(),
        Action::PrevSheet => "Previous sheet".into(),
        Action::NextSheet => "Next sheet".into(),
        Action::Export => "Export (PDF, HTML, Markdown)".into(),
        Action::Insert(_) => "Insert character".into(),
        Action::InsertNewline => "New line".into(),
        Action::Backspace => "Delete previous character".into(),
        Action::DeleteForward => "Delete next character".into(),
        Action::Move(m) => format!("Move {m:?}"),
        Action::Extend(m) => format!("Select {m:?}"),
        Action::Confirm => "Confirm".into(),
        Action::Cancel => "Cancel".into(),
        Action::PromptChar(_) => "Prompt character".into(),
        Action::PromptBackspace => "Prompt backspace".into(),
        Action::Noop => "No operation".into(),
    }
}

/// Config-file name for an action, when chords can express it.
/// Character insertions and prompt characters are structural typing
/// rather than shortcuts, so they carry no name.
pub fn action_name(action: &Action) -> Option<&'static str> {
    use crate::editor::Motion;
    use Action::*;
    let motion_suffix = |prefix: &str, m: &Motion| -> &'static str {
        match (prefix, m) {
            ("move", Motion::Left) => "move_left",
            ("move", Motion::Right) => "move_right",
            ("move", Motion::Up) => "move_up",
            ("move", Motion::Down) => "move_down",
            ("move", Motion::LineStart) => "move_home",
            ("move", Motion::LineEnd) => "move_end",
            ("move", Motion::PageUp) => "move_page_up",
            ("move", Motion::PageDown) => "move_page_down",
            ("move", Motion::BufferStart) => "move_buffer_start",
            ("move", Motion::BufferEnd) => "move_buffer_end",
            ("move", Motion::WordLeft) => "move_word_left",
            ("move", Motion::WordRight) => "move_word_right",
            (_, Motion::Left) => "extend_left",
            (_, Motion::Right) => "extend_right",
            (_, Motion::Up) => "extend_up",
            (_, Motion::Down) => "extend_down",
            (_, Motion::LineStart) => "extend_home",
            (_, Motion::LineEnd) => "extend_end",
            (_, Motion::PageUp) => "extend_page_up",
            (_, Motion::PageDown) => "extend_page_down",
            (_, Motion::BufferStart) => "extend_buffer_start",
            (_, Motion::BufferEnd) => "extend_buffer_end",
            (_, Motion::WordLeft) => "extend_word_left",
            (_, Motion::WordRight) => "extend_word_right",
        }
    };
    Some(match action {
        Exit => "exit",
        New => "new",
        NewText => "new_text",
        NewSheet => "new_sheet",
        Open => "open",
        NextTab => "tab_next",
        PrevTab => "tab_prev",
        CloseTab => "close_tab",
        ToggleSidebar => "toggle_sidebar",
        ZoomIn => "zoom_in",
        ZoomOut => "zoom_out",
        ZoomReset => "zoom_reset",
        CycleTheme => "cycle_theme",
        FillDown => "fill_down",
        FillRight => "fill_right",
        Save => "save",
        SaveAs => "save_as",
        ReadFile => "read_file",
        Find => "find",
        Replace => "replace",
        CutLine => "cut_line",
        Uncut => "uncut",
        ShowPosition => "show_position",
        Help => "help",
        Undo => "undo",
        Redo => "redo",
        SelectAll => "select_all",
        ToggleBold => "toggle_bold",
        ToggleItalic => "toggle_italic",
        ApplyHeading(1) => "heading_1",
        ApplyHeading(2) => "heading_2",
        ApplyHeading(3) => "heading_3",
        ApplyHeading(4) => "heading_4",
        ApplyHeading(5) => "heading_5",
        ApplyHeading(6) => "heading_6",
        ApplyHeading(7) => "heading_7",
        ApplyHeading(8) => "heading_8",
        ApplyHeading(9) => "heading_9",
        ApplyHeading(_) => return None,
        OpenLink => "open_link",
        PrevPage => "prev_page",
        NextPage => "next_page",
        FocusNext => "focus_next",
        SelectRow => "select_row",
        SelectCol => "select_col",
        PrevSheet => "prev_sheet",
        NextSheet => "next_sheet",
        Export => "export",
        Insert(_) => return None,
        InsertNewline => "insert_newline",
        Backspace => "backspace",
        DeleteForward => "delete_forward",
        Move(m) => motion_suffix("move", m),
        Extend(m) => motion_suffix("extend", m),
        Confirm => "confirm",
        Cancel => "cancel",
        PromptChar(_) => return None,
        PromptBackspace => "prompt_backspace",
        Noop => "noop",
    })
}

pub(crate) fn action_from_name(name: &str) -> Option<Action> {
    use crate::editor::Motion;
    use Action::*;
    Some(match name {
        "exit" => Exit,
        "new" => New,
        "new_text" => NewText,
        "new_sheet" => NewSheet,
        "open" => Open,
        "tab_next" => NextTab,
        "tab_prev" => PrevTab,
        "close_tab" => CloseTab,
        "toggle_sidebar" => ToggleSidebar,
        "zoom_in" => ZoomIn,
        "zoom_out" => ZoomOut,
        "zoom_reset" => ZoomReset,
        "cycle_theme" => CycleTheme,
        "fill_down" => FillDown,
        "fill_right" => FillRight,
        "save" => Save,
        "save_as" => SaveAs,
        "read_file" => ReadFile,
        "find" => Find,
        "replace" => Replace,
        "cut_line" => CutLine,
        "uncut" => Uncut,
        "show_position" => ShowPosition,
        "help" => Help,
        "undo" => Undo,
        "redo" => Redo,
        "select_all" => SelectAll,
        "toggle_bold" => ToggleBold,
        "toggle_italic" => ToggleItalic,
        "heading_1" => ApplyHeading(1),
        "heading_2" => ApplyHeading(2),
        "heading_3" => ApplyHeading(3),
        "heading_4" => ApplyHeading(4),
        "heading_5" => ApplyHeading(5),
        "heading_6" => ApplyHeading(6),
        "heading_7" => ApplyHeading(7),
        "heading_8" => ApplyHeading(8),
        "heading_9" => ApplyHeading(9),
        "open_link" => OpenLink,
        "prev_page" => PrevPage,
        "next_page" => NextPage,
        "focus_next" => FocusNext,
        "select_row" => SelectRow,
        "select_col" => SelectCol,
        "prev_sheet" => PrevSheet,
        "next_sheet" => NextSheet,
        "export" => Export,
        "insert_newline" => InsertNewline,
        "backspace" => Backspace,
        "delete_forward" => DeleteForward,
        "move_left" => Move(Motion::Left),
        "move_right" => Move(Motion::Right),
        "move_up" => Move(Motion::Up),
        "move_down" => Move(Motion::Down),
        "move_home" => Move(Motion::LineStart),
        "move_end" => Move(Motion::LineEnd),
        "move_page_up" => Move(Motion::PageUp),
        "move_page_down" => Move(Motion::PageDown),
        "move_buffer_start" => Move(Motion::BufferStart),
        "move_buffer_end" => Move(Motion::BufferEnd),
        "extend_left" => Extend(Motion::Left),
        "extend_right" => Extend(Motion::Right),
        "extend_up" => Extend(Motion::Up),
        "extend_down" => Extend(Motion::Down),
        "extend_home" => Extend(Motion::LineStart),
        "extend_end" => Extend(Motion::LineEnd),
        "extend_page_up" => Extend(Motion::PageUp),
        "extend_page_down" => Extend(Motion::PageDown),
        "extend_buffer_start" => Extend(Motion::BufferStart),
        "extend_buffer_end" => Extend(Motion::BufferEnd),
        "move_word_left" => Move(Motion::WordLeft),
        "move_word_right" => Move(Motion::WordRight),
        "extend_word_left" => Extend(Motion::WordLeft),
        "extend_word_right" => Extend(Motion::WordRight),
        "confirm" => Confirm,
        "cancel" => Cancel,
        "prompt_backspace" => PromptBackspace,
        "noop" => Noop,
        _ => return None,
    })
}
