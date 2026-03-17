// key input handling: normal, command, search, fuzzy find modes

use crossterm::event::{KeyCode, KeyEvent};
use std::fs;
use crate::error::Result;
use crate::keymap::{map_key, Action};
use crate::mode::Mode;
use crate::register::RegisterContent;
use super::{Editor, MarkAction, PendingOperator, TextObjectModifier};

impl Editor {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.file_changed_externally {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.file_changed_externally = false;
                    if let Some(path) = self.current_buffer().file_path().map(|p| p.to_path_buf()) {
                        if let Err(e) = self.open(&path) {
                            self.message = Some(format!("Error reloading file: {}", e));
                        } else {
                            self.message = Some("File reloaded.".to_string());
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.file_changed_externally = false;
                    self.message = Some("Reload cancelled.".to_string());
                }
                _ => {}
            }
            return Ok(());
        }
        // record key if recording macro
        if self.recording_register.is_some() {
            if key.code == KeyCode::Char('q')
                && self.mode == Mode::Normal
                && self.pending_operator == PendingOperator::None
                && self.pending_key.is_none()
                && self.pending_text_object.is_none()
                && self.waiting_for_mark.is_none()
                && !self.waiting_for_register
            {
                if let Some(reg) = self.recording_register {
                    self.registers
                        .set(Some(reg), RegisterContent::Macro(self.macro_buffer.clone()));
                    self.message = Some(format!("Recorded macro to @{}", reg));
                }
                self.recording_register = None;
                self.macro_buffer.clear();
                return Ok(());
            } else {
                self.macro_buffer.push(key);
            }
        }

        if self.mode == Mode::Command {
            self.handle_command_mode_key(key)?;
        } else if self.mode == Mode::Search {
            self.handle_search_mode_key(key)?;
        } else if self.mode == Mode::FuzzyFind {
            self.handle_fuzzy_find_key(key)?;
        } else {
            // handle mark operations (m, ', `)
            if self.mode == Mode::Normal && self.waiting_for_mark.is_some() {
                if let KeyCode::Char(c) = key.code {
                    let mark_action = self.waiting_for_mark.unwrap();
                    self.waiting_for_mark = None;
                    match mark_action {
                        MarkAction::Set => {
                            if ('a'..='z').contains(&c) {
                                let cursor = self.current_window().cursor;
                                self.current_buffer_mut().set_mark(c, (cursor.line, cursor.col));
                            } else if ('A'..='Z').contains(&c) {
                                let cursor = self.current_window().cursor;
                                self.global_marks.insert(c, (cursor.line, cursor.col));
                            }
                        }
                        MarkAction::Jump => {
                            let pos = if ('a'..='z').contains(&c) {
                                self.current_buffer().get_mark(c)
                            } else if ('A'..='Z').contains(&c) {
                                self.global_marks.get(&c).cloned()
                            } else { None };
                            if let Some((line, _col)) = pos {
                                let line_count = self.current_buffer().line_count();
                                let window = self.current_window_mut();
                                window.cursor.line = line.min(line_count.saturating_sub(1));
                                window.cursor.col = 0;
                                self.clamp_cursor();
                            }
                        }
                        MarkAction::JumpExact => {
                            let pos = if ('a'..='z').contains(&c) {
                                self.current_buffer().get_mark(c)
                            } else if ('A'..='Z').contains(&c) {
                                self.global_marks.get(&c).cloned()
                            } else { None };
                            if let Some((line, col)) = pos {
                                let line_count = self.current_buffer().line_count();
                                let window = self.current_window_mut();
                                window.cursor.line = line.min(line_count.saturating_sub(1));
                                window.cursor.col = col;
                                self.clamp_cursor();
                            }
                        }
                    }
                    return Ok(());
                }
            }

            // check for m, ', ` to start mark operations
            if self.mode == Mode::Normal
                && self.waiting_for_mark.is_none()
                && self.pending_operator == PendingOperator::None
            {
                match key.code {
                    KeyCode::Char('m') => { self.waiting_for_mark = Some(MarkAction::Set); return Ok(()); }
                    KeyCode::Char('\'') => { self.waiting_for_mark = Some(MarkAction::Jump); return Ok(()); }
                    KeyCode::Char('`') => { self.waiting_for_mark = Some(MarkAction::JumpExact); return Ok(()); }
                    KeyCode::Char('q') => {
                        if self.recording_register.is_none() {
                            self.pending_key = Some('q');
                            return Ok(());
                        }
                    }
                    KeyCode::Char('@') => { self.pending_key = Some('@'); return Ok(()); }
                    _ => {}
                }
            }

            // handle register selection (")
            if self.mode == Mode::Normal && self.waiting_for_register {
                if let KeyCode::Char(c) = key.code {
                    self.pending_register = Some(c);
                    self.waiting_for_register = false;
                    return Ok(());
                }
            }
            if self.mode == Mode::Normal && !self.waiting_for_register {
                if let KeyCode::Char('"') = key.code {
                    self.waiting_for_register = true;
                    return Ok(());
                }
            }

            // resolve surround pending
            if self.mode == Mode::Normal && self.surround_pending.is_some() {
                if let KeyCode::Char(c) = key.code {
                    let sp = self.surround_pending.unwrap();
                    match sp {
                        'c' => { // cs: first char is old, wait for new
                            self.surround_pending = Some('C');
                            self.surround_ys_pending = false;
                            self.pending_register = Some(c);
                            return Ok(());
                        }
                        'C' => { // cs: second char is new
                            let old_char = self.pending_register.unwrap_or('"');
                            self.pending_register = None;
                            self.surround_pending = None;
                            self.surround_change(old_char, c)?;
                            return Ok(());
                        }
                        'd' => { // ds: char to delete surrounding
                            self.surround_pending = None;
                            self.surround_delete(c)?;
                            return Ok(());
                        }
                        'y' => { // ys: simplified surround inner word
                            self.surround_pending = None;
                            self.surround_add_word(c)?;
                            return Ok(());
                        }
                        _ => { self.surround_pending = None; }
                    }
                } else if key.code == KeyCode::Esc {
                    self.surround_pending = None;
                    self.pending_register = None;
                    return Ok(());
                }
            }

            // handle count input
            if self.mode == Mode::Normal {
                if let KeyCode::Char(c) = key.code {
                    if c.is_ascii_digit() {
                        let digit = c.to_digit(10).unwrap() as usize;
                        if digit != 0 || self.count != 0 {
                            self.count = self.count * 10 + digit;
                            return Ok(());
                        }
                    }
                }
            }

            // handle multi-key sequences
            let action = if let Some(prefix) = self.pending_key {
                if prefix == 'q' {
                    if let KeyCode::Char(c) = key.code {
                        if ('a'..='z').contains(&c) || ('0'..='9').contains(&c) {
                            self.recording_register = Some(c);
                            self.macro_buffer.clear();
                            self.message = Some(format!("recording @{}", c));
                        }
                    }
                    self.pending_key = None;
                    return Ok(());
                } else if prefix == '@' {
                    if let KeyCode::Char(c) = key.code { self.play_macro(c)?; }
                    self.pending_key = None;
                    return Ok(());
                }
                let sequence_action = self.map_key_sequence(prefix, key);
                self.pending_key = None;
                sequence_action
            } else if key.code == KeyCode::Char('g') && self.mode == Mode::Normal {
                self.pending_key = Some('g');
                return Ok(());
            } else if key.code == KeyCode::Char('r') && self.mode == Mode::Normal {
                self.pending_key = Some('r');
                return Ok(());
            } else if key.code == KeyCode::Char('w') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && self.mode == Mode::Normal {
                self.pending_key = Some('\x17'); // Ctrl-W marker
                return Ok(());
            } else {
                map_key(key, &self.mode)
            };

            // detect surround: cs{old}{new}, ds{char}, ys{char}
            if self.pending_operator != PendingOperator::None
                && key.code == KeyCode::Char('s')
                && self.surround_pending.is_none()
            {
                match self.pending_operator {
                    PendingOperator::Change => {
                        self.surround_pending = Some('c');
                        self.pending_operator = PendingOperator::None;
                        return Ok(());
                    }
                    PendingOperator::Delete => {
                        self.surround_pending = Some('d');
                        self.pending_operator = PendingOperator::None;
                        return Ok(());
                    }
                    PendingOperator::Yank => {
                        self.surround_pending = Some('y');
                        self.pending_operator = PendingOperator::None;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // handle text object composition
            if self.pending_operator != PendingOperator::None {
                if matches!(key.code, KeyCode::Char('a') | KeyCode::Char('i'))
                    && self.pending_text_object.is_none()
                {
                    self.pending_text_object = if key.code == KeyCode::Char('a') {
                        Some(TextObjectModifier::Around)
                    } else {
                        Some(TextObjectModifier::Inner)
                    };
                    return Ok(());
                } else if self.pending_text_object.is_some() {
                    self.handle_text_object(key)?;
                    return Ok(());
                } else {
                    self.handle_operator_motion(action)?;
                }
            } else {
                self.execute_action(action)?;
            }

            if self.pending_operator == PendingOperator::None
                && self.pending_key.is_none()
                && self.pending_text_object.is_none()
            {
                self.count = 0;
            }
        }
        Ok(())
    }

    pub(super) fn map_key_sequence(&self, prefix: char, key: KeyEvent) -> Action {
        if prefix == 'g' {
            match key.code {
                KeyCode::Char('g') => Action::MoveFileStart,
                KeyCode::Char('e') => Action::MoveWordEndBack,
                KeyCode::Char('E') => Action::MoveWordEndBackBig,
                KeyCode::Char('_') => Action::MoveLineEndNonBlank,
                KeyCode::Char('0') => Action::MoveLineStartDisplay,
                KeyCode::Char('$') => Action::MoveLineEndDisplay,
                KeyCode::Char('J') => Action::JoinNoSpace,
                KeyCode::Char('u') => Action::MakeLowercase,
                KeyCode::Char('U') => Action::MakeUppercase,
                KeyCode::Char('~') => Action::ToggleCase,
                KeyCode::Char(';') => Action::JumpToChangeNext,
                KeyCode::Char(',') => Action::JumpToChangePrev,
                _ => Action::None,
            }
        } else if prefix == 'r' {
            match key.code {
                KeyCode::Char(c) => Action::Replace(c),
                _ => Action::None,
            }
        } else if prefix == '\x17' { // Ctrl-W prefix
            match key.code {
                KeyCode::Char('s') => Action::WindowSplitH,
                KeyCode::Char('v') => Action::WindowSplitV,
                KeyCode::Char('c') => Action::WindowClose,
                KeyCode::Char('h') => Action::WindowFocusLeft,
                KeyCode::Char('j') => Action::WindowFocusDown,
                KeyCode::Char('k') => Action::WindowFocusUp,
                KeyCode::Char('l') => Action::WindowFocusRight,
                KeyCode::Char('w') => Action::WindowCycle,
                KeyCode::Char('=') => Action::WindowEqualize,
                _ => Action::None,
            }
        } else { Action::None }
    }

    pub(super) fn generate_completions(&mut self) {
        let input = self.command_buffer.trim();
        if input.is_empty() { return; }
        self.completion_candidates.clear();
        if input.starts_with("e ") || input.starts_with("edit ") {
            let prefix = if let Some(p) = input.strip_prefix("e ") { p }
            else { input.strip_prefix("edit ").unwrap() };
            let dir = ".";
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.starts_with(prefix) {
                            let cmd_prefix = if input.starts_with("e ") { "e " } else { "edit " };
                            self.completion_candidates.push(format!("{}{}", cmd_prefix, name));
                        }
                    }
                }
            }
        } else if !input.contains(' ') {
            let commands = vec![
                "w", "write", "q", "quit", "wq", "x", "q!", "e", "edit",
                "bn", "bnext", "bp", "bprevious", "bd", "bdelete",
                "ls", "buffers", "sp", "split", "vsp", "vsplit", "close", "help", "set",
            ];
            for cmd in commands {
                if cmd.starts_with(input) { self.completion_candidates.push(cmd.to_string()); }
            }
        }
        self.completion_candidates.sort();
    }

    pub(super) fn handle_command_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.history_index = None;
                self.completion_candidates.clear();
                self.completion_index = None;
                self.clear_substitute_preview();
            }
            KeyCode::Enter => {
                if !self.command_buffer.is_empty() {
                    if self.command_history.last() != Some(&self.command_buffer) {
                        self.command_history.push(self.command_buffer.clone());
                    }
                }
                self.clear_substitute_preview();
                self.execute_command()?;
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.history_index = None;
                self.completion_candidates.clear();
                self.completion_index = None;
            }
            KeyCode::Up => {
                self.completion_candidates.clear();
                self.completion_index = None;
                if self.command_history.is_empty() { return Ok(()); }
                if self.history_index.is_none() {
                    self.history_index = Some(self.command_history.len() - 1);
                    self.command_buffer = self.command_history[self.command_history.len() - 1].clone();
                } else {
                    let idx = self.history_index.unwrap();
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                        self.command_buffer = self.command_history[idx - 1].clone();
                    }
                }
            }
            KeyCode::Down => {
                self.completion_candidates.clear();
                self.completion_index = None;
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.command_history.len() {
                        self.history_index = Some(idx + 1);
                        self.command_buffer = self.command_history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.command_buffer.clear();
                    }
                }
            }
            KeyCode::Tab => {
                if self.completion_candidates.is_empty() {
                    self.generate_completions();
                    if !self.completion_candidates.is_empty() {
                        self.completion_index = Some(0);
                        self.command_buffer = self.completion_candidates[0].clone();
                    }
                } else {
                    if let Some(idx) = self.completion_index {
                        let next_idx = (idx + 1) % self.completion_candidates.len();
                        self.completion_index = Some(next_idx);
                        self.command_buffer = self.completion_candidates[next_idx].clone();
                    }
                }
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                self.completion_candidates.clear();
                self.completion_index = None;
                self.update_substitute_preview();
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
                self.completion_candidates.clear();
                self.completion_index = None;
                self.update_substitute_preview();
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                    self.history_index = None;
                    self.clear_substitute_preview();
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn handle_search_mode_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_buffer.clear();
            }
            KeyCode::Enter => {
                if !self.search_buffer.is_empty() {
                    self.search_pattern = Some(self.search_buffer.clone());
                    self.execute_search()?;
                }
                self.mode = Mode::Normal;
                self.search_buffer.clear();
            }
            KeyCode::Char(c) => { self.search_buffer.push(c); }
            KeyCode::Backspace => {
                self.search_buffer.pop();
                if self.search_buffer.is_empty() { self.mode = Mode::Normal; }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn handle_fuzzy_find_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.fuzzy_finder = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                if let Some(ref finder) = self.fuzzy_finder {
                    if let Some(selected) = finder.selected_item() {
                        let selected = selected.to_string();
                        let finder_type = finder.finder_type;
                        self.fuzzy_finder = None;
                        self.mode = Mode::Normal;
                        match finder_type {
                            crate::fuzzy_finder::FinderType::Files => { self.open(&selected)?; }
                            crate::fuzzy_finder::FinderType::Buffers => {
                                if let Some(idx) = self.buffers.iter().position(|b| b.file_name() == selected) {
                                    self.windows[self.active_window].buffer_index = idx;
                                }
                            }
                            crate::fuzzy_finder::FinderType::Grep => {
                                let parts: Vec<&str> = selected.splitn(3, ':').collect();
                                if parts.len() >= 2 {
                                    let file = parts[0];
                                    if let Ok(line_num) = parts[1].parse::<usize>() {
                                        self.open(file)?;
                                        let target_line = line_num.saturating_sub(1);
                                        self.current_window_mut().cursor.line = target_line;
                                        self.current_window_mut().cursor.col = 0;
                                        let height = self.current_window().viewport.height;
                                        let half_height = height / 2;
                                        self.current_window_mut().viewport.offset_line =
                                            target_line.saturating_sub(half_height);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if let Some(ref mut finder) = self.fuzzy_finder { finder.select_prev(); }
            }
            KeyCode::Down | KeyCode::Tab => {
                if let Some(ref mut finder) = self.fuzzy_finder { finder.select_next(); }
            }
            KeyCode::Backspace => {
                if let Some(ref mut finder) = self.fuzzy_finder { finder.pop_char(); }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut finder) = self.fuzzy_finder { finder.push_char(c); }
            }
            _ => {}
        }
        Ok(())
    }
}
