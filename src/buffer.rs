// Text buffer implementation using ropey

use encoding_rs::Encoding;
use ropey::Rope;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
//...
#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
    file_path: Option<PathBuf>,
    modified: bool,
    line_ending: LineEnding,
    marks: HashMap<char, (usize, usize)>,
    encoding: Option<&'static Encoding>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            modified: false,
            line_ending: LineEnding::default(),
            marks: HashMap::new(),
            encoding: None,
        }
    }

    pub fn from_string(content: &str) -> Self {
        Self {
            rope: Rope::from_str(content),
            file_path: None,
            modified: false,
            line_ending: LineEnding::default(),
            marks: HashMap::new(),
            encoding: None,
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        use encoding_rs_io::DecodeReaderBytesBuilder;
        use std::fs::File;
        use std::io::Read;

        let file = File::open(&path)?;
        let mut reader = DecodeReaderBytesBuilder::new().build(file);

        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|e| Error::Io(e))?;

        // Detect line ending from file content
        let line_ending = LineEnding::detect(&content);

        // Normalize to LF for internal representation
        let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
        let rope = Rope::from_str(&normalized);

        Ok(Self {
            rope,
            file_path: Some(path.as_ref().to_path_buf()),
            modified: false,
            line_ending,
            marks: HashMap::new(),
        })
    }

    pub fn get_mark(&self, mark: char) -> Option<(usize, usize)> {
        self.marks.get(&mark).cloned()
    }

    pub fn set_mark(&mut self, mark: char, pos: (usize, usize)) {
        self.marks.insert(mark, pos);
    }

    pub fn get_all_marks(&self) -> Vec<(char, (usize, usize))> {
        let mut marks: Vec<_> = self.marks.iter().map(|(k, v)| (*k, *v)).collect();
        marks.sort_by_key(|(k, _)| *k);
        marks
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(ref path) = self.file_path {
            let content = self.rope.to_string();

            // Convert line endings to the original format
            let content_with_endings = match self.line_ending {
                LineEnding::LF => content,
                LineEnding::CRLF => content.replace('\n', "\r\n"),
                LineEnding::CR => content.replace('\n', "\r"),
            };

            fs::write(path, content_with_endings)?;
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
        
        let end_char = if end_col == usize::MAX {
            if end_line + 1 < self.rope.len_lines() {
                self.rope.line_to_char(end_line + 1)
            } else {
                self.rope.len_chars()
            }
        } else {
            self.rope.line_to_char(end_line) + end_col
        };

        if start_char <= end_char && end_char <= self.rope.len_chars() {
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

    pub fn clear_modified(&mut self) {
        self.modified = false;
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

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        self.line_ending = line_ending;
        self.modified = true;
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

    #[test]
    fn test_line_ending_detection_lf() {
        let content = "line1\nline2\nline3\n";
        assert_eq!(LineEnding::detect(content), LineEnding::LF);
    }

    #[test]
    fn test_line_ending_detection_crlf() {
        let content = "line1\r\nline2\r\nline3\r\n";
        assert_eq!(LineEnding::detect(content), LineEnding::CRLF);
    }

    #[test]
    fn test_line_ending_detection_cr() {
        let content = "line1\rline2\rline3\r";
        assert_eq!(LineEnding::detect(content), LineEnding::CR);
    }

    #[test]
    fn test_line_ending_default() {
        let buffer = Buffer::new();
        // Default should be platform-specific
        #[cfg(windows)]
        assert_eq!(buffer.line_ending(), LineEnding::CRLF);

        #[cfg(not(windows))]
        assert_eq!(buffer.line_ending(), LineEnding::LF);
    }

    #[test]
    fn test_set_line_ending() {
        let mut buffer = Buffer::new();
        buffer.set_line_ending(LineEnding::CRLF);
        assert_eq!(buffer.line_ending(), LineEnding::CRLF);
        assert_eq!(buffer.is_modified(), true);
    }

    #[test]
    fn test_line_ending_as_str() {
        assert_eq!(LineEnding::LF.as_str(), "\n");
        assert_eq!(LineEnding::CRLF.as_str(), "\r\n");
        assert_eq!(LineEnding::CR.as_str(), "\r");
    }

    // Edge case tests

    #[test]
    fn test_empty_buffer() {
        let buffer = Buffer::new();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
        assert_eq!(buffer.line_len(0), 0);
    }

    #[test]
    fn test_insert_at_invalid_position() {
        let mut buffer = Buffer::new();
        // Inserting at line beyond buffer should not panic
        buffer.insert_char(100, 0, 'x');
        // Should not have inserted anything
        assert_eq!(buffer.line_count(), 1);
    }

    #[test]
    fn test_delete_at_invalid_position() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        // Deleting beyond line length should not panic
        buffer.delete_char(0, 100);
        // Original character should still be there
        assert_eq!(buffer.get_line(0), Some("a".to_string()));
    }

    #[test]
    fn test_unicode_characters() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, '你');
        buffer.insert_char(0, 1, '好');
        buffer.insert_char(0, 2, '世');
        buffer.insert_char(0, 3, '界');
        let line = buffer.get_line(0).unwrap();
        assert_eq!(line, "你好世界");
        // Unicode chars count as 1 char each in Rust
        assert_eq!(line.chars().count(), 4);
    }

    #[test]
    fn test_emoji_characters() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, '😀');
        buffer.insert_char(0, 1, '🎉');
        buffer.insert_char(0, 2, '🚀');
        assert_eq!(buffer.get_line(0), Some("😀🎉🚀".to_string()));
    }

    #[test]
    fn test_mixed_unicode_ascii() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'H');
        buffer.insert_char(0, 1, 'e');
        buffer.insert_char(0, 2, 'l');
        buffer.insert_char(0, 3, 'l');
        buffer.insert_char(0, 4, 'o');
        buffer.insert_char(0, 5, '世');
        buffer.insert_char(0, 6, '界');
        assert_eq!(buffer.get_line(0), Some("Hello世界".to_string()));
    }

    #[test]
    fn test_delete_range_entire_line() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'h');
        buffer.insert_char(0, 1, 'e');
        buffer.insert_char(0, 2, 'l');
        buffer.insert_char(0, 3, 'l');
        buffer.insert_char(0, 4, 'o');
        buffer.delete_range(0, 0, 0, 5);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
    }

    #[test]
    fn test_delete_range_multiline() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_newline(0, 1);
        buffer.insert_char(1, 0, 'b');
        buffer.insert_newline(1, 1);
        buffer.insert_char(2, 0, 'c');

        // Delete from middle of line 0 to middle of line 2
        buffer.delete_range(0, 0, 1, 1);
        assert_eq!(buffer.line_count(), 2);
    }

    #[test]
    fn test_insert_newline_at_start() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_newline(0, 0);
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
        assert_eq!(buffer.get_line(1), Some("a".to_string()));
    }

    #[test]
    fn test_insert_newline_at_end() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_newline(0, 1);
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.get_line(0), Some("a".to_string()));
        assert_eq!(buffer.get_line(1), Some("".to_string()));
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let buffer = Buffer::new();
        assert_eq!(buffer.get_line(100), None);
    }

    #[test]
    fn test_line_len_out_of_bounds() {
        let buffer = Buffer::new();
        assert_eq!(buffer.line_len(100), 0);
    }

    #[test]
    fn test_consecutive_newlines() {
        let mut buffer = Buffer::new();
        buffer.insert_newline(0, 0);
        buffer.insert_newline(1, 0);
        buffer.insert_newline(2, 0);
        assert_eq!(buffer.line_count(), 4);
        assert_eq!(buffer.get_line(0), Some("".to_string()));
        assert_eq!(buffer.get_line(1), Some("".to_string()));
        assert_eq!(buffer.get_line(2), Some("".to_string()));
        assert_eq!(buffer.get_line(3), Some("".to_string()));
    }

    #[test]
    fn test_delete_char_at_line_start() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_char(0, 1, 'b');
        buffer.insert_char(0, 2, 'c');
        buffer.delete_char(0, 0);
        assert_eq!(buffer.get_line(0), Some("bc".to_string()));
    }

    #[test]
    fn test_delete_char_at_line_end() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, 'a');
        buffer.insert_char(0, 1, 'b');
        buffer.insert_char(0, 2, 'c');
        buffer.delete_char(0, 2);
        assert_eq!(buffer.get_line(0), Some("ab".to_string()));
    }

    #[test]
    fn test_whitespace_handling() {
        let mut buffer = Buffer::new();
        buffer.insert_char(0, 0, ' ');
        buffer.insert_char(0, 1, '\t');
        buffer.insert_char(0, 2, ' ');
        assert_eq!(buffer.line_len(0), 3);
        assert_eq!(buffer.get_line(0), Some(" \t ".to_string()));
    }

    #[test]
    fn test_very_long_line() {
        let mut buffer = Buffer::new();
        for i in 0..1000 {
            buffer.insert_char(0, i, 'x');
        }
        assert_eq!(buffer.line_len(0), 1000);
        let line = buffer.get_line(0).unwrap();
        assert_eq!(line.len(), 1000);
        assert!(line.chars().all(|c| c == 'x'));
    }

    #[test]
    fn test_many_lines() {
        let mut buffer = Buffer::new();
        // Insert first character
        buffer.insert_char(0, 0, 'a');

        // Create additional lines
        for i in 1..100 {
            buffer.insert_newline(i - 1, 1);
            buffer.insert_char(i, 0, 'a');
        }

        assert_eq!(buffer.line_count(), 100);
        for i in 0..100 {
            assert_eq!(buffer.get_line(i), Some("a".to_string()));
        }
    }
}
