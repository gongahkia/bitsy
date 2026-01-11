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
use crate::statusline::StatusLine;
use crate::terminal::Terminal;
use crate::viewport::Viewport;

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
