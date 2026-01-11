// Key mapping and input handling

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // Movement
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,
    MoveWordForwardBig,    // W
    MoveWordBackwardBig,   // B
    MoveWordEndBig,        // E
    MoveLineStart,
    MoveLineFirstNonBlank, // ^
    MoveLineEnd,
    MoveFileStart,
    MoveFileEnd,
    MoveParagraphForward,  // }
    MoveParagraphBackward, // {
    MoveMatchingBracket,   // %
    MovePageUp,            // Ctrl-b
    MovePageDown,          // Ctrl-f
    MoveHalfPageUp,        // Ctrl-u
    MoveHalfPageDown,      // Ctrl-d

    // Mode switching
    EnterInsertMode,
    EnterInsertModeBeginning,
    EnterInsertModeAppend,
    EnterInsertModeAppendEnd,
    EnterInsertModeNewLineBelow,
    EnterInsertModeNewLineAbove,
    EnterVisualMode,
    EnterVisualLineMode,
    EnterCommandMode,
    EnterNormalMode,

    // Editing
    DeleteChar,
    DeleteLine,
    InsertChar(char),
    InsertNewline,
    Undo,
    Redo,

    // Commands
    SaveFile,
    Quit,

    // Other
    None,
}

pub fn map_key(key: KeyEvent, mode: &crate::mode::Mode) -> Action {
    use crate::mode::Mode;

    match mode {
        Mode::Normal => map_normal_mode_key(key),
        Mode::Insert => map_insert_mode_key(key),
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => map_visual_mode_key(key),
        Mode::Command => Action::None, // Command mode has its own input handling
    }
}

fn map_normal_mode_key(key: KeyEvent) -> Action {
    match key.code {
        // Page/screen movement (check Ctrl keys first)
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::MovePageUp,
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::MovePageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::MoveHalfPageUp,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::MoveHalfPageDown,
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Redo,

        // Movement
        KeyCode::Char('h') => Action::MoveLeft,
        KeyCode::Char('j') => Action::MoveDown,
        KeyCode::Char('k') => Action::MoveUp,
        KeyCode::Char('l') => Action::MoveRight,
        KeyCode::Char('w') => Action::MoveWordForward,
        KeyCode::Char('b') => Action::MoveWordBackward,
        KeyCode::Char('e') => Action::MoveWordEnd,
        KeyCode::Char('W') => Action::MoveWordForwardBig,
        KeyCode::Char('B') => Action::MoveWordBackwardBig,
        KeyCode::Char('E') => Action::MoveWordEndBig,
        KeyCode::Char('0') => Action::MoveLineStart,
        KeyCode::Char('^') => Action::MoveLineFirstNonBlank,
        KeyCode::Char('$') => Action::MoveLineEnd,
        KeyCode::Char('g') => Action::MoveFileStart, // gg handled separately
        KeyCode::Char('G') => Action::MoveFileEnd,
        KeyCode::Char('}') => Action::MoveParagraphForward,
        KeyCode::Char('{') => Action::MoveParagraphBackward,
        KeyCode::Char('%') => Action::MoveMatchingBracket,

        // Mode switching
        KeyCode::Char('i') => Action::EnterInsertMode,
        KeyCode::Char('I') => Action::EnterInsertModeBeginning,
        KeyCode::Char('a') => Action::EnterInsertModeAppend,
        KeyCode::Char('A') => Action::EnterInsertModeAppendEnd,
        KeyCode::Char('o') => Action::EnterInsertModeNewLineBelow,
        KeyCode::Char('O') => Action::EnterInsertModeNewLineAbove,
        KeyCode::Char('v') => Action::EnterVisualMode,
        KeyCode::Char('V') => Action::EnterVisualLineMode,
        KeyCode::Char(':') => Action::EnterCommandMode,

        // Editing
        KeyCode::Char('x') => Action::DeleteChar,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::NONE) => Action::DeleteLine, // dd handled separately
        KeyCode::Char('u') => Action::Undo,

        _ => Action::None,
    }
}

fn map_insert_mode_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::EnterNormalMode,
        KeyCode::Char(c) => Action::InsertChar(c),
        KeyCode::Enter => Action::InsertNewline,
        KeyCode::Backspace => Action::DeleteChar,
        KeyCode::Left => Action::MoveLeft,
        KeyCode::Right => Action::MoveRight,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        _ => Action::None,
    }
}

fn map_visual_mode_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::EnterNormalMode,
        // Same movement keys as normal mode
        KeyCode::Char('h') => Action::MoveLeft,
        KeyCode::Char('j') => Action::MoveDown,
        KeyCode::Char('k') => Action::MoveUp,
        KeyCode::Char('l') => Action::MoveRight,
        _ => Action::None,
    }
}
