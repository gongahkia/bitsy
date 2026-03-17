// operator+motion composition, text objects, case/indent ops

use crossterm::event::{KeyCode, KeyEvent};
use crate::error::Result;
use crate::keymap::Action;
use crate::mode::Mode;
use crate::register::RegisterContent;
use super::{CaseChange, Editor, PendingOperator, TextObjectModifier};

impl Editor {
    pub(super) fn handle_text_object(&mut self, key: KeyEvent) -> Result<()> {
        let modifier = self.pending_text_object.unwrap();
        match key.code {
            KeyCode::Char('w') => { self.apply_text_object_word(modifier)?; }
            KeyCode::Char('W') => { self.apply_text_object_word_big(modifier)?; }
            KeyCode::Char('p') => { self.apply_text_object_paragraph(modifier)?; }
            KeyCode::Char('"') => { self.apply_text_object_quote(modifier, '"')?; }
            KeyCode::Char('\'') => { self.apply_text_object_quote(modifier, '\'')?; }
            KeyCode::Char('`') => { self.apply_text_object_quote(modifier, '`')?; }
            KeyCode::Char('(') | KeyCode::Char(')') => { self.apply_text_object_bracket(modifier, '(', ')')?; }
            KeyCode::Char('[') | KeyCode::Char(']') => { self.apply_text_object_bracket(modifier, '[', ']')?; }
            KeyCode::Char('{') | KeyCode::Char('}') => { self.apply_text_object_bracket(modifier, '{', '}')?; }
            KeyCode::Char('<') | KeyCode::Char('>') => { self.apply_text_object_bracket(modifier, '<', '>')?; }
            KeyCode::Char('s') => { self.apply_text_object_sentence(modifier)?; }
            KeyCode::Esc => {
                self.pending_operator = PendingOperator::None;
                self.pending_text_object = None;
                return Ok(());
            }
            _ => {
                self.pending_operator = PendingOperator::None;
                self.pending_text_object = None;
            }
        }
        self.pending_text_object = None;
        self.pending_operator = PendingOperator::None;
        self.clamp_cursor();
        Ok(())
    }

    pub(super) fn handle_operator_motion(&mut self, action: Action) -> Result<()> {
        let doubled = match (&self.pending_operator, &action) {
            (PendingOperator::Delete, Action::Delete) => Some("delete_line"),
            (PendingOperator::Yank, Action::Yank) => Some("yank_line"),
            (PendingOperator::Change, Action::Change) => Some("change_line"),
            _ => None,
        };
        if let Some(op) = doubled {
            if op != "yank_line" { self.save_undo_state(); }
            let count = if self.count == 0 { 1 } else { self.count };
            match op {
                "delete_line" => {
                    let start_line = self.current_window().cursor.line;
                    let line_count = self.current_buffer().line_count();
                    let end_line = (start_line + count - 1).min(line_count - 1);
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_delete(None, RegisterContent::Line(lines.clone()));
                    for _ in 0..count {
                        let line = self.current_window().cursor.line;
                        let buffer_line_count = self.current_buffer().line_count();
                        if line < buffer_line_count - 1 {
                            self.current_buffer_mut().delete_range(line, 0, line + 1, 0);
                        } else if line > 0 {
                            let prev_line_len = self.current_buffer().line_len(line - 1);
                            let current_line_len = self.current_buffer().line_len(line);
                            self.current_buffer_mut().delete_range(line - 1, prev_line_len, line, current_line_len);
                            self.current_window_mut().cursor.line -= 1;
                            break;
                        } else { break; }
                    }
                    self.message = Some(format!(
                        "{} line{} deleted",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    ));
                }
                "yank_line" => {
                    let start_line = self.current_window().cursor.line;
                    let end_line = (start_line + count - 1).min(self.current_buffer().line_count() - 1);
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_yank(None, RegisterContent::Line(lines.clone()));
                    self.message = Some(format!(
                        "{} line{} yanked",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    ));
                }
                "change_line" => {
                    let start_line = self.current_window().cursor.line;
                    let end_line = (start_line + count - 1).min(self.current_buffer().line_count() - 1);
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_delete(None, RegisterContent::Line(lines.clone()));
                    let line = self.current_window().cursor.line;
                    let line_len = self.current_buffer().line_len(line);
                    if line_len > 0 {
                        self.current_buffer_mut().delete_range(line, 0, line, line_len);
                    }
                    for _ in 1..count {
                        let line = self.current_window().cursor.line;
                        if line < self.current_buffer().line_count() - 1 {
                            self.current_buffer_mut().delete_range(line, 0, line + 1, 0);
                        } else { break; }
                    }
                    self.current_window_mut().cursor.col = 0;
                    self.mode = Mode::Insert;
                    self.message = Some(format!(
                        "{} line{} deleted",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    ));
                }
                _ => {}
            }
            if op == "delete_line" || op == "change_line" {
                self.record_change(action.clone());
            }
            self.pending_operator = PendingOperator::None;
            self.clamp_cursor();
            return Ok(());
        }

        if action == Action::EnterNormalMode {
            self.pending_operator = PendingOperator::None;
            return Ok(());
        }

        let start_line = self.current_window().cursor.line;
        let start_col = self.current_window().cursor.col;

        match action {
            Action::MoveUp | Action::MoveDown | Action::MoveLeft | Action::MoveRight
            | Action::MoveWordForward | Action::MoveWordBackward | Action::MoveWordEnd
            | Action::MoveWordEndBack | Action::MoveWordEndBackBig
            | Action::MoveWordForwardBig | Action::MoveWordBackwardBig | Action::MoveWordEndBig
            | Action::MoveLineStart | Action::MoveLineFirstNonBlank | Action::MoveLineEnd
            | Action::MoveLineEndNonBlank | Action::MoveLineStartDisplay | Action::MoveLineEndDisplay
            | Action::MoveFileStart | Action::MoveFileEnd
            | Action::MoveParagraphForward | Action::MoveParagraphBackward
            | Action::MoveSentenceForward | Action::MoveSentenceBackward
            | Action::FindChar(_) | Action::FindCharBack(_) | Action::TillChar(_) | Action::TillCharBack(_)
            | Action::RepeatLastFind | Action::RepeatLastFindReverse
            | Action::MoveToScreenTop | Action::MoveToScreenMiddle | Action::MoveToScreenBottom
            | Action::MoveMatchingBracket | Action::MoveToPercent
            | Action::MovePageUp | Action::MovePageDown | Action::MoveHalfPageUp | Action::MoveHalfPageDown => {
                let old_cursor = self.current_window().cursor;
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    self.execute_action(action.clone())?;
                }
                let end_line = self.current_window().cursor.line;
                let end_col = self.current_window().cursor.col;
                self.apply_operator_to_range(start_line, start_col, end_line, end_col)?;
                if self.pending_operator != PendingOperator::Yank
                    && self.pending_operator != PendingOperator::None
                {
                    self.record_change(action.clone());
                }
                if self.pending_operator == PendingOperator::Delete
                    || self.pending_operator == PendingOperator::Change
                {
                    self.current_window_mut().cursor = old_cursor;
                }
                self.pending_operator = PendingOperator::None;
            }
            _ => { self.pending_operator = PendingOperator::None; }
        }
        self.clamp_cursor();
        Ok(())
    }

    pub(super) fn apply_operator_to_range(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Result<()> {
        self.save_undo_state();
        let (start_line, start_col, end_line, end_col) =
            if start_line > end_line || (start_line == end_line && start_col > end_col) {
                (end_line, end_col, start_line, start_col)
            } else {
                (start_line, start_col, end_line, end_col)
            };
        match self.pending_operator {
            PendingOperator::Delete => {
                if start_col == 0 && end_col == usize::MAX {
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_delete(None, RegisterContent::Line(lines.clone()));
                    self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
                    self.message = Some(format!(
                        "{} line{} deleted",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    ));
                } else {
                    let deleted_text = self.get_range_text(start_line, start_col, end_line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted_text));
                    self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
                }
            }
            PendingOperator::Yank => {
                if start_col == 0 && end_col == usize::MAX {
                    let mut lines = Vec::new();
                    for line_idx in start_line..=end_line {
                        if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                            lines.push(line_text);
                        }
                    }
                    self.registers.set_yank(None, RegisterContent::Line(lines.clone()));
                    self.message = Some(format!(
                        "{} line{} yanked",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    ));
                } else {
                    let yanked_text = self.get_range_text(start_line, start_col, end_line, end_col);
                    let char_count = yanked_text.len();
                    self.registers.set_yank(None, RegisterContent::Char(yanked_text));
                    self.message = Some(format!("Yanked {} characters", char_count));
                }
                self.current_buffer_mut().set_mark('[', (start_line, start_col));
                self.current_buffer_mut().set_mark(']', (end_line, end_col));
            }
            PendingOperator::Change => {
                let deleted_text = self.get_range_text(start_line, start_col, end_line, end_col);
                self.registers.set_delete(None, RegisterContent::Char(deleted_text));
                self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
                self.mode = Mode::Insert;
            }
            PendingOperator::MakeLowercase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Lower);
            }
            PendingOperator::MakeUppercase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Upper);
            }
            PendingOperator::ToggleCase => {
                self.apply_case_change(start_line, start_col, end_line, end_col, CaseChange::Toggle);
            }
            PendingOperator::Indent => { self.apply_indent(start_line, end_line, true); }
            PendingOperator::Dedent => { self.apply_indent(start_line, end_line, false); }
            PendingOperator::AutoIndent => { self.apply_auto_indent(start_line, end_line); }
            PendingOperator::None => {}
        }
        Ok(())
    }

    pub(super) fn get_range_text(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        if start_line == end_line {
            if let Some(line_text) = self.current_buffer().get_line(start_line) {
                let end = if end_col == usize::MAX { line_text.len() } else { end_col.min(line_text.len()) };
                let start = start_col.min(line_text.len());
                return line_text[start..end].to_string();
            }
        } else {
            let mut result = String::new();
            for line_idx in start_line..=end_line {
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    if line_idx == start_line {
                        result.push_str(&line_text[start_col.min(line_text.len())..]);
                        result.push('\n');
                    } else if line_idx == end_line {
                        let end = if end_col == usize::MAX { line_text.len() } else { end_col.min(line_text.len()) };
                        result.push_str(&line_text[..end]);
                    } else {
                        result.push_str(&line_text);
                        result.push('\n');
                    }
                }
            }
            return result;
        }
        String::new()
    }

    pub(super) fn apply_case_change(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        case_change: CaseChange,
    ) {
        for line_idx in start_line..=end_line {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                let chars: Vec<char> = line_text.chars().collect();
                if chars.is_empty() { continue; }
                let (from, to) = if line_idx == start_line && line_idx == end_line {
                    (start_col.min(chars.len()), end_col.min(chars.len()))
                } else if line_idx == start_line {
                    (start_col.min(chars.len()), chars.len())
                } else if line_idx == end_line {
                    (0, end_col.min(chars.len()))
                } else {
                    (0, chars.len())
                };
                for col in from..to {
                    if col < chars.len() {
                        let old_char = chars[col];
                        let new_char = match case_change {
                            CaseChange::Lower => old_char.to_lowercase().collect::<Vec<char>>(),
                            CaseChange::Upper => old_char.to_uppercase().collect::<Vec<char>>(),
                            CaseChange::Toggle => {
                                if old_char.is_lowercase() {
                                    old_char.to_uppercase().collect::<Vec<char>>()
                                } else {
                                    old_char.to_lowercase().collect::<Vec<char>>()
                                }
                            }
                        };
                        if !new_char.is_empty() && new_char[0] != old_char {
                            self.current_buffer_mut().delete_char(line_idx, col);
                            for (i, ch) in new_char.iter().enumerate() {
                                self.current_buffer_mut().insert_char(line_idx, col + i, *ch);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn apply_indent(&mut self, start_line: usize, end_line: usize, indent_right: bool) {
        const SHIFTWIDTH: usize = 4;
        for line_idx in start_line..=end_line {
            if line_idx >= self.current_buffer().line_count() { break; }
            if indent_right {
                for i in 0..SHIFTWIDTH {
                    self.current_buffer_mut().insert_char(line_idx, i, ' ');
                }
            } else {
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    let mut chars_to_remove = 0;
                    let chars: Vec<char> = line_text.chars().collect();
                    for &ch in chars.iter().take(SHIFTWIDTH) {
                        if ch == ' ' { chars_to_remove += 1; }
                        else if ch == '\t' { chars_to_remove += 1; break; }
                        else { break; }
                    }
                    for _ in 0..chars_to_remove {
                        self.current_buffer_mut().delete_char(line_idx, 0);
                    }
                }
            }
        }
    }

    pub(super) fn apply_auto_indent(&mut self, start_line: usize, end_line: usize) {
        for line_idx in start_line..=end_line {
            if line_idx >= self.current_buffer().line_count() { break; }
            let mut indent_level = 0;
            if line_idx > 0 {
                for prev_line in (0..line_idx).rev() {
                    if let Some(prev_text) = self.current_buffer().get_line(prev_line) {
                        let trimmed = prev_text.trim_start();
                        if !trimmed.is_empty() {
                            indent_level = prev_text.len() - trimmed.len();
                            break;
                        }
                    }
                }
            }
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                let trimmed = line_text.trim_start();
                let current_indent = line_text.len() - trimmed.len();
                for _ in 0..current_indent {
                    self.current_buffer_mut().delete_char(line_idx, 0);
                }
                for i in 0..indent_level {
                    self.current_buffer_mut().insert_char(line_idx, i, ' ');
                }
            }
        }
    }

    // text object implementations
    pub(super) fn apply_text_object_word(&mut self, modifier: TextObjectModifier) -> Result<()> {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let mut start = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut end = start;
            if start < chars.len() && chars[start].is_whitespace() {
                if matches!(modifier, TextObjectModifier::Around) {
                    while end < chars.len() && chars[end].is_whitespace() { end += 1; }
                    if end < chars.len() { start = end; }
                }
            }
            while start > 0 && !chars[start - 1].is_whitespace() { start -= 1; }
            while end < chars.len() && !chars[end].is_whitespace() { end += 1; }
            if matches!(modifier, TextObjectModifier::Around) {
                while end < chars.len() && chars[end].is_whitespace() { end += 1; }
                if end == chars.len() || !chars[end - 1].is_whitespace() {
                    while start > 0 && chars[start - 1].is_whitespace() { start -= 1; }
                }
            }
            let line = self.current_window().cursor.line;
            self.apply_operator_to_range(line, start, line, end)?;
        }
        Ok(())
    }

    pub(super) fn apply_text_object_word_big(&mut self, modifier: TextObjectModifier) -> Result<()> {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let mut start = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut end = start;
            while start > 0 && !chars[start - 1].is_whitespace() { start -= 1; }
            while end < chars.len() && !chars[end].is_whitespace() { end += 1; }
            if matches!(modifier, TextObjectModifier::Around) {
                while end < chars.len() && chars[end].is_whitespace() { end += 1; }
            }
            let line = self.current_window().cursor.line;
            self.apply_operator_to_range(line, start, line, end)?;
        }
        Ok(())
    }

    pub(super) fn apply_text_object_paragraph(&mut self, modifier: TextObjectModifier) -> Result<()> {
        let line_count = self.current_buffer().line_count();
        let mut start_line = self.current_window().cursor.line;
        let mut end_line = self.current_window().cursor.line;
        while start_line > 0 {
            if let Some(text) = self.current_buffer().get_line(start_line - 1) {
                if text.trim().is_empty() { break; }
            }
            start_line -= 1;
        }
        while end_line < line_count - 1 {
            if let Some(text) = self.current_buffer().get_line(end_line + 1) {
                if text.trim().is_empty() { end_line += 1; break; }
            }
            end_line += 1;
        }
        if matches!(modifier, TextObjectModifier::Around) {
            while end_line < line_count - 1 {
                if let Some(text) = self.current_buffer().get_line(end_line + 1) {
                    if !text.trim().is_empty() { break; }
                    end_line += 1;
                } else { break; }
            }
        }
        self.apply_operator_to_range(start_line, 0, end_line, self.current_buffer().line_len(end_line))?;
        Ok(())
    }

    pub(super) fn apply_text_object_quote(&mut self, modifier: TextObjectModifier, quote: char) -> Result<()> {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let cursor_pos = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut start = None;
            let mut end = None;
            for i in (0..=cursor_pos).rev() {
                if chars[i] == quote { start = Some(i); break; }
            }
            if let Some(start_pos) = start {
                for i in (start_pos + 1)..chars.len() {
                    if chars[i] == quote { end = Some(i); break; }
                }
            }
            if let (Some(s), Some(e)) = (start, end) {
                let (range_start, range_end) = match modifier {
                    TextObjectModifier::Inner => (s + 1, e),
                    TextObjectModifier::Around => (s, e + 1),
                };
                let line = self.current_window().cursor.line;
                self.apply_operator_to_range(line, range_start, line, range_end)?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_text_object_bracket(
        &mut self,
        modifier: TextObjectModifier,
        open: char,
        close: char,
    ) -> Result<()> {
        if let Some(line_text) = self
            .current_buffer()
            .get_line(self.current_window().cursor.line)
        {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let cursor_pos = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut start = None;
            let mut end = None;
            let mut depth = 0;
            for i in (0..=cursor_pos).rev() {
                if chars[i] == close { depth += 1; }
                else if chars[i] == open {
                    if depth == 0 { start = Some(i); break; }
                    depth -= 1;
                }
            }
            if let Some(start_pos) = start {
                depth = 0;
                for i in start_pos..chars.len() {
                    if chars[i] == open { depth += 1; }
                    else if chars[i] == close {
                        depth -= 1;
                        if depth == 0 { end = Some(i); break; }
                    }
                }
            }
            if let (Some(s), Some(e)) = (start, end) {
                let (range_start, range_end) = match modifier {
                    TextObjectModifier::Inner => (s + 1, e),
                    TextObjectModifier::Around => (s, e + 1),
                };
                let line = self.current_window().cursor.line;
                self.apply_operator_to_range(line, range_start, line, range_end)?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_text_object_sentence(&mut self, modifier: TextObjectModifier) -> Result<()> {
        let line = self.current_window().cursor.line;
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let chars: Vec<char> = line_text.chars().collect();
            if chars.is_empty() { return Ok(()); }
            let col = self.current_window().cursor.col.min(chars.len().saturating_sub(1));
            let mut start = col;
            loop { // find sentence start: after prev sentence-ending punctuation + space, or line start
                if start == 0 { break; }
                if matches!(chars[start - 1], '.' | '!' | '?') {
                    break;
                }
                start -= 1;
            }
            while start < chars.len() && chars[start].is_whitespace() { start += 1; } // skip leading ws
            let mut end = col;
            while end < chars.len() {
                if matches!(chars[end], '.' | '!' | '?') {
                    end += 1; // include the punctuation
                    break;
                }
                end += 1;
            }
            if matches!(modifier, TextObjectModifier::Around) {
                while end < chars.len() && chars[end].is_whitespace() { end += 1; } // include trailing ws
            }
            if matches!(modifier, TextObjectModifier::Inner) {
                while start < chars.len() && chars[start].is_whitespace() { start += 1; }
            }
            self.apply_operator_to_range(line, start, line, end)?;
        }
        Ok(())
    }
}
