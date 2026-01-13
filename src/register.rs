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
}

impl RegisterManager {
    pub fn new() -> Self {
        Self {
            registers: HashMap::new(),
            unnamed: RegisterContent::Char(String::new()),
            last_yank: RegisterContent::Char(String::new()),
            last_delete: RegisterContent::Char(String::new()),
            clipboard: Clipboard::new().ok(),
        }
    }

    pub fn set(&mut self, register: Option<char>, content: RegisterContent) {
        // Store in unnamed register
        self.unnamed = content.clone();

        // Store in specified register if provided
        if let Some(reg) = register {
            if reg == '+' || reg == '*' {
                if let Some(cb) = &mut self.clipboard {
                    let _ = cb.set_text(content.as_string());
                }
            } else {
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

    pub fn get_unnamed(&self) -> &RegisterContent {
        &self.unnamed
    }
}

impl Default for RegisterManager {
    fn default() -> Self {
        Self::new()
    }
}
