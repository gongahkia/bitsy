// Status line rendering

use crate::cursor::Cursor;
use crate::mode::Mode;

pub struct StatusLine {
    mode: Mode,
    filename: String,
    cursor: Cursor,
    modified: bool,
    total_lines: usize,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            filename: "[No Name]".to_string(),
            cursor: Cursor::default(),
            modified: false,
            total_lines: 1,
        }
    }

    pub fn update(&mut self, mode: Mode, filename: &str, cursor: Cursor, modified: bool, total_lines: usize) {
        self.mode = mode;
        self.filename = filename.to_string();
        self.cursor = cursor;
        self.modified = modified;
        self.total_lines = total_lines;
    }

    pub fn render(&self, width: usize) -> String {
        let modified_indicator = if self.modified { "[+]" } else { "" };
        let mode_str = self.mode.as_str();
        let position = format!("{}:{}", self.cursor.line + 1, self.cursor.col + 1);
        let percentage = if self.total_lines > 0 {
            ((self.cursor.line + 1) * 100 / self.total_lines).min(100)
        } else {
            0
        };

        let left = format!(" {} {} {}", mode_str, self.filename, modified_indicator);
        let right = format!("{} {}% ", position, percentage);

        let padding_len = width.saturating_sub(left.len() + right.len());
        let padding = " ".repeat(padding_len);

        format!("{}{}{}", left, padding, right)
    }
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}
