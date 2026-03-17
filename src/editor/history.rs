// undo/redo, jump list, change list, macro recording

use crossterm::event::KeyEvent;
use crate::keymap::Action;
use crate::register::RegisterContent;
use crate::undo::UndoOp;
use super::Editor;

impl Editor {
    pub(super) fn save_undo_state(&mut self) {
        self.undo_manager.begin_group();
    }

    pub(super) fn end_undo_group(&mut self) {
        self.undo_manager.end_group();
    }

    // record+apply helpers -- use these instead of calling buffer methods directly
    // when inside an undo group

    pub(super) fn rec_insert_char(&mut self, line: usize, col: usize, ch: char) {
        self.undo_manager.record(UndoOp::InsertChar { line, col, ch });
        self.current_buffer_mut().insert_char(line, col, ch);
    }

    pub(super) fn rec_delete_char(&mut self, line: usize, col: usize) {
        let ch = self.current_buffer().get_char_at(line, col).unwrap_or('\0');
        self.undo_manager.record(UndoOp::DeleteChar { line, col, ch });
        self.current_buffer_mut().delete_char(line, col);
    }

    pub(super) fn rec_insert_newline(&mut self, line: usize, col: usize) {
        self.undo_manager.record(UndoOp::InsertNewline { line, col });
        self.current_buffer_mut().insert_newline(line, col);
    }

    pub(super) fn rec_delete_range(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) {
        let text = self.current_buffer().get_range_text(start_line, start_col, end_line, end_col);
        self.undo_manager.record(UndoOp::DeleteRange { line: start_line, col: start_col, text });
        self.current_buffer_mut().delete_range(start_line, start_col, end_line, end_col);
    }

    pub(super) fn undo(&mut self) {
        let buffer_idx = self.windows[self.active_window].buffer_index;
        if self.undo_manager.undo(&mut self.buffers[buffer_idx]) {
            self.message = Some("Undone".to_string());
        } else {
            self.message = Some("Already at oldest change".to_string());
        }
    }

    pub(super) fn redo(&mut self) {
        let buffer_idx = self.windows[self.active_window].buffer_index;
        if self.undo_manager.redo(&mut self.buffers[buffer_idx]) {
            self.message = Some("Redone".to_string());
        } else {
            self.message = Some("Already at newest change".to_string());
        }
    }

    pub(super) fn save_jump_position(&mut self) {
        let pos = (
            self.current_window().cursor.line,
            self.current_window().cursor.col,
        );
        self.current_buffer_mut().set_mark('\'', pos);
        self.current_buffer_mut().set_mark('`', pos);
        if self.jump_index < self.jump_list.len().saturating_sub(1) {
            self.jump_list.truncate(self.jump_index + 1);
        }
        if self.jump_list.is_empty() || self.jump_list.last() != Some(&pos) {
            self.jump_list.push(pos);
            self.jump_index = self.jump_list.len().saturating_sub(1);
        }
    }

    pub(super) fn record_change(&mut self, action: Action) {
        let count = if self.count == 0 { 0 } else { self.count };
        self.last_change = Some((action, count));
        let pos = (
            self.current_window().cursor.line,
            self.current_window().cursor.col,
        );
        self.change_list.push(pos);
        self.change_index = self.change_list.len().saturating_sub(1);
        if self.change_list.len() > 100 {
            self.change_list.remove(0);
            self.change_index = self.change_index.saturating_sub(1);
        }
    }

    pub(super) fn play_macro(&mut self, reg_char: char) -> crate::error::Result<()> {
        let actual_reg = if reg_char == '@' {
            self.last_macro_register
        } else {
            Some(reg_char)
        };
        if let Some(reg) = actual_reg {
            self.last_macro_register = Some(reg);
            if let Some(RegisterContent::Macro(keys)) = self.registers.get(Some(reg)) {
                let keys: Vec<KeyEvent> = keys.clone();
                let count = if self.count == 0 { 1 } else { self.count };
                for _ in 0..count {
                    for key in &keys {
                        self.handle_key(key.clone())?;
                    }
                }
            } else {
                self.message = Some(format!("Register @{} is empty or not a macro", reg));
            }
        } else {
            self.message = Some("No previously executed macro".to_string());
        }
        Ok(())
    }
}
