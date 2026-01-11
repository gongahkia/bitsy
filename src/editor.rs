// Main editor coordination

use crossterm::event::{Event, KeyCode, KeyEvent};
use crossterm::style::Color;
use std::path::Path;

use crate::buffer::Buffer;
use crate::command::{parse_command, Command};
use crate::cursor::Cursor;
use crate::error::Result;
use crate::keymap::{map_key, Action};
use crate::mode::Mode;
use crate::register::{RegisterContent, RegisterManager};
use crate::statusline::StatusLine;
use crate::terminal::Terminal;
use crate::viewport::Viewport;

#[derive(Debug, Clone, PartialEq)]
enum PendingOperator {
    None,
    Delete,
    Change,
    Yank,
}

pub struct Editor {
    terminal: Terminal,
    buffer: Buffer,
    cursor: Cursor,
    mode: Mode,
    viewport: Viewport,
    statusline: StatusLine,
    command_buffer: String,
    message: Option<String>,
    should_quit: bool,
    registers: RegisterManager,
    pending_operator: PendingOperator,
}

impl Editor {
    pub fn new() -> Result<Self> {
        let terminal = Terminal::new()?;
        let (width, height) = terminal.size();

        // Reserve 2 lines: 1 for status, 1 for command/message
        let viewport_height = (height as usize).saturating_sub(2);

        Ok(Self {
            terminal,
            buffer: Buffer::new(),
            cursor: Cursor::default(),
            mode: Mode::Normal,
            viewport: Viewport::new(width as usize, viewport_height),
            statusline: StatusLine::new(),
            command_buffer: String::new(),
            message: None,
            should_quit: false,
            registers: RegisterManager::new(),
            pending_operator: PendingOperator::None,
        })
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.buffer = Buffer::from_file(path)?;
        self.cursor = Cursor::default();
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
                self.viewport.resize(width as usize, viewport_height);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.mode == Mode::Command {
            self.handle_command_mode_key(key)?;
        } else {
            let action = map_key(key, &self.mode);
            self.execute_action(action)?;
        }
        Ok(())
    }

    fn handle_command_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Enter => {
                self.execute_command()?;
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_command(&mut self) -> Result<()> {
        let cmd = parse_command(&self.command_buffer)?;

        match cmd {
            Command::Write => {
                self.buffer.save()?;
                self.message = Some("File written".to_string());
            }
            Command::Quit => {
                if self.buffer.is_modified() {
                    self.message = Some("No write since last change (use :q! to force)".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            Command::WriteQuit => {
                self.buffer.save()?;
                self.should_quit = true;
            }
            Command::ForceQuit => {
                self.should_quit = true;
            }
            Command::Edit(filename) => {
                if self.buffer.is_modified() {
                    self.message = Some("No write since last change".to_string());
                } else {
                    match Buffer::from_file(&filename) {
                        Ok(new_buffer) => {
                            self.buffer = new_buffer;
                            self.cursor = Cursor::default();
                            self.message = Some(format!("Opened {}", filename));
                        }
                        Err(e) => {
                            self.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            Command::Unknown(cmd) => {
                self.message = Some(format!("Unknown command: {}", cmd));
            }
        }

        Ok(())
    }

    fn execute_action(&mut self, action: Action) -> Result<()> {
        match action {
            // Movement
            Action::MoveUp => {
                self.cursor.move_up(1);
                self.clamp_cursor();
            }
            Action::MoveDown => {
                self.cursor.move_down(1);
                self.clamp_cursor();
            }
            Action::MoveLeft => {
                self.cursor.move_left(1);
                self.clamp_cursor();
            }
            Action::MoveRight => {
                self.cursor.move_right(1);
                self.clamp_cursor();
            }
            Action::MoveWordForward => {
                self.move_word_forward();
            }
            Action::MoveWordBackward => {
                self.move_word_backward();
            }
            Action::MoveLineStart => {
                self.cursor.move_to_line_start();
            }
            Action::MoveLineEnd => {
                let line_len = self.buffer.line_len(self.cursor.line);
                self.cursor.move_to_line_end(line_len);
            }
            Action::MoveFileStart => {
                self.cursor.line = 0;
                self.cursor.col = 0;
            }
            Action::MoveFileEnd => {
                self.cursor.line = self.buffer.line_count().saturating_sub(1);
                self.cursor.col = 0;
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
            Action::MoveLineFirstNonBlank => {
                self.move_to_first_non_blank();
            }
            Action::MoveParagraphForward => {
                self.move_paragraph_forward();
            }
            Action::MoveParagraphBackward => {
                self.move_paragraph_backward();
            }
            Action::MoveMatchingBracket => {
                self.move_to_matching_bracket();
            }
            Action::MovePageUp => {
                let page_size = self.viewport.height;
                self.cursor.move_up(page_size);
                self.clamp_cursor();
            }
            Action::MovePageDown => {
                let page_size = self.viewport.height;
                self.cursor.move_down(page_size);
                self.clamp_cursor();
            }
            Action::MoveHalfPageUp => {
                let half_page = self.viewport.height / 2;
                self.cursor.move_up(half_page);
                self.clamp_cursor();
            }
            Action::MoveHalfPageDown => {
                let half_page = self.viewport.height / 2;
                self.cursor.move_down(half_page);
                self.clamp_cursor();
            }

            // Mode switching
            Action::EnterInsertMode => {
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeBeginning => {
                self.cursor.move_to_line_start();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppend => {
                self.cursor.move_right(1);
                self.clamp_cursor();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppendEnd => {
                let line_len = self.buffer.line_len(self.cursor.line);
                self.cursor.col = line_len;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineBelow => {
                let line = self.cursor.line;
                let line_len = self.buffer.line_len(line);
                self.buffer.insert_newline(line, line_len);
                self.cursor.line += 1;
                self.cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineAbove => {
                self.buffer.insert_newline(self.cursor.line, 0);
                self.cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterVisualMode => {
                self.mode = Mode::Visual;
            }
            Action::EnterVisualLineMode => {
                self.mode = Mode::VisualLine;
            }
            Action::EnterCommandMode => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
            Action::EnterNormalMode => {
                self.mode = Mode::Normal;
                // In normal mode, cursor should not go past last char
                let line_len = self.buffer.line_len(self.cursor.line);
                if line_len > 0 && self.cursor.col >= line_len {
                    self.cursor.col = line_len.saturating_sub(1);
                }
            }

            // Editing
            Action::InsertChar(c) => {
                if self.mode == Mode::Insert {
                    self.buffer.insert_char(self.cursor.line, self.cursor.col, c);
                    self.cursor.move_right(1);
                }
            }
            Action::InsertNewline => {
                if self.mode == Mode::Insert {
                    self.buffer.insert_newline(self.cursor.line, self.cursor.col);
                    self.cursor.line += 1;
                    self.cursor.col = 0;
                }
            }
            Action::DeleteChar => {
                if self.mode == Mode::Insert {
                    // Backspace in insert mode
                    if self.cursor.col > 0 {
                        self.cursor.move_left(1);
                        self.buffer.delete_char(self.cursor.line, self.cursor.col);
                    }
                } else if self.mode == Mode::Normal {
                    // x in normal mode
                    self.buffer.delete_char(self.cursor.line, self.cursor.col);
                    self.clamp_cursor();
                }
            }

            // Operators
            Action::Delete => {
                // Set pending operator
                self.pending_operator = PendingOperator::Delete;
            }
            Action::DeleteToEnd => {
                // Delete from cursor to end of line
                let line = self.cursor.line;
                let start_col = self.cursor.col;
                let end_col = self.buffer.line_len(line);
                if start_col < end_col {
                    let deleted = self.buffer.get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.buffer.delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
            }
            Action::Change => {
                self.pending_operator = PendingOperator::Change;
            }
            Action::ChangeToEnd => {
                // Change from cursor to end of line
                let line = self.cursor.line;
                let start_col = self.cursor.col;
                let end_col = self.buffer.line_len(line);
                if start_col < end_col {
                    let deleted = self.buffer.get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.buffer.delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
                self.mode = Mode::Insert;
            }
            Action::Yank => {
                self.pending_operator = PendingOperator::Yank;
            }
            Action::YankLine => {
                // Yank entire line
                if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
                    self.registers.set_yank(None, RegisterContent::Line(vec![line_text]));
                    self.message = Some("1 line yanked".to_string());
                }
            }
            Action::YankToEnd => {
                // Yank from cursor to end of line
                let line = self.cursor.line;
                let start_col = self.cursor.col;
                let line_len = self.buffer.line_len(line);
                if let Some(line_text) = self.buffer.get_line(line) {
                    if start_col < line_len {
                        let yanked = line_text[start_col..].to_string();
                        self.registers.set_yank(None, RegisterContent::Char(yanked));
                    }
                }
            }
            Action::Paste => {
                // Paste after cursor
                if let Some(content) = self.registers.get(None) {
                    match content {
                        RegisterContent::Char(text) => {
                            for ch in text.chars() {
                                if ch == '\n' {
                                    self.buffer.insert_newline(self.cursor.line, self.cursor.col);
                                    self.cursor.line += 1;
                                    self.cursor.col = 0;
                                } else {
                                    self.buffer.insert_char(self.cursor.line, self.cursor.col, ch);
                                    self.cursor.col += 1;
                                }
                            }
                        }
                        RegisterContent::Line(lines) => {
                            // Paste line(s) below current line
                            let insert_line = self.cursor.line + 1;
                            for (i, line) in lines.iter().enumerate() {
                                // Insert newline to create space
                                let target_line = insert_line + i;
                                if target_line > 0 {
                                    let prev_line = target_line - 1;
                                    let prev_line_len = self.buffer.line_len(prev_line);
                                    self.buffer.insert_newline(prev_line, prev_line_len);
                                }
                                // Insert the line content
                                for ch in line.chars() {
                                    self.buffer.insert_char(target_line, 0, ch);
                                }
                            }
                            self.cursor.line = insert_line;
                            self.cursor.col = 0;
                        }
                        _ => {}
                    }
                }
            }
            Action::PasteBefore => {
                // Paste before cursor
                if let Some(content) = self.registers.get(None) {
                    match content {
                        RegisterContent::Char(text) => {
                            for ch in text.chars() {
                                if ch == '\n' {
                                    self.buffer.insert_newline(self.cursor.line, self.cursor.col);
                                    self.cursor.line += 1;
                                    self.cursor.col = 0;
                                } else {
                                    self.buffer.insert_char(self.cursor.line, self.cursor.col, ch);
                                    self.cursor.col += 1;
                                }
                            }
                        }
                        RegisterContent::Line(lines) => {
                            // Paste line(s) above current line
                            for (i, line) in lines.iter().enumerate() {
                                let target_line = self.cursor.line + i;
                                // Insert newline to create space
                                if target_line > 0 {
                                    let prev_line = target_line.saturating_sub(1);
                                    let prev_line_len = self.buffer.line_len(prev_line);
                                    self.buffer.insert_newline(prev_line, prev_line_len);
                                }
                                // Insert the line content
                                for ch in line.chars() {
                                    self.buffer.insert_char(target_line, 0, ch);
                                }
                            }
                            self.cursor.col = 0;
                        }
                        _ => {}
                    }
                }
            }
            Action::Join => {
                // Join current line with next line
                let current_line = self.cursor.line;
                if current_line < self.buffer.line_count() - 1 {
                    let line1_len = self.buffer.line_len(current_line);

                    // Delete the newline at end of current line
                    self.buffer.delete_range(current_line, line1_len, current_line + 1, 0);

                    // Insert space if needed
                    if line1_len > 0 {
                        self.buffer.insert_char(current_line, line1_len, ' ');
                    }
                }
            }

            Action::Quit => {
                if !self.buffer.is_modified() {
                    self.should_quit = true;
                } else {
                    self.message = Some("No write since last change".to_string());
                }
            }

            _ => {}
        }

        // Ensure cursor stays within buffer bounds
        self.clamp_cursor();

        // Update viewport to keep cursor visible
        self.viewport.ensure_cursor_visible(self.cursor.line, self.cursor.col);

        Ok(())
    }

    fn clamp_cursor(&mut self) {
        let line_count = self.buffer.line_count().max(1);
        self.cursor.line = self.cursor.line.min(line_count - 1);

        let line_len = self.buffer.line_len(self.cursor.line);
        if self.mode == Mode::Normal && line_len > 0 {
            // In normal mode, cursor can't go past last character
            self.cursor.col = self.cursor.col.min(line_len.saturating_sub(1));
        } else if self.mode == Mode::Insert {
            // In insert mode, cursor can be at end of line
            self.cursor.col = self.cursor.col.min(line_len);
        }
    }

    fn move_word_forward(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let mut chars = line_text.chars().skip(self.cursor.col).peekable();
            let mut col = self.cursor.col;

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

            self.cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_backward(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            if self.cursor.col == 0 {
                return;
            }

            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.cursor.col.saturating_sub(1);

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

            self.cursor.col = col;
        }
    }

    fn move_word_end(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.cursor.col >= chars.len() {
                return;
            }

            let mut col = self.cursor.col;

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

            self.cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_forward_big(&mut self) {
        // WORD motion (space-separated)
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let mut chars = line_text.chars().skip(self.cursor.col).peekable();
            let mut col = self.cursor.col;

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

            self.cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_word_backward_big(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            if self.cursor.col == 0 {
                return;
            }

            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.cursor.col.saturating_sub(1);

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

            self.cursor.col = col;
        }
    }

    fn move_word_end_big(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.cursor.col >= chars.len() {
                return;
            }

            let mut col = self.cursor.col;

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

            self.cursor.col = col;
            self.clamp_cursor();
        }
    }

    fn move_to_first_non_blank(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = 0;

            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }

            self.cursor.col = col.min(chars.len().saturating_sub(1));
        }
    }

    fn move_paragraph_forward(&mut self) {
        let mut line = self.cursor.line + 1;
        let line_count = self.buffer.line_count();

        // Skip non-empty lines
        while line < line_count {
            if let Some(text) = self.buffer.get_line(line) {
                if text.trim().is_empty() {
                    break;
                }
            }
            line += 1;
        }

        // Skip empty lines
        while line < line_count {
            if let Some(text) = self.buffer.get_line(line) {
                if !text.trim().is_empty() {
                    break;
                }
            }
            line += 1;
        }

        self.cursor.line = line.min(line_count.saturating_sub(1));
        self.cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_paragraph_backward(&mut self) {
        if self.cursor.line == 0 {
            return;
        }

        let mut line = self.cursor.line.saturating_sub(1);

        // Skip non-empty lines
        loop {
            if let Some(text) = self.buffer.get_line(line) {
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
            if let Some(text) = self.buffer.get_line(line.saturating_sub(1)) {
                if !text.trim().is_empty() {
                    line -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        self.cursor.line = line;
        self.cursor.col = 0;
        self.clamp_cursor();
    }

    fn move_to_matching_bracket(&mut self) {
        if let Some(line_text) = self.buffer.get_line(self.cursor.line) {
            let chars: Vec<char> = line_text.chars().collect();
            if self.cursor.col >= chars.len() {
                return;
            }

            let current_char = chars[self.cursor.col];
            let matching_brackets = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

            for (open, close) in &matching_brackets {
                if current_char == *open {
                    // Search forward for closing bracket
                    let mut depth = 0;
                    for (i, &ch) in chars.iter().enumerate().skip(self.cursor.col) {
                        if ch == *open {
                            depth += 1;
                        } else if ch == *close {
                            depth -= 1;
                            if depth == 0 {
                                self.cursor.col = i;
                                return;
                            }
                        }
                    }
                } else if current_char == *close {
                    // Search backward for opening bracket
                    let mut depth = 0;
                    for i in (0..=self.cursor.col).rev() {
                        let ch = chars[i];
                        if ch == *close {
                            depth += 1;
                        } else if ch == *open {
                            depth -= 1;
                            if depth == 0 {
                                self.cursor.col = i;
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

        // Position cursor
        let screen_row = self.cursor.line.saturating_sub(self.viewport.offset_line);
        let screen_col = self.cursor.col.saturating_sub(self.viewport.offset_col);
        self.terminal.move_cursor(screen_col as u16, screen_row as u16)?;

        self.terminal.show_cursor()?;
        self.terminal.flush()?;

        Ok(())
    }

    fn render_buffer(&mut self) -> Result<()> {
        let (width, height) = self.terminal.size();
        let viewport_height = (height as usize).saturating_sub(2);

        for row in 0..viewport_height {
            let file_line = self.viewport.offset_line + row;

            self.terminal.move_cursor(0, row as u16)?;

            if file_line < self.buffer.line_count() {
                if let Some(line) = self.buffer.get_line(file_line) {
                    let start = self.viewport.offset_col.min(line.len());
                    let visible_line = &line[start..].chars().take(width as usize).collect::<String>();
                    self.terminal.print(visible_line)?;
                }
            } else {
                self.terminal.print_colored("~", Color::Blue)?;
            }
        }

        Ok(())
    }

    fn render_status_line(&mut self) -> Result<()> {
        let (width, height) = self.terminal.size();
        let status_row = height.saturating_sub(2);

        self.terminal.move_cursor(0, status_row)?;

        let filename = self.buffer.file_name();
        let total_lines = self.buffer.line_count();
        self.statusline.update(self.mode, &filename, self.cursor, self.buffer.is_modified(), total_lines);

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
        } else if let Some(ref msg) = self.message {
            self.terminal.print(msg)?;
            self.message = None; // Clear message after displaying
        }

        Ok(())
    }
}
