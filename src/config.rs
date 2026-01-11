// Editor configuration

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberMode {
    None,
    Absolute,
    Relative,
    RelativeAbsolute, // Hybrid: relative numbers with absolute for current line
}

#[derive(Debug, Clone)]
pub struct Config {
    pub line_numbers: LineNumberMode,
    pub show_current_line: bool,
}

impl Config {
    pub fn new() -> Self {
        Self {
            line_numbers: LineNumberMode::Absolute,
            show_current_line: true,
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
