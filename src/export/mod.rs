pub mod pdf;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use crate::document::{Color, Document, StyledLine};
use crate::mermaid::MermaidRenderer;

const CSS_DARK: &str = include_str!("../../assets/export-dark.css");
const CSS_LIGHT: &str = include_str!("../../assets/export-light.css");

pub enum ExportTheme {
    Dark,
    Light,
}

pub fn to_html(
    doc: &Document,
    export_theme: ExportTheme,
    custom_css: Option<&str>,
    base_dir: Option<&Path>,
) -> String {
    let css = match custom_css {
        Some(css) => css,
        None => match export_theme {
            ExportTheme::Dark => CSS_DARK,
            ExportTheme::Light => CSS_LIGHT,
        },
    };

    // Pre-render all mermaid and math blocks in parallel
    let mermaid_svgs = prerender_mermaid_svgs(doc);
    let math_svgs = prerender_math_svgs(doc);

    let mut out = String::new();
    out.push_str(&html_head(css));

    let mermaid_lines = &doc.mermaid_line_map;

    // Use cached display math map; build inline math map separately
    let display_math_lines = &doc.math_line_map;
    let mut inline_math_lines: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, b) in doc.math_blocks.iter().enumerate() {
        if !b.display {
            inline_math_lines.entry(b.line_idx).or_default().push(i);
        }
    }

    // Use cached image line map
    let image_lines = &doc.image_line_map;

    // Map heading line_idx -> level
    let heading_lines: HashMap<usize, usize> =
        doc.headings.iter().map(|h| (h.line_idx, h.level)).collect();

    // Build a set of line indices that belong to table blocks, and map start_line_idx -> table block index
    let mut table_line_set: HashSet<usize> = HashSet::new();
    let mut table_start_lines: HashMap<usize, usize> = HashMap::new();
    for (i, tb) in doc.table_blocks.iter().enumerate() {
        for idx in tb.start_line_idx..=tb.end_line_idx {
            table_line_set.insert(idx);
        }
        table_start_lines.insert(tb.start_line_idx, i);
    }

    let mut skip_until: Option<usize> = None;

    for (line_idx, line) in doc.lines.iter().enumerate() {
        if let Some(until) = skip_until {
            if line_idx <= until {
                continue;
            }
            skip_until = None;
        }

        // Mermaid placeholder
        if let Some(&block_idx) = mermaid_lines.get(&line_idx) {
            let block = &doc.mermaid_blocks[block_idx];
            skip_until = Some(line_idx + 1);
            match mermaid_svgs.get(&block_idx) {
                Some(Ok(svg_content)) => {
                    out.push_str("<div class=\"mermaid-diagram\">\n");
                    out.push_str(&sanitize_svg(svg_content));
                    out.push_str("\n</div>\n");
                }
                Some(Err(e)) => {
                    render_mermaid_fallback(block, &mut out, Some(e));
                }
                None => {
                    render_mermaid_fallback(block, &mut out, None);
                }
            }
            continue;
        }

        // Display math placeholder → render as SVG block
        if let Some(&block_idx) = display_math_lines.get(&line_idx) {
            let block = &doc.math_blocks[block_idx];
            let source_lines = block.source.lines().count();
            skip_until = Some(line_idx + source_lines);
            render_math_svg_from_cache(block, &math_svgs, block_idx, &mut out);
            continue;
        }

        // Skip heading decoration lines (overline/underline) — terminal-only feature
        if line.heading_decoration_for.is_some() {
            continue;
        }

        // Skip code block border lines (┌─, └─) — terminal-only feature
        if line.is_code_border {
            continue;
        }

        // Table block → render as semantic <table>
        if let Some(&tb_idx) = table_start_lines.get(&line_idx) {
            let tb = &doc.table_blocks[tb_idx];
            render_semantic_table(tb, &mut out);
            skip_until = Some(tb.end_line_idx);
            continue;
        }
        // Skip remaining lines of a table block (terminal-only box drawing)
        if table_line_set.contains(&line_idx) {
            continue;
        }

        // Image line → render as <img> tag
        if let Some(&img_idx) = image_lines.get(&line_idx) {
            let img = &doc.images[img_idx];
            let src = resolve_image_path(&img.path, base_dir);
            out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\" />\n",
                html_escape(&src),
                html_escape(&img.alt)
            ));
            continue;
        }

        // Heading line → use proper HTML heading tags
        if let Some(&level) = heading_lines.get(&line_idx) {
            let tag = format!("h{}", level.min(6));
            let text: String = line
                .spans
                .iter()
                .map(|s| html_escape(&s.content))
                // Skip the "# " prefix spans
                .skip(1)
                .collect();
            out.push_str(&format!("<{tag}>{text}</{tag}>\n"));
            continue;
        }

        // Regular line
        out.push_str("<div class=\"line");
        if line.line_bg.is_some() {
            out.push_str(" code-bg");
        }
        out.push_str("\">");
        if line.spans.is_empty() {
            out.push_str("&nbsp;");
        } else {
            let inline_indices: Vec<usize> = inline_math_lines
                .get(&line_idx)
                .cloned()
                .unwrap_or_default();
            let inline_blocks: Vec<&crate::document::MathBlock> = inline_indices
                .iter()
                .map(|&i| &doc.math_blocks[i])
                .collect();
            render_spans(line, &mut out, &inline_blocks, &inline_indices, &math_svgs);
        }
        out.push_str("</div>\n");
    }

    out.push_str(HTML_TAIL);
    out
}

/// Pre-render all mermaid blocks to SVG in parallel.
fn prerender_mermaid_svgs(doc: &Document) -> HashMap<usize, Result<String, String>> {
    if doc.mermaid_blocks.is_empty() {
        return HashMap::new();
    }
    let blocks: Vec<(usize, String)> = doc
        .mermaid_blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (i, b.source.clone()))
        .collect();

    std::thread::scope(|s| {
        let handles: Vec<_> = blocks
            .into_iter()
            .map(|(i, source)| {
                s.spawn(move || {
                    let mut renderer = MermaidRenderer::new();
                    match renderer.render_to_svg(&source) {
                        Ok(svg_path) => match std::fs::read_to_string(&svg_path) {
                            Ok(content) => (i, Ok(content)),
                            Err(e) => (i, Err(e.to_string())),
                        },
                        Err(e) => (i, Err(e.to_string())),
                    }
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// Pre-render all math blocks to SVG in parallel.
fn prerender_math_svgs(doc: &Document) -> HashMap<usize, Result<String, String>> {
    if doc.math_blocks.is_empty() {
        return HashMap::new();
    }
    let blocks: Vec<(usize, String, bool)> = doc
        .math_blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (i, b.source.clone(), b.display))
        .collect();

    std::thread::scope(|s| {
        let handles: Vec<_> = blocks
            .into_iter()
            .map(|(i, source, display)| {
                s.spawn(move || {
                    let mut renderer = crate::math::MathRenderer::new();
                    match renderer.render_to_svg(&source, display) {
                        Ok(svg_path) => match std::fs::read_to_string(&svg_path) {
                            Ok(content) => (i, Ok(content)),
                            Err(e) => (i, Err(e.to_string())),
                        },
                        Err(e) => (i, Err(e.to_string())),
                    }
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

fn render_mermaid_fallback(
    block: &crate::document::MermaidBlock,
    out: &mut String,
    error: Option<&str>,
) {
    out.push_str("<div class=\"code-bg\" style=\"padding:8px;margin:4px 0;\">");
    out.push_str(&format!(
        "<div style=\"color:#00bcd4;font-weight:bold;\">📊 Mermaid: {}</div>",
        html_escape(&block.diagram_type)
    ));
    if let Some(err) = error {
        out.push_str(&format!(
            "<div style=\"color:#f44336;font-size:0.9em;\">[render failed: {}]</div>",
            html_escape(err)
        ));
    }
    out.push_str("<pre style=\"color:#4caf50;margin:4px 0;\">");
    out.push_str(&html_escape(&block.source));
    out.push_str("</pre></div>\n");
}

fn render_math_svg_from_cache(
    block: &crate::document::MathBlock,
    math_svgs: &HashMap<usize, Result<String, String>>,
    block_idx: usize,
    out: &mut String,
) {
    let class = if block.display {
        "math-display"
    } else {
        "math-inline"
    };
    match math_svgs.get(&block_idx) {
        Some(Ok(svg_content)) => {
            let tag = if block.display { "div" } else { "span" };
            out.push_str(&format!("<{tag} class=\"{class}\">"));
            out.push_str(&sanitize_svg(svg_content));
            out.push_str(&format!("</{tag}>"));
            if block.display {
                out.push('\n');
            }
        }
        Some(Err(e)) => {
            render_math_fallback(block, out, Some(e));
        }
        None => {
            render_math_fallback(block, out, None);
        }
    }
}

fn render_math_fallback(block: &crate::document::MathBlock, out: &mut String, error: Option<&str>) {
    let delim = if block.display { "$$" } else { "$" };
    if block.display {
        out.push_str("<div class=\"code-bg\" style=\"padding:8px;margin:4px 0;\">");
        if let Some(err) = error {
            out.push_str(&format!(
                "<div style=\"color:#f44336;font-size:0.9em;\">[render failed: {}]</div>",
                html_escape(err)
            ));
        }
        out.push_str(&format!(
            "<pre>{}{}{}</pre>",
            delim,
            html_escape(&block.source),
            delim
        ));
        out.push_str("</div>\n");
    } else {
        out.push_str(&format!(
            "<code>{}{}{}</code>",
            delim,
            html_escape(&block.source),
            delim
        ));
    }
}

fn render_spans(
    line: &StyledLine,
    out: &mut String,
    inline_math_blocks: &[&crate::document::MathBlock],
    inline_math_indices: &[usize],
    math_svgs: &HashMap<usize, Result<String, String>>,
) {
    let is_code_line = line.line_bg.is_some() && !line.is_code_border;
    let mut math_idx_iter = inline_math_indices.iter();
    let mut math_block_iter = inline_math_blocks.iter();
    for (i, span) in line.spans.iter().enumerate() {
        // Inline math: replace $...$ span with SVG
        if span.content.starts_with('$')
            && span.content.ends_with('$')
            && span.style.italic
            && let Some(block) = math_block_iter.next()
            && let Some(&block_idx) = math_idx_iter.next()
        {
            render_math_svg_from_cache(block, math_svgs, block_idx, out);
            continue;
        }
        // Skip code block pipe prefix " │ " in HTML export
        if is_code_line && is_code_pipe_prefix(span) {
            continue;
        }

        // Skip link reference numbers like [1] — they follow a link span
        if is_link_ref_number(span, i, &line.spans) {
            continue;
        }

        // Link spans → render as <a href>
        if let Some(url) = &span.link_url {
            let mut styles = Vec::new();
            if let Some(fg) = &span.style.fg {
                styles.push(format!("color:{}", color_to_hex(fg)));
            }
            if span.style.bold {
                styles.push("font-weight:bold".to_string());
            }
            if span.style.italic {
                styles.push("font-style:italic".to_string());
            }
            let style_attr = if styles.is_empty() {
                String::new()
            } else {
                format!(" style=\"{}\"", styles.join(";"))
            };
            out.push_str(&format!(
                "<a href=\"{}\"{}>{}</a>",
                sanitize_url(url),
                style_attr,
                html_escape(&span.content)
            ));
            continue;
        }

        let mut styles = Vec::new();

        if let Some(fg) = &span.style.fg {
            styles.push(format!("color:{}", color_to_hex(fg)));
        }
        if let Some(bg) = &span.style.bg {
            styles.push(format!("background-color:{}", color_to_hex(bg)));
        }
        if span.style.bold {
            styles.push("font-weight:bold".to_string());
        }
        if span.style.italic {
            styles.push("font-style:italic".to_string());
        }
        if span.style.underline {
            styles.push("text-decoration:underline".to_string());
        }
        if span.style.strikethrough {
            styles.push("text-decoration:line-through".to_string());
        }
        if span.style.dim {
            styles.push("opacity:0.6".to_string());
        }

        let has_style = !styles.is_empty();
        if has_style {
            out.push_str("<span style=\"");
            out.push_str(&styles.join(";"));
            out.push_str("\">");
        }

        out.push_str(&html_escape(&span.content));

        if has_style {
            out.push_str("</span>");
        }
    }
}

/// Check if a span is a code block pipe prefix " │ " (terminal-only).
fn is_code_pipe_prefix(span: &crate::document::StyledSpan) -> bool {
    span.style.dim && span.content.contains('│')
}

/// Check if a span is a link reference number like "[1]" that follows a link span.
fn is_link_ref_number(
    span: &crate::document::StyledSpan,
    idx: usize,
    spans: &[crate::document::StyledSpan],
) -> bool {
    if !span.style.dim || span.link_url.is_some() {
        return false;
    }
    // Pattern: "[N]" where N is digits
    let content = span.content.trim();
    if content.starts_with('[') && content.ends_with(']') {
        let inner = &content[1..content.len() - 1];
        if inner.chars().all(|c| c.is_ascii_digit()) {
            // Must follow a span that has a link_url
            if idx > 0 {
                return spans[idx - 1].link_url.is_some();
            }
        }
    }
    false
}

fn render_semantic_table(tb: &crate::document::TableBlock, out: &mut String) {
    out.push_str("<table>\n");
    for (row_idx, row) in tb.rows.iter().enumerate() {
        if row_idx == 0 {
            out.push_str("<thead><tr>");
        } else {
            if row_idx == 1 {
                out.push_str("<tbody>\n");
            }
            out.push_str("<tr>");
        }
        for (col_idx, cell) in row.iter().enumerate() {
            let tag = if row_idx == 0 { "th" } else { "td" };
            let align = tb.alignments.get(col_idx).map(|a| match a {
                pulldown_cmark::Alignment::Left => "left",
                pulldown_cmark::Alignment::Center => "center",
                pulldown_cmark::Alignment::Right => "right",
                pulldown_cmark::Alignment::None => "",
            });
            let align_attr = match align {
                Some(a) if !a.is_empty() => format!(" style=\"text-align:{}\"", a),
                _ => String::new(),
            };
            out.push_str(&format!("<{tag}{align_attr}>"));
            for span in cell {
                let escaped = html_escape(&span.content);
                if span.style.bold {
                    out.push_str(&format!("<strong>{escaped}</strong>"));
                } else if span.style.italic {
                    out.push_str(&format!("<em>{escaped}</em>"));
                } else if span.style.fg.is_some()
                    && matches!(span.style.fg, Some(crate::document::Color::Red))
                    && span.style.bg.is_some()
                {
                    out.push_str(&format!(
                        "<code style=\"background:{}\">{escaped}</code>",
                        color_to_hex(&Color::DarkGray)
                    ));
                } else {
                    out.push_str(&escaped);
                }
            }
            out.push_str(&format!("</{tag}>"));
        }
        if row_idx == 0 {
            out.push_str("</tr></thead>\n");
        } else {
            out.push_str("</tr>\n");
        }
    }
    if tb.rows.len() > 1 {
        out.push_str("</tbody>\n");
    }
    out.push_str("</table>\n");
}

/// Resolve a relative image path to an absolute `file:///` URL.
fn resolve_image_path(path: &str, base_dir: Option<&Path>) -> String {
    let trimmed = path.trim();
    // Remote URLs: leave as-is
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    // Already absolute or data URI: leave as-is
    if trimmed.starts_with('/') || trimmed.starts_with("file://") || trimmed.starts_with("data:") {
        return trimmed.to_string();
    }
    // Resolve relative path against base_dir, with traversal protection
    if let Some(base) = base_dir {
        let abs = base.join(trimmed);
        if let Ok(canonical) = abs.canonicalize() {
            // Prevent path traversal: resolved path must be under base_dir
            if let Ok(canonical_base) = base.canonicalize()
                && canonical.starts_with(&canonical_base)
            {
                return format!("file://{}", canonical.display());
            }
            // Path escapes base_dir — use relative path as-is (won't resolve)
            return trimmed.to_string();
        }
        // File doesn't exist — only allow if joined path stays under base_dir
        let normalized = abs.to_string_lossy();
        if !normalized.contains("..") {
            return format!("file://{}", abs.display());
        }
        return trimmed.to_string();
    }
    trimmed.to_string()
}

fn html_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

fn sanitize_url(url: &str) -> String {
    // Strip control characters and whitespace that could be used to bypass checks
    let cleaned: String = url
        .chars()
        .filter(|c| !c.is_control() && *c != '\n' && *c != '\r' && *c != '\t')
        .collect();
    let trimmed = cleaned.trim().to_lowercase();
    // Allowlist: only permit safe URL schemes
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with('#')
        || trimmed.starts_with("file://")
    {
        html_escape(&cleaned)
    } else if trimmed.contains(':') {
        // Block unknown schemes (javascript:, data:, vbscript:, etc.)
        "#".to_string()
    } else {
        // Relative paths are allowed
        html_escape(&cleaned)
    }
}

static RE_EVENT_DQ: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r#"(?i) on[a-z]+="[^"]*""#).unwrap());
static RE_EVENT_SQ: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r"(?i) on[a-z]+='[^']*'").unwrap());
static RE_EVENT_NQ: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r"(?i) on[a-z]+=\S+").unwrap());

/// Sanitize SVG from trusted sources (Mermaid/Math renderers).
/// Removes `<script>` tags and event handlers but preserves `<foreignObject>`
/// which Mermaid uses for text labels.
fn sanitize_svg(svg: &str) -> String {
    // Strip null bytes that could bypass tag detection
    let mut result = svg.replace('\0', "");
    // Remove <script>...</script> blocks
    while let Some(start) = result.to_lowercase().find("<script") {
        if let Some(end) = result.to_lowercase()[start..].find("</script>") {
            result = format!("{}{}", &result[..start], &result[start + end + 9..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    // Remove <iframe>...</iframe> blocks
    while let Some(start) = result.to_lowercase().find("<iframe") {
        if let Some(end) = result.to_lowercase()[start..].find("</iframe>") {
            result = format!("{}{}", &result[..start], &result[start + end + 9..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    // Remove <embed ... /> or <embed>...</embed> (self-closing or paired)
    while let Some(start) = result.to_lowercase().find("<embed") {
        if let Some(end) = result.to_lowercase()[start..].find("</embed>") {
            result = format!("{}{}", &result[..start], &result[start + end + 8..]);
        } else if let Some(end) = result[start..].find('>') {
            result = format!("{}{}", &result[..start], &result[start + end + 1..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    // Remove <object>...</object> blocks
    while let Some(start) = result.to_lowercase().find("<object") {
        if let Some(end) = result.to_lowercase()[start..].find("</object>") {
            result = format!("{}{}", &result[..start], &result[start + end + 9..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    // Remove <use ... /> or <use>...</use>
    while let Some(start) = result.to_lowercase().find("<use") {
        if let Some(end) = result.to_lowercase()[start..].find("</use>") {
            result = format!("{}{}", &result[..start], &result[start + end + 6..]);
        } else if let Some(end) = result[start..].find('>') {
            result = format!("{}{}", &result[..start], &result[start + end + 1..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    // Remove event handler attributes (on*) with double-quoted, single-quoted, or unquoted values
    result = RE_EVENT_DQ.replace_all(&result, "").to_string();
    result = RE_EVENT_SQ.replace_all(&result, "").to_string();
    result = RE_EVENT_NQ.replace_all(&result, "").to_string();
    result
}

fn color_to_hex(color: &Color) -> String {
    match color {
        Color::Cyan => "#00bcd4".to_string(),
        Color::Green => "#4caf50".to_string(),
        Color::Yellow => "#ffeb3b".to_string(),
        Color::Magenta => "#e040fb".to_string(),
        Color::Red => "#f44336".to_string(),
        Color::Blue => "#42a5f5".to_string(),
        Color::Gray => "#9e9e9e".to_string(),
        Color::DarkGray => "#282828".to_string(),
        Color::White => "#ffffff".to_string(),
        Color::Black => "#000000".to_string(),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
    }
}

fn html_head(css: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>mdskim export</title>
<style>
{css}
</style>
</head>
<body>
"#
    )
}

const HTML_TAIL: &str = "</body>\n</html>\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_svg_removes_double_quoted_event_handler() {
        let input = r#"<svg><rect onclick="alert(1)" width="10"/></svg>"#;
        let result = sanitize_svg(input);
        assert!(!result.contains("onclick"));
        assert!(result.contains(r#"width="10""#));
    }

    #[test]
    fn sanitize_svg_removes_single_quoted_event_handler() {
        let input = "<svg><rect onclick='alert(1)' width='10'/></svg>";
        let result = sanitize_svg(input);
        assert!(!result.contains("onclick"));
        assert!(result.contains("width='10'"));
    }

    #[test]
    fn sanitize_svg_removes_unquoted_event_handler() {
        let input = "<svg><rect onmouseover=alert(1) width=\"10\"/></svg>";
        let result = sanitize_svg(input);
        assert!(!result.contains("onmouseover"));
        assert!(result.contains(r#"width="10""#));
    }

    #[test]
    fn sanitize_svg_removes_script_tags_case_insensitive() {
        let input = "<svg><Script>alert(1)</Script></svg>";
        let result = sanitize_svg(input);
        assert!(!result.to_lowercase().contains("script"));
    }

    #[test]
    fn sanitize_svg_preserves_foreign_object_for_trusted() {
        let input = "<svg><foreignObject><div>label text</div></foreignObject></svg>";
        let result = sanitize_svg(input);
        assert!(result.contains("foreignObject"));
        assert!(result.contains("label text"));
    }

    #[test]
    fn sanitize_svg_does_not_remove_foreign_object_by_default() {
        // foreignObject is preserved for trusted SVG (Mermaid text labels)
        // but script tags and event handlers are still removed
        let input = "<svg><foreignObject><div>text</div></foreignObject><script>bad</script></svg>";
        let result = sanitize_svg(input);
        assert!(result.contains("foreignObject"));
        assert!(!result.to_lowercase().contains("script"));
    }

    #[test]
    fn sanitize_svg_preserves_safe_content() {
        let input =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="20"/></svg>"#;
        let result = sanitize_svg(input);
        assert_eq!(result, input);
    }

    #[test]
    fn sanitize_svg_removes_mixed_case_event_handler() {
        let input = r#"<svg><rect onClick="alert(1)" width="10"/></svg>"#;
        let result = sanitize_svg(input);
        assert!(!result.to_lowercase().contains("onclick"));
        assert!(result.contains(r#"width="10""#));
    }

    #[test]
    fn sanitize_svg_removes_uppercase_event_handler() {
        let input = r#"<svg><rect ONMOUSEOVER="alert(1)" width="10"/></svg>"#;
        let result = sanitize_svg(input);
        assert!(!result.to_lowercase().contains("onmouseover"));
        assert!(result.contains(r#"width="10""#));
    }

    // --- sanitize_url tests ---

    #[test]
    fn sanitize_url_allows_http() {
        let result = sanitize_url("http://example.com");
        assert!(result.contains("example.com"));
    }

    #[test]
    fn sanitize_url_allows_https() {
        let result = sanitize_url("https://example.com");
        assert!(result.contains("example.com"));
    }

    #[test]
    fn sanitize_url_allows_mailto() {
        let result = sanitize_url("mailto:user@example.com");
        assert!(result.contains("user@example.com"));
    }

    #[test]
    fn sanitize_url_blocks_javascript() {
        let result = sanitize_url("javascript:alert(1)");
        assert_eq!(result, "#");
    }

    #[test]
    fn sanitize_url_blocks_javascript_case_insensitive() {
        let result = sanitize_url("JavaScript:alert(1)");
        assert_eq!(result, "#");
    }

    #[test]
    fn sanitize_url_blocks_data_scheme() {
        let result = sanitize_url("data:text/html,<script>alert(1)</script>");
        assert_eq!(result, "#");
    }

    #[test]
    fn sanitize_url_allows_relative_path() {
        let result = sanitize_url("images/photo.png");
        assert!(result.contains("images/photo.png"));
    }

    #[test]
    fn sanitize_url_allows_anchor() {
        let result = sanitize_url("#section-1");
        assert!(result.contains("#section-1"));
    }

    #[test]
    fn sanitize_url_strips_control_chars() {
        let result = sanitize_url("https://example.com/\n/path");
        assert!(!result.contains('\n'));
    }

    // --- resolve_image_path tests ---

    #[test]
    fn resolve_image_path_remote_url_unchanged() {
        let result = resolve_image_path("https://example.com/img.png", None);
        assert_eq!(result, "https://example.com/img.png");
    }

    #[test]
    fn resolve_image_path_http_unchanged() {
        let result = resolve_image_path("http://example.com/img.png", None);
        assert_eq!(result, "http://example.com/img.png");
    }

    #[test]
    fn resolve_image_path_absolute_unchanged() {
        let result = resolve_image_path("/absolute/path/img.png", None);
        assert_eq!(result, "/absolute/path/img.png");
    }

    #[test]
    fn resolve_image_path_data_uri_unchanged() {
        let result = resolve_image_path("data:image/png;base64,abc", None);
        assert_eq!(result, "data:image/png;base64,abc");
    }

    #[test]
    fn resolve_image_path_traversal_blocked() {
        // Create a temp dir to serve as base_dir
        let base = std::env::temp_dir().join("mdskim-test-img-resolve");
        let _ = std::fs::create_dir_all(&base);
        // ../../../etc/passwd should be blocked
        let result = resolve_image_path("../../../etc/passwd", Some(&base));
        // Should either return the trimmed path (not file://) or block traversal
        assert!(
            !result.contains("file://")
                || result.starts_with("file://") && result.contains("mdskim-test-img-resolve"),
            "path traversal should be blocked: {result}"
        );
        let _ = std::fs::remove_dir(&base);
    }

    #[test]
    fn resolve_image_path_no_base_dir_returns_as_is() {
        let result = resolve_image_path("relative/img.png", None);
        assert_eq!(result, "relative/img.png");
    }

    // --- html_escape tests ---

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("&"), "&amp;");
        assert_eq!(html_escape("<"), "&lt;");
        assert_eq!(html_escape(">"), "&gt;");
        assert_eq!(html_escape("\""), "&quot;");
        assert_eq!(html_escape("'"), "&#39;");
    }

    #[test]
    fn html_escape_mixed_content() {
        let result = html_escape("<script>alert('xss')</script>");
        assert_eq!(result, "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
    }

    #[test]
    fn html_escape_plain_text() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    #[test]
    fn html_escape_empty_string() {
        assert_eq!(html_escape(""), "");
    }
}
