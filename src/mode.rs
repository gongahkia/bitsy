// Vim modal system

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Replace,
    Visual,
    VisualLine,
    VisualBlock,
    Command,
    Search,
    FuzzyFind,
}

impl Mode {
    pub fn as_str(&self) -> &str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Replace => "REPLACE",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "VISUAL LINE",
            Mode::VisualBlock => "VISUAL BLOCK",
            Mode::Command => "COMMAND",
            Mode::Search => "SEARCH",
            Mode::FuzzyFind => "FINDER",
        }
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_default() {
        let mode = Mode::default();
        assert_eq!(mode, Mode::Normal);
    }

    #[test]
    fn test_mode_as_str() {
        assert_eq!(Mode::Normal.as_str(), "NORMAL");
        assert_eq!(Mode::Insert.as_str(), "INSERT");
        assert_eq!(Mode::Visual.as_str(), "VISUAL");
        assert_eq!(Mode::VisualLine.as_str(), "VISUAL LINE");
        assert_eq!(Mode::VisualBlock.as_str(), "VISUAL BLOCK");
        assert_eq!(Mode::Command.as_str(), "COMMAND");
    }

    #[test]
    fn test_mode_equality() {
        assert_eq!(Mode::Normal, Mode::Normal);
        assert_ne!(Mode::Normal, Mode::Insert);
        assert_ne!(Mode::Visual, Mode::VisualLine);
        assert_ne!(Mode::VisualLine, Mode::VisualBlock);
    }

    #[test]
    fn test_mode_clone() {
        let mode1 = Mode::Visual;
        let mode2 = mode1;
        assert_eq!(mode1, mode2);
    }

    #[test]
    fn test_mode_debug() {
        let mode = Mode::Normal;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Normal"));
    }
}
