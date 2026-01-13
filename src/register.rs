// Register system for yank, delete, and clipboard operations

use std::collections::HashMap;
use arboard::Clipboard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterContent {
    Char(String),      // Character-wise content
    Line(Vec<String>), // Line-wise content
    Block(Vec<String>), // Block-wise content
}

impl RegisterContent {
    pub fn as_string(&self) -> String {
        match self {
            RegisterContent::Char(s) => s.clone(),
            RegisterContent::Line(lines) => lines.join("\n") + "\n",
            RegisterContent::Block(lines) => lines.join("\n"),
        }
    }
}

pub struct RegisterManager {
    registers: HashMap<char, RegisterContent>,
    unnamed: RegisterContent,
    last_yank: RegisterContent,
    last_delete: RegisterContent,
    clipboard: Option<Clipboard>,
    filename: String, // %
    last_command: String, // :
    last_inserted: String, // .
}

impl RegisterManager {
    pub fn new() -> Self {
        Self {
            registers: HashMap::new(),
            unnamed: RegisterContent::Char(String::new()),
            last_yank: RegisterContent::Char(String::new()),
            last_delete: RegisterContent::Char(String::new()),
            clipboard: Clipboard::new().ok(),
            filename: String::new(),
            last_command: String::new(),
            last_inserted: String::new(),
        }
    }

    pub fn set(&mut self, register: Option<char>, content: RegisterContent) {
        // Black hole register: do nothing
        if register == Some('_') {
            return;
        }

        // Read-only registers: ignore set
        if let Some(reg) = register {
            if matches!(reg, '%' | '#' | ':' | '.') {
                return;
            }
        }

        // Store in unnamed register (unless it's the black hole, which we already handled)
        self.unnamed = content.clone();

        // Store in specified register if provided
        if let Some(reg) = register {
            if reg == '+' || reg == '*' {
                if let Some(cb) = &mut self.clipboard {
                    let _ = cb.set_text(content.as_string());
                }
            }
            else {
                self.registers.insert(reg, content.clone());
            }
        }
    }

    pub fn set_yank(&mut self, register: Option<char>, content: RegisterContent) {
        self.last_yank = content.clone();
        self.set(register, content);
    }

    pub fn set_delete(&mut self, register: Option<char>, content: RegisterContent) {
        self.last_delete = content.clone();
        self.set(register, content);
    }

    pub fn get(&mut self, register: Option<char>) -> Option<RegisterContent> {
        match register {
            None => Some(self.unnamed.clone()),
            Some('0') => Some(self.last_yank.clone()),
            Some('"') => Some(self.unnamed.clone()),
            Some('_') => None, // Black hole is empty
            Some('%') => Some(RegisterContent::Char(self.filename.clone())),
            Some(':') => Some(RegisterContent::Char(self.last_command.clone())),
            Some('.') => Some(RegisterContent::Char(self.last_inserted.clone())),
            Some('#') => None, // Alternate file not implemented
            Some('+') | Some('*') => {
                if let Some(cb) = &mut self.clipboard {
                    if let Ok(text) = cb.get_text() {
                        return Some(RegisterContent::Char(text));
                    }
                }
                None
            }
            Some(reg) if ('a'..='z').contains(&reg) || ('A'..='Z').contains(&reg) => {
                self.registers.get(&reg).cloned()
            }
            _ => None,
        }
    }

    pub fn update_filename(&mut self, name: String) {
        self.filename = name;
    }

    pub fn update_last_command(&mut self, cmd: String) {
        self.last_command = cmd;
    }

    pub fn update_last_inserted(&mut self, text: String) {
        self.last_inserted = text;
    }

    pub fn get_unnamed(&self) -> &RegisterContent {
        &self.unnamed
    }
}

impl Default for RegisterManager {
    fn default() -> Self {
        Self::new()
    }
}
