// Bitsy - A Vim-compatible text editor written in Rust
// Module declarations

pub mod buffer;
pub mod command;
pub mod command_bar;
pub mod config;
pub mod cursor;
pub mod editor;
pub mod event;
pub mod error;
pub mod filetype;
pub mod fuzzy;
pub mod fuzzy_finder;
pub mod keymap;
pub mod lsp;
pub mod mode;
pub mod motion;
pub mod operator;
pub mod register;
pub mod screen;
pub mod selection;
pub mod syntax;
pub mod statusline;
pub mod term_pane;
pub mod terminal;
pub mod theme;
pub mod viewport;
pub mod undo;
pub mod window;

// Re-export commonly used types
pub use editor::Editor;
pub use error::{Error, Result};
