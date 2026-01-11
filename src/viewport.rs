// Viewport for scrolling through buffer

#[derive(Debug)]
pub struct Viewport {
    pub offset_line: usize,
    pub offset_col: usize,
    pub width: usize,
    pub height: usize,
}

impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            offset_line: 0,
            offset_col: 0,
            width,
            height,
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.offset_line = self.offset_line.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.offset_line = self.offset_line.saturating_add(lines);
    }

    pub fn scroll_to(&mut self, line: usize) {
        self.offset_line = line;
    }

    pub fn ensure_cursor_visible(&mut self, cursor_line: usize, cursor_col: usize) {
        // Vertical scrolling
        if cursor_line < self.offset_line {
            self.offset_line = cursor_line;
        } else if cursor_line >= self.offset_line + self.height {
            self.offset_line = cursor_line.saturating_sub(self.height - 1);
        }

        // Horizontal scrolling
        if cursor_col < self.offset_col {
            self.offset_col = cursor_col;
        } else if cursor_col >= self.offset_col + self.width {
            self.offset_col = cursor_col.saturating_sub(self.width - 1);
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }
}
