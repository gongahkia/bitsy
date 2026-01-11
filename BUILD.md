# Cross-Platform Build Guide

This document describes how to build Bitsy for different platforms.

## Supported Platforms

- Linux (x86_64)
- macOS (x86_64 and ARM64/Apple Silicon)
- Windows (x86_64, MSVC and GNU)

## Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

## Building

### Standard Build (Current Platform)

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### Cross-Compilation

To build for other platforms, you'll need to install the target:

```bash
# Install target
rustup target add <target-triple>

# Build for target
cargo build --release --target <target-triple>
```

#### Target Triples

- **Linux**: `x86_64-unknown-linux-gnu`
- **macOS Intel**: `x86_64-apple-darwin`
- **macOS Apple Silicon**: `aarch64-apple-darwin`
- **Windows MSVC**: `x86_64-pc-windows-msvc`
- **Windows GNU**: `x86_64-pc-windows-gnu`

#### Examples

```bash
# Build for Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Build for macOS Intel
cargo build --release --target x86_64-apple-darwin

# Build for macOS Apple Silicon
cargo build --release --target aarch64-apple-darwin

# Build for Windows
cargo build --release --target x86_64-pc-windows-msvc
```

## Build Output

Release binaries will be located at:
- `target/release/bitsy` (current platform)
- `target/<target-triple>/release/bitsy[.exe]` (cross-compiled)

## CI/CD

The GitHub Actions workflow (`.github/workflows/ci.yml`) automatically builds and tests Bitsy on:
- Ubuntu (Linux)
- macOS (latest)
- Windows (latest)

## Optimization

The release profile is configured for maximum performance:
- **opt-level**: 3 (maximum optimization)
- **lto**: true (link-time optimization)
- **codegen-units**: 1 (better optimization, slower compile)
- **strip**: true (remove debug symbols)

## Troubleshooting

### Linux

If you encounter linking errors, ensure you have the required development packages:

```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora/RHEL
sudo dnf install gcc
```

### Windows

For Windows builds, you may need to install:
- Visual Studio Build Tools (for MSVC target)
- MinGW-w64 (for GNU target)

### macOS

Xcode Command Line Tools are required:

```bash
xcode-select --install
```

## Testing Cross-Platform Builds

```bash
# Run tests
cargo test

# Run with specific features
cargo test --all-features

# Run on all platforms via CI
# Push to GitHub and CI will run automatically
```
