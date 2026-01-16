// File type detection

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    Markdown,
    Text,
    Unknown,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Rust => "rust",
            FileType::Python => "python",
            FileType::JavaScript => "javascript",
            FileType::TypeScript => "typescript",
            FileType::Go => "go",
            FileType::C => "c",
            FileType::Cpp => "cpp",
            FileType::Markdown => "markdown",
            FileType::Text => "text",
            FileType::Unknown => "unknown",
        }
    }
}

pub fn detect_file_type(path: &std::path::Path, content: &str) -> FileType {
    // 1. Check file extension
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext {
            "rs" => return FileType::Rust,
            "py" => return FileType::Python,
            "js" => return FileType::JavaScript,
            "ts" => return FileType::TypeScript,
            "go" => return FileType::Go,
            "c" | "h" => return FileType::C,
            "cpp" | "hpp" | "cxx" | "hxx" => return FileType::Cpp,
            "md" | "markdown" => return FileType::Markdown,
            "txt" => return FileType::Text,
            _ => {}
        }
    }

    // 2. Check shebang
    if let Some(first_line) = content.lines().next() {
        if first_line.starts_with("#!") {
            if first_line.contains("python") {
                return FileType::Python;
            }
            if first_line.contains("node") {
                return FileType::JavaScript;
            }
        }
    }

    // 3. Check modeline (simple version)
    // Look in the first and last 5 lines
    for line in content.lines().take(5).chain(content.lines().rev().take(5)) {
        if let Some(ft_pos) = line.find("ft=") {
            let modeline = &line[ft_pos + 3..];
            let filetype = modeline
                .split(|c: char| c.is_whitespace() || c == ':')
                .next()
                .unwrap_or("");
            match filetype {
                "rust" => return FileType::Rust,
                "python" => return FileType::Python,
                "javascript" => return FileType::JavaScript,
                "typescript" => return FileType::TypeScript,
                "go" => return FileType::Go,
                "c" => return FileType::C,
                "cpp" => return FileType::Cpp,
                "markdown" => return FileType::Markdown,
                _ => {}
            }
        }
    }

    FileType::Unknown
}
