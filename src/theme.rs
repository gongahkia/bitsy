// theme configuration: webspinner default + custom palettes

use crossterm::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,          // purple "web silk"
    pub string: Color,
    pub warning: Color,
    pub error: Color,
    pub comment: Color,
    pub current_line_bg: Color,
    pub gutter_fg: Color,
    pub gutter_current: Color,
    pub gutter_separator: Color,
    pub statusline_bg: Color,
    pub statusline_fg: Color,
    pub mode_normal_bg: Color,
    pub mode_normal_fg: Color,
    pub mode_insert_bg: Color,
    pub mode_insert_fg: Color,
    pub mode_visual_bg: Color,
    pub mode_visual_fg: Color,
    pub mode_command_bg: Color,
    pub mode_command_fg: Color,
    pub mode_search_bg: Color,
    pub mode_search_fg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub search_match_bg: Color,
    pub search_match_fg: Color,
    pub substitute_bg: Color,
    pub substitute_fg: Color,
    pub finder_selected_bg: Color,
    pub finder_prompt_fg: Color,
    pub tilde_fg: Color,
}

fn hex(r: u8, g: u8, b: u8) -> Color { Color::Rgb { r, g, b } }

impl Theme {
    pub fn webspinner() -> Self { // default theme
        Self {
            bg: hex(0x1a, 0x1b, 0x26),
            fg: hex(0xc0, 0xca, 0xf5),
            accent: hex(0xbb, 0x9a, 0xf7),       // purple web silk
            string: hex(0x9e, 0xce, 0x6a),
            warning: hex(0xe0, 0xaf, 0x68),
            error: hex(0xf7, 0x76, 0x8e),
            comment: hex(0x56, 0x5f, 0x89),
            current_line_bg: hex(0x24, 0x28, 0x3b),
            gutter_fg: hex(0x56, 0x5f, 0x89),
            gutter_current: hex(0xbb, 0x9a, 0xf7), // accent purple
            gutter_separator: hex(0x3b, 0x40, 0x61),
            statusline_bg: hex(0x24, 0x28, 0x3b),
            statusline_fg: hex(0xc0, 0xca, 0xf5),
            mode_normal_bg: hex(0x7a, 0xa2, 0xf7),  // blue
            mode_normal_fg: hex(0x1a, 0x1b, 0x26),
            mode_insert_bg: hex(0x9e, 0xce, 0x6a),  // green
            mode_insert_fg: hex(0x1a, 0x1b, 0x26),
            mode_visual_bg: hex(0xbb, 0x9a, 0xf7),  // purple
            mode_visual_fg: hex(0x1a, 0x1b, 0x26),
            mode_command_bg: hex(0xe0, 0xaf, 0x68),  // orange
            mode_command_fg: hex(0x1a, 0x1b, 0x26),
            mode_search_bg: hex(0xe0, 0xaf, 0x68),
            mode_search_fg: hex(0x1a, 0x1b, 0x26),
            selection_bg: hex(0x28, 0x3b, 0x8a),
            selection_fg: hex(0xc0, 0xca, 0xf5),
            search_match_bg: hex(0xe0, 0xaf, 0x68),
            search_match_fg: hex(0x1a, 0x1b, 0x26),
            substitute_bg: hex(0xf7, 0x76, 0x8e),
            substitute_fg: hex(0x1a, 0x1b, 0x26),
            finder_selected_bg: hex(0x28, 0x3b, 0x8a),
            finder_prompt_fg: hex(0x7a, 0xa2, 0xf7),
            tilde_fg: hex(0x3b, 0x40, 0x61),
        }
    }

    pub fn from_toml(table: &toml::Table) -> Self {
        let mut theme = Self::webspinner();
        fn parse_color(val: &toml::Value) -> Option<Color> {
            val.as_str().and_then(|s| {
                let s = s.trim_start_matches('#');
                if s.len() == 6 {
                    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                    Some(Color::Rgb { r, g, b })
                } else { None }
            })
        }
        macro_rules! set_color {
            ($field:ident, $key:expr) => {
                if let Some(v) = table.get($key) {
                    if let Some(c) = parse_color(v) { theme.$field = c; }
                }
            };
        }
        set_color!(bg, "background");
        set_color!(fg, "foreground");
        set_color!(accent, "accent");
        set_color!(string, "string");
        set_color!(warning, "warning");
        set_color!(error, "error");
        set_color!(comment, "comment");
        set_color!(current_line_bg, "current_line_bg");
        theme
    }
}

impl Default for Theme {
    fn default() -> Self { Self::webspinner() }
}

// deserialize helper for config loading
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThemeConfig {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub accent: Option<String>,
    pub string: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub comment: Option<String>,
    pub current_line_bg: Option<String>,
}
