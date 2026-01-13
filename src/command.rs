// Command mode implementation

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub start: usize, // 1-indexed
    pub end: usize,   // 1-indexed
}

#[derive(Debug)]
pub enum Command {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Edit(String),
    GoToLine(usize),
    Substitute { pattern: String, replacement: String, global: bool, range: Option<Range> },
    Set { option: String, value: Option<String> },
    Delete { range: Option<Range> },
    Help(Option<String>),
    BufferNext,
    BufferPrevious,
    BufferList,
    BufferDelete(Option<usize>),
    Split,
    VerticalSplit,
    CloseWindow,
    Unknown(String),
}

pub fn parse_command(input: &str) -> Result<Command> {
    let input = input.trim();

    if input.is_empty() {
        return Err(Error::ParseError("Empty command".to_string()));
    }

    // Remove leading colon if present
    let input = input.strip_prefix(':').unwrap_or(input);

    // Parse range if present
    let (range, command) = parse_range(input);

    match command {
        "w" | "write" => Ok(Command::Write),
        "q" | "quit" => Ok(Command::Quit),
        "wq" | "x" => Ok(Command::WriteQuit),
        "q!" => Ok(Command::ForceQuit),
        "d" | "delete" => Ok(Command::Delete { range }),
        _ => {
            // Try to parse substitute command
            if command.starts_with("s/") {
                return parse_substitute(command, range);
            }

            // Try to parse as line number
            if let Ok(line_num) = command.parse::<usize>() {
                return Ok(Command::GoToLine(line_num));
            }

            if let Some(filename) = command.strip_prefix("e ") {
                Ok(Command::Edit(filename.trim().to_string()))
            } else if let Some(filename) = command.strip_prefix("edit ") {
                Ok(Command::Edit(filename.trim().to_string()))
            } else if let Some(set_args) = command.strip_prefix("set ") {
                parse_set(set_args.trim())
            } else if command == "set" {
                // :set with no args - TODO: show current settings
                Ok(Command::Unknown("set".to_string()))
            } else if let Some(topic) = command.strip_prefix("help ") {
                Ok(Command::Help(Some(topic.trim().to_string())))
            } else if command == "help" || command == "h" {
                Ok(Command::Help(None))
            } else if command == "bn" || command == "bnext" {
                Ok(Command::BufferNext)
            } else if command == "bp" || command == "bprevious" {
                Ok(Command::BufferPrevious)
            } else if command == "ls" || command == "buffers" {
                Ok(Command::BufferList)
            } else if command == "bd" || command == "bdelete" {
                Ok(Command::BufferDelete(None))
            } else if let Some(buf_num) = command.strip_prefix("bd ") {
                if let Ok(num) = buf_num.trim().parse::<usize>() {
                    Ok(Command::BufferDelete(Some(num)))
                } else {
                    Ok(Command::Unknown(command.to_string()))
                }
            } else if command == "sp" || command == "split" {
                Ok(Command::Split)
            } else if command == "vsp" || command == "vsplit" {
                Ok(Command::VerticalSplit)
            } else if command == "close" || command == "clo" {
                Ok(Command::CloseWindow)
            } else {
                Ok(Command::Unknown(command.to_string()))
            }
        }
    }
}

fn parse_range(input: &str) -> (Option<Range>, &str) {
    // Handle % (all lines)
    if input.starts_with('%') {
        return (Some(Range { start: 1, end: usize::MAX }), &input[1..]);
    }

    // Handle line,line format (e.g., 1,10)
    if let Some(comma_pos) = input.find(',') {
        let before_comma = &input[..comma_pos];
        let after_comma = &input[comma_pos + 1..];

        // Find where the command starts (after the range)
        if let Some(cmd_start) = after_comma.find(|c: char| !c.is_ascii_digit()) {
            let end_str = &after_comma[..cmd_start];
            if let (Ok(start), Ok(end)) = (before_comma.parse::<usize>(), end_str.parse::<usize>()) {
                return (Some(Range { start, end }), &after_comma[cmd_start..]);
            }
        }
    }

    // Handle single line number (e.g., 10d)
    if let Some(first_non_digit) = input.find(|c: char| !c.is_ascii_digit()) {
        let line_str = &input[..first_non_digit];
        if let Ok(line) = line_str.parse::<usize>() {
            return (Some(Range { start: line, end: line }), &input[first_non_digit..]);
        }
    }

    (None, input)
}

fn parse_substitute(input: &str, range: Option<Range>) -> Result<Command> {
    let input = input.strip_prefix("s/").unwrap_or(input);

    // Parse s/pattern/replacement/flags
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() < 2 {
        return Err(Error::ParseError("Invalid substitute syntax".to_string()));
    }

    let pattern = parts[0].to_string();
    let replacement = parts[1].to_string();
    let flags = if parts.len() > 2 { parts[2] } else { "" };

    let global = flags.contains('g');

    Ok(Command::Substitute {
        pattern,
        replacement,
        global,
        range,
    })
}

fn parse_set(args: &str) -> Result<Command> {
    // Parse set command: "option" or "option=value"
    if let Some((option, value)) = args.split_once('=') {
        Ok(Command::Set {
            option: option.trim().to_string(),
            value: Some(value.trim().to_string()),
        })
    } else {
        Ok(Command::Set {
            option: args.to_string(),
            value: None,
        })
    }
}
