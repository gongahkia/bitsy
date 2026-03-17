// execute_action: massive dispatch for all vim actions

use crate::error::Result;
use crate::keymap::Action;
use crate::mode::Mode;
use crate::register::RegisterContent;
use crate::selection::Selection;
use super::{Editor, PendingOperator};

impl Editor {
    pub(super) fn execute_action(&mut self, action: Action) -> Result<()> {
        match action {
            // movement
            Action::MoveUp => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_up(count);
                self.clamp_cursor();
            }
            Action::MoveDown => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_down(count);
                self.clamp_cursor();
            }
            Action::MoveLeft => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_left(count);
                self.clamp_cursor();
            }
            Action::MoveRight => {
                let count = if self.count == 0 { 1 } else { self.count };
                self.current_window_mut().cursor.move_right(count);
                self.clamp_cursor();
            }
            Action::MoveWordForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_word_forward(); }
            }
            Action::MoveWordBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_word_backward(); }
            }
            Action::MoveLineStart => { self.current_window_mut().cursor.move_to_line_start(); }
            Action::MoveLineEnd => {
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.move_to_line_end(line_len);
            }
            Action::MoveFileStart => {
                self.save_jump_position();
                let target_line = if self.count > 0 {
                    (self.count - 1).min(self.current_buffer().line_count().saturating_sub(1))
                } else { 0 };
                self.current_window_mut().cursor.line = target_line;
                self.current_window_mut().cursor.col = 0;
            }
            Action::MoveFileEnd => {
                self.save_jump_position();
                let last_line = self.current_buffer().line_count().saturating_sub(1);
                self.current_window_mut().cursor.line = last_line;
                self.current_window_mut().cursor.col = 0;
            }
            Action::MoveWordEnd => { self.move_word_end(); }
            Action::MoveWordForwardBig => { self.move_word_forward_big(); }
            Action::MoveWordBackwardBig => { self.move_word_backward_big(); }
            Action::MoveWordEndBig => { self.move_word_end_big(); }
            Action::MoveWordEndBack => { self.move_word_end_back(); }
            Action::MoveWordEndBackBig => { self.move_word_end_back_big(); }
            Action::MoveLineFirstNonBlank => { self.move_to_first_non_blank(); }
            Action::MoveLineEndNonBlank => { self.move_to_line_end_non_blank(); }
            Action::MoveLineStartDisplay => { self.current_window_mut().cursor.move_to_line_start(); }
            Action::MoveLineEndDisplay => {
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.move_to_line_end(line_len);
            }
            Action::MoveSentenceForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_sentence_forward(); }
            }
            Action::MoveSentenceBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_sentence_backward(); }
            }
            Action::FindChar(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.find_char(ch, super::FindDirection::Forward); }
            }
            Action::FindCharBack(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.find_char(ch, super::FindDirection::Backward); }
            }
            Action::TillChar(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.find_char(ch, super::FindDirection::Till); }
            }
            Action::TillCharBack(ch) => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.find_char(ch, super::FindDirection::TillBack); }
            }
            Action::RepeatLastFind => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.repeat_last_find(false); }
            }
            Action::RepeatLastFindReverse => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.repeat_last_find(true); }
            }
            Action::MoveToScreenTop => {
                let offset_line = self.current_window().viewport.offset_line;
                self.current_window_mut().cursor.line = offset_line;
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::MoveToScreenMiddle => {
                let offset_line = self.current_window().viewport.offset_line;
                let height = self.current_window().viewport.height;
                let middle_line = offset_line + (height / 2);
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = middle_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::MoveToScreenBottom => {
                let offset_line = self.current_window().viewport.offset_line;
                let height = self.current_window().viewport.height;
                let bottom_line = offset_line + height - 1;
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = bottom_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Action::ScrollTopToScreen => {
                let line = self.current_window().cursor.line;
                self.current_window_mut().viewport.offset_line = line;
            }
            Action::ScrollMiddleToScreen => {
                let line = self.current_window().cursor.line;
                let height = self.current_window().viewport.height;
                self.current_window_mut().viewport.offset_line = line.saturating_sub(height / 2);
            }
            Action::ScrollBottomToScreen => {
                let line = self.current_window().cursor.line;
                let height = self.current_window().viewport.height;
                self.current_window_mut().viewport.offset_line = line.saturating_sub(height.saturating_sub(1));
            }
            Action::MoveParagraphForward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_paragraph_forward(); }
            }
            Action::MoveParagraphBackward => {
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count { self.move_paragraph_backward(); }
            }
            Action::MoveMatchingBracket => {
                self.save_jump_position();
                self.move_to_matching_bracket();
            }
            Action::MovePageUp => {
                let page_size = self.current_window().viewport.height;
                self.current_window_mut().cursor.move_up(page_size);
                self.clamp_cursor();
            }
            Action::MovePageDown => {
                let page_size = self.current_window().viewport.height;
                self.current_window_mut().cursor.move_down(page_size);
                self.clamp_cursor();
            }
            Action::MoveHalfPageUp => {
                let half_page = self.current_window().viewport.height / 2;
                self.current_window_mut().cursor.move_up(half_page);
                self.clamp_cursor();
            }
            Action::MoveHalfPageDown => {
                let half_page = self.current_window().viewport.height / 2;
                self.current_window_mut().cursor.move_down(half_page);
                self.clamp_cursor();
            }
            Action::MoveToPercent => {
                self.save_jump_position();
                if self.count > 0 {
                    let percent = self.count.min(100);
                    let total_lines = self.current_buffer().line_count();
                    let target_line = (total_lines * percent) / 100;
                    self.current_window_mut().cursor.line = target_line.saturating_sub(1).min(total_lines.saturating_sub(1));
                    self.current_window_mut().cursor.col = 0;
                } else { self.move_to_matching_bracket(); }
                self.clamp_cursor();
            }

            // mode switching
            Action::EnterInsertMode => {
                self.showing_landing_page = false;
                self.mode = Mode::Insert;
                self.emit_event(crate::event::EditorEvent::ModeChanged { mode: "Insert".to_string() });
            }
            Action::EnterInsertModeBeginning => {
                self.showing_landing_page = false;
                self.current_window_mut().cursor.move_to_line_start();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppend => {
                self.showing_landing_page = false;
                self.current_window_mut().cursor.move_right(1);
                self.clamp_cursor();
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeAppendEnd => {
                self.showing_landing_page = false;
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.current_window_mut().cursor.col = line_len;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineBelow => {
                self.showing_landing_page = false;
                self.save_undo_state();
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                self.rec_insert_newline(line, line_len);
                self.current_window_mut().cursor.line += 1;
                self.current_window_mut().cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterInsertModeNewLineAbove => {
                self.showing_landing_page = false;
                self.save_undo_state();
                let line = self.current_window().cursor.line;
                self.rec_insert_newline(line, 0);
                self.current_window_mut().cursor.col = 0;
                self.mode = Mode::Insert;
            }
            Action::EnterReplaceMode => {
                self.showing_landing_page = false;
                self.mode = Mode::Replace;
            }
            Action::EnterVisualMode => {
                self.mode = Mode::Visual;
                let cursor = self.current_window().cursor;
                self.selection = Some(Selection::from_cursor(cursor, Mode::Visual));
            }
            Action::EnterVisualLineMode => {
                self.mode = Mode::VisualLine;
                let cursor = self.current_window().cursor;
                self.selection = Some(Selection::from_cursor(cursor, Mode::VisualLine));
            }
            Action::EnterCommandMode => {
                if matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
                    if let Some(ref selection) = self.selection {
                        let (start, end) = selection.range();
                        self.visual_cmd_range = Some((start.line, end.line));
                        self.substitute_preview_range = Some((start.line, end.line));
                    }
                } else { self.visual_cmd_range = None; }
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
            Action::EnterNormalMode => {
                if self.mode == Mode::Visual || self.mode == Mode::VisualLine {
                    if let Some(selection) = &self.selection {
                        let (start_pos, end_pos) = selection.range();
                        self.current_buffer_mut().set_mark('<', (start_pos.line, start_pos.col));
                        self.current_buffer_mut().set_mark('>', (end_pos.line, end_pos.col));
                    }
                }
                let was_insert = self.mode == Mode::Insert;
                self.mode = Mode::Normal;
                self.selection = None;
                let line = self.current_window().cursor.line;
                let line_len = self.current_buffer().line_len(line);
                if line_len > 0 && self.current_window().cursor.col >= line_len {
                    self.current_window_mut().cursor.col = line_len.saturating_sub(1);
                }
                if was_insert { self.emit_event(crate::event::EditorEvent::InsertLeave); }
                self.emit_event(crate::event::EditorEvent::ModeChanged { mode: "Normal".to_string() });
            }

            // editing
            Action::InsertChar(c) => {
                if self.mode == Mode::Insert {
                    self.save_undo_state();
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.rec_insert_char(line, col, c);
                    self.current_window_mut().cursor.move_right(1);
                    self.emit_event(crate::event::EditorEvent::InsertChar { ch: c });
                }
            }
            Action::InsertNewline => {
                if self.mode == Mode::Insert || self.mode == Mode::Replace {
                    self.save_undo_state();
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.rec_insert_newline(line, col);
                    self.current_window_mut().cursor.line += 1;
                    self.current_window_mut().cursor.col = 0;
                }
            }
            Action::DeleteChar => {
                if self.mode == Mode::Insert || self.mode == Mode::Replace {
                    self.save_undo_state();
                    if self.current_window().cursor.col > 0 {
                        self.current_window_mut().cursor.move_left(1);
                        let line = self.current_window().cursor.line;
                        let col = self.current_window().cursor.col;
                        self.rec_delete_char(line, col);
                    } else if self.current_window().cursor.line > 0 {
                        let prev_line_len = self.current_buffer().line_len(self.current_window().cursor.line - 1);
                        let current_line = self.current_window().cursor.line;
                        self.rec_delete_range(current_line - 1, prev_line_len, current_line, 0);
                        self.current_window_mut().cursor.line -= 1;
                        self.current_window_mut().cursor.col = prev_line_len;
                    }
                } else if self.mode == Mode::Normal {
                    self.save_undo_state();
                    self.record_change(action.clone());
                    let line = self.current_window().cursor.line;
                    let col = self.current_window().cursor.col;
                    self.rec_delete_char(line, col);
                    self.clamp_cursor();
                }
            }
            Action::Replace(ch) => {
                self.save_undo_state();
                let line = self.current_window().cursor.line;
                let col = self.current_window().cursor.col;
                if self.mode == Mode::Replace {
                    if col < self.current_buffer().line_len(line) {
                        self.rec_delete_char(line, col);
                        self.rec_insert_char(line, col, ch);
                    } else {
                        self.rec_insert_char(line, col, ch);
                    }
                    self.current_window_mut().cursor.move_right(1);
                } else {
                    self.record_change(action.clone());
                    if col < self.current_buffer().line_len(line) {
                        self.rec_delete_char(line, col);
                        self.rec_insert_char(line, col, ch);
                    }
                }
            }

            Action::Undo => { self.undo(); }
            Action::Redo => { self.redo(); }

            // operators
            Action::Delete => {
                if matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
                    if let Some(selection) = self.selection.clone() {
                        let (start, end) = selection.range();
                        self.pending_operator = PendingOperator::Delete;
                        self.apply_operator_to_range(start.line, start.col, end.line, end.col)?;
                        self.pending_operator = PendingOperator::None;
                        self.mode = Mode::Normal;
                        self.selection = None;
                    }
                } else { self.pending_operator = PendingOperator::Delete; }
            }
            Action::DeleteToEnd => {
                self.save_undo_state();
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let end_col = self.current_buffer().line_len(line);
                if start_col < end_col {
                    let deleted = self.current_buffer().get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.current_buffer_mut().delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
            }
            Action::Change => { self.pending_operator = PendingOperator::Change; }
            Action::ChangeToEnd => {
                self.save_undo_state();
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let end_col = self.current_buffer().line_len(line);
                if start_col < end_col {
                    let deleted = self.current_buffer().get_line(line)
                        .map(|text| text[start_col..end_col].to_string())
                        .unwrap_or_default();
                    self.current_buffer_mut().delete_range(line, start_col, line, end_col);
                    self.registers.set_delete(None, RegisterContent::Char(deleted));
                }
                self.mode = Mode::Insert;
            }
            Action::Yank => {
                if matches!(self.mode, Mode::VisualLine) {
                    if let Some(selection) = self.selection.clone() {
                        let (start, end) = selection.range();
                        let mut lines = Vec::new();
                        for line_idx in start.line..=end.line {
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
                        self.mode = Mode::Normal;
                        self.selection = None;
                    }
                } else { self.pending_operator = PendingOperator::Yank; }
            }
            Action::YankLine => {
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    self.registers.set_yank(None, RegisterContent::Line(vec![line_text]));
                    self.message = Some("1 line yanked".to_string());
                }
            }
            Action::YankToEnd => {
                let line = self.current_window().cursor.line;
                let start_col = self.current_window().cursor.col;
                let line_len = self.current_buffer().line_len(line);
                if let Some(line_text) = self.current_buffer().get_line(line) {
                    if start_col < line_len {
                        let yanked = line_text[start_col..].to_string();
                        self.registers.set_yank(None, RegisterContent::Char(yanked));
                    }
                }
            }
            Action::Paste => {
                self.save_undo_state();
                let start_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    if let Some(content) = self.registers.get(None) {
                        match content {
                            RegisterContent::Char(text) => {
                                for ch in text.chars() {
                                    if ch == '\n' {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_newline(line, col);
                                        self.current_window_mut().cursor.line += 1;
                                        self.current_window_mut().cursor.col = 0;
                                    } else {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_char(line, col, ch);
                                        self.current_window_mut().cursor.col += 1;
                                    }
                                }
                            }
                            RegisterContent::Line(lines) => {
                                let insert_line = self.current_window().cursor.line + 1;
                                for (i, line) in lines.iter().enumerate() {
                                    let target_line = insert_line + i;
                                    if target_line > 0 {
                                        let prev_line = target_line - 1;
                                        let prev_line_len = self.current_buffer().line_len(prev_line);
                                        self.current_buffer_mut().insert_newline(prev_line, prev_line_len);
                                    }
                                    for (idx, ch) in line.chars().enumerate() {
                                        self.current_buffer_mut().insert_char(target_line, idx, ch);
                                    }
                                }
                                self.current_window_mut().cursor.line = insert_line;
                                self.current_window_mut().cursor.col = 0;
                            }
                            _ => {}
                        }
                    }
                }
                let end_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                self.current_buffer_mut().set_mark('[', start_pos);
                self.current_buffer_mut().set_mark(']', end_pos);
            }
            Action::PasteBefore => {
                self.save_undo_state();
                let start_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    if let Some(content) = self.registers.get(None) {
                        match content {
                            RegisterContent::Char(text) => {
                                for ch in text.chars() {
                                    if ch == '\n' {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_newline(line, col);
                                        self.current_window_mut().cursor.line += 1;
                                        self.current_window_mut().cursor.col = 0;
                                    } else {
                                        let line = self.current_window().cursor.line;
                                        let col = self.current_window().cursor.col;
                                        self.current_buffer_mut().insert_char(line, col, ch);
                                        self.current_window_mut().cursor.col += 1;
                                    }
                                }
                            }
                            RegisterContent::Line(lines) => {
                                let insert_line = self.current_window().cursor.line;
                                for (i, line) in lines.iter().enumerate() {
                                    let target_line = insert_line + i;
                                    if target_line > 0 {
                                        let prev_line = target_line.saturating_sub(1);
                                        let prev_line_len = self.current_buffer().line_len(prev_line);
                                        self.current_buffer_mut().insert_newline(prev_line, prev_line_len);
                                    } else {
                                        self.current_buffer_mut().insert_newline(0, 0);
                                    }
                                    for (idx, ch) in line.chars().enumerate() {
                                        self.current_buffer_mut().insert_char(target_line, idx, ch);
                                    }
                                }
                                self.current_window_mut().cursor.line = insert_line;
                                self.current_window_mut().cursor.col = 0;
                            }
                            _ => {}
                        }
                    }
                }
                let end_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                self.current_buffer_mut().set_mark('[', start_pos);
                self.current_buffer_mut().set_mark(']', end_pos);
            }
            Action::Join => {
                self.save_undo_state();
                let current_line = self.current_window().cursor.line;
                if current_line < self.current_buffer().line_count() - 1 {
                    let line1_len = self.current_buffer().line_len(current_line);
                    self.current_buffer_mut().delete_range(current_line, line1_len, current_line + 1, 0);
                    if line1_len > 0 {
                        self.current_buffer_mut().insert_char(current_line, line1_len, ' ');
                    }
                }
            }
            Action::JoinNoSpace => {
                self.save_undo_state();
                let current_line = self.current_window().cursor.line;
                if current_line < self.current_buffer().line_count() - 1 {
                    let line1_len = self.current_buffer().line_len(current_line);
                    self.current_buffer_mut().delete_range(current_line, line1_len, current_line + 1, 0);
                }
            }
            Action::MakeLowercase => { self.pending_operator = PendingOperator::MakeLowercase; }
            Action::MakeUppercase => { self.pending_operator = PendingOperator::MakeUppercase; }
            Action::ToggleCase => { self.pending_operator = PendingOperator::ToggleCase; }
            Action::Indent => { self.pending_operator = PendingOperator::Indent; }
            Action::Dedent => { self.pending_operator = PendingOperator::Dedent; }
            Action::AutoIndent => { self.pending_operator = PendingOperator::AutoIndent; }

            Action::RepeatLastChange => {
                if let Some((last_action, last_count)) = self.last_change.clone() {
                    let saved_count = self.count;
                    self.count = if saved_count > 0 { saved_count } else { last_count };
                    self.execute_action(last_action)?;
                    self.count = saved_count;
                }
            }

            Action::LspCompletion => {
                if let Some(path) = self.current_buffer().file_path().map(|p| p.to_path_buf()) {
                    let uri = format!("file://{}", path.display());
                    let line = self.current_window().cursor.line as u32;
                    let col = self.current_window().cursor.col as u32;
                    match self.lsp_client.completion(&uri, line, col) {
                        Ok(items) if !items.is_empty() => {
                            let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
                            self.message = Some(format!("Completions: {}", labels.join(", ")));
                        }
                        Ok(_) => { self.message = Some("No completions".to_string()); }
                        Err(e) => { self.message = Some(format!("LSP error: {}", e)); }
                    }
                }
            }
            Action::OpenFileFinder => { self.open_file_finder(); }

            Action::Quit => {
                if !self.current_buffer().is_modified() {
                    self.should_quit = true;
                } else { self.message = Some("No write since last change".to_string()); }
            }

            // search
            Action::SearchForward => {
                self.mode = Mode::Search;
                self.search_buffer.clear();
                self.search_forward = true;
            }
            Action::SearchBackward => {
                self.mode = Mode::Search;
                self.search_buffer.clear();
                self.search_forward = false;
            }
            Action::SearchNext => {
                if self.search_pattern.is_some() { self.execute_search()?; }
            }
            Action::SearchPrevious => {
                if self.search_pattern.is_some() {
                    self.search_forward = !self.search_forward;
                    self.execute_search()?;
                    self.search_forward = !self.search_forward;
                }
            }
            Action::SearchWordForward => {
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    let chars: Vec<char> = line_text.chars().collect();
                    if self.current_window().cursor.col < chars.len() {
                        let mut start = self.current_window().cursor.col;
                        let mut end = self.current_window().cursor.col;
                        while start > 0 && !chars[start - 1].is_whitespace() && chars[start - 1].is_alphanumeric() { start -= 1; }
                        while end < chars.len() && !chars[end].is_whitespace() && chars[end].is_alphanumeric() { end += 1; }
                        let word: String = chars[start..end].iter().collect();
                        if !word.is_empty() {
                            self.search_pattern = Some(word);
                            self.search_forward = true;
                            self.execute_search()?;
                        }
                    }
                }
            }
            Action::SearchWordBackward => {
                if let Some(line_text) = self.current_buffer().get_line(self.current_window().cursor.line) {
                    let chars: Vec<char> = line_text.chars().collect();
                    if self.current_window().cursor.col < chars.len() {
                        let mut start = self.current_window().cursor.col;
                        let mut end = self.current_window().cursor.col;
                        while start > 0 && !chars[start - 1].is_whitespace() && chars[start - 1].is_alphanumeric() { start -= 1; }
                        while end < chars.len() && !chars[end].is_whitespace() && chars[end].is_alphanumeric() { end += 1; }
                        let word: String = chars[start..end].iter().collect();
                        if !word.is_empty() {
                            self.search_pattern = Some(word);
                            self.search_forward = false;
                            self.execute_search()?;
                        }
                    }
                }
            }

            // change list navigation
            Action::JumpToChangeNext => {
                if self.change_index < self.change_list.len().saturating_sub(1) {
                    self.change_index += 1;
                    let (line, col) = self.change_list[self.change_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpToChangePrev => {
                if self.change_index > 0 {
                    self.change_index -= 1;
                    let (line, col) = self.change_list[self.change_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpBack => {
                if self.jump_list.is_empty() { return Ok(()); }
                let current_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
                if self.jump_index == self.jump_list.len() - 1 {
                    if self.jump_list[self.jump_index] != current_pos {
                        self.jump_list.push(current_pos);
                        self.jump_index = self.jump_list.len() - 1;
                    }
                }
                if self.jump_index > 0 {
                    self.jump_index -= 1;
                    let (line, col) = self.jump_list[self.jump_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }
            Action::JumpForward => {
                if self.jump_index < self.jump_list.len().saturating_sub(1) {
                    self.jump_index += 1;
                    let (line, col) = self.jump_list[self.jump_index];
                    self.current_window_mut().cursor.line = line.min(self.current_buffer().line_count().saturating_sub(1));
                    self.current_window_mut().cursor.col = col;
                    self.clamp_cursor();
                }
            }

            // window management
            Action::WindowSplitH => {
                let buf_idx = self.current_window().buffer_index;
                let (w, h) = self.terminal.size();
                let vh = (h as usize).saturating_sub(2);
                let new_win = crate::window::Window::new(buf_idx, w as usize, vh / 2);
                let new_idx = self.windows.len();
                self.windows.push(new_win);
                self.layout.split_vertical(self.active_window, new_idx);
                self.recalculate_window_rects();
            }
            Action::WindowSplitV => {
                let buf_idx = self.current_window().buffer_index;
                let (w, h) = self.terminal.size();
                let vh = (h as usize).saturating_sub(2);
                let new_win = crate::window::Window::new(buf_idx, w as usize / 2, vh);
                let new_idx = self.windows.len();
                self.windows.push(new_win);
                self.layout.split_horizontal(self.active_window, new_idx);
                self.recalculate_window_rects();
            }
            Action::WindowClose => {
                let leaves = self.layout.leaves();
                if leaves.len() > 1 {
                    self.layout.remove(self.active_window);
                    let remaining = self.layout.leaves();
                    self.active_window = remaining.first().copied().unwrap_or(0);
                    self.recalculate_window_rects();
                }
            }
            Action::WindowFocusLeft => { self.focus_direction(-1, 0); }
            Action::WindowFocusDown => { self.focus_direction(0, 1); }
            Action::WindowFocusUp => { self.focus_direction(0, -1); }
            Action::WindowFocusRight => { self.focus_direction(1, 0); }
            Action::WindowCycle => {
                let leaves = self.layout.leaves();
                if let Some(pos) = leaves.iter().position(|&i| i == self.active_window) {
                    self.active_window = leaves[(pos + 1) % leaves.len()];
                }
            }
            Action::WindowEqualize => { self.recalculate_window_rects(); }

            _ => {}
        }
        self.clamp_cursor();
        let cursor = self.current_window().cursor;
        self.current_window_mut().viewport.ensure_cursor_visible(cursor.line, cursor.col);
        Ok(())
    }
}
