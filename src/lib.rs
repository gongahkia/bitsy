// Bitsy - A Vim-compatible text editor written in Rust
// Module declarations

pub mod buffer;
pub mod command;
pub mod cursor;
pub mod editor;
pub mod error;
pub mod keymap;
pub mod mode;
pub mod motion;
pub mod operator;
pub mod statusline;
pub mod terminal;
pub mod viewport;

// Re-export commonly used types
pub use editor::Editor;
pub use error::{Error, Result};
