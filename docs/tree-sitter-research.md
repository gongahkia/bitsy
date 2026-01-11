# Tree-sitter Research for Syntax Highlighting Integration

## Overview

Tree-sitter is an incremental parsing library that can be used to build syntax highlighting, code navigation, and other editor features. This document contains research findings for integrating tree-sitter into Bitsy.

## What is Tree-sitter?

Tree-sitter is a parser generator tool and an incremental parsing library. It builds a concrete syntax tree for a source file and efficiently updates that syntax tree as the source file is edited. Key features include:

- **Incremental parsing**: Updates syntax trees in less than a millisecond after edits
- **Error recovery**: Produces syntax trees even for files with syntax errors
- **Query-based highlighting**: Uses declarative pattern-matching queries
- **Multiple language support**: Supports 100+ programming languages

## Rust Crates

### Core Crate: `tree-sitter`

Latest version: 0.24

```toml
[dependencies]
tree-sitter = "0.24"
```

This is the main library for parsing with tree-sitter. It provides:
- Parser creation and configuration
- Incremental parsing support
- Tree querying capabilities
- Language loading

**Documentation**: https://docs.rs/tree-sitter

### Highlighting Crate: `tree-sitter-highlight`

Latest version: 0.25.4

```toml
[dependencies]
tree-sitter-highlight = "0.25"
```

This crate provides syntax highlighting functionality:
- `Highlighter` struct for performing syntax highlighting
- `HighlightConfiguration` for loading highlight queries
- Recognition of highlight names (keywords, functions, etc.)
- Language injection support for embedded languages

**Documentation**: https://docs.rs/tree-sitter-highlight/latest/tree_sitter_highlight/

### Language-Specific Crates

Each language requires a separate grammar crate:

```toml
[dependencies]
tree-sitter-rust = "0.23"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-go = "0.23"
tree-sitter-c = "0.23"
# ... etc
```

## Integration Architecture

### 1. Parser Setup

```rust
use tree_sitter::{Parser, Language};

// Create parser
let mut parser = Parser::new();

// Set language
parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
```

### 2. Incremental Parsing

```rust
// Initial parse
let tree = parser.parse(source_code, None)?;

// After edit, reparse incrementally
let new_tree = parser.parse(new_source_code, Some(&tree))?;
```

### 3. Syntax Highlighting

```rust
use tree_sitter_highlight::{Highlighter, HighlightConfiguration};

let mut highlighter = Highlighter::new();

// Load highlight configuration
let config = HighlightConfiguration::new(
    language,
    highlight_query,
    injection_query,
    locals_query,
)?;

// Perform highlighting
let highlights = highlighter.highlight(&config, source_code, None, |_| None)?;
```

### 4. Query Files

Tree-sitter uses three types of query files (`.scm` files):

- **highlights.scm**: Defines which syntax nodes should be highlighted and how
- **injections.scm**: Defines embedded language regions (e.g., JavaScript in HTML)
- **locals.scm**: Defines scopes for semantic highlighting

Example highlights.scm:
```scheme
; Keywords
(keyword) @keyword

; Functions
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function.call)

; Types
(type_identifier) @type

; Strings
(string) @string

; Comments
(comment) @comment
```

## Integration Plan for Bitsy

### Phase 1: Basic Integration
1. Add `tree-sitter` and `tree-sitter-highlight` dependencies
2. Create a `syntax` module for handling language grammars
3. Implement basic highlighting for a single language (Rust)
4. Store highlight queries in `queries/` directory

### Phase 2: File Type Detection
1. Implement file extension to language mapping
2. Support shebang detection
3. Support modeline detection
4. Auto-detect language on file open

### Phase 3: Incremental Parsing
1. Hook into buffer modification events
2. Update syntax tree on edits
3. Cache syntax trees per buffer
4. Optimize for large files

### Phase 4: Multi-Language Support
1. Add grammars for common languages (Python, JS, Go, C, etc.)
2. Support language injection (embedded languages)
3. Lazy-load grammars to reduce binary size

### Phase 5: Advanced Features
1. Implement semantic highlighting
2. Add code folding based on syntax tree
3. Support rainbow brackets
4. Add custom query language support

## Performance Considerations

- **Lazy Loading**: Load grammars only when needed
- **Background Parsing**: Parse in a separate thread to avoid blocking UI
- **Incremental Updates**: Use tree-sitter's incremental parsing for edits
- **Viewport Culling**: Only highlight visible portions for large files
- **Caching**: Cache parsed trees and highlight results

## Memory Requirements

- Typical syntax tree size: ~1-2x source file size
- Each language grammar: ~500KB-2MB
- Highlight queries: ~10-50KB per language

## Alternative Approaches Considered

1. **Regex-based highlighting** (rejected: not accurate, hard to maintain)
2. **TextMate grammars** (rejected: slower, less accurate than tree-sitter)
3. **Language servers** (future: can complement tree-sitter)

## Recommended Dependencies

```toml
# Core parsing
tree-sitter = "0.24"
tree-sitter-highlight = "0.25"

# Common language grammars
tree-sitter-rust = "0.23"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-go = "0.23"
tree-sitter-c = "0.23"
tree-sitter-cpp = "0.23"
tree-sitter-json = "0.23"
tree-sitter-toml = "0.23"
tree-sitter-yaml = "0.23"
tree-sitter-markdown = "0.23"
```

## References

- [Tree-sitter Official Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Syntax Highlighting Guide](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html)
- [tree-sitter crate on crates.io](https://crates.io/crates/tree-sitter)
- [tree-sitter-highlight crate on crates.io](https://crates.io/crates/tree-sitter-highlight)
- [Rust API Documentation](https://docs.rs/tree-sitter)
- [Incremental Parsing Tutorial (Strumenta)](https://tomassetti.me/incremental-parsing-using-tree-sitter/)
- [Tree-sitter GitHub Repository](https://github.com/tree-sitter/tree-sitter)
- [Tree-sitter Rust Grammar](https://github.com/tree-sitter/tree-sitter-rust)

## Implementation Timeline

This research sets the foundation for **Phase 16: Syntax Highlighting** in the PRD, which includes 10 tasks for full tree-sitter integration. The research shows that tree-sitter is the right choice for Bitsy due to its performance, accuracy, and active maintenance.
