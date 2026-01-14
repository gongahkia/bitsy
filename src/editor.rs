// Main editor coordination

use crossterm::event::{Event, KeyCode, KeyEvent};
use crossterm::style::Color;
use std::collections::HashMap;
use std::path::Path;
use std::fs;

use crate::buffer::Buffer;
use crate::command::{parse_command, Command};
use crate::config::{Config, LineNumberMode};
use crate::cursor::Cursor;
use crate::error::Result;
use crate::keymap::{map_key, Action};
use crate::mode::Mode;
use crate::register::{RegisterContent, RegisterManager};
use crate::selection::Selection;
use crate::statusline::StatusLine;
use crate::terminal::Terminal;
use crate::window::{Window, Layout};

#[derive(Debug, Clone, PartialEq)]
enum PendingOperator {
    None,
    Delete,
    Change,
    Yank,
    MakeLowercase,
    MakeUppercase,
    ToggleCase,
    Indent,
    Dedent,
    AutoIndent,
}

pub struct Editor {
    terminal: Terminal,
    buffers: Vec<Buffer>,
    mode: Mode,
    statusline: StatusLine,
    command_buffer: String,
    message: Option<String>,
    should_quit: bool,
    registers: RegisterManager,
    pending_operator: PendingOperator,
    config: Config,
    selection: Option<Selection>,
    last_find: Option<(char, FindDirection)>,
    pending_key: Option<char>,
    count: usize, // Count for operator/motion repetition (e.g., 3dw, 5j)
    last_change: Option<(Action, usize)>, // Last change for dot repeat (action, count)
    search_buffer: String,
    search_pattern: Option<String>,
    search_forward: bool, // Direction of last search
    pending_text_object: Option<TextObjectModifier>, // Waiting for text object (a or i)
    pending_register: Option<char>, // Register selected with " prefix
    waiting_for_register: bool, // True when " was just pressed
    global_marks: HashMap<char, (usize, usize)>, // Global mark positions (line, col)
    change_list: Vec<(usize, usize)>, // List of change positions
    change_index: usize, // Current position in change list
    waiting_for_mark: Option<MarkAction>, // Waiting for mark character after m, ', or `
    command_history: Vec<String>,
    history_index: Option<usize>,
    completion_candidates: Vec<String>,
    completion_index: Option<usize>,
    jump_list: Vec<(usize, usize)>,
    jump_index: usize,
    recording_register: Option<char>,
    macro_buffer: Vec<KeyEvent>,
    last_macro_register: Option<char>,
    windows: Vec<Window>,
    layout: Layout,
    active_window: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkAction {
    Set,        // m (set mark)
    Jump,       // ' (jump to line)
    JumpExact,  // ` (jump to exact position)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextObjectModifier {
    Around, // a (includes surrounding whitespace/delimiters)
    Inner,  // i (excludes surrounding whitespace/delimiters)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FindDirection {
    Forward,    // f
    Backward,   // F
    Till,       // t
    TillBack,   // T
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseChange {
    Lower,
    Upper,
    Toggle,
}

impl Editor {
    pub fn new() -> Result<Self> {
        let terminal = Terminal::new()?;
        let (width, height) = terminal.size();

        // Reserve 2 lines: 1 for status, 1 for command/message
        let viewport_height = (height as usize).saturating_sub(2);
        
        let window = Window::new(0, width as usize, viewport_height);

        Ok(Self {
            terminal,
            buffers: vec![Buffer::new()],
            mode: Mode::Normal,
            statusline: StatusLine::new(),
            command_buffer: String::new(),
            message: None,
            should_quit: false,
            registers: RegisterManager::new(),
            pending_operator: PendingOperator::None,
            config: Config::default(),
            selection: None,
            last_find: None,
            pending_key: None,
            count: 0,
            last_change: None,
            search_buffer: String::new(),
            search_pattern: None,
            search_forward: true,
            pending_text_object: None,
            pending_register: None,
            waiting_for_register: false,
            global_marks: HashMap::new(),
            change_list: Vec::new(),
            change_index: 0,
            waiting_for_mark: None,
            command_history: Vec::new(),
            history_index: None,
            completion_candidates: Vec::new(),
            completion_index: None,
            jump_list: Vec::new(),
            jump_index: 0,
            recording_register: None,
            macro_buffer: Vec::new(),
            last_macro_register: None,
            windows: vec![window],
            layout: Layout::new_leaf(0),
            active_window: 0,
        })
    }

    fn current_buffer(&self) -> &Buffer {
        &self.buffers[self.windows[self.active_window].buffer_index]
    }
    
    fn current_buffer_mut(&mut self) -> &mut Buffer {
        let buffer_idx = self.windows[self.active_window].buffer_index;
        &mut self.buffers[buffer_idx]
    }

    fn current_window(&self) -> &Window {
        &self.windows[self.active_window]
    }

    fn current_window_mut(&mut self) -> &mut Window {
        &mut self.windows[self.active_window]
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let buffer = Buffer::from_file(&path)?;
        self.buffers[0] = buffer;
        self.windows[0].cursor = Cursor::default();
        self.registers.update_filename(path.as_ref().to_string_lossy().to_string());
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            self.render()?;

            if self.should_quit {
                break;
            }

            if let Some(event) = self.terminal.read_event()? {
                self.handle_event(event)?;
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key)?,
            Event::Resize(width, height) => {
                self.terminal.update_size()?;
                let viewport_height = (height as usize).saturating_sub(2);
                // For now, resize only active window. In future with splits, we need to recalculate layout.
                self.windows[self.active_window].viewport.resize(width as usize, viewport_height);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Record key if recording macro
        if self.recording_register.is_some() {
            // Don't record 'q' if it's going to stop recording
            if key.code == KeyCode::Char('q') && self.mode == Mode::Normal && self.pending_operator == PendingOperator::None && self.pending_key.is_none() && self.pending_text_object.is_none() && self.waiting_for_mark.is_none() && !self.waiting_for_register {
                // Stop recording
                if let Some(reg) = self.recording_register {
                    self.registers.set(Some(reg), RegisterContent::Macro(self.macro_buffer.clone()));
                    self.message = Some(format!("Recorded macro to @{}", reg));
                }
                self.recording_register = None;
                self.macro_buffer.clear();
                return Ok(());
            } else {
                self.macro_buffer.push(key);
            }
        }

        if self.mode == Mode::Command {
            self.handle_command_mode_key(key)?;
        } else if self.mode == Mode::Search {
            self.handle_search_mode_key(key)?;
        } else {
            // Handle mark operations (m, ', `)
            if self.mode == Mode::Normal && self.waiting_for_mark.is_some() {
                if let KeyCode::Char(c) = key.code {
                    let mark_action = self.waiting_for_mark.unwrap();
                    self.waiting_for_mark = None;
                    match mark_action {
                        MarkAction::Set => {
                            if ('a'..='z').contains(&c) {
                                let cursor = self.current_window().cursor;
                                self.current_buffer_mut().set_mark(c, (cursor.line, cursor.col));
                            } else if ('A'..='Z').contains(&c) {
                                let cursor = self.current_window().cursor;
                                self.global_marks.insert(c, (cursor.line, cursor.col));
                            }
                        }
                        MarkAction::Jump => {
                            let pos = if ('a'..='z').contains(&c) {
                                self.current_buffer().get_mark(c)
                            } else if ('A'..='Z').contains(&c) {
                                self.global_marks.get(&c).cloned()
                            } else {
                                None
                            };

                            if let Some((line, _col)) = pos {
                                let line_count = self.current_buffer().line_count();
                                let window = self.current_window_mut();
                                window.cursor.line = line.min(line_count.saturating_sub(1));
                                window.cursor.col = 0;
                                self.clamp_cursor();
                            }
                        }
                        MarkAction::JumpExact => {
                            let pos = if ('a'..='z').contains(&c) {
                                self.current_buffer().get_mark(c)
                            } else if ('A'..='Z').contains(&c) {
                                self.global_marks.get(&c).cloned()
                            } else {
                                None
                            };

                            if let Some((line, col)) = pos {
                                let line_count = self.current_buffer().line_count();
                                let window = self.current_window_mut();
                                window.cursor.line = line.min(line_count.saturating_sub(1));
                                window.cursor.col = col;
                                self.clamp_cursor();
                            }
                        }
                    }
                    return Ok(());
                }
            }

            // Check for m, ', ` to start mark operations
            if self.mode == Mode::Normal && self.waiting_for_mark.is_none() && self.pending_operator == PendingOperator::None {
                match key.code {
                    KeyCode::Char('m') => {
                        self.waiting_for_mark = Some(MarkAction::Set);
                        return Ok(());
                    }
                    KeyCode::Char('\'') => {
                        self.waiting_for_mark = Some(MarkAction::Jump);
                        return Ok(());
                    }
                    KeyCode::Char('`') => {
                        self.waiting_for_mark = Some(MarkAction::JumpExact);
                        return Ok(());
                    }
                    KeyCode::Char('q') => {
                        if self.recording_register.is_none() {
                            // Start waiting for register to record to
                            self.pending_key = Some('q');
                            return Ok(());
                        }
                    }
                    KeyCode::Char('@') => {
                        self.pending_key = Some('@');
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // Handle register selection (")
            if self.mode == Mode::Normal && self.waiting_for_register {
                if let KeyCode::Char(c) = key.code {
                    self.pending_register = Some(c);
                    self.waiting_for_register = false;
                    return Ok(());
                }
            }

            // Check for " to start register selection
            if self.mode == Mode::Normal && !self.waiting_for_register {
                if let KeyCode::Char('"') = key.code {
                    self.waiting_for_register = true;
                    return Ok(());
                }
            }

            // Handle count input (digits in normal mode)
            if self.mode == Mode::Normal {
                if let KeyCode::Char(c) = key.code {
                    if c.is_ascii_digit() {
                        let digit = c.to_digit(10).unwrap() as usize;
                        // Prevent leading zero unless it's the first digit and we have no count
                        if digit != 0 || self.count != 0 {
                            self.count = self.count * 10 + digit;
                            return Ok(());
                        }
                    }
                }
            }

            // Handle multi-key sequences (like gJ, gg, ge, etc.)
            let action = if let Some(prefix) = self.pending_key {
                if prefix == 'q' {
                    // Start recording to register
                    if let KeyCode::Char(c) = key.code {
                        if ('a'..='z').contains(&c) || ('0'..='9').contains(&c) {
                            self.recording_register = Some(c);
                            self.macro_buffer.clear();
                            self.message = Some(format!("recording @{}", c));
                        }
                    }
                    self.pending_key = None;
                    return Ok(());
                } else if prefix == '@' {
                    // Play macro from register
                    if let KeyCode::Char(c) = key.code {
                        self.play_macro(c)?;
                    }
                    self.pending_key = None;
                    return Ok(());
                }
                
                let sequence_action = self.map_key_sequence(prefix, key);
                self.pending_key = None; // Clear pending key after processing
                sequence_action
            } else if key.code == KeyCode::Char('g') && self.mode == Mode::Normal {
                // 'g' is a prefix key, wait for next key
                self.pending_key = Some('g');
                return Ok(());
            } else if key.code == KeyCode::Char('r') && self.mode == Mode::Normal {
                // 'r' is a prefix key (replace character), wait for next key
                self.pending_key = Some('r');
                return Ok(());
            } else {
                map_key(key, &self.mode)
            };

            // Handle text object composition (for operators like daw, ci", etc.)
            if self.pending_operator != PendingOperator::None {
                // Check if user is entering a text object (a or i)
                if matches!(key.code, KeyCode::Char('a') | KeyCode::Char('i')) && self.pending_text_object.is_none() {
                    self.pending_text_object = if key.code == KeyCode::Char('a') {
                        Some(TextObjectModifier::Around)
                    } else {
                        Some(TextObjectModifier::Inner)
                    };
                    return Ok(());
                } else if self.pending_text_object.is_some() {
                    // User has typed a/i, now waiting for text object type
                    self.handle_text_object(key)?;
                    return Ok(());
                } else {
                    // Normal operator-motion composition
                    self.handle_operator_motion(action)?;
                }
            } else {
                self.execute_action(action)?;
            }

            // Reset count after executing action (unless waiting for more input)
            if self.pending_operator == PendingOperator::None && self.pending_key.is_none() && self.pending_text_object.is_none() {
                self.count = 0;
            }
        }
        Ok(())
    }

    fn map_key_sequence(&self, prefix: char, key: KeyEvent) -> Action {
        if prefix == 'g' {
            match key.code {
                KeyCode::Char('g') => Action::MoveFileStart, // gg
                KeyCode::Char('e') => Action::MoveWordEndBack, // ge
                KeyCode::Char('E') => Action::MoveWordEndBackBig, // gE
                KeyCode::Char('_') => Action::MoveLineEndNonBlank, // g_
                KeyCode::Char('0') => Action::MoveLineStartDisplay, // g0
                KeyCode::Char('$') => Action::MoveLineEndDisplay, // g$
                KeyCode::Char('J') => Action::JoinNoSpace, // gJ
                KeyCode::Char('u') => Action::MakeLowercase, // gu
                KeyCode::Char('U') => Action::MakeUppercase, // gU
                KeyCode::Char('~') => Action::ToggleCase, // g~
                KeyCode::Char(';') => Action::JumpToChangeNext, // g;
                KeyCode::Char(',') => Action::JumpToChangePrev, // g,
                _ => Action::None,
            }
        } else if prefix == 'r' {
            match key.code {
                KeyCode::Char(c) => Action::Replace(c), // r{char}
                _ => Action::None,
            }
        } else {
            Action::None
        }
    }

    fn generate_completions(&mut self) {
        let input = self.command_buffer.trim();
        if input.is_empty() {
            return;
        }

        self.completion_candidates.clear();

        if input.starts_with("e ") || input.starts_with("edit ") {
            // File completion
            let prefix = if let Some(p) = input.strip_prefix("e ") { p } else { input.strip_prefix("edit ").unwrap() };
            // Simple handling for now: just list current directory
            let dir = "."; 
            
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.starts_with(prefix) {
                             let cmd_prefix = if input.starts_with("e ") { "e " } else { "edit " };
                             self.completion_candidates.push(format!("{}{}", cmd_prefix, name));
                        }
                    }
                }
            }
        } else if !input.contains(' ') {
            // Command completion
            let commands = vec![
                "w", "write", "q", "quit", "wq", "x", "q!", "e", "edit", 
                "bn", "bnext", "bp", "bprevious", "bd", "bdelete", "ls", "buffers",
                "sp", "split", "vsp", "vsplit", "close", "help", "set"
            ];
            
            for cmd in commands {
                if cmd.starts_with(input) {
                    self.completion_candidates.push(cmd.to_string());
                }
            }
        }
        self.completion_candidates.sort();
    }

    fn handle_command_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.history_index = None;
                self.completion_candidates.clear();
                self.completion_index = None;
            }
            KeyCode::Enter => {
                if !self.command_buffer.is_empty() {
                    // Add to history if different from last entry
                    if self.command_history.last() != Some(&self.command_buffer) {
                        self.command_history.push(self.command_buffer.clone());
                    }
                }
                self.execute_command()?;
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.history_index = None;
                self.completion_candidates.clear();
                self.completion_index = None;
            }
            KeyCode::Up => {
                self.completion_candidates.clear();
                self.completion_index = None;
                if self.command_history.is_empty() {
                    return Ok(());
                }

                if self.history_index.is_none() {
                    // Start navigating from end
                    self.history_index = Some(self.command_history.len() - 1);
                    self.command_buffer = self.command_history[self.command_history.len() - 1].clone();
                } else {
                    // Move up (backwards) in history
                    let idx = self.history_index.unwrap();
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                        self.command_buffer = self.command_history[idx - 1].clone();
                    }
                }
            }
            KeyCode::Down => {
                self.completion_candidates.clear();
                self.completion_index = None;
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.command_history.len() {
                        // Move down (forwards) in history
                        self.history_index = Some(idx + 1);
                        self.command_buffer = self.command_history[idx + 1].clone();
                    } else {
                        // Moved past end of history, clear buffer
                        self.history_index = None;
                        self.command_buffer.clear();
                    }
                }
            }
            KeyCode::Tab => {
                if self.completion_candidates.is_empty() {
                    // Generate candidates
                    self.generate_completions();
                    if !self.completion_candidates.is_empty() {
                        self.completion_index = Some(0);
                        self.command_buffer = self.completion_candidates[0].clone();
                    }
                } else {
                    // Cycle through candidates
                    if let Some(idx) = self.completion_index {
                        let next_idx = (idx + 1) % self.completion_candidates.len();
                        self.completion_index = Some(next_idx);
                        self.command_buffer = self.completion_candidates[next_idx].clone();
                    }
                }
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                self.completion_candidates.clear();
                self.completion_index = None;
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
                self.completion_candidates.clear();
                self.completion_index = None;
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                    self.history_index = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_buffer.clear();
            }
            KeyCode::Enter => {
                if !self.search_buffer.is_empty() {
                    self.search_pattern = Some(self.search_buffer.clone());
                    self.execute_search()?;
                }
                self.mode = Mode::Normal;
                self.search_buffer.clear();
            }
            KeyCode::Char(c) => {
                self.search_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.search_buffer.pop();
                if self.search_buffer.is_empty() {
                    self.mode = Mode::Normal;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_text_object(&mut self, key: KeyEvent) -> Result<()> {
        let modifier = self.pending_text_object.unwrap();

        match key.code {
            KeyCode::Char('w') => {
                // Word text object
                self.apply_text_object_word(modifier)?;
            }
            KeyCode::Char('W') => {
                // WORD text object (space-separated)
                self.apply_text_object_word_big(modifier)?;
            }
            KeyCode::Char('p') => {
                // Paragraph text object
                self.apply_text_object_paragraph(modifier)?;
            }
            KeyCode::Char('"') => {
                // Double quote text object
                self.apply_text_object_quote(modifier, '"')?;
            }
            KeyCode::Char('\'') => {
                // Single quote text object
                self.apply_text_object_quote(modifier, '\'')?;
            }
            KeyCode::Char('`') => {
                // Backtick text object
                self.apply_text_object_quote(modifier, '`')?;
            }
            KeyCode::Char('(') | KeyCode::Char(')') => {
                // Parentheses text object
                self.apply_text_object_bracket(modifier, '(', ')')?;
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                // Square bracket text object
                self.apply_text_object_bracket(modifier, '[', ']')?;
            }
            KeyCode::Char('{') | KeyCode::Char('}') => {
                // Curly brace text object
                self.apply_text_object_bracket(modifier, '{', '}')?;
            }
            KeyCode::Char('<') | KeyCode::Char('>') => {
                // Angle bracket text object
                self.apply_text_object_bracket(modifier, '<', '>')?;
            }
            KeyCode::Esc => {
                // Cancel
                self.pending_operator = PendingOperator::None;
                self.pending_text_object = None;
                return Ok(());
            }
            _ => {
                // Unknown text object, cancel
                self.pending_operator = PendingOperator::None;
                self.pending_text_object = None;
            }
        }

        self.pending_text_object = None;
        self.pending_operator = PendingOperator::None;
        self.clamp_cursor();
        Ok(())
    }

    fn execute_command(&mut self) -> Result<()> {
        self.registers.update_last_command(self.command_buffer.clone());
        let cmd = parse_command(&self.command_buffer)?;

        match cmd {
            Command::Write => {
                self.current_buffer_mut().save()?;
                self.message = Some("File written".to_string());
            }
            Command::Quit => {
                if self.current_buffer().is_modified() {
                    self.message = Some("No write since last change (use :q! to force)".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            Command::WriteQuit => {
                self.current_buffer_mut().save()?;
                self.should_quit = true;
            }
            Command::ForceQuit => {
                self.should_quit = true;
            }
            Command::Edit(filename) => {
                if self.current_buffer().is_modified() {
                    self.message = Some("No write since last change".to_string());
                } else {
                    match Buffer::from_file(&filename) {
                        Ok(new_buffer) => {
                            self.buffers[self.windows[self.active_window].buffer_index] = new_buffer;
                            self.windows[self.active_window].cursor = Cursor::default();
                            self.message = Some(format!("Opened {}", filename));
                        }
                        Err(e) => {
                            self.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            Command::GoToLine(line_num) => {
                self.save_jump_position();
                // Convert 1-indexed to 0-indexed
                let target_line = line_num.saturating_sub(1);
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = target_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Command::Substitute { pattern, replacement, global, range } => {
                let current_line = self.current_window().cursor.line;
                let line_count = self.current_buffer().line_count();
                
                let (start_line, end_line) = if let Some(r) = range {
                    let start = r.start.saturating_sub(1); // Convert to 0-indexed
                    let end = if r.end == usize::MAX {
                        line_count.saturating_sub(1)
                    } else {
                        r.end.saturating_sub(1).min(line_count.saturating_sub(1))
                    };
                    (start, end)
                } else {
                    // No range, use current line
                    (current_line, current_line)
                };

                let mut total_count = 0;
                for line in start_line..=end_line {
                    total_count += self.execute_substitute_line(line, &pattern, &replacement, global);
                }

                let range_count = end_line - start_line + 1;
                if total_count > 0 {
                    self.message = Some(format!("{} substitution{} on {} line{}",
                        total_count,
                        if total_count == 1 { "" } else { "s" },
                        range_count,
                        if range_count != 1 { "s" } else { "" }
                    ));
                } else {
                    self.message = Some("Pattern not found".to_string());
                }
            }
            Command::Delete { range } => {
                if let Some(r) = range {
                    let line_count = self.current_buffer().line_count();
                    let start = r.start.saturating_sub(1).min(line_count.saturating_sub(1));
                    let end = if r.end == usize::MAX {
                        line_count.saturating_sub(1)
                    } else {
                        r.end.saturating_sub(1).min(line_count.saturating_sub(1))
                    };

                    // Delete lines from end to start to maintain indices
                    for line in (start..=end).rev() {
                        let line_len = self.current_buffer().line_len(line);
                        self.current_buffer_mut().delete_range(line, 0, line, line_len);
                        // Also delete the newline if not the last line
                        if line < self.current_buffer().line_count() {
                            self.current_buffer_mut().delete_char(line, 0);
                        }
                    }

                    let deleted_count = end - start + 1;
                    self.message = Some(format!("{} line{} deleted", deleted_count, if deleted_count != 1 { "s" } else { "" }));
                    self.clamp_cursor();
                } else {
                    self.message = Some("No range specified".to_string());
                }
            }
            Command::Set { option, value } => {
                match self.config.set(&option, value.as_deref()) {
                    Ok(()) => {
                        self.message = Some(format!("{} set", option));
                    }
                    Err(e) => {
                        self.message = Some(e);
                    }
                }
            }
            Command::Help(topic) => {
                let help_text = if let Some(t) = topic {
                    self.get_help_topic(&t)
                } else {
                    self.get_general_help()
                };
                self.message = Some(help_text);
            }
            Command::BufferNext => {
                // Single buffer for now - show message
                self.message = Some("Already at the last buffer".to_string());
            }
            Command::BufferPrevious => {
                // Single buffer for now - show message
                self.message = Some("Already at the first buffer".to_string());
            }
            Command::BufferList => {
                // Show current buffer info
                let buf_name = self.current_buffer().file_name();
                let modified = if self.current_buffer().is_modified() { "[+]" } else { "" };
                self.message = Some(format!("1 %a   \"{}\" {} line {}",
                    buf_name,
                    modified,
                    self.current_buffer().line_count()
                ));
            }
            Command::BufferDelete(_num) => {
                // Cannot delete the only buffer
                self.message = Some("Cannot delete last buffer".to_string());
            }
            Command::Split => {
                self.message = Some("Horizontal split not yet implemented".to_string());
            }
            Command::VerticalSplit => {
                self.message = Some("Vertical split not yet implemented".to_string());
            }
            Command::CloseWindow => {
                self.message = Some("Window closing not yet implemented".to_string());
            }
            Command::Registers => {
                let regs = self.registers.get_all_registers();
                if regs.is_empty() {
                    self.message = Some("No registers populated".to_string());
                } else {
                    let output = regs.iter()
                        .map(|(k, v)| format!("\"{} {}", k, v))
                        .collect::<Vec<_>>()
                        .join("  ");
                    self.message = Some(output);
                }
            }
            Command::Marks => {
                let mut marks = self.current_buffer().get_all_marks();
                // Add global marks
                for (k, v) in &self.global_marks {
                    marks.push((*k, *v));
                }
                marks.sort_by_key(|(k, _)| *k);
                
                if marks.is_empty() {
                    self.message = Some("No marks set".to_string());
                } else {
                    let output = marks.iter()
                        .map(|(k, (line, col))| format!("{} {}:{}", k, line + 1, col))
                        .collect::<Vec<_>>()
                        .join("  ");
                    self.message = Some(output);
                }
            }
            Command::Unknown(cmd) => {
                self.message = Some(format!("Unknown command: {}", cmd));
            }
        }

        Ok(())
    }

    fn get_general_help(&self) -> String {
        "Bitsy - Vim-compatible text editor | :help [topic] for more | :q to quit".to_string()
    }

    fn get_help_topic(&self, topic: &str) -> String {
        match topic {
            "motions" | "movement" => {
                "Motions: hjkl (left/down/up/right), w/b/e (word), gg/G (file start/end), %/0/$ (line), f/F/t/T (find char), /? (search)".to_string()
            }
            "operators" | "editing" => {
                "Operators: d (delete), c (change), y (yank), p/P (paste), J (join), u (undo), . (repeat), >/< (indent), = (auto-indent)".to_string()
            }
            "modes" => {
                "Modes: i/I/a/A/o/O (insert), ESC (normal), v/V (visual), : (command), R (replace)".to_string()
            }
            "commands" => {
                ":Commands: :w (write), :q (quit), :e (edit), :s/find/replace/g (substitute), :set (options), :help, :d (delete)".to_string()
            }
            "textobjects" | "objects" => {
                "Text objects: aw/iw (word), ap/ip (paragraph), a\"/i\" (quotes), a(/i( (parens), a[/i[, a{/i{, a</i<".to_string()
            }
            "marks" => {
                "Marks: m{a-z} (set mark), '{a-z} (jump to mark line), `{a-z} (jump to exact position), g; (next change), g, (prev change)".to_string()
            }
            "search" => {
                "Search: /{pattern} (forward), ?{pattern} (backward), n (next), N (prev), * (word forward), # (word backward)".to_string()
            }
            "ranges" => {
                "Ranges: % (all lines), 1,10 (lines 1-10), . (current line). Use with :s, :d. Example: %s/old/new/g".to_string()
            }
            _ => {
                format!("No help for '{}'. Try :help motions, :help operators, :help commands", topic)
            }
        }
    }

    fn execute_search(&mut self) -> Result<()> {
        if let Some(pattern) = self.search_pattern.clone() {
            let start_line = self.current_window().cursor.line;
            let start_col = if self.search_forward {
                self.current_window().cursor.col + 1
            } else {
                self.current_window().cursor.col.saturating_sub(1)
            };

            let old_pos = (self.current_window().cursor.line, self.current_window().cursor.col);

            if self.search_forward {
                // Search forward
                if self.search_forward_from(start_line, start_col, &pattern) {
                    self.current_buffer_mut().set_mark('\'', old_pos);
                    self.current_buffer_mut().set_mark('`', old_pos);
                    return Ok(());
                }
                self.message = Some("Pattern not found".to_string());
            } else {
                // Search backward
                if self.search_backward_from(start_line, start_col, &pattern) {
                    self.current_buffer_mut().set_mark('\'', old_pos);
                    self.current_buffer_mut().set_mark('`', old_pos);
                    return Ok(());
                }
                self.message = Some("Pattern not found".to_string());
            }
        }
        Ok(())
    }

    fn search_forward_from(&mut self, start_line: usize, start_col: usize, pattern: &str) -> bool {
        let line_count = self.current_buffer().line_count();

        // Search from current position to end of current line
        if let Some(line_text) = self.current_buffer().get_line(start_line) {
            if let Some(pos) = line_text[start_col.min(line_text.len())..].find(pattern) {
                let window = self.current_window_mut();
                window.cursor.line = start_line;
                window.cursor.col = start_col + pos;
                return true;
            }
        }

        // Search remaining lines
        for line_idx in (start_line + 1)..line_count {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.find(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    return true;
                }
            }
        }

        // Wrap around to start
        for line_idx in 0..start_line {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.find(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    self.message = Some("search hit BOTTOM, continuing at TOP".to_string());
                    return true;
                }
            }
        }

        false
    }

    fn search_backward_from(&mut self, start_line: usize, start_col: usize, pattern: &str) -> bool {
        // Search from current position backwards in current line
        if let Some(line_text) = self.current_buffer().get_line(start_line) {
            let search_text = &line_text[..start_col.min(line_text.len())];
            if let Some(pos) = search_text.rfind(pattern) {
                let window = self.current_window_mut();
                window.cursor.line = start_line;
                window.cursor.col = pos;
                return true;
            }
        }

        // Search previous lines
        if start_line > 0 {
            for line_idx in (0..start_line).rev() {
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    if let Some(pos) = line_text.rfind(pattern) {
                        let window = self.current_window_mut();
                        window.cursor.line = line_idx;
                        window.cursor.col = pos;
                        return true;
                    }
                }
            }
        }

        // Wrap around to end
        let line_count = self.current_buffer().line_count();
        for line_idx in (start_line + 1..line_count).rev() {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.rfind(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    self.message = Some("search hit TOP, continuing at BOTTOM".to_string());
                    return true;
                }
            }
        }

        false
    }

    fn execute_substitute_line(&mut self, line: usize, pattern: &str, replacement: &str, global: bool) -> usize {
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let new_text = if global {
                line_text.replace(pattern, replacement)
            } else {
                line_text.replacen(pattern, replacement, 1)
            };

            let count = if global {
                line_text.matches(pattern).count()
            } else {
                if line_text.contains(pattern) { 1 } else { 0 }
            };

            if count > 0 {
                // Replace the entire line
                let line_len = self.current_buffer().line_len(line);
                self.current_buffer_mut().delete_range(line, 0, line, line_len);
                for (i, ch) in new_text.chars().enumerate() {
                    self.current_buffer_mut().insert_char(line, i, ch);
                }
            }

            count
        } else {
            0
        }
    }

    fn execute_substitute_all(&mut self, pattern: &str, replacement: &str, global: bool) -> usize {
        let line_count = self.current_buffer().line_count();
        let mut total_count = 0;

        for line in 0..line_count {
            total_count += self.execute_substitute_line(line, pattern, replacement, global);
        }

        total_count
    }

    fn apply_text_object_word(&mut self, modifier: TextObjectModifier) -> Result<()> {
        // Find word boundaries around cursor
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                return Ok(());
            }

            let mut start = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut end = start;

            // If on whitespace, move to next word for 'aw', stay for 'iw'
            if start < chars.len() && chars[start].is_whitespace() {
                if matches!(modifier, TextObjectModifier::Around) {
                    // Skip whitespace to find next word
                    while end < chars.len() && chars[end].is_whitespace() {
                        end += 1;
                    }
                    if end < chars.len() {
                        start = end;
                    }
                }
            }

            // Find word start
            while start > 0 && !chars[start - 1].is_whitespace() {
                start -= 1;
            }

            // Find word end
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }

            // For 'aw', include trailing whitespace
            if matches!(modifier, TextObjectModifier::Around) {
                while end < chars.len() && chars[end].is_whitespace() {
                    end += 1;
                }
                // If no trailing whitespace, include leading whitespace
                if end == chars.len() || !chars[end - 1].is_whitespace() {
                    while start > 0 && chars[start - 1].is_whitespace() {
                        start -= 1;
                    }
                }
            }

            let line = self.current_window().cursor.line;
            self.apply_operator_to_range(line, start, line, end)?;
        }
        Ok(())
    }

    fn apply_text_object_word_big(&mut self, modifier: TextObjectModifier) -> Result<()> {
        // Similar to apply_text_object_word but for WORD (space-separated)
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                return Ok(());
            }

            let mut start = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut end = start;

            // Find WORD start
            while start > 0 && !chars[start - 1].is_whitespace() {
                start -= 1;
            }

            // Find WORD end
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }

            // For 'aW', include trailing whitespace
            if matches!(modifier, TextObjectModifier::Around) {
                while end < chars.len() && chars[end].is_whitespace() {
                    end += 1;
                }
            }

            let line = self.current_window().cursor.line;
            self.apply_operator_to_range(line, start, line, end)?;
        }
        Ok(())
    }

    fn apply_text_object_paragraph(&mut self, modifier: TextObjectModifier) -> Result<()> {
        let line_count = self.current_buffer().line_count();
        let mut start_line = self.current_window().cursor.line;
        let mut end_line = self.current_window().cursor.line;

        // Find paragraph start
        while start_line > 0 {
            if let Some(text) = self.current_buffer().get_line(start_line - 1) {
                if text.trim().is_empty() {
                    break;
                }
            }
            start_line -= 1;
        }

        // Find paragraph end
        while end_line < line_count - 1 {
            if let Some(text) = self.current_buffer().get_line(end_line + 1) {
                if text.trim().is_empty() {
                    end_line += 1;
                    break;
                }
            }
            end_line += 1;
        }

        // For 'ap', include blank lines
        if matches!(modifier, TextObjectModifier::Around) {
            while end_line < line_count - 1 {
                if let Some(text) = self.current_buffer().get_line(end_line + 1) {
                    if !text.trim().is_empty() {
                        break;
                    }
                    end_line += 1;
                } else {
                    break;
                }
            }
        }

        self.apply_operator_to_range(start_line, 0, end_line, self.current_buffer().line_len(end_line))?;
        Ok(())
    }

    fn apply_text_object_quote(&mut self, modifier: TextObjectModifier, quote: char) -> Result<()> {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                return Ok(());
            }

            let cursor_pos = self.current_window().cursor.col.min(chars.len().saturating_sub(1));

            // Find the quote pair around cursor
            let mut start = None;
            let mut end = None;

            // Search backward for opening quote
            for i in (0..=cursor_pos).rev() {
                if chars[i] == quote {
                    start = Some(i);
                    break;
                }
            }

            // Search forward for closing quote
            if let Some(start_pos) = start {
                for i in (start_pos + 1)..chars.len() {
                    if chars[i] == quote {
                        end = Some(i);
                        break;
                    }
                }
            }

            if let (Some(s), Some(e)) = (start, end) {
                let (range_start, range_end) = match modifier {
                    TextObjectModifier::Inner => (s + 1, e),
                    TextObjectModifier::Around => (s, e + 1),
                };

                let line = self.current_window().cursor.line;
                self.apply_operator_to_range(line, range_start, line, range_end)?;
            }
        }
        Ok(())
    }

    fn apply_text_object_bracket(&mut self, modifier: TextObjectModifier, open: char, close: char) -> Result<()> {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                return Ok(());
            }

            let cursor_pos = self.current_window().cursor.col.min(chars.len().saturating_sub(1));

            // Find matching brackets around cursor
            let mut start = None;
            let mut end = None;
            let mut depth = 0;

            // Search backward for opening bracket
            for i in (0..=cursor_pos).rev() {
                if chars[i] == close {
                    depth += 1;
                } else if chars[i] == open {
                    if depth == 0 {
                        start = Some(i);
                        break;
                    }
                    depth -= 1;
                }
            }

            // Search forward for closing bracket
            if let Some(start_pos) = start {
                depth = 0;
                for i in start_pos..chars.len() {
                    if chars[i] == open {
                        depth += 1;
                    } else if chars[i] == close {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i);
                            break;
                        }
                    }
                }
            }

            if let (Some(s), Some(e)) = (start, end) {
                let (range_start, range_end) = match modifier {
                    TextObjectModifier::Inner => (s + 1, e),
                    TextObjectModifier::Around => (s, e + 1),
                };

                let line = self.current_window().cursor.line;
                self.apply_operator_to_range(line, range_start, line, range_end)?;
            }
        }
        Ok(())
    }

    fn handle_operator_motion(&mut self, action: Action) -> Result<()> {
        // Handle operator doubling (dd, yy, cc)
        let doubled = match (&self.pending_operator, &action) {
            (PendingOperator::Delete, Action::Delete) => Some("delete_line"),
            (PendingOperator::Yank, Action::Yank) => Some("yank_line"),
            (PendingOperator::Change, Action::Change) => Some("change_line"),
            _ => None,
        };

        if let Some(op) = doubled {
            // Operator was doubled, apply to whole line(s) with count
            let count = if self.count == 0 { 1 } else { self.count };
            match op {
                "delete_line" => {
                    let start_line = self.current_window().cursor.line;
                    let line_count = self.current_buffer().line_count();
                    let end_line = (start_line + count - 1).min(line_count - 1);

                    // Collect lines being deleted
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_delete(None, RegisterContent::Line(lines));

                    // Delete the lines
                    for _ in 0..count {
                        let line = self.current_window().cursor.line;
                        let buffer_line_count = self.current_buffer().line_count();
                        if line < buffer_line_count - 1 {
                            // Not last line - delete line and its newline
                            self.current_buffer_mut().delete_range(line, 0, line + 1, 0);
                        } else if line > 0 {
                            // Last line - delete from end of previous line
                            let prev_line_len = self.current_buffer().line_len(line - 1);
                            let current_line_len = self.current_buffer().line_len(line);
                            self.current_buffer_mut().delete_range(line - 1, prev_line_len, line, current_line_len);
                            self.current_window_mut().cursor.line -= 1;
                            break;
                        } else {
                            break;
                        }
                    }
                }
                "yank_line" => {
                    let start_line = self.current_window().cursor.line;
                    let end_line = (start_line + count - 1).min(self.current_buffer().line_count() - 1);

                    // Collect lines being yanked
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_yank(None, RegisterContent::Line(lines.clone()));
                    self.message = Some(format!("{} line{} yanked", lines.len(), if lines.len() == 1 { "" } else { "s" }));
                }
                "change_line" => {
                    let start_line = self.current_window().cursor.line;
                    let end_line = (start_line + count - 1).min(self.current_buffer().line_count() - 1);

                    // Collect lines being deleted
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_delete(None, RegisterContent::Line(lines));

                    // Delete line content(s) and enter insert mode
                    let line = self.current_window().cursor.line;
                    let line_len = self.current_buffer().line_len(line);
                    if line_len > 0 {
                        self.current_buffer_mut().delete_range(line, 0, line, line_len);
                    }

                    // Delete additional lines if count > 1
                    for _ in 1..count {
                        let line = self.current_window().cursor.line;
                        if line < self.current_buffer().line_count() - 1 {
                            self.current_buffer_mut().delete_range(line, 0, line + 1, 0);
                        } else {
                            break;
                        }
                    }

                    self.current_window_mut().cursor.col = 0;
                    self.mode = Mode::Insert;
                }
                _ => {}
            }

            // Record change for dot repeat (not for yank)
            if op == "delete_line" || op == "change_line" {
                self.record_change(action.clone());
            }

            self.pending_operator = PendingOperator::None;
            self.clamp_cursor();
            return Ok(());
        }

        // Handle Escape to cancel operator
        if action == Action::EnterNormalMode {
            self.pending_operator = PendingOperator::None;
            return Ok(());
        }

        // Apply operator to motion
        let start_line = self.current_window().cursor.line;
        let start_col = self.current_window().cursor.col;

        // Execute the motion
        match action {
            // Movement actions
            Action::MoveUp | Action::MoveDown | Action::MoveLeft | Action::MoveRight |
            Action::MoveWordForward | Action::MoveWordBackward | Action::MoveWordEnd |
            Action::MoveWordEndBack | Action::MoveWordEndBackBig |
            Action::MoveWordForwardBig | Action::MoveWordBackwardBig | Action::MoveWordEndBig |
            Action::MoveLineStart | Action::MoveLineFirstNonBlank | Action::MoveLineEnd |
            Action::MoveLineEndNonBlank | Action::MoveLineStartDisplay | Action::MoveLineEndDisplay |
            Action::MoveFileStart | Action::MoveFileEnd |
            Action::MoveParagraphForward | Action::MoveParagraphBackward |
            Action::MoveSentenceForward | Action::MoveSentenceBackward |
            Action::FindChar(_) | Action::FindCharBack(_) | Action::TillChar(_) | Action::TillCharBack(_) |
            Action::RepeatLastFind | Action::RepeatLastFindReverse |
            Action::MoveToScreenTop | Action::MoveToScreenMiddle | Action::MoveToScreenBottom |
            Action::MoveMatchingBracket | Action::MoveToPercent |
            Action::MovePageUp | Action::MovePageDown |
            Action::MoveHalfPageUp | Action::MoveHalfPageDown => {
                // Save current position
                let old_cursor = self.current_window().cursor;

                // Execute the motion (repeat count times)
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.execute_action(action.clone())?;
                }

                // Get the range
                let end_line = self.current_window().cursor.line;
                let end_col = self.current_window().cursor.col;

                // Apply the operator to the range
                self.apply_operator_to_range(start_line, start_col, end_line, end_col)?;

                // Record change for dot repeat (not for yank)
                if self.pending_operator != PendingOperator::Yank && self.pending_operator != PendingOperator::None {
                    self.record_change(action.clone());
                }

                // Restore cursor for delete/change
                if self.pending_operator == PendingOperator::Delete || self.pending_operator == PendingOperator::Change {
                    self.current_window_mut().cursor = old_cursor;
                }

                self.pending_operator = PendingOperator::None;
            }
            _ => {
                // Not a motion, cancel the operator
                self.pending_operator = PendingOperator::None;
            }
        }

        self.clamp_cursor();
        Ok(())
    }

    fn apply_operator_to_range(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Result<()> {
        // Normalize range (ensure start comes before end)
        let (start_line, start_col, end_line, end_col) = if start_line > end_line || (start_line == end_line && start_col > end_col) {
            (end_line, end_col, start_line, start_col)
        } else {
            (start_line, start_col, end_line, end_col)
        };

        match self.pending_operator {
            PendingOperator::Delete => {
                // Get the text being deleted
                let deleted_text = self.get_range_text(start_line, start_col, end_line, end_col);
                self.registers.set_delete(None, RegisterContent::Char(deleted_text));

                // Delete the range
                self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
            }
            PendingOperator::Yank => {
                // Get the text being yanked
                let yanked_text = self.get_range_text(start_line, start_col, end_line, end_col);
                let char_count = yanked_text.len();
                self.registers.set_yank(None, RegisterContent::Char(yanked_text));
                self.message = Some(format!("Yanked {} characters", char_count));
                
                // Set change marks
                self.current_buffer_mut().set_mark('[', (start_line, start_col));
                self.current_buffer_mut().set_mark(']', (end_line, end_col));
            }
            PendingOperator::Change => {
                // Get the text being deleted
                let deleted_text = self.get_range_text(start_line, start_col, end_line, end_col);
                self.registers.set_delete(None, RegisterContent::Char(deleted_text));

                // Delete the range and enter insert mode
                self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
                self.mode = Mode::Insert;
            }
            PendingOperator::MakeLowercase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Lower);
            }
            PendingOperator::MakeUppercase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Upper);
            }
            PendingOperator::ToggleCase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Toggle);
            }
            PendingOperator::Indent => {
                self.apply_indent(start_line, end_line, true);
            }
            PendingOperator::Dedent => {
                self.apply_indent(start_line, end_line, false);
            }
            PendingOperator::AutoIndent => {
                self.apply_auto_indent(start_line, end_line);
            }
            PendingOperator::None => {}
        }

        Ok(())
    }

    fn get_range_text(&self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> String {
        if start_line == end_line {
            // Same line
            if let Some(line_text) = self.current_buffer().get_line(start_line) {
                let end = end_col.min(line_text.len());
                let start = start_col.min(line_text.len());
                return line_text[start..end].to_string();
            }
        } else {
            // Multiple lines
            let mut result = String::new();
            for line_idx in start_line..=end_line {
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    if line_idx == start_line {
                        result.push_str(&line_text[start_col.min(line_text.len())..]);
                        result.push('\n');
                    } else if line_idx == end_line {
                        result.push_str(&line_text[..end_col.min(line_text.len())]);
                    } else {
                        result.push_str(&line_text);
                        result.push('\n');
                    }
                }
            }
            return result;
        }
        String::new()
    }

    fn apply_case_change(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize, case_change: CaseChange) {
        for line_idx in start_line..=end_line {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                let chars: Vec<char> = line_text.chars().collect();
                if chars.is_empty() {
                    continue;
                }

                let (from, to) = if line_idx == start_line && line_idx == end_line {
                    // Same line
                    (start_col.min(chars.len()), end_col.min(chars.len()))
                } else if line_idx == start_line {
                    // First line
                    (start_col.min(chars.len()), chars.len())
                } else if line_idx == end_line {
                    // Last line
                    (0, end_col.min(chars.len()))
                } else {
                    // Middle line
                    (0, chars.len())
                };

                // Apply case change to each character in range
                for col in from..to {
                    if col < chars.len() {
                        let old_char = chars[col];
                        let new_char = match case_change {
                            CaseChange::Lower => old_char.to_lowercase().collect::<Vec<char>>(),
                            CaseChange::Upper => old_char.to_uppercase().collect::<Vec<char>>(),
                            CaseChange::Toggle => {
                                if old_char.is_lowercase() {
                                    old_char.to_uppercase().collect::<Vec<char>>()
                                } else {
                                    old_char.to_lowercase().collect::<Vec<char>>()
                                }
                            }
                        };

                        // Delete old char and insert new char(s)
                        if new_char.len() > 0 && new_char[0] != old_char {
                            self.current_buffer_mut().delete_char(line_idx, col);
                            for (i, ch) in new_char.iter().enumerate() {
                                self.current_buffer_mut().insert_char(line_idx, col + i, *ch);
                            }
                        }
                    }
                }
            }
        }
    }

    fn apply_indent(&mut self, start_line: usize, end_line: usize, indent_right: bool) {
        const SHIFTWIDTH: usize = 4;

        for line_idx in start_line..=end_line {
            if line_idx >= self.current_buffer().line_count() {
                break;
            }

            if indent_right {
                // Add indentation (shiftwidth spaces at the beginning)
                for i in 0..SHIFTWIDTH {
                    self.current_buffer_mut().insert_char(line_idx, i, ' ');
                }
            } else {
                // Remove indentation (up to shiftwidth spaces/tabs from the beginning)
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    let mut chars_to_remove = 0;
                    let chars: Vec<char> = line_text.chars().collect();

                    for &ch in chars.iter().take(SHIFTWIDTH) {
                        if ch == ' ' {
                            chars_to_remove += 1;
                        } else if ch == '\t' {
                            chars_to_remove += 1;
                            break; // One tab counts as full shiftwidth
                        } else {
                            break; // Stop at first non-whitespace
                        }
                    }

                    for _ in 0..chars_to_remove {
                        self.current_buffer_mut().delete_char(line_idx, 0);
                    }
                }
            }
        }
    }

    fn apply_auto_indent(&mut self, start_line: usize, end_line: usize) {
        // Simple auto-indent: for each line, match the indentation of the previous non-empty line
        for line_idx in start_line..=end_line {
            if line_idx >= self.current_buffer().line_count() {
                break;
            }

            // Find the indentation of the previous non-empty line
            let mut indent_level = 0;
            if line_idx > 0 {
                for prev_line in (0..line_idx).rev() {
                    if let Some(prev_text) = self.current_buffer().get_line(prev_line) {
                        let trimmed = prev_text.trim_start();
                        if !trimmed.is_empty() {
                            indent_level = prev_text.len() - trimmed.len();
                            break;
                        }
                    }
                }
            }

            // Remove existing indentation on this line
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                let trimmed = line_text.trim_start();
                let current_indent = line_text.len() - trimmed.len();

                for _ in 0..current_indent {
                    self.current_buffer_mut().delete_char(line_idx, 0);
                }

                // Add the new indentation
                for i in 0..indent_level {
                    self.current_buffer_mut().insert_char(line_idx, i, ' ');
                }
            }
        }
    }

    fn record_change(&mut self, action: Action) {
        // Record this action for dot repeat
        // Use current count if set, otherwise use 0 (will default to 1 on replay)
        let count = if self.count == 0 { 0 } else { self.count };
        self.last_change = Some((action, count));

        // Add position to change list
        let pos = (self.current_window().cursor.line, self.current_window().cursor.col);
        self.change_list.push(pos);
        self.change_index = self.change_list.len().saturating_sub(1);

        // Limit change list size to prevent unbounded growth
        if self.change_list.len() > 100 {
            self.change_list.remove(0);
            self.change_index = self.change_index.saturating_sub(1);
        }
    }

    fn play_macro(&mut self, reg_char: char) -> Result<()> {
        let actual_reg = if reg_char == '@' {
            self.last_macro_register
        } else {
            Some(reg_char)
        };

        if let Some(reg) = actual_reg {
            self.last_macro_register = Some(reg);
            
            if let Some(RegisterContent::Macro(keys)) = self.registers.get(Some(reg)) {
                let keys = keys.clone(); // Clone to avoid borrow checker issues
                
                // Play back keys
                // We need to handle counts.
                let count = if self.count == 0 { 1 } else { self.count };
                
                // Nesting protection? Vim allows recursion but checks depth.
                // For now, let's just play it.
                
                for _ in 0..count {
                    for key in &keys {
                        self.handle_key(key.clone())?;
                    }
                }
            } else {
                self.message = Some(format!("Register @{} is empty or not a macro", reg));
            }
        } else {
            self.message = Some("No previously executed macro".to_string());
        }
        
        Ok(())
    }

    fn save_jump_position(&mut self) {
        let pos = (self.current_window().cursor.line, self.current_window().cursor.col);
        self.current_buffer_mut().set_mark('\'', pos);
        self.current_buffer_mut().set_mark('`', pos);

        // If we traveled back in history, truncate future
        if self.jump_index < self.jump_list.len().saturating_sub(1) {
            self.jump_list.truncate(self.jump_index + 1);
        }

        // Add current position if list is empty or different from last entry
        if self.jump_list.is_empty() || self.jump_list.last() != Some(&pos) {
            self.jump_list.push(pos);
            self.jump_index = self.jump_list.len().saturating_sub(1);
        }
    }

    fn execute_action(&mut self, action: Action) -> Result<()> {
        match action {
            // Movement
            Action::MoveUp => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_up(count);
                self.clamp_cursor();
            }
            Action::MoveDown => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_down(count);
                self.clamp_cursor();
            }
            Action::MoveLeft => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_left(count);
                self.clamp_cursor();
            }
            Action::MoveRight => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_right(count);
                self.clamp_cursor();
            }
            Action::MoveWordForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_word_forward();
                }
            }
            Action::MoveWordBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_word_backward();
                }
            }
            Action::MoveLineStart => {
                self.current_window_mut().cursor.move_to_line_start();
            }
            Action::MoveLineEnd => {
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.move_to_line_end(line_len);
            }
            Action::MoveFileStart => {
                self.save_jump_position();
                // Support count: gg or 10gg (go to line 10)
                let target_line = if self.count > 0 {
                    // Count is 1-indexed, convert to 0-indexed
                    (self.count - 1).min(self.current_buffer().line_count().saturating_sub(1))
                } else {
                    0
                };
                self.current_window_mut().cursor.line = target_line;
                self.current_window_mut().cursor.col = 0;
            }
            Action::MoveFileEnd => {
                self.save_jump_position();
                let last_line = self.current_buffer().line_count().saturating_sub(1);
                self.current_window_mut().cursor.line = last_line;
                self.current_window_mut().cursor.col = 0;
            }
            Action::MoveWordEnd => {
                self.move_word_end();
            }
            Action::MoveWordForwardBig => {
                self.move_word_forward_big();
            }
            Action::MoveWordBackwardBig => {
                self.move_word_backward_big();
            }
            Action::MoveWordEndBig => {
                self.move_word_end_big();
            }
            Action::MoveWordEndBack => {
                self.move_word_end_back();
            }
            Action::MoveWordEndBackBig => {
                self.move_word_end_back_big();
            }
            Action::MoveLineFirstNonBlank => {
                self.move_to_first_non_blank();
            }
            Action::MoveLineEndNonBlank => {
                self.move_to_line_end_non_blank();
            }
            Action::MoveLineStartDisplay => {
                // For now, treat same as MoveLineStart (no line wrapping yet)
                self.current_window_mut().cursor.move_to_line_start();
            }
            Action::MoveLineEndDisplay => {
                // For now, treat same as MoveLineEnd (no line wrapping yet)
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.move_to_line_end(line_len);
            }
            Action::MoveSentenceForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_sentence_forward();
                }
            }
            Action::MoveSentenceBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_sentence_backward();
                }
            }
            Action::FindChar(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.find_char(ch, FindDirection::Forward);
                }
            }
            Action::FindCharBack(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.find_char(ch, FindDirection::Backward);
                }
            }
            Action::TillChar(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.find_char(ch, FindDirection::Till);
                }
            }
            Action::TillCharBack(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.find_char(ch, FindDirection::TillBack);
                }
            }
            Action::RepeatLastFind => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.repeat_last_find(false);
                }
            }
            Action::RepeatLastFindReverse => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.repeat_last_find(true);
                }
            }
            Action::MoveToScreenTop => {
                let offset_line = self.current_window().viewport.offset_line;
                self.current_window_mut().cursor.line = offset_line;
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::MoveToScreenMiddle => {
                let offset_line = self.current_window().viewport.offset_line;
                let height = self.current_window().viewport.height;
                let middle_line = offset_line + (height / 2);
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = middle_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::MoveToScreenBottom => {
                let offset_line = self.current_window().viewport.offset_line;
                let height = self.current_window().viewport.height;
                let bottom_line = offset_line + height - 1;
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = bottom_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::ScrollTopToScreen => {
                let line = self.current_window().cursor.line;
                self.current_window_mut().viewport.offset_line = line;
            }
            Action::ScrollMiddleToScreen => {
                let line = self.current_window().cursor.line;
                let height = self.current_window().viewport.height;
                let half_height = height / 2;
                self.current_window_mut().viewport.offset_line = line.saturating_sub(half_height);
            }
            Action::ScrollBottomToScreen => {
                let line = self.current_window().cursor.line;
                let height = self.current_window().viewport.height;
                let bottom_offset = height.saturating_sub(1);
                self.current_window_mut().viewport.offset_line = line.saturating_sub(bottom_offset);
            }
            Action::MoveParagraphForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_paragraph_forward();
                }
            }
            Action::MoveParagraphBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.move_paragraph_backward();
                }
            }
            Action::MoveMatchingBracket => {
                self.save_jump_position();
                self.move_to_matching_bracket();
            }
            Action::MovePageUp => {
                let page_size = self.current_window().viewport.height;
                self.current_window_mut().cursor.move_up(page_size);
                self.clamp_cursor();
            }
            Action::MovePageDown => {
                let page_size = self.current_window().viewport.height;
                self.current_window_mut().cursor.move_down(page_size);
                self.clamp_cursor();
            }
            Action::MoveHalfPageUp => {
                let half_page = self.current_window().viewport.height / 2;
                self.current_window_mut().cursor.move_up(half_page);
                self.clamp_cursor();
            }
            Action::MoveHalfPageDown => {
                let half_page = self.current_window().viewport.height / 2;
                self.current_window_mut().cursor.move_down(half_page);
                self.clamp_cursor();
            }
            Action::MoveToPercent => {
                self.save_jump_position();
                if self.count > 0 {
                    // Move to percentage of file (e.g., 50% goes to line at 50% of file)
                    let percent = self.count.min(100);
                    let total_lines = self.current_buffer().line_count();
                    let target_line = (total_lines * percent) / 100;
                    self.current_window_mut().cursor.line = target_line.saturating_sub(1).min(total_lines.saturating_sub(1));
                    self.current_window_mut().cursor.col = 0;
                } else {
                    // No count: fallback to matching bracket behavior
                    self.move_to_matching_bracket();
                }
                self.clamp_cursor();
            }

            // Mode switching
            Action::EnterInsertMode => {
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeBeginning => {
                self.current_window_mut().cursor.move_to_line_start();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppend => {
                self.current_window_mut().cursor.move_right(1);
                self.clamp_cursor();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppendEnd => {
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.col = line_len;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineBelow => {
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_buffer_mut().insert_newline(line, line_len);
                self.current_window_mut().cursor.line += 1;
                self.current_window_mut().cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineAbove => {
                let line = self.current_window().cursor.line;
                self.current_buffer_mut().insert_newline(line, 0);
                self.current_window_mut().cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterReplaceMode => {
                self.mode = Mode::Replace;
            }
            Action::EnterVisualMode => {
                self.mode = Mode::Visual;
                let cursor = self.current_window().cursor;
                self.selection = Some(Selection::from_cursor(cursor, Mode::Visual));
            }
            Action::EnterVisualLineMode => {
                self.mode = Mode::VisualLine;
                let cursor = self.current_window().cursor;
                self.selection = Some(Selection::from_cursor(cursor, Mode::VisualLine));
            }
            Action::EnterCommandMode => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
            Action::EnterNormalMode => {
                if self.mode == Mode::Visual || self.mode == Mode::VisualLine {
                    // Update < and > marks
                    if let Some(selection) = &self.selection {
                        let (start_pos, end_pos) = selection.range();
                        self.current_buffer_mut().set_mark('<', (start_pos.line, start_pos.col));
                        self.current_buffer_mut().set_mark('>', (end_pos.line, end_pos.col));
                    }
                }
                self.mode = Mode::Normal;
                self.selection = None; // Clear selection when leaving visual mode
                // In normal mode, cursor should not go past last char
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                if line_len > 0 && self.current_window().cursor.col >= line_len {
                    self.current_window_mut().cursor.col = line_len.saturating_sub(1);
                }
            }

            // Editing
            Action::InsertChar(c) => {
                if self.mode == Mode::Insert {
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.current_buffer_mut().insert_char(line, col, c);
                    self.current_window_mut().cursor.move_right(1);
                }
            }
            Action::InsertNewline => {
                if self.mode == Mode::Insert || self.mode == Mode::Replace {
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.current_buffer_mut().insert_newline(line, col);
                    self.current_window_mut().cursor.line += 1;
                    self.current_window_mut().cursor.col = 0;
                }
            }
            Action::DeleteChar => {
                if self.mode == Mode::Insert || self.mode == Mode::Replace {
                    // Backspace in insert/replace mode
                    if self.current_window().cursor.col > 0 {
                        self.current_window_mut().cursor.move_left(1);
                        let line = self.current_window().cursor.line;
                        let col = self.current_window().cursor.col;
                        self.current_buffer_mut().delete_char(line, col);
                    }
                } else if self.mode == Mode::Normal {
                    // x in normal mode - record for dot repeat
                    self.record_change(action.clone());
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.current_buffer_mut().delete_char(line, col);
                    self.clamp_cursor();
                }
            }
            Action::Replace(ch) => {
                let line = self.current_window().cursor.line;
                let col = self.current_window().cursor.col;

                if self.mode == Mode::Replace {
                    // R mode - continuous replace, move cursor right
                    if col < self.current_buffer().line_len(line) {
                        // Replace existing character
                        self.current_buffer_mut().delete_char(line, col);
                        self.current_buffer_mut().insert_char(line, col, ch);
                    } else {
                        // At end of line, insert instead
                        self.current_buffer_mut().insert_char(line, col, ch);
                    }
                    self.current_window_mut().cursor.move_right(1);
                } else {
                    // r{char} - single character replace in normal mode - record for dot repeat
                    self.record_change(action.clone());
                    if col < self.current_buffer().line_len(line) {
                        self.current_buffer_mut().delete_char(line, col);
                        self.current_buffer_mut().insert_char(line, col, ch);
                    }
                }
            }

            // Operators
            Action::Delete => {
                // Set pending operator
                self.pending_operator = PendingOperator::Delete;
            }
            Action::DeleteToEnd => {
                // Delete from cursor to end of line
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let end_col = self.current_buffer().line_len(line);
                if start_col < end_col {
                    let deleted = self.current_buffer().get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.current_buffer_mut().delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
            }
            Action::Change => {
                self.pending_operator = PendingOperator::Change;
            }
            Action::ChangeToEnd => {
                // Change from cursor to end of line
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let end_col = self.current_buffer().line_len(line);
                if start_col < end_col {
                    let deleted = self.current_buffer().get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.current_buffer_mut().delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
                self.mode = Mode::Insert;
            }
            Action::Yank => {
                self.pending_operator = PendingOperator::Yank;
            }
            Action::YankLine => {
                // Yank entire line
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    self.registers.set_yank(None, RegisterContent::Line(vec![line_text]));
                    self.message = Some("1 line yanked".to_string());
                }
            }
            Action::YankToEnd => {
                // Yank from cursor to end of line
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let line_len = self.current_buffer().line_len(line);
                if let Some(line_text) = self.current_buffer().get_line(line) {
                    if start_col < line_len {
                        let yanked = line_text[start_col..].to_string();
                        self.registers.set_yank(None, RegisterContent::Char(yanked));
                    }
                }
            }
            Action::Paste => {
                let start_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                
                // Paste after cursor (repeat count times)
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    if let Some(content) = self.registers.get(None) {
                        match content {
                            RegisterContent::Char(text) => {
                                for ch in text.chars() {
                                    if ch == '\n' {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_newline(line, col);
                                        self.current_window_mut().cursor.line += 1;
                                        self.current_window_mut().cursor.col = 0;
                                    } else {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_char(line, col, ch);
                                        self.current_window_mut().cursor.col += 1;
                                    }
                                }
                            }
                            RegisterContent::Line(lines) => {
                                // Paste line(s) below current line
                                let insert_line = self.current_window().cursor.line + 1;
                                for (i, line) in lines.iter().enumerate() {
                                    // Insert newline to create space
                                    let target_line = insert_line + i;
                                    if target_line > 0 {
                                        let prev_line = target_line - 1;
                                        let prev_line_len = self.current_buffer().line_len(prev_line);
                                        self.current_buffer_mut().insert_newline(prev_line, prev_line_len);
                                    }
                                    // Insert the line content
                                    for ch in line.chars() {
                                        self.current_buffer_mut().insert_char(target_line, 0, ch);
                                    }
                                }
                                self.current_window_mut().cursor.line = insert_line;
                                self.current_window_mut().cursor.col = 0;
                            }
                            _ => {}
                        }
                    }
                }
                
                let end_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                self.current_buffer_mut().set_mark('[', start_pos);
                self.current_buffer_mut().set_mark(']', end_pos);
            }
            Action::PasteBefore => {
                let start_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                
                // Paste before cursor (repeat count times)
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    if let Some(content) = self.registers.get(None) {
                        match content {
                            RegisterContent::Char(text) => {
                                for ch in text.chars() {
                                    if ch == '\n' {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_newline(line, col);
                                        self.current_window_mut().cursor.line += 1;
                                        self.current_window_mut().cursor.col = 0;
                                    } else {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_char(line, col, ch);
                                        self.current_window_mut().cursor.col += 1;
                                    }
                                }
                            }
                            RegisterContent::Line(lines) => {
                                // Paste line(s) above current line
                                for (i, line) in lines.iter().enumerate() {
                                    let target_line = self.current_window().cursor.line + i;
                                    // Insert newline to create space
                                    if target_line > 0 {
                                        let prev_line = target_line.saturating_sub(1);
                                        let prev_line_len = self.current_buffer().line_len(prev_line);
                                        self.current_buffer_mut().insert_newline(prev_line, prev_line_len);
                                    }
                                    // Insert the line content
                                    for ch in line.chars() {
                                        self.current_buffer_mut().insert_char(target_line, 0, ch);
                                    }
                                }
                                self.current_window_mut().cursor.col = 0;
                            }
                            _ => {}
                        }
                    }
                }
                
                let end_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                self.current_buffer_mut().set_mark('[', start_pos);
                self.current_buffer_mut().set_mark(']', end_pos);
            }
            Action::Join => {
                // Join current line with next line
                let current_line = self.current_window().cursor.line;
                if current_line < self.current_buffer().line_count() - 1 {
                    let line1_len = self.current_buffer().line_len(current_line);

                    // Delete the newline at end of current line
                    self.current_buffer_mut().delete_range(current_line, line1_len, current_line + 1, 0);

                    // Insert space if needed
                    if line1_len > 0 {
                        self.current_buffer_mut().insert_char(current_line, line1_len, ' ');
                    }
                }
            }
            Action::JoinNoSpace => {
                // gJ - Join current line with next line without space
                let current_line = self.current_window().cursor.line;
                if current_line < self.current_buffer().line_count() - 1 {
                    let line1_len = self.current_buffer().line_len(current_line);
                    // Delete the newline at end of current line
                    self.current_buffer_mut().delete_range(current_line, line1_len, current_line + 1, 0);
                }
            }
            Action::MakeLowercase => {
                self.pending_operator = PendingOperator::MakeLowercase;
            }
            Action::MakeUppercase => {
                self.pending_operator = PendingOperator::MakeUppercase;
            }
            Action::ToggleCase => {
                self.pending_operator = PendingOperator::ToggleCase;
            }
            Action::Indent => {
                self.pending_operator = PendingOperator::Indent;
            }
            Action::Dedent => {
                self.pending_operator = PendingOperator::Dedent;
            }
            Action::AutoIndent => {
                self.pending_operator = PendingOperator::AutoIndent;
            }

            Action::RepeatLastChange => {
                // Dot command - repeat last change
                if let Some((last_action, last_count)) = self.last_change.clone() {
                    // Use current count if specified, otherwise use stored count
                    let saved_count = self.count;
                    self.count = if saved_count > 0 { saved_count } else { last_count };

                    // Execute the last action
                    self.execute_action(last_action)?;

                    // Restore count (will be reset later in handle_key)
                    self.count = saved_count;
                }
            }

            Action::Quit => {
                if !self.current_buffer().is_modified() {
                    self.should_quit = true;
                } else {
                    self.message = Some("No write since last change".to_string());
                }
            }

            // Search
            Action::SearchForward => {
                self.mode = Mode::Search;
                self.search_buffer.clear();
                self.search_forward = true;
            }
            Action::SearchBackward => {
                self.mode = Mode::Search;
                self.search_buffer.clear();
                self.search_forward = false;
            }
            Action::SearchNext => {
                // Repeat last search in same direction
                if self.search_pattern.is_some() {
                    self.execute_search()?;
                }
            }
            Action::SearchPrevious => {
                // Repeat last search in opposite direction
                if self.search_pattern.is_some() {
                    self.search_forward = !self.search_forward;
                    self.execute_search()?;
                    self.search_forward = !self.search_forward; // Restore direction
                }
            }
            Action::SearchWordForward => {
                // Search for word under cursor forward
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    let chars: Vec<char> = line_text.chars().collect();
                    if self.current_window().cursor.col < chars.len() {
                        // Extract word under cursor
                        let mut start = self.current_window().cursor.col;
                        let mut end = self.current_window().cursor.col;

                        // Find word boundaries
                        while start > 0 && !chars[start - 1].is_whitespace() && chars[start - 1].is_alphanumeric() {
                            start -= 1;
                        }
                        while end < chars.len() && !chars[end].is_whitespace() && chars[end].is_alphanumeric() {
                            end += 1;
                        }

                        let word: String = chars[start..end].iter().collect();
                        if !word.is_empty() {
                            self.search_pattern = Some(word);
                            self.search_forward = true;
                            self.execute_search()?;
                        }
                    }
                }
            }
            Action::SearchWordBackward => {
                // Search for word under cursor backward
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    let chars: Vec<char> = line_text.chars().collect();
                    if self.current_window().cursor.col < chars.len() {
                        // Extract word under cursor
                        let mut start = self.current_window().cursor.col;
                        let mut end = self.current_window().cursor.col;

                        // Find word boundaries
                        while start > 0 && !chars[start - 1].is_whitespace() && chars[start - 1].is_alphanumeric() {
                            start -= 1;
                        }
                        while end < chars.len() && !chars[end].is_whitespace() && chars[end].is_alphanumeric() {
                            end += 1;
                        }

                        let word: String = chars[start..end].iter().collect();
                        if !word.is_empty() {
                            self.search_pattern = Some(word);
                            self.search_forward = false;
                            self.execute_search()?;
                        }
                    }
                }
            }

            // Change list navigation
            Action::JumpToChangeNext => {
                if self.change_index < self.change_list.len().saturating_sub(1) {
                    self.change_index += 1;
                    let (line, col) = self.change_list[self.change_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpToChangePrev => {
                if self.change_index > 0 {
                    self.change_index -= 1;
                    let (line, col) = self.change_list[self.change_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpBack => {
                if self.jump_list.is_empty() {
                    return Ok(());
                }
                
                let current_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                // If we are at the tip and have drifted, save current position
                if self.jump_index == self.jump_list.len() - 1 {
                    if self.jump_list[self.jump_index] != current_pos {
                        self.jump_list.push(current_pos);
                        self.jump_index = self.jump_list.len() - 1;
                    }
                }
                
                if self.jump_index > 0 {
                    self.jump_index -= 1;
                    let (line, col) = self.jump_list[self.jump_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpForward => {
                if self.jump_index < self.jump_list.len().saturating_sub(1) {
                    self.jump_index += 1;
                    let (line, col) = self.jump_list[self.jump_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }

            _ => {}
        }

        // Ensure cursor stays within buffer bounds
        self.clamp_cursor();

        // Update viewport to keep cursor visible
        let cursor = self.current_window().cursor;
        self.current_window_mut().viewport.ensure_cursor_visible(cursor.line, cursor.col);

        Ok(())
    }

    fn clamp_cursor(&mut self) {
        let line_count = self.current_buffer().line_count().max(1);
        self.current_window_mut().cursor.line = self.current_window().cursor.line.min(line_count - 1);

        let line_len = self.current_buffer().line_len(self.current_window().cursor.line);
        if self.mode == Mode::Normal && line_len > 0 {
            // In normal mode, cursor can't go past last character
            self.current_window_mut().cursor.col = self.current_window().cursor.col.min(line_len.saturating_sub(1));
        } else if self.mode == Mode::Insert {
            // In insert mode, cursor can be at end of line
            self.current_window_mut().cursor.col = self.current_window().cursor.col.min(line_len);
        }

        // Update selection if in visual mode
        if matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
            let cursor = self.current_window().cursor;
            if let Some(ref mut selection) = self.selection {
                selection.update_cursor(cursor.into());
            }
        }
    }

    fn move_word_forward(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let mut chars = line_text.chars().skip(self.current_window().cursor.col).peekable();
            let mut col = self.current_window().cursor.col;

            // Skip current word
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                chars.next();
                col += 1;
            }

            // Skip whitespace
            while let Some(&ch) = chars.peek() {
                if !ch.is_whitespace() {
                    break;
                }
                chars.next();
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_backward(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            if self.current_window().cursor.col == 0 {
                return;
            }

            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.current_window().cursor.col.saturating_sub(1);

            // Skip whitespace
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }

            // Skip word
            while col > 0 && !chars[col].is_whitespace() {
                col -= 1;
            }

            // Move past the whitespace
            if chars[col].is_whitespace() && col < chars.len() - 1 {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
        }
    }

    fn move_word_end(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col >= chars.len() {
                return;
            }

            let mut col = self.current_window().cursor.col;

            // If on whitespace, skip to next word
            if chars[col].is_whitespace() {
                while col < chars.len() && chars[col].is_whitespace() {
                    col += 1;
                }
            }

            // Move to end of current word
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_forward_big(&mut self) {
        // WORD motion (space-separated)
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let mut chars = line_text.chars().skip(self.current_window().cursor.col).peekable();
            let mut col = self.current_window().cursor.col;

            // Skip current WORD (non-whitespace)
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                chars.next();
                col += 1;
            }

            // Skip whitespace
            while let Some(&ch) = chars.peek() {
                if !ch.is_whitespace() {
                    break;
                }
                chars.next();
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_backward_big(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            if self.current_window().cursor.col == 0 {
                return;
            }

            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.current_window().cursor.col.saturating_sub(1);

            // Skip whitespace
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }

            // Skip WORD
            while col > 0 && !chars[col].is_whitespace() {
                col -= 1;
            }

            // Move past the whitespace
            if chars[col].is_whitespace() && col < chars.len() - 1 {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
        }
    }

    fn move_word_end_big(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col >= chars.len() {
                return;
            }

            let mut col = self.current_window().cursor.col;

            // If on whitespace, skip to next WORD
            if chars[col].is_whitespace() {
                while col < chars.len() && chars[col].is_whitespace() {
                    col += 1;
                }
            }

            // Move to end of current WORD
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_to_first_non_blank(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = 0;

            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }

            self.current_window_mut().cursor.col = col.min(chars.len().saturating_sub(1));
        }
    }

    fn move_word_end_back(&mut self) {
        // ge - move to end of previous word
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col == 0 {
                // Try previous line
                if self.current_window().cursor.line > 0 {
                    self.current_window_mut().cursor.line -= 1;
                    let prev_line_len = self.current_buffer().line_len(self.current_window().cursor.line);
                    self.current_window_mut().cursor.col = prev_line_len.saturating_sub(1);
                }
                return;
            }

            let mut col = self.current_window().cursor.col.saturating_sub(1);

            // Skip whitespace
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }

            // Skip to start of word
            while col > 0 && !chars[col.saturating_sub(1)].is_whitespace() {
                col -= 1;
            }

            // Move to end of that word
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
        }
    }

    fn move_word_end_back_big(&mut self) {
        // gE - move to end of previous WORD
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col == 0 {
                // Try previous line
                if self.current_window().cursor.line > 0 {
                    self.current_window_mut().cursor.line -= 1;
                    let prev_line_len = self.current_buffer().line_len(self.current_window().cursor.line);
                    self.current_window_mut().cursor.col = prev_line_len.saturating_sub(1);
                }
                return;
            }

            let mut col = self.current_window().cursor.col.saturating_sub(1);

            // Skip whitespace
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }

            // Skip to start of WORD
            while col > 0 && !chars[col.saturating_sub(1)].is_whitespace() {
                col -= 1;
            }

            // Move to end of that WORD
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() {
                col += 1;
            }

            self.current_window_mut().cursor.col = col;
        }
    }

    fn move_to_line_end_non_blank(&mut self) {
        // g_ - move to last non-blank character of line
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                self.current_window_mut().cursor.col = 0;
                return;
            }

            let mut col = chars.len() - 1;

            // Find last non-whitespace character
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }

            self.current_window_mut().cursor.col = col;
        }
    }

    fn move_sentence_forward(&mut self) {
        // ) - move to next sentence
        // Sentences end with . ! ? followed by space/newline
        let line_count = self.current_buffer().line_count();
        let mut line = self.current_window().cursor.line;
        let mut col = self.current_window().cursor.col + 1;

        while line < line_count {
            if let Some(line_text) = self.current_buffer().get_line(line) {
                let chars: Vec<char> = line_text.chars().collect();

                while col < chars.len() {
                    if matches!(chars[col], '.' | '!' | '?') {
                        // Check if followed by space or end of line
                        if col + 1 >= chars.len() || chars[col + 1].is_whitespace() {
                            // Skip whitespace after sentence end
                            col += 1;
                            while col < chars.len() && chars[col].is_whitespace() {
                                col += 1;
                            }

                            // If we're at end of line, move to next line
                            if col >= chars.len() {
                                line += 1;
                                col = 0;
                                // Skip empty lines
                                while line < line_count {
                                    if let Some(next_line) = self.current_buffer().get_line(line) {
                                        if !next_line.trim().is_empty() {
                                            break;
                                        }
                                    }
                                    line += 1;
                                }
                            }

                            self.current_window_mut().cursor.line = line.min(line_count.saturating_sub(1));
                            self.current_window_mut().cursor.col = col;
                            self.clamp_cursor();
                            return;
                        }
                    }
                    col += 1;
                }
            }
            line += 1;
            col = 0;
        }

        // Reached end of buffer
        self.current_window_mut().cursor.line = line_count.saturating_sub(1);
        let last_line_len = self.current_buffer().line_len(self.current_window().cursor.line);
        self.current_window_mut().cursor.col = last_line_len.saturating_sub(1);
        self.clamp_cursor();
    }

    fn move_sentence_backward(&mut self) {
        // ( - move to previous sentence
        if self.current_window().cursor.line == 0 && self.current_window().cursor.col == 0 {
            return;
        }

        let mut line = self.current_window().cursor.line;
        let mut col = self.current_window().cursor.col.saturating_sub(1);

        loop {
            if let Some(line_text) = self.current_buffer().get_line(line) {
                let chars: Vec<char> = line_text.chars().collect();

                while col > 0 {
                    if matches!(chars[col], '.' | '!' | '?') {
                        // Check if followed by space or end of line
                        if col + 1 >= chars.len() || chars[col + 1].is_whitespace() {
                            // Move past the sentence end
                            col += 1;
                            while col < chars.len() && chars[col].is_whitespace() {
                                col += 1;
                            }

                            self.current_window_mut().cursor.line = line;
                            self.current_window_mut().cursor.col = col.min(chars.len().saturating_sub(1));
                            self.clamp_cursor();
                            return;
                        }
                    }
                    col = col.saturating_sub(1);
                }
            }

            if line == 0 {
                break;
            }

            line -= 1;
            col = self.current_buffer().line_len(line);
        }

        // Reached start of buffer
        self.current_window_mut().cursor.line = 0;
        self.current_window_mut().cursor.col = 0;
    }

    fn find_char(&mut self, ch: char, direction: FindDirection) {
        // Store for repeat
        self.last_find = Some((ch, direction));

        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                return;
            }

            match direction {
                FindDirection::Forward => {
                    // f - find forward (inclusive)
                    let start = self.current_window().cursor.col + 1;
                    for i in start..chars.len() {
                        if chars[i] == ch {
                            self.current_window_mut().cursor.col = i;
                            return;
                        }
                    }
                }
                FindDirection::Backward => {
                    // F - find backward (inclusive)
                    if self.current_window().cursor.col == 0 {
                        return;
                    }
                    let start = self.current_window().cursor.col - 1;
                    for i in (0..=start).rev() {
                        if chars[i] == ch {
                            self.current_window_mut().cursor.col = i;
                            return;
                        }
                    }
                }
                FindDirection::Till => {
                    // t - till forward (stop before)
                    let start = self.current_window().cursor.col + 1;
                    for i in start..chars.len() {
                        if chars[i] == ch {
                            if i > 0 {
                                self.current_window_mut().cursor.col = i - 1;
                            }
                            return;
                        }
                    }
                }
                FindDirection::TillBack => {
                    // T - till backward (stop after)
                    if self.current_window().cursor.col == 0 {
                        return;
                    }
                    let start = self.current_window().cursor.col - 1;
                    for i in (0..=start).rev() {
                        if chars[i] == ch {
                            if i < chars.len() - 1 {
                                self.current_window_mut().cursor.col = i + 1;
                            }
                            return;
                        }
                    }
                }
            }
        }
    }

    fn repeat_last_find(&mut self, reverse: bool) {
        if let Some((ch, direction)) = self.last_find {
            let direction = if reverse {
                // Reverse the direction
                match direction {
                    FindDirection::Forward => FindDirection::Backward,
                    FindDirection::Backward => FindDirection::Forward,
                    FindDirection::Till => FindDirection::TillBack,
                    FindDirection::TillBack => FindDirection::Till,
                }
            } else {
                direction
            };

            self.find_char(ch, direction);
        }
    }

    fn move_to_screen_top(&mut self) {
        // H - Move cursor to top of visible screen
        self.current_window_mut().cursor.line = self.current_window().viewport.offset_line;
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_to_screen_middle(&mut self) {
        // M - Move cursor to middle of visible screen
        let middle_line = self.current_window().viewport.offset_line + (self.current_window().viewport.height / 2);
        self.current_window_mut().cursor.line = middle_line.min(self.current_buffer().line_count().saturating_sub(1));
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_to_screen_bottom(&mut self) {
        // L - Move cursor to bottom of visible screen
        let bottom_line = self.current_window().viewport.offset_line + self.current_window().viewport.height - 1;
        self.current_window_mut().cursor.line = bottom_line.min(self.current_buffer().line_count().saturating_sub(1));
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    fn scroll_top_to_screen(&mut self) {
        // zt - Scroll so current line is at top of screen
        self.current_window_mut().viewport.offset_line = self.current_window().cursor.line;
    }

    fn scroll_middle_to_screen(&mut self) {
        // zz - Scroll so current line is at middle of screen
        let half_height = self.current_window().viewport.height / 2;
        self.current_window_mut().viewport.offset_line = self.current_window().cursor.line.saturating_sub(half_height);
    }

    fn scroll_bottom_to_screen(&mut self) {
        // zb - Scroll so current line is at bottom of screen
        let bottom_offset = self.current_window().viewport.height.saturating_sub(1);
        self.current_window_mut().viewport.offset_line = self.current_window().cursor.line.saturating_sub(bottom_offset);
    }

    fn move_paragraph_forward(&mut self) {
        let mut line = self.current_window().cursor.line + 1;
        let line_count = self.current_buffer().line_count();

        // Skip non-empty lines
        while line < line_count {
            if let Some(text) = self.current_buffer().get_line(line) {
                if text.trim().is_empty() {
                    break;
                }
            }
            line += 1;
        }

        // Skip empty lines
        while line < line_count {
            if let Some(text) = self.current_buffer().get_line(line) {
                if !text.trim().is_empty() {
                    break;
                }
            }
            line += 1;
        }

        self.current_window_mut().cursor.line = line.min(line_count.saturating_sub(1));
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_paragraph_backward(&mut self) {
        if self.current_window().cursor.line == 0 {
            return;
        }

        let mut line = self.current_window().cursor.line.saturating_sub(1);

        // Skip non-empty lines
        loop {
            if let Some(text) = self.current_buffer().get_line(line) {
                if text.trim().is_empty() {
                    break;
                }
            }
            if line == 0 {
                break;
            }
            line -= 1;
        }

        // Skip empty lines
        loop {
            if line == 0 {
                break;
            }
            if let Some(text) = self.current_buffer().get_line(line.saturating_sub(1)) {
                if !text.trim().is_empty() {
                    line -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        self.current_window_mut().cursor.line = line;
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_to_matching_bracket(&mut self) {
        if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if self.current_window().cursor.col >= chars.len() {
                return;
            }

            let current_char = chars[self.current_window().cursor.col];
            let matching_brackets = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

            for (open, close) in &matching_brackets {
                if current_char == *open {
                    // Search forward for closing bracket
                    let mut depth = 0;
                    for (i, &ch) in chars.iter().enumerate().skip(self.current_window().cursor.col) {
                        if ch == *open {
                            depth += 1;
                        } else if ch == *close {
                            depth -= 1;
                            if depth == 0 {
                                self.current_window_mut().cursor.col = i;
                                return;
                            }
                        }
                    }
                } else if current_char == *close {
                    // Search backward for opening bracket
                    let mut depth = 0;
                    for i in (0..=self.current_window().cursor.col).rev() {
                        let ch = chars[i];
                        if ch == *close {
                            depth += 1;
                        } else if ch == *open {
                            depth -= 1;
                            if depth == 0 {
                                self.current_window_mut().cursor.col = i;
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        self.terminal.clear_screen()?;

        // Render buffer content
        self.render_buffer()?;

        // Render status line
        self.render_status_line()?;

        // Render command line or message
        self.render_command_line()?;

        // Position cursor (account for line number gutter)
        let line_num_width = self.config.line_number_width(self.current_buffer().line_count());
        let screen_row = self.current_window().cursor.line.saturating_sub(self.current_window().viewport.offset_line);
        let screen_col = self.current_window().cursor.col.saturating_sub(self.current_window().viewport.offset_col) + line_num_width;
        self.terminal.move_cursor(screen_col as u16, screen_row as u16)?;

        self.terminal.show_cursor()?;
        self.terminal.flush()?;

        Ok(())
    }

    fn render_buffer(&mut self) -> Result<()> {
        let (width, height) = self.terminal.size();
        let viewport_height = (height as usize).saturating_sub(2);
        let line_num_width = self.config.line_number_width(self.current_buffer().line_count());

        let offset_line = self.current_window().viewport.offset_line;
        let offset_col = self.current_window().viewport.offset_col;

        for row in 0..viewport_height {
            let file_line = offset_line + row;

            self.terminal.move_cursor(0, row as u16)?;

            if file_line < self.current_buffer().line_count() {
                // Render line number
                self.render_line_number(file_line, line_num_width)?;

                // Render line content with selection highlighting
                if let Some(line) = self.current_buffer().get_line(file_line) {
                    let start = offset_col.min(line.len());
                    let available_width = (width as usize).saturating_sub(line_num_width);
                    self.render_line_content(file_line, &line, start, available_width)?;
                }
            } else {
                // Render empty line indicator
                if line_num_width > 0 {
                    let padding = " ".repeat(line_num_width.saturating_sub(1));
                    self.terminal.print_colored(&padding, Color::DarkGrey)?;
                }
                self.terminal.print_colored("~", Color::Blue)?;
            }
        }

        Ok(())
    }

    fn render_line_content(&mut self, line: usize, line_text: &str, start_col: usize, available_width: usize) -> Result<()> {
        let chars: Vec<char> = line_text.chars().collect();

        // If no selection or not in visual mode, render normally
        if self.selection.is_none() || !matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
            let visible_text: String = chars[start_col.min(chars.len())..]
                .iter()
                .take(available_width)
                .collect();
            self.terminal.print(&visible_text)?;
            return Ok(());
        }

        // Render with selection highlighting
        if let Some(ref selection) = self.selection {
            for (col_idx, &ch) in chars.iter().enumerate().skip(start_col).take(available_width) {
                if selection.contains(line, col_idx) {
                    // Character is selected - render with highlight
                    self.terminal.print_with_bg(&ch.to_string(), Color::White, Color::Blue)?;
                } else {
                    // Character is not selected - render normally
                    self.terminal.print(&ch.to_string())?;
                }
            }
        }

        Ok(())
    }

    fn render_line_number(&mut self, line: usize, width: usize) -> Result<()> {
        if width == 0 {
            return Ok(());
        }

        let number = match self.config.line_numbers {
            LineNumberMode::None => return Ok(()),
            LineNumberMode::Absolute => {
                format!("{:>width$} ", line + 1, width = width - 1)
            }
            LineNumberMode::Relative => {
                let distance = if line == self.current_window().cursor.line {
                    line + 1
                } else {
                    (line as isize - self.current_window().cursor.line as isize).abs() as usize
                };
                format!("{:>width$} ", distance, width = width - 1)
            }
            LineNumberMode::RelativeAbsolute => {
                let distance = if line == self.current_window().cursor.line {
                    line + 1
                } else {
                    (line as isize - self.current_window().cursor.line as isize).abs() as usize
                };
                format!("{:>width$} ", distance, width = width - 1)
            }
        };

        // Highlight current line number
        if line == self.current_window().cursor.line && self.config.show_current_line {
            self.terminal.print_colored(&number, Color::Yellow)?;
        } else {
            self.terminal.print_colored(&number, Color::DarkGrey)?;
        }

        Ok(())
    }

    fn render_status_line(&mut self) -> Result<()> {
        let (width, height) = self.terminal.size();
        let status_row = height.saturating_sub(2);

        self.terminal.move_cursor(0, status_row)?;

        let filename = self.current_buffer().file_name();
        let total_lines = self.current_buffer().line_count();
        let cursor = self.current_window().cursor;
        let modified = self.current_buffer().is_modified();
        
        self.statusline.update(self.mode, &filename, cursor, modified, total_lines);

        let status_text = self.statusline.render(width as usize);
        self.terminal.print_colored(&status_text, Color::Black)?;

        Ok(())
    }

    fn render_command_line(&mut self) -> Result<()> {
        let (_, height) = self.terminal.size();
        let command_row = height.saturating_sub(1);

        self.terminal.move_cursor(0, command_row)?;

        if self.mode == Mode::Command {
            let cmd_line = format!(":{}", self.command_buffer);
            self.terminal.print(&cmd_line)?;
        } else if self.mode == Mode::Search {
            let search_line = if self.search_forward {
                format!("/{}", self.search_buffer)
            } else {
                format!("?{}", self.search_buffer)
            };
            self.terminal.print(&search_line)?;
        } else if let Some(ref msg) = self.message {
            self.terminal.print(msg)?;
            // self.message = None; // Do not clear message here, logic should be elsewhere
        }

        Ok(())
    }
}
