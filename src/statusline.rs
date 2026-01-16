// Status line rendering

use crate::cursor::Cursor;
use crate::mode::Mode;
use crossterm::style::Color;
use std::process::Command;

pub struct StatusLineComponent {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

pub struct StatusLine {
    mode: Mode,
    filename: String,
    cursor: Cursor,
    modified: bool,
    total_lines: usize,
    git_branch: Option<String>,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            filename: "[No Name]".to_string(),
            cursor: Cursor::default(),
            modified: false,
            total_lines: 1,
            git_branch: get_git_branch(),
        }
    }

    pub fn update(
        &mut self,
        mode: Mode,
        filename: &str,
        cursor: Cursor,
        modified: bool,
        total_lines: usize,
    ) {
        self.mode = mode;
        self.filename = filename.to_string();
        self.cursor = cursor;
        self.modified = modified;
        self.total_lines = total_lines;
        // Maybe refresh git branch periodically? For now, only on update.
        if self.git_branch.is_none() {
            self.git_branch = get_git_branch();
        }
    }

    pub fn render(&self, width: usize) -> Vec<StatusLineComponent> {
        let (mode_bg, mode_fg) = match self.mode {
            Mode::Normal => (Color::Blue, Color::White),
            Mode::Insert => (Color::Green, Color::Black),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => (Color::Magenta, Color::White),
            Mode::Command => (Color::DarkGrey, Color::White),
            Mode::Search => (Color::Yellow, Color::Black),
            _ => (Color::DarkGrey, Color::White),
        };

        let mut left_components = vec![
            StatusLineComponent {
                text: format!(" {} ", self.mode.as_str().to_uppercase()),
                fg: mode_fg,
                bg: mode_bg,
                bold: true,
            },
            StatusLineComponent {
                text: format!(" {} ", self.filename),
                fg: Color::White,
                bg: Color::DarkGrey,
                bold: false,
            },
        ];

        if self.modified {
            left_components.push(StatusLineComponent {
                text: " [+] ".to_string(),
                fg: Color::Yellow,
                bg: Color::DarkGrey,
                bold: true,
            });
        }

        if let Some(branch) = &self.git_branch {
            left_components.push(StatusLineComponent {
                text: format!("  {} ", branch), // Nerd Font icon
                fg: Color::White,
                bg: Color::DarkMagenta,
                bold: false,
            });
        }

        let position = format!("{}:{}", self.cursor.line + 1, self.cursor.col + 1);
        let percentage = if self.total_lines > 0 {
            ((self.cursor.line + 1) * 100 / self.total_lines).min(100)
        } else {
            0
        };

        let right_components = vec![
            StatusLineComponent {
                text: format!(" {}% ", percentage),
                fg: mode_fg,
                bg: mode_bg,
                bold: true,
            },
            StatusLineComponent {
                text: format!(" {} ", position),
                fg: Color::White,
                bg: Color::DarkGrey,
                bold: false,
            },
        ];

        let left_len: usize = left_components.iter().map(|c| c.text.len()).sum();
        let right_len: usize = right_components.iter().map(|c| c.text.len()).sum();
        let padding_len = width.saturating_sub(left_len + right_len);

        let mut components = left_components;
        components.push(StatusLineComponent {
            text: " ".repeat(padding_len),
            fg: Color::White,
            bg: Color::DarkGrey,
            bold: false,
        });
        components.extend(right_components);

        components
    }
}

fn get_git_branch() -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}
