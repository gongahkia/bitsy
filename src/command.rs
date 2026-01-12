// Command mode implementation

use crate::error::{Error, Result};

#[derive(Debug)]
pub enum Command {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Edit(String),
    GoToLine(usize),
    Substitute { pattern: String, replacement: String, global: bool, all_lines: bool },
    Unknown(String),
}

pub fn parse_command(input: &str) -> Result<Command> {
    let input = input.trim();

    if input.is_empty() {
        return Err(Error::ParseError("Empty command".to_string()));
    }

    // Remove leading colon if present
    let input = input.strip_prefix(':').unwrap_or(input);

    match input {
        "w" | "write" => Ok(Command::Write),
        "q" | "quit" => Ok(Command::Quit),
        "wq" | "x" => Ok(Command::WriteQuit),
        "q!" => Ok(Command::ForceQuit),
        _ => {
            // Try to parse substitute command
            if input.starts_with("s/") || input.starts_with("%s/") {
                return parse_substitute(input);
            }

            // Try to parse as line number
            if let Ok(line_num) = input.parse::<usize>() {
                return Ok(Command::GoToLine(line_num));
            }

            if let Some(filename) = input.strip_prefix("e ") {
                Ok(Command::Edit(filename.trim().to_string()))
            } else if let Some(filename) = input.strip_prefix("edit ") {
                Ok(Command::Edit(filename.trim().to_string()))
            } else {
                Ok(Command::Unknown(input.to_string()))
            }
        }
    }
}

fn parse_substitute(input: &str) -> Result<Command> {
    let all_lines = input.starts_with("%s/");
    let input = if all_lines { &input[2..] } else { &input[1..] };

    // Parse s/pattern/replacement/flags
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() < 3 {
        return Err(Error::ParseError("Invalid substitute syntax".to_string()));
    }

    let pattern = parts[1].to_string();
    let replacement = parts[2].to_string();
    let flags = if parts.len() > 3 { parts[3] } else { "" };

    let global = flags.contains('g');

    Ok(Command::Substitute {
        pattern,
        replacement,
        global,
        all_lines,
    })
}
