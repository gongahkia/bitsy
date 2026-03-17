// operation-based undo/redo system

use crate::buffer::Buffer;

#[derive(Debug, Clone)]
pub enum UndoOp {
    InsertChar { line: usize, col: usize, ch: char },
    DeleteChar { line: usize, col: usize, ch: char },
    InsertNewline { line: usize, col: usize },
    DeleteNewline { line: usize, col: usize }, // join line+1 into line at col
    InsertRange { line: usize, col: usize, text: String },
    DeleteRange { line: usize, col: usize, text: String },
}

impl UndoOp {
    fn inverse(&self) -> UndoOp {
        match self {
            UndoOp::InsertChar { line, col, ch } => UndoOp::DeleteChar { line: *line, col: *col, ch: *ch },
            UndoOp::DeleteChar { line, col, ch } => UndoOp::InsertChar { line: *line, col: *col, ch: *ch },
            UndoOp::InsertNewline { line, col } => UndoOp::DeleteNewline { line: *line, col: *col },
            UndoOp::DeleteNewline { line, col } => UndoOp::InsertNewline { line: *line, col: *col },
            UndoOp::InsertRange { line, col, text } => UndoOp::DeleteRange { line: *line, col: *col, text: text.clone() },
            UndoOp::DeleteRange { line, col, text } => UndoOp::InsertRange { line: *line, col: *col, text: text.clone() },
        }
    }

    pub fn apply(&self, buffer: &mut Buffer) {
        match self {
            UndoOp::InsertChar { line, col, ch } => {
                buffer.insert_char(*line, *col, *ch);
            }
            UndoOp::DeleteChar { line, col, .. } => {
                buffer.delete_char(*line, *col);
            }
            UndoOp::InsertNewline { line, col } => {
                buffer.insert_newline(*line, *col);
            }
            UndoOp::DeleteNewline { line, col } => {
                // join: delete the newline at end of `line`, merging line+1 into line
                buffer.delete_range(*line, *col, *line + 1, 0);
            }
            UndoOp::InsertRange { line, col, text } => {
                let mut cur_line = *line;
                let mut cur_col = *col;
                for ch in text.chars() {
                    if ch == '\n' {
                        buffer.insert_newline(cur_line, cur_col);
                        cur_line += 1;
                        cur_col = 0;
                    } else {
                        buffer.insert_char(cur_line, cur_col, ch);
                        cur_col += 1;
                    }
                }
            }
            UndoOp::DeleteRange { line, col, text } => {
                // compute end position from text
                let mut end_line = *line;
                let mut end_col = *col;
                for ch in text.chars() {
                    if ch == '\n' {
                        end_line += 1;
                        end_col = 0;
                    } else {
                        end_col += 1;
                    }
                }
                buffer.delete_range(*line, *col, end_line, end_col);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UndoGroup {
    ops: Vec<UndoOp>,
}

pub struct UndoManager {
    undo_stack: Vec<UndoGroup>,
    redo_stack: Vec<UndoGroup>,
    current_group: Option<UndoGroup>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: None,
        }
    }

    pub fn begin_group(&mut self) {
        if let Some(group) = self.current_group.take() { // auto-close prev
            if !group.ops.is_empty() {
                self.undo_stack.push(group);
            }
        }
        self.current_group = Some(UndoGroup { ops: Vec::new() });
        self.redo_stack.clear();
    }

    pub fn record(&mut self, op: UndoOp) {
        if let Some(ref mut group) = self.current_group {
            group.ops.push(op);
        } else {
            // auto-create group if none open
            self.current_group = Some(UndoGroup { ops: vec![op] });
            self.redo_stack.clear();
        }
    }

    pub fn end_group(&mut self) {
        if let Some(group) = self.current_group.take() {
            if !group.ops.is_empty() {
                self.undo_stack.push(group);
            }
        }
    }

    pub fn undo(&mut self, buffer: &mut Buffer) -> bool {
        self.end_group(); // close any open group
        if let Some(group) = self.undo_stack.pop() {
            let mut inverse_ops = Vec::new();
            // apply inverse ops in reverse order
            for op in group.ops.iter().rev() {
                let inv = op.inverse();
                inv.apply(buffer);
                inverse_ops.push(op.clone());
            }
            // push original ops (not inversed) to redo stack
            self.redo_stack.push(group);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, buffer: &mut Buffer) -> bool {
        if let Some(group) = self.redo_stack.pop() {
            // re-apply ops in forward order
            for op in &group.ops {
                op.apply(buffer);
            }
            self.undo_stack.push(group);
            true
        } else {
            false
        }
    }

    pub fn has_undo(&self) -> bool {
        !self.undo_stack.is_empty() || self.current_group.as_ref().map_or(false, |g| !g.ops.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn test_undo_insert_char() {
        let mut buffer = Buffer::new();
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::InsertChar { line: 0, col: 0, ch: 'a' });
        buffer.insert_char(0, 0, 'a');
        um.end_group();
        assert_eq!(buffer.get_line(0), Some("a".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
        um.redo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("a".to_string()));
    }

    #[test]
    fn test_undo_delete_char() {
        let mut buffer = Buffer::from_string("hello");
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::DeleteChar { line: 0, col: 1, ch: 'e' });
        buffer.delete_char(0, 1);
        um.end_group();
        assert_eq!(buffer.get_line(0), Some("hllo".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("hello".to_string()));
    }

    #[test]
    fn test_undo_insert_newline() {
        let mut buffer = Buffer::from_string("hello");
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::InsertNewline { line: 0, col: 2 });
        buffer.insert_newline(0, 2);
        um.end_group();
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), Some("he".to_string()));
        assert_eq!(buffer.get_line(1), Some("llo".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.get_line(0), Some("hello".to_string()));
    }

    #[test]
    fn test_undo_delete_range() {
        let mut buffer = Buffer::from_string("hello world");
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::DeleteRange { line: 0, col: 5, text: " world".to_string() });
        buffer.delete_range(0, 5, 0, 11);
        um.end_group();
        assert_eq!(buffer.get_line(0), Some("hello".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("hello world".to_string()));
    }

    #[test]
    fn test_undo_multiple_ops_in_group() {
        let mut buffer = Buffer::new();
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::InsertChar { line: 0, col: 0, ch: 'a' });
        buffer.insert_char(0, 0, 'a');
        um.record(UndoOp::InsertChar { line: 0, col: 1, ch: 'b' });
        buffer.insert_char(0, 1, 'b');
        um.record(UndoOp::InsertChar { line: 0, col: 2, ch: 'c' });
        buffer.insert_char(0, 2, 'c');
        um.end_group();
        assert_eq!(buffer.get_line(0), Some("abc".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
        um.redo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("abc".to_string()));
    }

    #[test]
    fn test_undo_multiline_delete_range() {
        let mut buffer = Buffer::from_string("line1\nline2\nline3");
        let mut um = UndoManager::new();
        um.begin_group();
        let text = "line1\nline2\n".to_string();
        um.record(UndoOp::DeleteRange { line: 0, col: 0, text: text });
        buffer.delete_range(0, 0, 2, 0);
        um.end_group();
        assert_eq!(buffer.get_line(0), Some("line3".to_string()));
        um.undo(&mut buffer);
        assert_eq!(buffer.get_line(0), Some("line1".to_string()));
        assert_eq!(buffer.get_line(1), Some("line2".to_string()));
        assert_eq!(buffer.get_line(2), Some("line3".to_string()));
    }

    #[test]
    fn test_redo_clears_on_new_edit() {
        let mut buffer = Buffer::new();
        let mut um = UndoManager::new();
        um.begin_group();
        um.record(UndoOp::InsertChar { line: 0, col: 0, ch: 'a' });
        buffer.insert_char(0, 0, 'a');
        um.end_group();
        um.undo(&mut buffer);
        // new edit should clear redo
        um.begin_group();
        um.record(UndoOp::InsertChar { line: 0, col: 0, ch: 'b' });
        buffer.insert_char(0, 0, 'b');
        um.end_group();
        assert!(!um.redo(&mut buffer)); // redo stack cleared
    }
}
