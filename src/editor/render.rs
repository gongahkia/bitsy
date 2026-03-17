// rendering via double-buffered cell grid with webspinner theme

use crossterm::cursor::SetCursorStyle;
use crossterm::style::Color;
use crate::config::LineNumberMode;
use crate::error::Result;
use crate::mode::Mode;
use super::Editor;

impl Editor {
    pub(super) fn render(&mut self) -> Result<()> {
        self.terminal.hide_cursor()?;
        let bg = self.theme.bg;
        self.screen.clear(bg);
        if self.mode == Mode::FuzzyFind && self.fuzzy_finder.is_some() {
            self.render_fuzzy_finder_cells();
        } else {
            self.render_buffer_cells();
        }
        self.render_status_line_cells();
        self.render_command_line_cells();
        self.screen.flush().map_err(|e| crate::error::Error::EditorError(e.to_string()))?;

        // cursor style per mode
        use std::io;
        match self.mode {
            Mode::Insert => { crossterm::execute!(io::stdout(), SetCursorStyle::BlinkingBar)?; }
            Mode::Replace => { crossterm::execute!(io::stdout(), SetCursorStyle::BlinkingUnderScore)?; }
            _ => { crossterm::execute!(io::stdout(), SetCursorStyle::BlinkingBlock)?; }
        }

        // position cursor
        if self.mode == Mode::FuzzyFind {
            if let Some(ref finder) = self.fuzzy_finder {
                let prompt_len = finder.prompt().len();
                let cursor_col = prompt_len + finder.query.len();
                self.terminal.move_cursor(cursor_col as u16, 0)?;
            }
        } else {
            let win_rect = self.windows[self.active_window].rect;
            let (padding, _) = if self.zen_mode {
                let zen_width = self.config.zen_mode_width;
                let padding = win_rect.width.saturating_sub(zen_width) / 2;
                (padding, zen_width)
            } else { (0, win_rect.width) };
            let line_num_width = self.config.line_number_width(self.current_buffer().line_count());
            let gutter_extra = if line_num_width > 0 { 1 } else { 0 };
            let screen_row = win_rect.y + self.current_window().cursor.line
                .saturating_sub(self.current_window().viewport.offset_line);
            let screen_col = win_rect.x + padding + line_num_width + gutter_extra
                + self.current_window().cursor.col
                    .saturating_sub(self.current_window().viewport.offset_col);
            self.terminal.move_cursor(screen_col as u16, screen_row as u16)?;
        }
        self.terminal.show_cursor()?;
        self.terminal.flush()?;
        Ok(())
    }

    fn render_fuzzy_finder_cells(&mut self) {
        let width = self.screen.width;
        let height = self.screen.height.saturating_sub(2);
        let bg = self.theme.bg;
        let fg = self.theme.fg;
        let prompt_fg = self.theme.finder_prompt_fg;
        let sel_bg = self.theme.finder_selected_bg;
        if let Some(ref finder) = self.fuzzy_finder {
            // render prompt + query on row 0
            let prompt = finder.prompt();
            self.screen.put_str(0, 0, prompt, prompt_fg, bg);
            self.screen.put_str(0, prompt.len(), &finder.query, fg, bg);
            // render matches
            let matches = finder.visible_matches();
            for (i, m) in matches.iter().take(height.saturating_sub(1)).enumerate() {
                let row = i + 1;
                let is_selected = i == finder.selected_index;
                let row_bg = if is_selected { sel_bg } else { bg };
                let item = &m.item;
                self.screen.put_str(row, 0, item, fg, row_bg);
                // fill rest of row
                for col in item.chars().count()..width {
                    self.screen.put_char(row, col, ' ', fg, row_bg);
                }
            }
        }
    }

    fn render_buffer_cells(&mut self) {
        let (term_width, term_height) = self.terminal.size();
        let viewport_height = (term_height as usize).saturating_sub(2);
        let bounding = crate::window::Rect { x: 0, y: 0, width: term_width as usize, height: viewport_height };
        let rects = self.layout.calculate_rects(bounding);
        for (win_idx, rect) in rects.clone() {
            self.render_single_window(win_idx, rect);
        }
        self.draw_window_separators(&rects, bounding);
    }

    fn render_single_window(&mut self, win_idx: usize, rect: crate::window::Rect) {
        let bg = self.theme.bg;
        let buf_idx = self.windows[win_idx].buffer_index;
        let cursor_line = self.windows[win_idx].cursor.line;
        let offset_line = self.windows[win_idx].viewport.offset_line;
        let offset_col = self.windows[win_idx].viewport.offset_col;

        let (padding, text_width) = if self.zen_mode {
            let zen_width = self.config.zen_mode_width;
            let padding = rect.width.saturating_sub(zen_width) / 2;
            (padding, zen_width.min(rect.width))
        } else { (0, rect.width) };

        let line_num_width = self.config.line_number_width(self.buffers[buf_idx].line_count());
        let gutter_extra = if line_num_width > 0 { 1 } else { 0 };
        let effective_text_width = text_width.saturating_sub(line_num_width + gutter_extra);

        let search_pattern = self.search_pattern.clone();
        let sub_pattern = self.substitute_preview_pattern.clone();
        let sub_range = self.substitute_preview_range;

        // syntax highlighting
        let file_ext = self.buffers[buf_idx].file_path()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let syntax_colors = if self.syntax.supports(&file_ext) {
            let margin = 5;
            let vis_start = offset_line.saturating_sub(margin);
            let vis_end = (offset_line + rect.height + margin).min(self.buffers[buf_idx].line_count());
            let mut visible_text = String::new();
            let mut line_byte_offsets: Vec<usize> = Vec::new();
            for l in vis_start..vis_end {
                line_byte_offsets.push(visible_text.len());
                if let Some(lt) = self.buffers[buf_idx].get_line(l) {
                    visible_text.push_str(&lt);
                }
                visible_text.push('\n');
            }
            let spans = self.syntax.highlight(&file_ext, visible_text.as_bytes());
            let theme = &self.theme;
            let mut color_map: Vec<Vec<Option<Color>>> = Vec::new();
            let vis_chars: Vec<char> = visible_text.chars().collect();
            let mut byte_to_char = vec![0usize; visible_text.len() + 1];
            let mut byte_pos = 0;
            for (ci, ch) in vis_chars.iter().enumerate() {
                let len = ch.len_utf8();
                for b in 0..len { byte_to_char[byte_pos + b] = ci; }
                byte_pos += len;
            }
            if byte_pos < byte_to_char.len() { byte_to_char[byte_pos] = vis_chars.len(); }
            for l in vis_start..vis_end {
                let rel = l - vis_start;
                if let Some(lt) = self.buffers[buf_idx].get_line(l) {
                    let char_count = lt.chars().count();
                    let mut colors = vec![None; char_count];
                    let line_byte_start = line_byte_offsets.get(rel).copied().unwrap_or(0);
                    let line_byte_end = line_byte_offsets.get(rel + 1).copied()
                        .unwrap_or(visible_text.len()).saturating_sub(1);
                    for &(bs, be, hi) in &spans {
                        if be <= line_byte_start || bs >= line_byte_end { continue; }
                        let cs = byte_to_char.get(bs.max(line_byte_start)).copied().unwrap_or(0);
                        let ce = byte_to_char.get(be.min(line_byte_end)).copied().unwrap_or(char_count);
                        let line_char_start = byte_to_char.get(line_byte_start).copied().unwrap_or(0);
                        let color = crate::syntax::highlight_color(hi, theme);
                        for ci in cs..ce {
                            let local = ci.saturating_sub(line_char_start);
                            if local < char_count { colors[local] = Some(color); }
                        }
                    }
                    color_map.push(colors);
                } else {
                    color_map.push(Vec::new());
                }
            }
            Some((vis_start, color_map))
        } else { None };

        for row in 0..rect.height {
            let screen_row = rect.y + row;
            let file_line = offset_line + row;
            if file_line < self.buffers[buf_idx].line_count() {
                self.render_line_number_cells_at(screen_row, rect.x + padding, file_line, line_num_width, cursor_line);
                if gutter_extra > 0 {
                    let sep_col = rect.x + padding + line_num_width;
                    self.screen.put_char(screen_row, sep_col, '\u{2502}', self.theme.gutter_separator, bg);
                }
                if let Some(line) = self.buffers[buf_idx].get_line(file_line) {
                    let text_start_col = rect.x + padding + line_num_width + gutter_extra;
                    let line_colors = syntax_colors.as_ref().and_then(|(vis_start, cmap)| {
                        let idx = file_line.checked_sub(*vis_start)?;
                        cmap.get(idx)
                    });
                    self.render_line_content_cells(
                        screen_row, text_start_col, file_line, &line, offset_col,
                        effective_text_width, &search_pattern, &sub_pattern, sub_range,
                        line_colors,
                    );
                    if file_line == cursor_line && self.config.show_current_line && win_idx == self.active_window {
                        let cur_bg = self.theme.current_line_bg;
                        for col in text_start_col..text_start_col + effective_text_width {
                            if col < self.screen.width {
                                if self.screen.cells[screen_row][col].bg == bg {
                                    self.screen.cells[screen_row][col].bg = cur_bg;
                                }
                            }
                        }
                    }
                }
            } else {
                let tilde_col = rect.x + padding + line_num_width + gutter_extra;
                if tilde_col < self.screen.width {
                    self.screen.put_char(screen_row, tilde_col, '~', self.theme.tilde_fg, bg);
                }
            }
        }
    }

    fn draw_window_separators(&mut self, rects: &[(usize, crate::window::Rect)], _bounding: crate::window::Rect) {
        let bg = self.theme.bg;
        let sep_color = self.theme.gutter_separator;
        let active_sep = self.theme.accent;
        for i in 0..rects.len() {
            let (wi, ri) = &rects[i];
            for j in (i + 1)..rects.len() {
                let (wj, rj) = &rects[j];
                let is_active = *wi == self.active_window || *wj == self.active_window;
                let color = if is_active { active_sep } else { sep_color };
                // vertical separator: right edge of ri == left edge of rj (side by side)
                if ri.x + ri.width < rj.x && rj.x.saturating_sub(ri.x + ri.width) <= 1 {
                    let col = ri.x + ri.width;
                    let y_start = ri.y.max(rj.y);
                    let y_end = (ri.y + ri.height).min(rj.y + rj.height);
                    for row in y_start..y_end {
                        if col < self.screen.width && row < self.screen.height {
                            self.screen.put_char(row, col, '\u{2502}', color, bg);
                        }
                    }
                }
                // horizontal separator: bottom of ri == top of rj (stacked)
                if ri.y + ri.height < rj.y && rj.y.saturating_sub(ri.y + ri.height) <= 1 {
                    let row = ri.y + ri.height;
                    let x_start = ri.x.max(rj.x);
                    let x_end = (ri.x + ri.width).min(rj.x + rj.width);
                    for col in x_start..x_end {
                        if col < self.screen.width && row < self.screen.height {
                            self.screen.put_char(row, col, '\u{2500}', color, bg);
                        }
                    }
                }
            }
        }
    }

    fn render_line_number_cells_at(&mut self, row: usize, col_start: usize, line: usize, width: usize, cursor_line: usize) {
        if width == 0 { return; }
        let bg = self.theme.bg;
        let number = match self.config.line_numbers {
            LineNumberMode::None => return,
            LineNumberMode::Absolute => format!("{:>w$}", line + 1, w = width - 1),
            LineNumberMode::Relative | LineNumberMode::RelativeAbsolute => {
                let distance = if line == cursor_line {
                    line + 1
                } else {
                    (line as isize - cursor_line as isize).abs() as usize
                };
                format!("{:>w$}", distance, w = width - 1)
            }
        };
        let is_current = line == cursor_line && self.config.show_current_line;
        let color = if is_current { self.theme.gutter_current } else { self.theme.gutter_fg };
        self.screen.put_str(row, col_start, &number, color, bg);
        self.screen.put_char(row, col_start + width - 1, ' ', color, bg);
    }

    fn render_line_content_cells(
        &mut self,
        row: usize,
        start_screen_col: usize,
        file_line: usize,
        line_text: &str,
        offset_col: usize,
        available_width: usize,
        search_pattern: &Option<String>,
        sub_pattern: &Option<String>,
        sub_range: Option<(usize, usize)>,
        syntax_colors: Option<&Vec<Option<Color>>>,
    ) {
        let bg = self.theme.bg;
        let fg = self.theme.fg;
        let chars: Vec<char> = line_text.chars().collect();

        let search_matches: Vec<(usize, usize)> = if self.config.highlight_search {
            search_pattern.as_ref().map_or(Vec::new(), |p| find_all_matches(&chars, p))
        } else { Vec::new() };

        let substitute_matches: Vec<(usize, usize)> = sub_pattern.as_ref().map_or(Vec::new(), |p| {
            if let Some((sl, el)) = sub_range {
                if file_line >= sl && file_line <= el { find_all_matches(&chars, p) }
                else { Vec::new() }
            } else { Vec::new() }
        });

        let in_visual = matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock);
        let selection_ref = if in_visual { self.selection.as_ref() } else { None };

        for (i, &ch) in chars.iter().enumerate().skip(offset_col).take(available_width) {
            let screen_col = start_screen_col + (i - offset_col);
            if screen_col >= self.screen.width { break; }
            let is_selected = selection_ref.map_or(false, |s| s.contains(file_line, i));
            let is_sub = substitute_matches.iter().any(|(s, e)| i >= *s && i < *e);
            let is_search = search_matches.iter().any(|(s, e)| i >= *s && i < *e);

            let (cell_fg, cell_bg) = if is_selected {
                (self.theme.selection_fg, self.theme.selection_bg)
            } else if is_sub {
                (self.theme.substitute_fg, self.theme.substitute_bg)
            } else if is_search {
                (self.theme.search_match_fg, self.theme.search_match_bg)
            } else {
                // apply syntax color if available
                let syn_fg = syntax_colors
                    .and_then(|colors| colors.get(i).copied().flatten())
                    .unwrap_or(fg);
                (syn_fg, bg)
            };
            self.screen.put_char(row, screen_col, ch, cell_fg, cell_bg);
        }
    }

    fn render_status_line_cells(&mut self) {
        let height = self.screen.height;
        let width = self.screen.width;
        let row = height.saturating_sub(2);
        if row >= height { return; }

        let filename = self.current_buffer().file_name();
        let total_lines = self.current_buffer().line_count();
        let cursor = self.current_window().cursor;
        let modified = self.current_buffer().is_modified();
        let read_only = self.current_buffer().is_read_only();
        let file_type = self.current_buffer().file_type().as_str();
        self.statusline.update(self.mode, &filename, file_type, cursor, modified, read_only, total_lines);

        // mode segment
        let (mode_bg, mode_fg) = self.mode_colors();
        let mode_icon = match self.mode {
            Mode::Normal => "\u{f0b0e}", // spider icon placeholder, use N
            _ => "",
        };
        let mode_text = format!(" {}{} ", mode_icon, self.mode.as_str().to_uppercase());
        let sep = " // "; // web-silk separator

        // build segments
        let mut col = 0;
        // mode
        self.screen.put_str_bold(row, col, &mode_text, mode_fg, mode_bg);
        col += mode_text.len();
        // separator
        self.screen.put_str(row, col, sep, self.theme.accent, self.theme.statusline_bg);
        col += sep.len();
        // filename
        let fname = format!("{}", filename);
        self.screen.put_str(row, col, &fname, self.theme.statusline_fg, self.theme.statusline_bg);
        col += fname.len();
        // modified indicator
        if modified {
            let mod_text = " [+]";
            self.screen.put_str(row, col, mod_text, self.theme.warning, self.theme.statusline_bg);
            col += mod_text.len();
        }
        if read_only {
            let ro_text = " [RO]";
            self.screen.put_str(row, col, ro_text, self.theme.error, self.theme.statusline_bg);
            col += ro_text.len();
        }
        // git branch from statusline
        if let Some(branch) = self.statusline.git_branch() {
            let git_text = format!(" // \u{e0a0} {}", branch);
            self.screen.put_str(row, col, &git_text, self.theme.accent, self.theme.statusline_bg);
            col += git_text.len();
        }

        // right side
        let position = format!("{}:{}", cursor.line + 1, cursor.col + 1);
        let percentage = if total_lines > 0 {
            ((cursor.line + 1) * 100 / total_lines).min(100)
        } else { 0 };
        let right_text = format!("{} // {}% // {} ", file_type, percentage, position);
        let right_start = width.saturating_sub(right_text.len());

        // fill middle with statusline bg
        for c in col..right_start {
            if c < width { self.screen.put_char(row, c, ' ', self.theme.statusline_fg, self.theme.statusline_bg); }
        }
        // right segments
        self.screen.put_str(row, right_start, &right_text, self.theme.statusline_fg, self.theme.statusline_bg);
    }

    fn render_command_line_cells(&mut self) {
        let height = self.screen.height;
        let width = self.screen.width;
        let row = height.saturating_sub(1);
        if row >= height { return; }
        let bg = self.theme.statusline_bg;
        let fg = self.theme.fg;

        let text = match self.mode {
            Mode::Command => format!(":{}", self.command_buffer),
            Mode::Search => {
                let prefix = if self.search_forward { "/" } else { "?" };
                format!("{}{}", prefix, self.search_buffer)
            }
            _ => {
                if let Some(ref msg) = self.message { msg.clone() }
                else { String::new() }
            }
        };
        self.screen.put_str(row, 0, &text, fg, bg);
        for c in text.len()..width {
            self.screen.put_char(row, c, ' ', fg, bg);
        }
        // recording indicator
        if let Some(reg) = self.recording_register {
            let rec_text = format!("recording @{}", reg);
            let rec_start = width.saturating_sub(rec_text.len() + 1);
            self.screen.put_str(row, rec_start, &rec_text, self.theme.error, bg);
        }
    }

    fn mode_colors(&self) -> (Color, Color) {
        match self.mode {
            Mode::Normal => (self.theme.mode_normal_bg, self.theme.mode_normal_fg),
            Mode::Insert => (self.theme.mode_insert_bg, self.theme.mode_insert_fg),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => (self.theme.mode_visual_bg, self.theme.mode_visual_fg),
            Mode::Command => (self.theme.mode_command_bg, self.theme.mode_command_fg),
            Mode::Search => (self.theme.mode_search_bg, self.theme.mode_search_fg),
            _ => (self.theme.statusline_bg, self.theme.statusline_fg),
        }
    }

    pub(super) fn find_all_matches_in_line(&self, line_text: &str, pattern: &str) -> Vec<(usize, usize)> {
        let chars: Vec<char> = line_text.chars().collect();
        find_all_matches(&chars, pattern)
    }

    pub(super) fn update_substitute_preview(&mut self) {
        self.substitute_preview_pattern = None;
        self.substitute_preview_range = None;
        if self.mode != Mode::Command || self.command_buffer.is_empty() { return; }
        let cmd = &self.command_buffer;
        let (range, rest) = if cmd.starts_with('%') {
            let line_count = self.current_buffer().line_count();
            (Some((0, line_count.saturating_sub(1))), &cmd[1..])
        } else if let Some(comma_pos) = cmd.find(',') {
            let before_comma = &cmd[..comma_pos];
            let after_comma = &cmd[comma_pos + 1..];
            if let Some(cmd_start) = after_comma.find(|c: char| !c.is_ascii_digit()) {
                let end_str = &after_comma[..cmd_start];
                if let (Ok(start), Ok(end)) = (before_comma.parse::<usize>(), end_str.parse::<usize>()) {
                    (Some((start.saturating_sub(1), end.saturating_sub(1))), &after_comma[cmd_start..])
                } else { (None, cmd.as_str()) }
            } else { (None, cmd.as_str()) }
        } else if let Some(first_non_digit) = cmd.find(|c: char| !c.is_ascii_digit()) {
            let line_str = &cmd[..first_non_digit];
            if let Ok(line) = line_str.parse::<usize>() {
                (Some((line.saturating_sub(1), line.saturating_sub(1))), &cmd[first_non_digit..])
            } else { (None, cmd.as_str()) }
        } else { (None, cmd.as_str()) };
        if rest.starts_with("s/") {
            let pattern_start = 2;
            let default_range = self.visual_cmd_range.unwrap_or((
                self.current_window().cursor.line, self.current_window().cursor.line,
            ));
            if let Some(pattern_end) = rest[pattern_start..].find('/') {
                let pattern = &rest[pattern_start..pattern_start + pattern_end];
                if !pattern.is_empty() {
                    self.substitute_preview_pattern = Some(pattern.to_string());
                    self.substitute_preview_range = range.or(Some(default_range));
                }
            } else if rest.len() > 2 {
                let pattern = &rest[pattern_start..];
                if !pattern.is_empty() {
                    self.substitute_preview_pattern = Some(pattern.to_string());
                    self.substitute_preview_range = range.or(Some(default_range));
                }
            }
        }
    }

    pub(super) fn clear_substitute_preview(&mut self) {
        self.substitute_preview_pattern = None;
        self.substitute_preview_range = None;
        self.visual_cmd_range = None;
    }
}

fn find_all_matches(chars: &[char], pattern: &str) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    if pattern.is_empty() { return matches; }
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i + pattern_chars.len() <= chars.len() {
        let slice: String = chars[i..i + pattern_chars.len()].iter().collect();
        if slice == pattern {
            matches.push((i, i + pattern_chars.len()));
            i += pattern_chars.len();
        } else { i += 1; }
    }
    matches
}
