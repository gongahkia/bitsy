// Text buffer implementation using ropey

use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug)]
pub struct Buffer {
    rope: Rope,
    file_path: Option<PathBuf>,
    modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            modified: false,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)?;
        let rope = Rope::from_str(&content);

        Ok(Self {
            rope,
            file_path: Some(path.as_ref().to_path_buf()),
            modified: false,
        })
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(ref path) = self.file_path {
            let content = self.rope.to_string();
            fs::write(path, content)?;
            self.modified = false;
            Ok(())
        } else {
            Err(Error::EditorError("No file path set".to_string()))
        }
    }

    pub fn save_as<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.file_path = Some(path.as_ref().to_path_buf());
        self.save()
    }

    pub fn insert_char(&mut self, line: usize, col: usize, ch: char) {
        if line < self.line_count() {
            let line_start = self.rope.line_to_char(line);
            let insert_pos = line_start + col.min(self.line_len(line));
            self.rope.insert_char(insert_pos, ch);
            self.modified = true;
        }
    }

    pub fn insert_newline(&mut self, line: usize, col: usize) {
        if line < self.line_count() {
            let line_start = self.rope.line_to_char(line);
            let insert_pos = line_start + col.min(self.line_len(line));
            self.rope.insert_char(insert_pos, '\n');
            self.modified = true;
        }
    }

    pub fn delete_char(&mut self, line: usize, col: usize) {
        if line < self.line_count() && col < self.line_len(line) {
            let line_start = self.rope.line_to_char(line);
            let delete_pos = line_start + col;
            self.rope.remove(delete_pos..delete_pos + 1);
            self.modified = true;
        }
    }

    pub fn delete_range(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) {
        let start_char = self.rope.line_to_char(start_line) + start_col;
        let end_char = self.rope.line_to_char(end_line) + end_col;
        if start_char < end_char && end_char <= self.rope.len_chars() {
            self.rope.remove(start_char..end_char);
            self.modified = true;
        }
    }

    pub fn get_line(&self, line: usize) -> Option<String> {
        if line < self.line_count() {
            let start = self.rope.line_to_char(line);
            let end = self.rope.line_to_char(line + 1);
            let line_text = self.rope.slice(start..end).to_string();
            Some(line_text.trim_end_matches('\n').to_string())
        } else {
            None
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line_len(&self, line: usize) -> usize {
        if line < self.line_count() {
            let start = self.rope.line_to_char(line);
            let end = self.rope.line_to_char(line + 1);
            let line_text = self.rope.slice(start..end).to_string();
            line_text.trim_end_matches('\n').len()
        } else {
            0
        }
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[No Name]")
            .to_string()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer = Buffer::new();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.is_modified(), false);
        assert_eq!(buffer.file_path(), None);
    }

    #[test]
    fn test_insert_char() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'h');
        buffer.insert_char(0, 1, 'i');
        assert_eq!(buffer.get_line(0), Some("hi".to_string()));
        assert_eq!(buffer.is_modified(), true);
    }

    #[test]
    fn test_insert_newline() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_newline(0, 1);
        buffer.insert_char(1, 0, 'b');
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), Some("a".to_string()));
        assert_eq!(buffer.get_line(1), Some("b".to_string()));
    }

    #[test]
    fn test_delete_char() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'h');
        buffer.insert_char(0, 1, 'e');
        buffer.insert_char(0, 2, 'l');
        buffer.insert_char(0, 3, 'l');
        buffer.insert_char(0, 4, 'o');
        buffer.delete_char(0, 1);
        assert_eq!(buffer.get_line(0), Some("hllo".to_string()));
    }

    #[test]
    fn test_delete_range() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'h');
        buffer.insert_char(0, 1, 'e');
        buffer.insert_char(0, 2, 'l');
        buffer.insert_char(0, 3, 'l');
        buffer.insert_char(0, 4, 'o');
        buffer.delete_range(0, 1, 0, 4);
        assert_eq!(buffer.get_line(0), Some("ho".to_string()));
    }

    #[test]
    fn test_line_len() {
        let mut buffer = Buffer::new();
        assert_eq!(buffer.line_len(0), 0);
        buffer.insert_char(0, 0, 'h');
        buffer.insert_char(0, 1, 'i');
        assert_eq!(buffer.line_len(0), 2);
    }

    #[test]
    fn test_multiline_operations() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'l');
        buffer.insert_char(0, 1, 'i');
        buffer.insert_char(0, 2, 'n');
        buffer.insert_char(0, 3, 'e');
        buffer.insert_char(0, 4, '1');
        buffer.insert_newline(0, 5);
        buffer.insert_char(1, 0, 'l');
        buffer.insert_char(1, 1, 'i');
        buffer.insert_char(1, 2, 'n');
        buffer.insert_char(1, 3, 'e');
        buffer.insert_char(1, 4, '2');

        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), Some("line1".to_string()));
        assert_eq!(buffer.get_line(1), Some("line2".to_string()));
    }
}
