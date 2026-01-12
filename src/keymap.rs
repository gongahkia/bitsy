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
    MoveWordEndBack,       // ge (end of previous word)
    MoveWordForwardBig,    // W
    MoveWordBackwardBig,   // B
    MoveWordEndBig,        // E
    MoveWordEndBackBig,    // gE (end of previous WORD)
    MoveLineStart,
    MoveLineFirstNonBlank, // ^
    MoveLineEnd,
    MoveLineEndNonBlank,   // g_ (last non-blank character)
    MoveLineStartDisplay,  // g0 (first character of screen line)
    MoveLineEndDisplay,    // g$ (last character of screen line)
    MoveFileStart,
    MoveFileEnd,
    MoveParagraphForward,  // }
    MoveParagraphBackward, // {
    MoveMatchingBracket,   // %
    MovePageUp,            // Ctrl-b
    MovePageDown,          // Ctrl-f
    MoveHalfPageUp,        // Ctrl-u
    MoveHalfPageDown,      // Ctrl-d
    MoveSentenceForward,   // )
    MoveSentenceBackward,  // (
    FindChar(char),        // f{char}
    FindCharBack(char),    // F{char}
    TillChar(char),        // t{char}
    TillCharBack(char),    // T{char}
    RepeatLastFind,        // ;
    RepeatLastFindReverse, // ,
    MoveToScreenTop,       // H
    MoveToScreenMiddle,    // M
    MoveToScreenBottom,    // L
    ScrollTopToScreen,     // zt
    ScrollMiddleToScreen,  // zz
    ScrollBottomToScreen,  // zb
    MoveToPercent,         // count% (e.g., 50% goes to 50% of file)

    // Mode switching
    EnterInsertMode,
    EnterInsertModeBeginning,
    EnterInsertModeAppend,
    EnterInsertModeAppendEnd,
    EnterInsertModeNewLineBelow,
    EnterInsertModeNewLineAbove,
    EnterReplaceMode,
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

    // Operators
    Delete,              // d (waits for motion)
    DeleteToEnd,         // D
    Change,              // c (waits for motion)
    ChangeToEnd,         // C
    ChangeLine,          // cc
    Yank,                // y (waits for motion)
    YankLine,            // yy
    YankToEnd,           // Y
    Paste,               // p
    PasteBefore,         // P
    Join,                // J
    JoinNoSpace,         // gJ
    Replace(char),       // r
    MakeLowercase,       // gu (waits for motion)
    MakeUppercase,       // gU (waits for motion)
    ToggleCase,          // g~ (waits for motion)
    Indent,              // > (waits for motion)
    Dedent,              // < (waits for motion)
    AutoIndent,          // = (waits for motion)

    // Commands
    SaveFile,
    Quit,

    // Search
    SearchForward,       // / (initiate forward search)
    SearchBackward,      // ? (initiate backward search)
    SearchNext,          // n (repeat last search)
    SearchPrevious,      // N (repeat last search in opposite direction)
    SearchWordForward,   // * (search word under cursor forward)
    SearchWordBackward,  // # (search word under cursor backward)

    // Marks
    SetMark(char),       // m{char} (set mark)
    JumpToMark(char),    // '{char} (jump to mark line)
    JumpToMarkExact(char), // `{char} (jump to mark exact position)
    JumpToChangeNext,    // g; (jump to next change)
    JumpToChangePrev,    // g, (jump to previous change)

    // Other
    RepeatLastChange,    // . (dot command)
    None,
}

pub fn map_key(key: KeyEvent, mode: &crate::mode::Mode) -> Action {
    use crate::mode::Mode;

    match mode {
        Mode::Normal => map_normal_mode_key(key),
        Mode::Insert => map_insert_mode_key(key),
        Mode::Replace => map_replace_mode_key(key),
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => map_visual_mode_key(key),
        Mode::Command => Action::None, // Command mode has its own input handling
        Mode::Search => Action::None, // Search mode has its own input handling
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
        // 'g' is handled as a prefix key in editor.rs (for gg, ge, gJ, gu, etc.)
        KeyCode::Char('G') => Action::MoveFileEnd,
        KeyCode::Char('}') => Action::MoveParagraphForward,
        KeyCode::Char('{') => Action::MoveParagraphBackward,
        // % has dual use: without count, it matches brackets; with count, it goes to percentage
        KeyCode::Char('%') => Action::MoveToPercent,
        KeyCode::Char(')') => Action::MoveSentenceForward,
        KeyCode::Char('(') => Action::MoveSentenceBackward,
        KeyCode::Char('H') => Action::MoveToScreenTop,
        KeyCode::Char('M') => Action::MoveToScreenMiddle,
        KeyCode::Char('L') => Action::MoveToScreenBottom,

        // Mode switching
        KeyCode::Char('i') => Action::EnterInsertMode,
        KeyCode::Char('I') => Action::EnterInsertModeBeginning,
        KeyCode::Char('a') => Action::EnterInsertModeAppend,
        KeyCode::Char('A') => Action::EnterInsertModeAppendEnd,
        KeyCode::Char('o') => Action::EnterInsertModeNewLineBelow,
        KeyCode::Char('O') => Action::EnterInsertModeNewLineAbove,
        KeyCode::Char('R') => Action::EnterReplaceMode,
        KeyCode::Char('v') => Action::EnterVisualMode,
        KeyCode::Char('V') => Action::EnterVisualLineMode,
        KeyCode::Char(':') => Action::EnterCommandMode,

        // Editing
        KeyCode::Char('x') => Action::DeleteChar,
        KeyCode::Char('u') => Action::Undo,
        KeyCode::Char('.') => Action::RepeatLastChange,

        // Operators
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('D') => Action::DeleteToEnd,
        KeyCode::Char('c') => Action::Change,
        KeyCode::Char('C') => Action::ChangeToEnd,
        KeyCode::Char('y') => Action::Yank,
        KeyCode::Char('Y') => Action::YankToEnd,
        KeyCode::Char('p') => Action::Paste,
        KeyCode::Char('P') => Action::PasteBefore,
        KeyCode::Char('J') => Action::Join,
        KeyCode::Char('>') => Action::Indent,
        KeyCode::Char('<') => Action::Dedent,
        KeyCode::Char('=') => Action::AutoIndent,

        // Search
        KeyCode::Char('/') => Action::SearchForward,
        KeyCode::Char('?') => Action::SearchBackward,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrevious,
        KeyCode::Char('*') => Action::SearchWordForward,
        KeyCode::Char('#') => Action::SearchWordBackward,

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

fn map_replace_mode_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::EnterNormalMode,
        KeyCode::Char(c) => Action::Replace(c), // In replace mode, use Replace action
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
