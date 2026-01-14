// Status line rendering

use crate::buffer::Buffer;
use crate::cursor::Cursor;
use crate::mode::Mode;
use crate::terminal::Terminal;
use crossterm::queue;
use crossterm::style::{Color, SetBackgroundColor, SetForegroundColor};
use std::process::Command as OsCommand;

pub struct StatusLine;

impl StatusLine {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        terminal: &mut Terminal,
        mode: Mode,
        buffer: &Buffer,
        cursor: &Cursor,
        is_pending_operator: bool,
        recording_macro: Option<char>,
    ) -> Result<(), std::io::Error> {
        let (width, height) = terminal.size();
        let line_y = height - 2;

        let (mode_str, bg_color) = match mode {
            Mode::Normal => ("NORMAL", Color::Blue),
            Mode::Insert => ("INSERT", Color::Green),
            Mode::Visual => ("VISUAL", Color::Yellow),
            Mode::VisualLine => ("V-LINE", Color::Yellow),
            Mode::VisualBlock => ("V-BLOCK", Color::Yellow),
            Mode::Command => ("COMMAND", Color::Magenta),
            Mode::Search => ("SEARCH", Color::Cyan),
            Mode::Replace => ("REPLACE", Color::Red),
        };

        let modified_indicator = if buffer.is_modified() { "[+]" } else { "" };
        let filename = buffer.file_name();

        let position = format!("{}:{}", cursor.line + 1, cursor.col + 1);
        let percentage = if buffer.line_count() > 0 {
            ((cursor.line + 1) * 100 / buffer.line_count()).min(100)
        } else {
            0
        };
        let position_str = format!("{} {}%", position, percentage);

        let git_branch = get_git_branch().unwrap_or_default();

        let operator_str = if is_pending_operator { "..." } else { "" };
        let recording_str = if let Some(reg) = recording_macro {
            format!("rec @{}", reg)
        } else {
            "".to_string()
        };

        // Left side
        let left_part = format!(" {} ", mode_str);
        let mid_part = format!(" {}{}", filename, modified_indicator);
        let git_part = if !git_branch.is_empty() {
            format!("  {} ", git_branch)
        } else {
            "".to_string()
        };

        // Right side
        let right_part = format!(" {} ", position_str);
        let recording_part = if !recording_str.is_empty() {
            format!(" {} ", recording_str)
        } else {
            "".to_string()
        };

        let total_len = left_part.len()
            + mid_part.len()
            + git_part.len()
            + right_part.len()
            + recording_part.len()
            + operator_str.len();
        let padding = " ".repeat(width.saturating_sub(total_len));

        // --- Rendering ---
        queue!(
            terminal.stdout,
            crossterm::cursor::MoveTo(0, line_y),
            // Mode
            SetBackgroundColor(bg_color),
            SetForegroundColor(Color::Black),
            crossterm::style::Print(&left_part),
            // Separator
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(bg_color),
            crossterm::style::Print(""),
            // Filename
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::White),
            crossterm::style::Print(&mid_part),
            // Git branch
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::Cyan),
            crossterm::style::Print(&git_part),
            // Padding
            SetBackgroundColor(Color::DarkGrey),
            crossterm::style::Print(&padding),
            // Recording
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::Red),
            crossterm::style::Print(&recording_part),
            // Operator pending
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::Yellow),
            crossterm::style::Print(operator_str),
            // Separator
            SetBackgroundColor(bg_color),
            SetForegroundColor(Color::DarkGrey),
            crossterm::style::Print(""),
            // Position
            SetBackgroundColor(bg_color),
            SetForegroundColor(Color::Black),
            crossterm::style::Print(&right_part),
            // Reset
            crossterm::style::ResetColor
        )?;

        Ok(())
    }
}

fn get_git_branch() -> Option<String> {
    let output = OsCommand::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}
