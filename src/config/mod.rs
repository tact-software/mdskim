use std::collections::HashMap;
use std::path::PathBuf;

use clap::ValueEnum;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Full,
    Fast,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub(crate) theme: Option<ThemeChoice>,
    pub(crate) keybindings: KeybindingsConfig,
    pub(crate) headings: HeadingsConfig,
    /// Path to custom CSS file for HTML export. Replaces the built-in theme CSS.
    pub(crate) export_css: Option<String>,
    /// Directory containing custom .sublime-syntax files for syntax highlighting.
    pub(crate) syntax_dir: Option<String>,
    /// Render mode: "full" (default) or "fast" (skip Mermaid/Math rendering).
    pub(crate) render_mode: Option<RenderMode>,
    /// Warning message from config loading (e.g. parse errors).
    #[serde(skip)]
    pub(crate) warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HeadingsConfig {
    pub(crate) h1: HeadingStyle,
    pub(crate) h2: HeadingStyle,
    pub(crate) h3: HeadingStyle,
    pub(crate) h4: HeadingStyle,
    pub(crate) h5: HeadingStyle,
    pub(crate) h6: HeadingStyle,
}

impl Default for HeadingsConfig {
    fn default() -> Self {
        Self {
            h1: HeadingStyle {
                decoration: HeadingDecoration::DoubleLine,
                bold: true,
                dim: false,
            },
            h2: HeadingStyle {
                decoration: HeadingDecoration::HeavyUnderline,
                bold: true,
                dim: false,
            },
            h3: HeadingStyle {
                decoration: HeadingDecoration::LightUnderline,
                bold: true,
                dim: false,
            },
            h4: HeadingStyle {
                decoration: HeadingDecoration::DottedUnderline,
                bold: true,
                dim: false,
            },
            h5: HeadingStyle {
                decoration: HeadingDecoration::None,
                bold: true,
                dim: false,
            },
            h6: HeadingStyle {
                decoration: HeadingDecoration::None,
                bold: false,
                dim: true,
            },
        }
    }
}

impl HeadingsConfig {
    pub fn for_level(&self, level: usize) -> &HeadingStyle {
        match level {
            1 => &self.h1,
            2 => &self.h2,
            3 => &self.h3,
            4 => &self.h4,
            5 => &self.h5,
            _ => &self.h6,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HeadingStyle {
    pub(crate) decoration: HeadingDecoration,
    pub(crate) bold: bool,
    pub(crate) dim: bool,
}

impl Default for HeadingStyle {
    fn default() -> Self {
        Self {
            decoration: HeadingDecoration::None,
            bold: true,
            dim: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingDecoration {
    /// ═══ above and below
    DoubleLine,
    /// ═══ above only
    DoubleOverline,
    /// ═══ below only
    DoubleUnderline,
    /// ━━━ above and below
    HeavyLine,
    /// ━━━ above only
    HeavyOverline,
    /// ━━━ below only
    HeavyUnderline,
    /// ─── above and below
    LightLine,
    /// ─── above only
    LightOverline,
    /// ─── below only
    LightUnderline,
    /// ┄┄┄ below only
    DottedUnderline,
    /// ╌╌╌ below only
    DashedUnderline,
    #[default]
    None,
}

impl HeadingDecoration {
    pub fn overline_char(&self) -> Option<&str> {
        match self {
            Self::DoubleLine | Self::DoubleOverline => Some("═"),
            Self::HeavyLine | Self::HeavyOverline => Some("━"),
            Self::LightLine | Self::LightOverline => Some("─"),
            _ => None,
        }
    }

    pub fn underline_char(&self) -> Option<&str> {
        match self {
            Self::DoubleLine | Self::DoubleUnderline => Some("═"),
            Self::HeavyLine | Self::HeavyUnderline => Some("━"),
            Self::LightLine | Self::LightUnderline => Some("─"),
            Self::DottedUnderline => Some("┄"),
            Self::DashedUnderline => Some("╌"),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub(crate) preset: KeyPreset,
    pub(crate) custom: HashMap<String, String>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            preset: KeyPreset::Vim,
            custom: HashMap::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyPreset {
    #[default]
    Vim,
}

/// Key-action mapping resolved from preset + custom overrides.
pub struct Keymap {
    map: HashMap<String, String>,
}

impl Keymap {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let mut map = match config.preset {
            KeyPreset::Vim => vim_defaults(),
        };
        // Custom overrides
        for (key, action) in &config.custom {
            map.insert(key.clone(), action.clone());
        }
        Self { map }
    }

    pub fn get_action(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|s| s.as_str())
    }
}

fn vim_defaults() -> HashMap<String, String> {
    let pairs = [
        ("q", "quit"),
        ("j", "scroll_down"),
        ("k", "scroll_up"),
        ("d", "half_page_down"),
        ("u", "half_page_up"),
        ("g", "go_to_top_pending"),
        ("G", "go_to_bottom"),
        ("/", "search"),
        ("n", "search_next"),
        ("N", "search_prev"),
        ("]", "next_heading"),
        ("[", "prev_heading"),
        ("t", "toggle_toc"),
        ("l", "toggle_links"),
        ("o", "open_link"),
        ("r", "reload"),
        ("?", "toggle_help"),
        ("s", "toggle_toc_pane"),
        ("z", "toggle_fold"),
        ("Z", "fold_all"),
        ("U", "unfold_all"),
        ("0", "go_to_top"),
    ];
    pairs
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => Self {
                    warning: Some(format!("Config parse error: {e}")),
                    ..Self::default()
                },
            },
            Err(e) => Self {
                warning: Some(format!("Config read error: {e}")),
                ..Self::default()
            },
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("mdskim/config.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/mdskim/config.toml")
    } else {
        PathBuf::from("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_theme() {
        let config = Config::default();
        assert!(config.theme.is_none());
        assert!(config.warning.is_none());
    }

    #[test]
    fn default_config_vim_preset() {
        let config = Config::default();
        assert_eq!(config.keybindings.preset, KeyPreset::Vim);
    }

    #[test]
    fn default_headings_config() {
        let h = HeadingsConfig::default();
        assert!(h.h1.bold);
        assert!(!h.h1.dim);
        assert!(!h.h6.bold);
        assert!(h.h6.dim);
    }

    #[test]
    fn deserialize_partial_config() {
        let toml_str = r#"theme = "dark""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme, Some(ThemeChoice::Dark));
        assert_eq!(config.keybindings.preset, KeyPreset::Vim);
    }

    #[test]
    fn deserialize_with_keybindings() {
        let toml_str = r#"
[keybindings]
preset = "vim"

[keybindings.custom]
x = "quit"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.keybindings.custom.get("x").unwrap(), "quit");
    }

    #[test]
    fn invalid_toml_is_error() {
        let bad_toml = "this is not valid { toml";
        let config: Result<Config, _> = toml::from_str(bad_toml);
        assert!(config.is_err());
    }

    #[test]
    fn keymap_from_config_with_custom_override() {
        let mut custom = HashMap::new();
        custom.insert("x".to_string(), "quit".to_string());
        let kbconfig = KeybindingsConfig {
            preset: KeyPreset::Vim,
            custom,
        };
        let keymap = Keymap::from_config(&kbconfig);
        assert_eq!(keymap.get_action("x"), Some("quit"));
        assert_eq!(keymap.get_action("j"), Some("scroll_down"));
    }

    #[test]
    fn keymap_custom_overrides_default() {
        let mut custom = HashMap::new();
        custom.insert("j".to_string(), "quit".to_string());
        let kbconfig = KeybindingsConfig {
            preset: KeyPreset::Vim,
            custom,
        };
        let keymap = Keymap::from_config(&kbconfig);
        assert_eq!(keymap.get_action("j"), Some("quit"));
    }

    #[test]
    fn keymap_unknown_key_returns_none() {
        let kbconfig = KeybindingsConfig::default();
        let keymap = Keymap::from_config(&kbconfig);
        assert_eq!(keymap.get_action("X"), None);
    }

    #[test]
    fn headings_config_for_level() {
        let h = HeadingsConfig::default();
        assert!(h.for_level(1).bold);
        assert!(h.for_level(6).dim);
        assert!(h.for_level(7).dim);
    }

    #[test]
    fn deserialize_render_mode() {
        let toml_str = r#"render_mode = "fast""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.render_mode, Some(RenderMode::Fast));
    }
}
