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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_default() {
        let cursor = Cursor::default();
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new(5, 10);
        assert_eq!(cursor.line, 5);
        assert_eq!(cursor.col, 10);
    }

    #[test]
    fn test_move_up() {
        let mut cursor = Cursor::new(5, 0);
        cursor.move_up(2);
        assert_eq!(cursor.line, 3);
    }

    #[test]
    fn test_move_up_saturating() {
        let mut cursor = Cursor::new(1, 0);
        cursor.move_up(5);
        assert_eq!(cursor.line, 0);
    }

    #[test]
    fn test_move_down() {
        let mut cursor = Cursor::new(5, 0);
        cursor.move_down(3);
        assert_eq!(cursor.line, 8);
    }

    #[test]
    fn test_move_left() {
        let mut cursor = Cursor::new(0, 10);
        cursor.move_left(3);
        assert_eq!(cursor.col, 7);
    }

    #[test]
    fn test_move_left_saturating() {
        let mut cursor = Cursor::new(0, 2);
        cursor.move_left(5);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_move_right() {
        let mut cursor = Cursor::new(0, 5);
        cursor.move_right(3);
        assert_eq!(cursor.col, 8);
    }

    #[test]
    fn test_move_to_line_start() {
        let mut cursor = Cursor::new(0, 10);
        cursor.move_to_line_start();
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_move_to_line_end() {
        let mut cursor = Cursor::new(0, 0);
        cursor.move_to_line_end(20);
        assert_eq!(cursor.col, 19);
    }
}
