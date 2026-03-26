use std::sync::{LazyLock, OnceLock};

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::document::{Color, SpanStyle, StyledSpan};

use super::CODE_BG;

pub(super) static DEFAULT_THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
pub(super) static DEFAULT_SYNTAX_SET: LazyLock<SyntaxSet> =
    LazyLock::new(SyntaxSet::load_defaults_nonewlines);

/// Cached custom syntax set built from a user-provided syntax directory.
/// Initialized once on first use; subsequent calls return the cached value.
static CUSTOM_SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Load a syntax set, optionally augmented with definitions from a directory.
pub(super) fn load_syntax_set(syntax_dir: Option<&str>) -> &'static SyntaxSet {
    if let Some(dir) = syntax_dir {
        let path = std::path::Path::new(dir);
        if path.is_dir() {
            return CUSTOM_SYNTAX_SET.get_or_init(|| {
                let mut builder = SyntaxSet::load_defaults_nonewlines().into_builder();
                if builder.add_from_folder(path, true).is_ok() {
                    builder.build()
                } else {
                    SyntaxSet::load_defaults_nonewlines()
                }
            });
        }
    }
    &DEFAULT_SYNTAX_SET
}

pub(super) fn highlight_code(
    code: &str,
    lang: Option<&str>,
    syntax_set: &SyntaxSet,
    theme: &syntect::highlighting::Theme,
) -> Vec<Vec<StyledSpan>> {
    let syntax = lang
        .and_then(|l| syntax_set.find_syntax_by_token(l))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    let mut result = Vec::new();

    for line in code.lines() {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();
        let spans: Vec<StyledSpan> = ranges
            .into_iter()
            .map(|(style, text)| {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let mut span_style = SpanStyle {
                    fg: Some(fg),
                    bg: Some(CODE_BG),
                    ..Default::default()
                };
                if style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::BOLD)
                {
                    span_style.bold = true;
                }
                if style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::ITALIC)
                {
                    span_style.italic = true;
                }
                StyledSpan {
                    content: text.to_string(),
                    style: span_style,
                    link_url: None,
                }
            })
            .collect();
        result.push(spans);
    }

    // Handle empty code buffer (no lines)
    if result.is_empty() {
        result.push(vec![]);
    }

    result
}
