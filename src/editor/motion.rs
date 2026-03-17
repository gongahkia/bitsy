// vim motion implementations: word, line, sentence, paragraph, find, bracket

use crate::mode::Mode;
use super::{Editor, FindDirection};

impl Editor {
    pub(super) fn clamp_cursor(&mut self) {
        let line_count = self.current_buffer().line_count().max(1);
        self.current_window_mut().cursor.line =
            self.current_window().cursor.line.min(line_count - 1);
        let line_len = self
            .current_buffer()
            .line_len(self.current_window().cursor.line);
        if self.mode == Mode::Normal && line_len > 0 {
            self.current_window_mut().cursor.col = self
                .current_window()
                .cursor
                .col
                .min(line_len.saturating_sub(1));
        } else if self.mode == Mode::Insert {
            self.current_window_mut().cursor.col = self.current_window().cursor.col.min(line_len);
        }
        if matches!(
            self.mode,
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        ) {
            let cursor = self.current_window().cursor;
            if let Some(ref mut selection) = self.selection {
                selection.update_cursor(cursor.into());
            }
        }
    }

    pub(super) fn move_word_forward(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let mut chars = line_text
                .chars()
                .skip(self.current_window().cursor.col)
                .peekable();
            let mut col = self.current_window().cursor.col;
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() { break; }
                chars.next();
                col += 1;
            }
            while let Some(&ch) = chars.peek() {
                if !ch.is_whitespace() { break; }
                chars.next();
                col += 1;
            }
            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    pub(super) fn move_word_backward(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            if self.current_window().cursor.col == 0 { return; }
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.current_window().cursor.col.saturating_sub(1);
            while col > 0 && chars[col].is_whitespace() { col -= 1; }
            while col > 0 && !chars[col].is_whitespace() { col -= 1; }
            if chars[col].is_whitespace() && col < chars.len() - 1 { col += 1; }
            self.current_window_mut().cursor.col = col;
        }
    }

    pub(super) fn move_word_end(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col >= chars.len() { return; }
            let mut col = self.current_window().cursor.col;
            if chars[col].is_whitespace() {
                while col < chars.len() && chars[col].is_whitespace() { col += 1; }
            }
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() { col += 1; }
            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    pub(super) fn move_word_forward_big(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let mut chars = line_text
                .chars()
                .skip(self.current_window().cursor.col)
                .peekable();
            let mut col = self.current_window().cursor.col;
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() { break; }
                chars.next();
                col += 1;
            }
            while let Some(&ch) = chars.peek() {
                if !ch.is_whitespace() { break; }
                chars.next();
                col += 1;
            }
            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    pub(super) fn move_word_backward_big(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            if self.current_window().cursor.col == 0 { return; }
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = self.current_window().cursor.col.saturating_sub(1);
            while col > 0 && chars[col].is_whitespace() { col -= 1; }
            while col > 0 && !chars[col].is_whitespace() { col -= 1; }
            if chars[col].is_whitespace() && col < chars.len() - 1 { col += 1; }
            self.current_window_mut().cursor.col = col;
        }
    }

    pub(super) fn move_word_end_big(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col >= chars.len() { return; }
            let mut col = self.current_window().cursor.col;
            if chars[col].is_whitespace() {
                while col < chars.len() && chars[col].is_whitespace() { col += 1; }
            }
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() { col += 1; }
            self.current_window_mut().cursor.col = col;
            self.clamp_cursor();
        }
    }

    pub(super) fn move_to_first_non_blank(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            let mut col = 0;
            while col < chars.len() && chars[col].is_whitespace() { col += 1; }
            self.current_window_mut().cursor.col = col.min(chars.len().saturating_sub(1));
        }
    }

    pub(super) fn move_word_end_back(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col == 0 {
                if self.current_window().cursor.line > 0 {
                    self.current_window_mut().cursor.line -= 1;
                    let prev_line_len = self
                        .current_buffer()
                        .line_len(self.current_window().cursor.line);
                    self.current_window_mut().cursor.col = prev_line_len.saturating_sub(1);
                }
                return;
            }
            let mut col = self.current_window().cursor.col.saturating_sub(1);
            while col > 0 && chars[col].is_whitespace() { col -= 1; }
            while col > 0 && !chars[col.saturating_sub(1)].is_whitespace() { col -= 1; }
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() { col += 1; }
            self.current_window_mut().cursor.col = col;
        }
    }

    pub(super) fn move_word_end_back_big(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() || self.current_window().cursor.col == 0 {
                if self.current_window().cursor.line > 0 {
                    self.current_window_mut().cursor.line -= 1;
                    let prev_line_len = self
                        .current_buffer()
                        .line_len(self.current_window().cursor.line);
                    self.current_window_mut().cursor.col = prev_line_len.saturating_sub(1);
                }
                return;
            }
            let mut col = self.current_window().cursor.col.saturating_sub(1);
            while col > 0 && chars[col].is_whitespace() { col -= 1; }
            while col > 0 && !chars[col.saturating_sub(1)].is_whitespace() { col -= 1; }
            while col < chars.len() - 1 && !chars[col + 1].is_whitespace() { col += 1; }
            self.current_window_mut().cursor.col = col;
        }
    }

    pub(super) fn move_to_line_end_non_blank(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() {
                self.current_window_mut().cursor.col = 0;
                return;
            }
            let mut col = chars.len() - 1;
            while col > 0 && chars[col].is_whitespace() { col -= 1; }
            self.current_window_mut().cursor.col = col;
        }
    }

    pub(super) fn move_sentence_forward(&mut self) {
        let line_count = self.current_buffer().line_count();
        let mut line = self.current_window().cursor.line;
        let mut col = self.current_window().cursor.col + 1;
        while line < line_count {
            if let Some(line_text) = self.current_buffer().get_line(line) {
                let chars: Vec<char> = line_text.chars().collect();
                while col < chars.len() {
                    if matches!(chars[col], '.' | '!' | '?') {
                        if col + 1 >= chars.len() || chars[col + 1].is_whitespace() {
                            col += 1;
                            while col < chars.len() && chars[col].is_whitespace() { col += 1; }
                            if col >= chars.len() {
                                line += 1;
                                col = 0;
                                while line < line_count {
                                    if let Some(next_line) = self.current_buffer().get_line(line) {
                                        if !next_line.trim().is_empty() { break; }
                                    }
                                    line += 1;
                                }
                            }
                            self.current_window_mut().cursor.line =
                                line.min(line_count.saturating_sub(1));
                            self.current_window_mut().cursor.col = col;
                            self.clamp_cursor();
                            return;
                        }
                    }
                    col += 1;
                }
            }
            line += 1;
            col = 0;
        }
        self.current_window_mut().cursor.line = line_count.saturating_sub(1);
        let last_line_len = self
            .current_buffer()
            .line_len(self.current_window().cursor.line);
        self.current_window_mut().cursor.col = last_line_len.saturating_sub(1);
        self.clamp_cursor();
    }

    pub(super) fn move_sentence_backward(&mut self) {
        if self.current_window().cursor.line == 0 && self.current_window().cursor.col == 0 {
            return;
        }
        let mut line = self.current_window().cursor.line;
        let mut col = self.current_window().cursor.col.saturating_sub(1);
        loop {
            if let Some(line_text) = self.current_buffer().get_line(line) {
                let chars: Vec<char> = line_text.chars().collect();
                while col > 0 {
                    if matches!(chars[col], '.' | '!' | '?') {
                        if col + 1 >= chars.len() || chars[col + 1].is_whitespace() {
                            col += 1;
                            while col < chars.len() && chars[col].is_whitespace() { col += 1; }
                            self.current_window_mut().cursor.line = line;
                            self.current_window_mut().cursor.col =
                                col.min(chars.len().saturating_sub(1));
                            self.clamp_cursor();
                            return;
                        }
                    }
                    col = col.saturating_sub(1);
                }
            }
            if line == 0 { break; }
            line -= 1;
            col = self.current_buffer().line_len(line);
        }
        self.current_window_mut().cursor.line = 0;
        self.current_window_mut().cursor.col = 0;
    }

    pub(super) fn find_char(&mut self, ch: char, direction: FindDirection) {
        self.last_find = Some((ch, direction));
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return; }
            match direction {
                FindDirection::Forward => {
                    let start = self.current_window().cursor.col + 1;
                    for i in start..chars.len() {
                        if chars[i] == ch {
                            self.current_window_mut().cursor.col = i;
                            return;
                        }
                    }
                }
                FindDirection::Backward => {
                    if self.current_window().cursor.col == 0 { return; }
                    let start = self.current_window().cursor.col - 1;
                    for i in (0..=start).rev() {
                        if chars[i] == ch {
                            self.current_window_mut().cursor.col = i;
                            return;
                        }
                    }
                }
                FindDirection::Till => {
                    let start = self.current_window().cursor.col + 1;
                    for i in start..chars.len() {
                        if chars[i] == ch {
                            if i > 0 { self.current_window_mut().cursor.col = i - 1; }
                            return;
                        }
                    }
                }
                FindDirection::TillBack => {
                    if self.current_window().cursor.col == 0 { return; }
                    let start = self.current_window().cursor.col - 1;
                    for i in (0..=start).rev() {
                        if chars[i] == ch {
                            if i < chars.len() - 1 { self.current_window_mut().cursor.col = i + 1; }
                            return;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn repeat_last_find(&mut self, reverse: bool) {
        if let Some((ch, direction)) = self.last_find {
            let direction = if reverse {
                match direction {
                    FindDirection::Forward => FindDirection::Backward,
                    FindDirection::Backward => FindDirection::Forward,
                    FindDirection::Till => FindDirection::TillBack,
                    FindDirection::TillBack => FindDirection::Till,
                }
            } else {
                direction
            };
            self.find_char(ch, direction);
        }
    }

    pub(super) fn move_paragraph_forward(&mut self) {
        let mut line = self.current_window().cursor.line + 1;
        let line_count = self.current_buffer().line_count();
        while line < line_count {
            if let Some(text) = self.current_buffer().get_line(line) {
                if text.trim().is_empty() { break; }
            }
            line += 1;
        }
        while line < line_count {
            if let Some(text) = self.current_buffer().get_line(line) {
                if !text.trim().is_empty() { break; }
            }
            line += 1;
        }
        self.current_window_mut().cursor.line = line.min(line_count.saturating_sub(1));
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    pub(super) fn move_paragraph_backward(&mut self) {
        if self.current_window().cursor.line == 0 { return; }
        let mut line = self.current_window().cursor.line.saturating_sub(1);
        loop {
            if let Some(text) = self.current_buffer().get_line(line) {
                if text.trim().is_empty() { break; }
            }
            if line == 0 { break; }
            line -= 1;
        }
        loop {
            if line == 0 { break; }
            if let Some(text) = self.current_buffer().get_line(line.saturating_sub(1)) {
                if !text.trim().is_empty() {
                    line -= 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        self.current_window_mut().cursor.line = line;
        self.current_window_mut().cursor.col = 0;
        self.clamp_cursor();
    }

    pub(super) fn move_to_matching_bracket(&mut self) {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if self.current_window().cursor.col >= chars.len() { return; }
            let current_char = chars[self.current_window().cursor.col];
            let matching_brackets = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];
            for (open, close) in &matching_brackets {
                if current_char == *open {
                    let mut depth = 0;
                    for (i, &ch) in chars
                        .iter()
                        .enumerate()
                        .skip(self.current_window().cursor.col)
                    {
                        if ch == *open { depth += 1; }
                        else if ch == *close {
                            depth -= 1;
                            if depth == 0 {
                                self.current_window_mut().cursor.col = i;
                                return;
                            }
                        }
                    }
                } else if current_char == *close {
                    let mut depth = 0;
                    for i in (0..=self.current_window().cursor.col).rev() {
                        let ch = chars[i];
                        if ch == *close { depth += 1; }
                        else if ch == *open {
                            depth -= 1;
                            if depth == 0 {
                                self.current_window_mut().cursor.col = i;
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}
