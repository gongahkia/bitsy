// Cursor position tracking

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn move_up(&mut self, lines: usize) {
        self.line = self.line.saturating_sub(lines);
    }

    pub fn move_down(&mut self, lines: usize) {
        self.line = self.line.saturating_add(lines);
    }

    pub fn move_left(&mut self, cols: usize) {
        self.col = self.col.saturating_sub(cols);
    }

    pub fn move_right(&mut self, cols: usize) {
        self.col = self.col.saturating_add(cols);
    }

    pub fn move_to_line_start(&mut self) {
        self.col = 0;
    }

    pub fn move_to_line_end(&mut self, line_len: usize) {
        self.col = line_len.saturating_sub(1);
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self { line: 0, col: 0 }
    }
}
