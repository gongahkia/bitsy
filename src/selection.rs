// Selection/range tracking for visual mode

use crate::cursor::Cursor;
use crate::mode::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl From<Cursor> for Position {
    fn from(cursor: Cursor) -> Self {
        Self {
            line: cursor.line,
            col: cursor.col,
        }
    }
}

impl From<Position> for Cursor {
    fn from(pos: Position) -> Self {
        Cursor::new(pos.line, pos.col)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    anchor: Position,
    cursor: Position,
    mode: Mode,
}

impl Selection {
    pub fn new(anchor: Position, cursor: Position, mode: Mode) -> Self {
        Self {
            anchor,
            cursor,
            mode,
        }
    }

    pub fn from_cursor(cursor: Cursor, mode: Mode) -> Self {
        let pos = Position::from(cursor);
        Self {
            anchor: pos,
            cursor: pos,
            mode,
        }
    }

    pub fn update_cursor(&mut self, cursor: Position) {
        self.cursor = cursor;
    }

    pub fn anchor(&self) -> Position {
        self.anchor
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Get the normalized range (start, end) regardless of selection direction
    pub fn range(&self) -> (Position, Position) {
        let (start, end) = if self.anchor.line < self.cursor.line
            || (self.anchor.line == self.cursor.line && self.anchor.col <= self.cursor.col)
        {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        };

        match self.mode {
            Mode::Visual => (start, end),
            Mode::VisualLine => {
                // For VisualLine, include entire lines
                (
                    Position {
                        line: start.line,
                        col: 0,
                    },
                    Position {
                        line: end.line,
                        col: usize::MAX, // Will be clamped to line length
                    },
                )
            }
            Mode::VisualBlock => {
                // For VisualBlock, keep as-is (rectangular selection)
                (start, end)
            }
            _ => (start, end),
        }
    }

    /// Check if a position is within the selection
    pub fn contains(&self, line: usize, col: usize) -> bool {
        let (start, end) = self.range();

        match self.mode {
            Mode::Visual => {
                if line < start.line || line > end.line {
                    return false;
                }

                if start.line == end.line {
                    // Single line selection
                    col >= start.col && col <= end.col
                } else if line == start.line {
                    // First line of multi-line selection
                    col >= start.col
                } else if line == end.line {
                    // Last line of multi-line selection
                    col <= end.col
                } else {
                    // Middle lines of multi-line selection
                    true
                }
            }
            Mode::VisualLine => {
                // Entire lines are selected
                line >= start.line && line <= end.line
            }
            Mode::VisualBlock => {
                // Rectangular selection
                if line < start.line || line > end.line {
                    return false;
                }
                let (min_col, max_col) = if start.col <= end.col {
                    (start.col, end.col)
                } else {
                    (end.col, start.col)
                };
                col >= min_col && col <= max_col
            }
            _ => false,
        }
    }

    /// Get the selected text from a buffer
    pub fn get_text<F>(&self, get_line: F) -> String
    where
        F: Fn(usize) -> Option<String>,
    {
        let (start, end) = self.range();
        let mut result = String::new();

        for line_idx in start.line..=end.line {
            if let Some(line_text) = get_line(line_idx) {
                match self.mode {
                    Mode::Visual => {
                        if start.line == end.line {
                            // Single line selection
                            let start_col = start.col.min(line_text.len());
                            let end_col = end.col.min(line_text.len());
                            result.push_str(&line_text[start_col..=end_col]);
                        } else if line_idx == start.line {
                            // First line of multi-line selection
                            let start_col = start.col.min(line_text.len());
                            result.push_str(&line_text[start_col..]);
                            result.push('\n');
                        } else if line_idx == end.line {
                            // Last line of multi-line selection
                            let end_col = end.col.min(line_text.len());
                            result.push_str(&line_text[..=end_col]);
                        } else {
                            // Middle lines
                            result.push_str(&line_text);
                            result.push('\n');
                        }
                    }
                    Mode::VisualLine => {
                        result.push_str(&line_text);
                        result.push('\n');
                    }
                    Mode::VisualBlock => {
                        let (min_col, max_col) = if start.col <= end.col {
                            (start.col, end.col)
                        } else {
                            (end.col, start.col)
                        };
                        let start_col = min_col.min(line_text.len());
                        let end_col = max_col.min(line_text.len());
                        if start_col < line_text.len() {
                            result.push_str(&line_text[start_col..=end_col]);
                        }
                        if line_idx < end.line {
                            result.push('\n');
                        }
                    }
                    _ => {}
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_creation() {
        let anchor = Position { line: 0, col: 0 };
        let cursor = Position { line: 0, col: 5 };
        let selection = Selection::new(anchor, cursor, Mode::Visual);

        assert_eq!(selection.anchor(), anchor);
        assert_eq!(selection.cursor(), cursor);
        assert_eq!(selection.mode(), Mode::Visual);
    }

    #[test]
    fn test_selection_range_forward() {
        let selection = Selection::new(
            Position { line: 0, col: 0 },
            Position { line: 2, col: 5 },
            Mode::Visual,
        );

        let (start, end) = selection.range();
        assert_eq!(start.line, 0);
        assert_eq!(start.col, 0);
        assert_eq!(end.line, 2);
        assert_eq!(end.col, 5);
    }

    #[test]
    fn test_selection_range_backward() {
        let selection = Selection::new(
            Position { line: 2, col: 5 },
            Position { line: 0, col: 0 },
            Mode::Visual,
        );

        let (start, end) = selection.range();
        assert_eq!(start.line, 0);
        assert_eq!(start.col, 0);
        assert_eq!(end.line, 2);
        assert_eq!(end.col, 5);
    }

    #[test]
    fn test_selection_contains_visual() {
        let selection = Selection::new(
            Position { line: 1, col: 2 },
            Position { line: 3, col: 5 },
            Mode::Visual,
        );

        // Test contains
        assert!(selection.contains(1, 2));
        assert!(selection.contains(1, 5));
        assert!(selection.contains(2, 0));
        assert!(selection.contains(3, 5));

        // Test not contains
        assert!(!selection.contains(0, 0));
        assert!(!selection.contains(1, 1));
        assert!(!selection.contains(3, 6));
        assert!(!selection.contains(4, 0));
    }

    #[test]
    fn test_selection_contains_visual_line() {
        let selection = Selection::new(
            Position { line: 1, col: 2 },
            Position { line: 3, col: 5 },
            Mode::VisualLine,
        );

        // Entire lines should be selected
        assert!(selection.contains(1, 0));
        assert!(selection.contains(1, 100));
        assert!(selection.contains(2, 0));
        assert!(selection.contains(3, 0));

        // Outside lines should not be selected
        assert!(!selection.contains(0, 0));
        assert!(!selection.contains(4, 0));
    }

    #[test]
    fn test_selection_contains_visual_block() {
        let selection = Selection::new(
            Position { line: 1, col: 2 },
            Position { line: 3, col: 5 },
            Mode::VisualBlock,
        );

        // Within the block
        assert!(selection.contains(1, 2));
        assert!(selection.contains(2, 3));
        assert!(selection.contains(3, 5));

        // Outside the block
        assert!(!selection.contains(1, 1));
        assert!(!selection.contains(1, 6));
        assert!(!selection.contains(0, 3));
        assert!(!selection.contains(4, 3));
    }

    #[test]
    fn test_update_cursor() {
        let mut selection = Selection::from_cursor(Cursor::new(0, 0), Mode::Visual);
        assert_eq!(selection.cursor(), Position { line: 0, col: 0 });

        selection.update_cursor(Position { line: 2, col: 5 });
        assert_eq!(selection.cursor(), Position { line: 2, col: 5 });
        assert_eq!(selection.anchor(), Position { line: 0, col: 0 });
    }
}
