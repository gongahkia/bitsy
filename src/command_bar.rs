// Renders the command bar at the bottom of the editor

use crate::mode::Mode;

pub struct CommandBar {
    // Colors, styles can be added here later
}

impl CommandBar {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(
        &self,
        width: usize,
        mode: Mode,
        command_buffer: &str,
        search_buffer: &str,
        search_forward: bool,
        message: &Option<String>,
    ) -> String {
        let mut output = String::new();

        let content = if mode == Mode::Command {
            format!(":{}", command_buffer)
        } else if mode == Mode::Search {
            if search_forward {
                format!("/{}", search_buffer)
            } else {
                format!("?{}", search_buffer)
            }
        } else if let Some(msg) = message {
            msg.clone()
        } else {
            String::new()
        };

        output.push_str(&content);

        // Fill the rest of the line with spaces
        if output.len() < width {
            output.push_str(&" ".repeat(width - output.len()));
        }

        output
    }
}
