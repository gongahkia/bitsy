// command execution: :commands, search, substitute, help, finders

use std::path::PathBuf;
use crate::buffer::Buffer;
use crate::command::{parse_command, Command};
use crate::cursor::Cursor;
use crate::error::Result;
use crate::fuzzy_finder::FuzzyFinder;
use crate::mode::Mode;
use super::Editor;

impl Editor {
    pub(super) fn execute_command(&mut self) -> Result<()> {
        self.registers.update_last_command(self.command_buffer.clone());
        let cmd_str = self.command_buffer.clone();
        let cmd = parse_command(&self.command_buffer)?;
        match cmd {
            Command::Write(path) => {
                if let Some(p) = path {
                    let p_str = p.clone();
                    if let Err(e) = self.current_buffer_mut().save_as(p) {
                        self.message = Some(format!("Error: {}", e));
                    } else {
                        self.message = Some("File written".to_string());
                        self.emit_event(crate::event::EditorEvent::BufferSave { path: p_str });
                    }
                } else {
                    if self.current_buffer().file_path().is_some() {
                        let fpath = self.current_buffer().file_path().unwrap().to_string_lossy().to_string();
                        if let Err(e) = self.current_buffer_mut().save() {
                            self.message = Some(format!("Error: {}", e));
                        } else {
                            self.message = Some("File written".to_string());
                            self.emit_event(crate::event::EditorEvent::BufferSave { path: fpath });
                        }
                    } else { self.message = Some("No file name. Use :w <filename>".to_string()); }
                }
            }
            Command::Quit => {
                if self.viewing_help {
                    if let Some(return_buffer) = self.help_return_buffer.take() {
                        self.buffers[0] = return_buffer;
                        if let Some(return_cursor) = self.help_return_cursor.take() {
                            self.windows[0].cursor = return_cursor;
                        }
                        self.showing_landing_page = self.was_showing_landing_page;
                        self.viewing_help = false;
                        self.message = Some("Returned from help".to_string());
                    }
                } else if self.current_buffer().is_modified() {
                    if self.current_buffer().file_path().is_none() {
                        self.message = Some("No file name. Use :w <filename> to save.".to_string());
                    } else {
                        self.message = Some("No write since last change (use :q! to force)".to_string());
                    }
                } else { self.should_quit = true; }
            }
            Command::WriteQuit(path) => {
                let save_result = if let Some(p) = path {
                    self.current_buffer_mut().save_as(p)
                } else {
                    if self.current_buffer().file_path().is_some() {
                        self.current_buffer_mut().save()
                    } else {
                        self.message = Some("No file name. Use :w <filename>".to_string());
                        return Ok(());
                    }
                };
                if let Err(e) = save_result {
                    self.message = Some(format!("Error: {}", e));
                } else { self.should_quit = true; }
            }
            Command::ForceQuit => { self.should_quit = true; }
            Command::Edit(filename) => {
                let was_landing_page = self.showing_landing_page;
                if was_landing_page { self.showing_landing_page = false; }
                if self.current_buffer().is_modified() && !was_landing_page {
                    self.message = Some("No write since last change".to_string());
                } else {
                    match Buffer::from_file(&filename, &self.config) {
                        Ok(new_buffer) => {
                            self.buffers[self.windows[self.active_window].buffer_index] = new_buffer;
                            self.windows[self.active_window].cursor = Cursor::default();
                            self.message = Some(format!("Opened {}", filename));
                        }
                        Err(e) => { self.message = Some(format!("Error: {}", e)); }
                    }
                }
            }
            Command::GoToLine(line_num) => {
                self.save_jump_position();
                let target_line = line_num.saturating_sub(1);
                let line_count = self.current_buffer().line_count();
                self.current_window_mut().cursor.line = target_line.min(line_count.saturating_sub(1));
                self.current_window_mut().cursor.col = 0;
                self.clamp_cursor();
            }
            Command::Substitute { pattern, replacement, global, range } => {
                self.save_undo_state();
                let current_line = self.current_window().cursor.line;
                let line_count = self.current_buffer().line_count();
                let (start_line, end_line) = if let Some(r) = range {
                    let start = r.start.saturating_sub(1);
                    let end = if r.end == usize::MAX {
                        line_count.saturating_sub(1)
                    } else { r.end.saturating_sub(1).min(line_count.saturating_sub(1)) };
                    (start, end)
                } else if let Some((vs, ve)) = self.visual_cmd_range {
                    (vs.min(line_count.saturating_sub(1)), ve.min(line_count.saturating_sub(1)))
                } else { (current_line, current_line) };
                self.visual_cmd_range = None;
                let mut total_count = 0;
                for line in start_line..=end_line {
                    total_count += self.execute_substitute_line(line, &pattern, &replacement, global);
                }
                let range_count = end_line - start_line + 1;
                if total_count > 0 {
                    self.message = Some(format!(
                        "{} substitution{} on {} line{}",
                        total_count,
                        if total_count == 1 { "" } else { "s" },
                        range_count,
                        if range_count != 1 { "s" } else { "" }
                    ));
                } else { self.message = Some("Pattern not found".to_string()); }
            }
            Command::Delete { range } => {
                self.save_undo_state();
                if let Some(r) = range {
                    let line_count = self.current_buffer().line_count();
                    let start = r.start.saturating_sub(1).min(line_count.saturating_sub(1));
                    let end = if r.end == usize::MAX {
                        line_count.saturating_sub(1)
                    } else { r.end.saturating_sub(1).min(line_count.saturating_sub(1)) };
                    for line in (start..=end).rev() {
                        let line_len = self.current_buffer().line_len(line);
                        self.current_buffer_mut().delete_range(line, 0, line, line_len);
                        if line < self.current_buffer().line_count() {
                            self.current_buffer_mut().delete_char(line, 0);
                        }
                    }
                    let deleted_count = end - start + 1;
                    self.message = Some(format!(
                        "{} line{} deleted",
                        deleted_count,
                        if deleted_count != 1 { "s" } else { "" }
                    ));
                    self.clamp_cursor();
                } else { self.message = Some("No range specified".to_string()); }
            }
            Command::Set { option, value } => match self.config.set(&option, value.as_deref()) {
                Ok(()) => { self.message = Some(format!("{} set", option)); }
                Err(e) => { self.message = Some(e); }
            },
            Command::Help(topic) => {
                if let Some(ref t) = topic {
                    let help_text = self.get_help_topic(t);
                    self.message = Some(help_text);
                } else {
                    self.help_return_buffer = Some(self.buffers[0].clone());
                    self.help_return_cursor = Some(self.windows[0].cursor);
                    self.was_showing_landing_page = self.showing_landing_page;
                    let help_text = r#"Bitsy Keybinds

NORMAL MODE
  h/j/k/l         Move left/down/up/right
  w/b/e           Move by word
  0/$             Line start/end
  gg/G            File start/end
  %               Matching bracket
  Ctrl-b/f/u/d    Page/half-page up/down
  ( ) { }         Sentence/paragraph
  :               Command mode
  i/a/I/A/o/O     Insert/append
  v/V             Visual/visual line mode
  d/y/c           Delete/yank/change (waits for motion)
  dd/yy/cc        Delete/yank/change line
  p/P             Paste after/before
  u/ctrl-r        Undo/redo
  .               Repeat last change
  >/<             Indent/dedent
  =               Auto-indent
  J               Join lines
  m{char}         Set mark
  '{char}/`{char}  Jump to mark
  q{reg}/@{reg}   Record/play macro
  "/0-9a-z        Registers

VISUAL MODE
  h/j/k/l         Move selection
  d/y             Delete/yank selection
  ESC             Exit visual mode

COMMANDS
  :w              Write file
  :q              Quit (or return from help)
  :e <file>       Edit file
  :set <opt>      Set option
  :help           Show help
  :d <range>      Delete lines
  :s/find/rep/g   Substitute

SEARCH
  /pattern        Search forward
  ?pattern        Search backward
  n/N             Next/prev match
  * #             Search word under cursor

note: this is a help buffer - :q to return, or edit as you like!
"#;
                    self.buffers[0] = Buffer::from_string(help_text);
                    self.windows[0].cursor = Cursor::default();
                    self.viewing_help = true;
                    self.showing_landing_page = false;
                    self.message = Some(":q to return to previous buffer".to_string());
                }
            }
            Command::BufferNext => { self.message = Some("Already at the last buffer".to_string()); }
            Command::BufferPrevious => { self.message = Some("Already at the first buffer".to_string()); }
            Command::BufferList => {
                let buf_name = self.current_buffer().file_name();
                let modified = if self.current_buffer().is_modified() { "[+]" } else { "" };
                self.message = Some(format!(
                    "1 %a   \"{}\" {} line {}",
                    buf_name, modified, self.current_buffer().line_count()
                ));
            }
            Command::BufferDelete(_num) => { self.message = Some("Cannot delete last buffer".to_string()); }
            Command::Split => {
                let buf_idx = self.current_window().buffer_index;
                let (w, h) = self.terminal.size();
                let vh = (h as usize).saturating_sub(2);
                let new_win = crate::window::Window::new(buf_idx, w as usize, vh / 2);
                let new_idx = self.windows.len();
                self.windows.push(new_win);
                self.layout.split_vertical(self.active_window, new_idx);
                self.recalculate_window_rects();
                self.message = Some("Split horizontal".to_string());
            }
            Command::VerticalSplit => {
                let buf_idx = self.current_window().buffer_index;
                let (w, h) = self.terminal.size();
                let vh = (h as usize).saturating_sub(2);
                let new_win = crate::window::Window::new(buf_idx, w as usize / 2, vh);
                let new_idx = self.windows.len();
                self.windows.push(new_win);
                self.layout.split_horizontal(self.active_window, new_idx);
                self.recalculate_window_rects();
                self.message = Some("Split vertical".to_string());
            }
            Command::CloseWindow => {
                let leaves = self.layout.leaves();
                if leaves.len() <= 1 {
                    self.message = Some("Cannot close last window".to_string());
                } else {
                    self.layout.remove(self.active_window);
                    let remaining = self.layout.leaves();
                    self.active_window = remaining.first().copied().unwrap_or(0);
                    self.recalculate_window_rects();
                    self.message = Some("Window closed".to_string());
                }
            }
            Command::Registers => {
                let regs = self.registers.get_all_registers();
                if regs.is_empty() {
                    self.message = Some("No registers populated".to_string());
                } else {
                    let output = regs.iter()
                        .map(|(k, v)| format!("\"{} {}", k, v))
                        .collect::<Vec<_>>()
                        .join("  ");
                    self.message = Some(output);
                }
            }
            Command::Marks => {
                let mut marks = self.current_buffer().get_all_marks();
                for (k, v) in &self.global_marks { marks.push((*k, *v)); }
                marks.sort_by_key(|(k, _)| *k);
                if marks.is_empty() {
                    self.message = Some("No marks set".to_string());
                } else {
                    let output = marks.iter()
                        .map(|(k, (line, col))| format!("{} {}:{}", k, line + 1, col))
                        .collect::<Vec<_>>()
                        .join("  ");
                    self.message = Some(output);
                }
            }
            Command::Goyo => {
                self.zen_mode = !self.zen_mode;
                if self.zen_mode {
                    self.message = Some("Zen mode enabled".to_string());
                } else { self.message = Some("Zen mode disabled".to_string()); }
            }
            Command::Files => { self.open_file_finder(); }
            Command::Buffers => { self.open_buffer_finder(); }
            Command::Grep(pattern) => { self.open_grep_finder(&pattern); }
            Command::Terminal => {
                self.message = Some("Terminal: use :!<cmd> to run commands".to_string());
            }
            Command::Shell(cmd) => {
                match crate::term_pane::run_command(&cmd) {
                    Ok(output) => {
                        let trimmed = output.trim_end();
                        if trimmed.lines().count() <= 1 {
                            self.message = Some(trimmed.to_string());
                        } else {
                            self.help_return_buffer = Some(self.buffers[0].clone());
                            self.help_return_cursor = Some(self.windows[0].cursor);
                            self.was_showing_landing_page = self.showing_landing_page;
                            self.buffers[0] = Buffer::from_string(trimmed);
                            self.windows[0].cursor = Cursor::default();
                            self.viewing_help = true;
                            self.showing_landing_page = false;
                            self.message = Some(":q to return to previous buffer".to_string());
                        }
                    }
                    Err(e) => { self.message = Some(format!("Shell error: {}", e)); }
                }
            }
            Command::Filter { cmd, range } => {
                self.save_undo_state();
                let line_count = self.current_buffer().line_count();
                let (start_line, end_line) = if let Some(r) = range {
                    let s = r.start.saturating_sub(1);
                    let e = if r.end == usize::MAX { line_count.saturating_sub(1) }
                    else { r.end.saturating_sub(1).min(line_count.saturating_sub(1)) };
                    (s, e)
                } else if let Some((vs, ve)) = self.visual_cmd_range {
                    (vs.min(line_count.saturating_sub(1)), ve.min(line_count.saturating_sub(1)))
                } else {
                    let cl = self.current_window().cursor.line;
                    (cl, cl)
                };
                self.visual_cmd_range = None;
                let mut input_lines = Vec::new();
                for line in start_line..=end_line {
                    if let Some(text) = self.current_buffer().get_line(line) {
                        input_lines.push(text);
                    }
                }
                let input = input_lines.join("\n") + "\n";
                match crate::term_pane::filter_through_command(&input, &cmd) {
                    Ok(output) => {
                        // delete old lines (in reverse to keep indices stable)
                        for line in (start_line..=end_line).rev() {
                            let ll = self.current_buffer().line_len(line);
                            self.current_buffer_mut().delete_range(line, 0, line, ll);
                            // delete the newline char joining this line to next (except last line)
                            if line > start_line {
                                let prev_ll = self.current_buffer().line_len(line - 1);
                                if line < self.current_buffer().line_count() {
                                    self.current_buffer_mut().delete_range(line - 1, prev_ll, line, 0);
                                }
                            }
                        }
                        // insert replacement text char-by-char on start_line
                        let trimmed = output.trim_end_matches('\n');
                        let mut cur_line = start_line;
                        let mut cur_col = 0;
                        for ch in trimmed.chars() {
                            if ch == '\n' {
                                self.current_buffer_mut().insert_newline(cur_line, cur_col);
                                cur_line += 1;
                                cur_col = 0;
                            } else {
                                self.current_buffer_mut().insert_char(cur_line, cur_col, ch);
                                cur_col += 1;
                            }
                        }
                        self.message = Some(format!("Filtered {} lines through {}", end_line - start_line + 1, cmd));
                    }
                    Err(e) => { self.message = Some(format!("Filter error: {}", e)); }
                }
                self.clamp_cursor();
            }
            Command::Normal { keys, range } => {
                let line_count = self.current_buffer().line_count();
                let (start, end) = if let Some(r) = range {
                    let s = r.start.saturating_sub(1).min(line_count.saturating_sub(1));
                    let e = if r.end == usize::MAX { line_count.saturating_sub(1) }
                    else { r.end.saturating_sub(1).min(line_count.saturating_sub(1)) };
                    (s, e)
                } else { (0, line_count.saturating_sub(1)) };
                for line in start..=end {
                    self.current_window_mut().cursor.line = line;
                    self.current_window_mut().cursor.col = 0;
                    for ch in keys.chars() {
                        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                        let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
                        self.handle_key(key)?;
                    }
                }
            }
            Command::Unknown(cmd) => { self.message = Some(format!("Unknown command: {}", cmd)); }
        }
        self.emit_event(crate::event::EditorEvent::CommandExecuted { command: cmd_str });
        Ok(())
    }

    pub(super) fn get_help_topic(&self, topic: &str) -> String {
        match topic {
            "motions" | "movement" => "Motions: hjkl (left/down/up/right), w/b/e (word), gg/G (file start/end), %/0/$ (line), /? (search)".to_string(),
            "operators" | "editing" => "Operators: d (delete), c (change), y (yank), p/P (paste), J (join), u (undo), . (repeat), >/< (indent), = (auto-indent)".to_string(),
            "modes" => "Modes: i/I/a/A/o/O (insert), ESC (normal), v/V (visual), : (command), R (replace)".to_string(),
            "commands" => ":Commands: :w (write), :q (quit), :e (edit), :s/find/replace/g (substitute), :set (options), :help, :d (delete)".to_string(),
            "textobjects" | "objects" => "Text objects: aw/iw (word), ap/ip (paragraph), a\"/i\" (quotes), a(/i( (parens), a[/i[, a{/i{, a</i<".to_string(),
            "marks" => "Marks: m{a-z} (set mark), '{a-z} (jump to mark line), `{a-z} (jump to exact position), g; (next change), g, (prev change)".to_string(),
            "search" => "Search: /{pattern} (forward), ?{pattern} (backward), n (next), N (prev), * (word forward), # (word backward)".to_string(),
            "ranges" => "Ranges: % (all lines), 1,10 (lines 1-10), . (current line). Use with :s, :d. Example: %s/old/new/g".to_string(),
            _ => format!("No help for '{}'. Try :help motions, :help operators, :help commands", topic),
        }
    }

    pub(super) fn execute_search(&mut self) -> Result<()> {
        if let Some(pattern) = self.search_pattern.clone() {
            let start_line = self.current_window().cursor.line;
            let start_col = if self.search_forward {
                self.current_window().cursor.col + 1
            } else {
                self.current_window().cursor.col.saturating_sub(1)
            };
            let old_pos = (self.current_window().cursor.line, self.current_window().cursor.col);
            if self.search_forward {
                if self.search_forward_from(start_line, start_col, &pattern) {
                    self.current_buffer_mut().set_mark('\'', old_pos);
                    self.current_buffer_mut().set_mark('`', old_pos);
                    return Ok(());
                }
                self.message = Some("Pattern not found".to_string());
            } else {
                if self.search_backward_from(start_line, start_col, &pattern) {
                    self.current_buffer_mut().set_mark('\'', old_pos);
                    self.current_buffer_mut().set_mark('`', old_pos);
                    return Ok(());
                }
                self.message = Some("Pattern not found".to_string());
            }
        }
        Ok(())
    }

    fn search_forward_from(&mut self, start_line: usize, start_col: usize, pattern: &str) -> bool {
        let line_count = self.current_buffer().line_count();
        if let Some(line_text) = self.current_buffer().get_line(start_line) {
            if let Some(pos) = line_text[start_col.min(line_text.len())..].find(pattern) {
                let window = self.current_window_mut();
                window.cursor.line = start_line;
                window.cursor.col = start_col + pos;
                return true;
            }
        }
        for line_idx in (start_line + 1)..line_count {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.find(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    return true;
                }
            }
        }
        for line_idx in 0..start_line {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.find(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    self.message = Some("search hit BOTTOM, continuing at TOP".to_string());
                    return true;
                }
            }
        }
        false
    }

    fn search_backward_from(&mut self, start_line: usize, start_col: usize, pattern: &str) -> bool {
        if let Some(line_text) = self.current_buffer().get_line(start_line) {
            let search_text = &line_text[..start_col.min(line_text.len())];
            if let Some(pos) = search_text.rfind(pattern) {
                let window = self.current_window_mut();
                window.cursor.line = start_line;
                window.cursor.col = pos;
                return true;
            }
        }
        if start_line > 0 {
            for line_idx in (0..start_line).rev() {
                if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                    if let Some(pos) = line_text.rfind(pattern) {
                        let window = self.current_window_mut();
                        window.cursor.line = line_idx;
                        window.cursor.col = pos;
                        return true;
                    }
                }
            }
        }
        let line_count = self.current_buffer().line_count();
        for line_idx in (start_line + 1..line_count).rev() {
            if let Some(line_text) = self.current_buffer().get_line(line_idx) {
                if let Some(pos) = line_text.rfind(pattern) {
                    let window = self.current_window_mut();
                    window.cursor.line = line_idx;
                    window.cursor.col = pos;
                    self.message = Some("search hit TOP, continuing at BOTTOM".to_string());
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn execute_substitute_line(
        &mut self,
        line: usize,
        pattern: &str,
        replacement: &str,
        global: bool,
    ) -> usize {
        if let Some(line_text) = self.current_buffer().get_line(line) {
            let new_text = if global {
                line_text.replace(pattern, replacement)
            } else {
                line_text.replacen(pattern, replacement, 1)
            };
            let count = if global {
                line_text.matches(pattern).count()
            } else {
                if line_text.contains(pattern) { 1 } else { 0 }
            };
            if count > 0 {
                let line_len = self.current_buffer().line_len(line);
                self.current_buffer_mut().delete_range(line, 0, line, line_len);
                for (i, ch) in new_text.chars().enumerate() {
                    self.current_buffer_mut().insert_char(line, i, ch);
                }
            }
            count
        } else { 0 }
    }

    pub(super) fn open_file_finder(&mut self) {
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.fuzzy_finder = Some(FuzzyFinder::files(&base_path));
        self.mode = Mode::FuzzyFind;
    }

    pub(super) fn open_buffer_finder(&mut self) {
        let buffer_names: Vec<String> = self.buffers.iter().map(|b| b.file_name()).collect();
        self.fuzzy_finder = Some(FuzzyFinder::buffers(buffer_names));
        self.mode = Mode::FuzzyFind;
    }

    pub(super) fn open_grep_finder(&mut self, pattern: &str) {
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.fuzzy_finder = Some(FuzzyFinder::grep(&base_path, pattern));
        self.mode = Mode::FuzzyFind;
    }
}
