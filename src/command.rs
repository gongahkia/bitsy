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
