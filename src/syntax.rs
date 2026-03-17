// tree-sitter based syntax highlighting

use crossterm::style::Color;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};
use crate::theme::Theme;

// highlight names recognized by our theme
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute", "comment", "constant", "constant.builtin", "constructor",
    "embedded", "escape", "function", "function.builtin", "function.macro",
    "keyword", "label", "namespace", "number", "operator", "property",
    "punctuation", "punctuation.bracket", "punctuation.delimiter",
    "punctuation.special", "string", "string.special", "tag", "type",
    "type.builtin", "variable", "variable.builtin", "variable.parameter",
];

pub struct SyntaxHighlighter {
    highlighter: Highlighter,
    configs: Vec<(&'static str, HighlightConfiguration)>, // (extension, config)
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let highlighter = Highlighter::new();
        let mut sh = Self { highlighter, configs: Vec::new() };
        sh.register_languages();
        sh
    }

    fn register_languages(&mut self) {
        #[cfg(feature = "lang-rust")]
        self.register_lang("rs", tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY, "", "");
        #[cfg(feature = "lang-python")]
        self.register_lang("py", tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY, "", "");
        #[cfg(feature = "lang-js")]
        {
            self.register_lang("js", tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTIONS_QUERY,
                tree_sitter_javascript::LOCALS_QUERY);
            self.register_lang("jsx", tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTIONS_QUERY,
                tree_sitter_javascript::LOCALS_QUERY);
        }
        #[cfg(feature = "lang-c")]
        self.register_lang("c", tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY, "", "");
        #[cfg(feature = "lang-c")]
        self.register_lang("h", tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY, "", "");
        #[cfg(feature = "lang-go")]
        self.register_lang("go", tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY, "", "");
        #[cfg(feature = "lang-toml")]
        self.register_lang("toml", tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY, "", "");
        #[cfg(feature = "lang-json")]
        self.register_lang("json", tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY, "", "");
        #[cfg(feature = "lang-md")]
        self.register_lang("md", tree_sitter_md::LANGUAGE.into(),
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK, tree_sitter_md::INJECTION_QUERY_BLOCK, "");
    }

    fn register_lang(&mut self, ext: &'static str, lang: tree_sitter::Language,
        highlights: &str, injections: &str, locals: &str)
    {
        if let Ok(mut config) = HighlightConfiguration::new(lang, ext, highlights, injections, locals) {
            config.configure(HIGHLIGHT_NAMES);
            self.configs.push((ext, config));
        }
    }

    /// get highlight spans for a source string by file extension
    /// returns vec of (byte_start, byte_end, highlight_index) for each token
    pub fn highlight(&mut self, ext: &str, source: &[u8]) -> Vec<(usize, usize, usize)> {
        let config = self.configs.iter().find(|(e, _)| *e == ext);
        let config = match config { Some((_, c)) => c, None => return Vec::new() };
        let events = self.highlighter.highlight(config, source, None, |_| None);
        let events = match events { Ok(e) => e, Err(_) => return Vec::new() };
        let mut spans = Vec::new();
        let mut current_highlight: Option<usize> = None;
        for event in events {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    if let Some(hi) = current_highlight {
                        spans.push((start, end, hi));
                    }
                }
                Ok(HighlightEvent::HighlightStart(Highlight(idx))) => {
                    current_highlight = Some(idx);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    current_highlight = None;
                }
                Err(_) => break,
            }
        }
        spans
    }

    /// check if we have a grammar for this extension
    pub fn supports(&self, ext: &str) -> bool {
        self.configs.iter().any(|(e, _)| *e == ext)
    }
}

/// map highlight index to theme color
pub fn highlight_color(index: usize, theme: &Theme) -> Color {
    let name = HIGHLIGHT_NAMES.get(index).copied().unwrap_or("");
    match name {
        "comment" => theme.comment,
        "string" | "string.special" => theme.string,
        "number" | "constant" | "constant.builtin" => theme.warning, // orange
        "keyword" => theme.accent,                                    // purple
        "function" | "function.builtin" | "function.macro" => Color::Rgb { r: 0x7a, g: 0xa2, b: 0xf7 }, // blue
        "type" | "type.builtin" | "constructor" => Color::Rgb { r: 0x2a, g: 0xc3, b: 0xde }, // cyan
        "variable.builtin" => Color::Rgb { r: 0xf7, g: 0x76, b: 0x8e }, // red
        "operator" => theme.fg,
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => theme.comment,
        "attribute" | "label" => theme.warning,
        "namespace" => theme.accent,
        "property" | "variable.parameter" => Color::Rgb { r: 0x7d, g: 0xcf, b: 0xff },
        "escape" | "embedded" => theme.error,
        "tag" => theme.error,
        _ => theme.fg,
    }
}
