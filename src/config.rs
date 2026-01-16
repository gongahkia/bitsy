// Editor configuration
use serde::Deserialize;
use std::fs;
use toml;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum LineNumberMode {
    None,
    Absolute,
    Relative,
    RelativeAbsolute, // Hybrid: relative numbers with absolute for current line
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub line_numbers: LineNumberMode,
    pub show_current_line: bool,
    pub tab_width: usize,
    pub expand_tab: bool, // Use spaces instead of tabs
    pub auto_indent: bool,
    pub highlight_search: bool,
    pub ignore_case: bool,
    pub smart_case: bool, // Override ignorecase when search has uppercase
    pub zen_mode: bool,
    pub zen_mode_width: usize,
}

impl Config {
    pub fn new() -> Self {
        Self {
            line_numbers: LineNumberMode::Absolute,
            show_current_line: true,
            tab_width: 4,
            expand_tab: true,
            auto_indent: true,
            highlight_search: true,
            ignore_case: false,
            smart_case: true,
            zen_mode: false,
            zen_mode_width: 80,
        }
    }

    pub fn load_from_file(path: &str) -> Self {
        let default_config = Self::new();
        match fs::read_to_string(path) {
            Ok(content) => {
                match toml::from_str(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        log::warn!("Failed to parse config file: {}", e);
                        default_config
                    }
                }
            }
            Err(_) => default_config,
        }
    }

    pub fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "number" | "nu" => {
                self.line_numbers = LineNumberMode::Absolute;
                Ok(())
            }
            "nonumber" | "nonu" => {
                self.line_numbers = LineNumberMode::None;
                Ok(())
            }
            "relativenumber" | "rnu" => {
                self.line_numbers = LineNumberMode::Relative;
                Ok(())
            }
            "norelativenumber" | "nornu" => {
                if self.line_numbers == LineNumberMode::Relative || self.line_numbers == LineNumberMode::RelativeAbsolute {
                    self.line_numbers = LineNumberMode::Absolute;
                }
                Ok(())
            }
            "expandtab" | "et" => {
                self.expand_tab = true;
                Ok(())
            }
            "noexpandtab" | "noet" => {
                self.expand_tab = false;
                Ok(())
            }
            "autoindent" | "ai" => {
                self.auto_indent = true;
                Ok(())
            }
            "noautoindent" | "noai" => {
                self.auto_indent = false;
                Ok(())
            }
            "hlsearch" | "hls" => {
                self.highlight_search = true;
                Ok(())
            }
            "nohlsearch" | "nohls" => {
                self.highlight_search = false;
                Ok(())
            }
            "ignorecase" | "ic" => {
                self.ignore_case = true;
                Ok(())
            }
            "noignorecase" | "noic" => {
                self.ignore_case = false;
                Ok(())
            }
            "smartcase" | "scs" => {
                self.smart_case = true;
                Ok(())
            }
            "nosmartcase" | "noscs" => {
                self.smart_case = false;
                Ok(())
            }
            "tabstop" | "ts" => {
                if let Some(val) = value {
                    if let Ok(width) = val.parse::<usize>() {
                        self.tab_width = width.max(1).min(16);
                        Ok(())
                    } else {
                        Err(format!("Invalid value for tabstop: {}", val))
                    }
                } else {
                    Err("tabstop requires a value".to_string())
                }
            }
            _ => Err(format!("Unknown option: {}", option))
        }
    }

    pub fn line_number_width(&self, max_line: usize) -> usize {
        match self.line_numbers {
            LineNumberMode::None => 0,
            _ => {
                // Calculate width needed for line numbers
                let digits = if max_line == 0 {
                    1
                } else {
                    (max_line as f64).log10().floor() as usize + 1
                };
                // Add space for gutter: " <number> "
                digits + 2
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.line_numbers, LineNumberMode::Absolute);
        assert_eq!(config.show_current_line, true);
    }

    #[test]
    fn test_line_number_width() {
        let config = Config::new();
        assert_eq!(config.line_number_width(9), 3); // " 9 "
        assert_eq!(config.line_number_width(99), 4); // " 99 "
        assert_eq!(config.line_number_width(999), 5); // " 999 "
    }

    #[test]
    fn test_line_number_width_none() {
        let mut config = Config::new();
        config.line_numbers = LineNumberMode::None;
        assert_eq!(config.line_number_width(999), 0);
    }
}
