// surround operations: cs, ds, ys

use crate::error::Result;
use super::Editor;

impl Editor {
    pub(super) fn surround_change(&mut self, old_char: char, new_char: char) -> Result<()> {
        let (open_old, close_old) = surround_pair(old_char);
        let (open_new, close_new) = surround_pair(new_char);
        let line = self.current_window().cursor.line;
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let chars: Vec<char> = line_text.chars().collect();
            let col = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let (start, end) = match find_surround_pair(&chars, col, open_old, close_old) {
                Some(pair) => pair,
                None => return Ok(()),
            };
            self.save_undo_state();
            self.current_buffer_mut().delete_char(line, end);
            self.current_buffer_mut().insert_char(line, end, close_new);
            self.current_buffer_mut().delete_char(line, start);
            self.current_buffer_mut().insert_char(line, start, open_new);
        }
        Ok(())
    }

    pub(super) fn surround_delete(&mut self, ch: char) -> Result<()> {
        let (open, close) = surround_pair(ch);
        let line = self.current_window().cursor.line;
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let chars: Vec<char> = line_text.chars().collect();
            let col = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let (start, end) = match find_surround_pair(&chars, col, open, close) {
                Some(pair) => pair,
                None => return Ok(()),
            };
            self.save_undo_state();
            self.current_buffer_mut().delete_char(line, end); // delete close first (higher index)
            self.current_buffer_mut().delete_char(line, start);
        }
        Ok(())
    }

    pub(super) fn surround_add_word(&mut self, ch: char) -> Result<()> {
        let (open, close) = surround_pair(ch);
        let line = self.current_window().cursor.line;
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let col = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut start = col;
            let mut end = col;
            while start > 0 && chars[start - 1].is_alphanumeric() || (start > 0 && chars[start - 1] == '_') { start -= 1; }
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') { end += 1; }
            self.save_undo_state();
            self.current_buffer_mut().insert_char(line, end, close);
            self.current_buffer_mut().insert_char(line, start, open);
        }
        Ok(())
    }
}

fn surround_pair(ch: char) -> (char, char) {
    match ch {
        '(' | ')' | 'b' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' | 'B' => ('{', '}'),
        '<' | '>' => ('<', '>'),
        _ => (ch, ch), // quotes and other chars are symmetric
    }
}

fn find_surround_pair(chars: &[char], cursor: usize, open: char, close: char) -> Option<(usize, usize)> {
    if open == close { // symmetric (quotes)
        let mut first = None;
        let mut second = None;
        for i in 0..chars.len() {
            if chars[i] == open {
                if first.is_none() {
                    first = Some(i);
                } else if i >= cursor { // found pair containing cursor
                    second = Some(i);
                    if let Some(s) = first {
                        if s <= cursor { return Some((s, i)); }
                    }
                    first = second;
                    second = None;
                } else {
                    first = Some(i); // move start forward
                    second = None;
                }
            }
        }
        None
    } else { // asymmetric (brackets)
        let mut start = None;
        let mut depth = 0;
        for i in (0..=cursor).rev() {
            if chars[i] == close && i != cursor { depth += 1; }
            else if chars[i] == open {
                if depth == 0 { start = Some(i); break; }
                depth -= 1;
            }
        }
        let start = start?;
        depth = 0;
        for i in start..chars.len() {
            if chars[i] == open { depth += 1; }
            else if chars[i] == close {
                depth -= 1;
                if depth == 0 { return Some((start, i)); }
            }
        }
        None
    }
}
