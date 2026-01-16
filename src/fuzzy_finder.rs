// Fuzzy finder UI component

use crate::fuzzy::{FuzzyMatch, FuzzyMatcher};
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Type of fuzzy finder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinderType {
    Files,
    Buffers,
    Grep,
}

/// State for the fuzzy finder UI
#[derive(Debug)]
pub struct FuzzyFinder {
    /// Type of finder
    pub finder_type: FinderType,
    /// Current input query
    pub query: String,
    /// All available candidates
    candidates: Vec<String>,
    /// Filtered and scored matches
    pub matches: Vec<FuzzyMatch>,
    /// Currently selected index in matches
    pub selected_index: usize,
    /// Maximum number of results to show
    pub max_results: usize,
    /// Fuzzy matcher
    matcher: FuzzyMatcher,
}

impl FuzzyFinder {
    pub fn new(finder_type: FinderType) -> Self {
        Self {
            finder_type,
            query: String::new(),
            candidates: Vec::new(),
            matches: Vec::new(),
            selected_index: 0,
            max_results: 20,
            matcher: FuzzyMatcher::default(),
        }
    }

    /// Create a new file finder
    pub fn files(base_path: &PathBuf) -> Self {
        let mut finder = Self::new(FinderType::Files);
        finder.candidates = collect_files(base_path);
        finder.update_matches();
        finder
    }

    /// Create a new buffer finder with given buffer names
    pub fn buffers(buffer_names: Vec<String>) -> Self {
        let mut finder = Self::new(FinderType::Buffers);
        finder.candidates = buffer_names;
        finder.update_matches();
        finder
    }

    /// Create a new grep finder searching for pattern in files
    pub fn grep(base_path: &PathBuf, pattern: &str) -> Self {
        let mut finder = Self::new(FinderType::Grep);
        finder.candidates = grep_files(base_path, pattern);
        finder.update_matches();
        finder
    }

    /// Update the query and refresh matches
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.update_matches();
        self.selected_index = 0;
    }

    /// Append a character to the query
    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.update_matches();
        self.selected_index = 0;
    }

    /// Remove the last character from query
    pub fn pop_char(&mut self) {
        self.query.pop();
        self.update_matches();
        self.selected_index = 0;
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_matches();
        self.selected_index = 0;
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.matches.len().min(self.max_results) {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&str> {
        self.matches
            .get(self.selected_index)
            .map(|m| m.item.as_str())
    }

    /// Update matches based on current query
    fn update_matches(&mut self) {
        self.matches = self.matcher.fuzzy_match_all(&self.query, &self.candidates);
    }

    /// Get visible matches (limited by max_results)
    pub fn visible_matches(&self) -> &[FuzzyMatch] {
        let end = self.matches.len().min(self.max_results);
        &self.matches[..end]
    }

    /// Get the prompt string for the finder type
    pub fn prompt(&self) -> &str {
        match self.finder_type {
            FinderType::Files => "Files> ",
            FinderType::Buffers => "Buffers> ",
            FinderType::Grep => "Grep> ",
        }
    }
}

/// Search for pattern in files and return results as "file:line:content"
fn grep_files(base_path: &PathBuf, pattern: &str) -> Vec<String> {
    let mut results = Vec::new();
    let max_results = 100;

    let regex = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return results, // Invalid regex, return empty
    };

    for entry in WalkDir::new(base_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e) && !is_ignored(e))
    {
        if results.len() >= max_results {
            break;
        }

        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                // Skip binary files (simple heuristic: check extension)
                let path = entry.path();
                if is_likely_binary(path) {
                    continue;
                }

                if let Ok(content) = fs::read_to_string(path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            if let Ok(relative) = path.strip_prefix(base_path) {
                                let truncated_line = if line.len() > 80 {
                                    format!("{}...", &line[..77])
                                } else {
                                    line.to_string()
                                };
                                results.push(format!(
                                    "{}:{}:{}",
                                    relative.display(),
                                    line_num + 1,
                                    truncated_line.trim()
                                ));
                                if results.len() >= max_results {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

/// Check if file is likely binary based on extension
fn is_likely_binary(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "bmp"
                | "webp"
                | "pdf"
                | "zip"
                | "tar"
                | "gz"
                | "rar"
                | "7z"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
                | "a"
                | "o"
                | "obj"
                | "class"
                | "pyc"
                | "pyo"
                | "wasm"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
                | "mp3"
                | "mp4"
                | "avi"
                | "mov"
                | "mkv"
                | "webm"
                | "sqlite"
                | "db"
                | "swp"
        )
    } else {
        false
    }
}

/// Collect all files in a directory tree (excluding hidden and common ignore patterns)
fn collect_files(base_path: &PathBuf) -> Vec<String> {
    let mut files = Vec::new();

    for entry in WalkDir::new(base_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| !is_hidden(e) && !is_ignored(e))
    {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                if let Ok(relative) = entry.path().strip_prefix(base_path) {
                    files.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }

    files
}

/// Check if entry is hidden (starts with .)
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Check if entry should be ignored (common patterns)
fn is_ignored(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    matches!(
        name.as_ref(),
        "node_modules" | "target" | "dist" | "build" | "__pycache__" | ".git" | "vendor"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_finder_new() {
        let finder = FuzzyFinder::new(FinderType::Files);
        assert_eq!(finder.finder_type, FinderType::Files);
        assert!(finder.query.is_empty());
        assert_eq!(finder.selected_index, 0);
    }

    #[test]
    fn test_fuzzy_finder_query() {
        let mut finder = FuzzyFinder::new(FinderType::Buffers);
        finder.candidates = vec![
            "main.rs".to_string(),
            "buffer.rs".to_string(),
            "editor.rs".to_string(),
        ];
        finder.update_matches();

        finder.set_query("main");
        assert_eq!(finder.matches.len(), 1);
        assert_eq!(finder.selected_item(), Some("main.rs"));
    }

    #[test]
    fn test_fuzzy_finder_navigation() {
        let mut finder = FuzzyFinder::new(FinderType::Buffers);
        finder.candidates = vec![
            "aaa.rs".to_string(),
            "bbb.rs".to_string(),
            "ccc.rs".to_string(),
        ];
        finder.update_matches();

        assert_eq!(finder.selected_index, 0);
        finder.select_next();
        assert_eq!(finder.selected_index, 1);
        finder.select_next();
        assert_eq!(finder.selected_index, 2);
        finder.select_next(); // Should not go past end
        assert_eq!(finder.selected_index, 2);
        finder.select_prev();
        assert_eq!(finder.selected_index, 1);
    }

    #[test]
    fn test_fuzzy_finder_push_pop() {
        let mut finder = FuzzyFinder::new(FinderType::Files);
        finder.push_char('a');
        finder.push_char('b');
        assert_eq!(finder.query, "ab");
        finder.pop_char();
        assert_eq!(finder.query, "a");
        finder.clear_query();
        assert!(finder.query.is_empty());
    }
}
