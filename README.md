# `Bitsy`

A Vim-compatible text editor written in Rust.

## Features

Bitsy is a terminal-based text editor that aims to provide a familiar Vim-like editing experience with modern features.

### Current Features (Minimal Viable Editor)
- Vim modal editing (Normal, Insert, Visual modes)
- Basic text editing operations
- File loading and saving
- Status line with mode indicator
- Command mode (`:w`, `:q`, `:e`, etc.)
- Basic Vim motions (h, j, k, l, w, b, 0, $, gg, G)
- Efficient text buffer using rope data structure

## Building

### Prerequisites
- Rust 1.70 or later
- Cargo (comes with Rust)

### Build Instructions

```bash
# Clone or navigate to the repository
cd bitsy

# Build in debug mode
cargo build

# Build in release mode (optimized)
cargo build --release

# Run directly
cargo run

# Run with a file
cargo run -- filename.txt

# Install locally
cargo install --path .
```

## Usage

```bash
# Start with empty buffer
bitsy

# Open a file
bitsy myfile.txt
```

### Keyboard Commands

#### Normal Mode
- `h`, `j`, `k`, `l` - Move left, down, up, right
- `w`, `b` - Move forward/backward by word
- `0`, `$` - Move to start/end of line
- `gg`, `G` - Move to start/end of file
- `i` - Enter insert mode at cursor
- `I` - Enter insert mode at line start
- `a` - Enter insert mode after cursor
- `A` - Enter insert mode at line end
- `o` - Insert new line below and enter insert mode
- `O` - Insert new line above and enter insert mode
- `v` - Enter visual mode
- `V` - Enter visual line mode
- `x` - Delete character under cursor
- `:` - Enter command mode

#### Insert Mode
- `Esc` - Return to normal mode
- Type normally to insert text
- Arrow keys for navigation
- `Backspace` - Delete previous character

#### Command Mode
- `:w` - Save file
- `:q` - Quit (warns if unsaved changes)
- `:wq` or `:x` - Save and quit
- `:q!` - Force quit without saving
- `:e <filename>` - Open file

## Project Structure

```
src/
├── main.rs         # Entry point
├── lib.rs          # Module declarations
├── editor.rs       # Main editor coordination
├── terminal.rs     # Terminal handling
├── buffer.rs       # Text buffer (rope-based)
├── cursor.rs       # Cursor tracking
├── mode.rs         # Vim mode state machine
├── keymap.rs       # Key mapping
├── viewport.rs     # Viewport and scrolling
├── statusline.rs   # Status line rendering
├── command.rs      # Command parsing
├── motion.rs       # Vim motions (future)
└── operator.rs     # Vim operators (future)
```

## Development

### Running Tests
```bash
cargo test
```

### Linting
```bash
cargo clippy
```

### Formatting
```bash
cargo fmt
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.
