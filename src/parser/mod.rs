mod autolink;
mod highlight;
mod table;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::parsing::SyntaxSet;

use crate::config::HeadingsConfig;
use crate::document::{
    Color, Document, Heading, ImageRef, Link, MathBlock, MermaidBlock, SpanStyle, StyledLine,
    StyledSpan,
};

use highlight::{DEFAULT_THEME_SET, highlight_code, load_syntax_set};

fn detect_mermaid_type(source: &str) -> String {
    let first_line = source.lines().next().unwrap_or("").trim().to_lowercase();
    if first_line.starts_with("flowchart") || first_line.starts_with("graph") {
        "flowchart".to_string()
    } else if first_line.starts_with("sequencediagram") {
        "sequence diagram".to_string()
    } else if first_line.starts_with("classdiagram") {
        "class diagram".to_string()
    } else if first_line.starts_with("statediagram") {
        "state diagram".to_string()
    } else if first_line.starts_with("erdiagram") {
        "ER diagram".to_string()
    } else if first_line.starts_with("gantt") {
        "gantt chart".to_string()
    } else if first_line.starts_with("pie") {
        "pie chart".to_string()
    } else if first_line.starts_with("gitgraph") {
        "git graph".to_string()
    } else {
        "diagram".to_string()
    }
}

pub fn parse_markdown(
    source: &str,
    headings_config: &HeadingsConfig,
    syntax_dir: Option<&str>,
) -> Document {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SMART_PUNCTUATION;
    let syntax_set = load_syntax_set(syntax_dir);
    let highlight_theme = DEFAULT_THEME_SET
        .themes
        .get("base16-eighties.dark")
        .or_else(|| DEFAULT_THEME_SET.themes.values().next())
        .expect("syntect DEFAULT_THEME_SET contains no themes");

    let mut ctx = ParseContext::default();

    if !try_parse(
        source,
        options,
        headings_config,
        syntax_set,
        highlight_theme,
        &mut ctx,
    ) {
        // Retry with definition list disabled (pulldown-cmark 0.12 bug:
        // blockquote + definition list can panic)
        let fallback_options = options & !Options::ENABLE_DEFINITION_LIST;
        ctx = ParseContext::default();
        eprintln!(
            "WARN: Parser error with definition lists enabled, retrying without. \
             Definition lists may not render."
        );
        try_parse(
            source,
            fallback_options,
            headings_config,
            syntax_set,
            highlight_theme,
            &mut ctx,
        );
    }

    ctx.flush_line();
    let plain_lines: Vec<String> = ctx
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_str()).collect())
        .collect();

    let mermaid_line_map: std::collections::HashMap<usize, usize> = ctx
        .mermaid_blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.line_idx, i))
        .collect();

    let math_line_map: std::collections::HashMap<usize, usize> = ctx
        .math_blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.display)
        .map(|(i, b)| (b.line_idx, i))
        .collect();

    let image_line_map: std::collections::HashMap<usize, usize> = ctx
        .images
        .iter()
        .enumerate()
        .map(|(i, img)| (img.line_idx, i))
        .collect();

    Document {
        lines: ctx.lines,
        headings: ctx.headings,
        links: ctx.links,
        mermaid_blocks: ctx.mermaid_blocks,
        math_blocks: ctx.math_blocks,
        images: ctx.images,
        table_blocks: ctx.table.blocks,
        plain_lines,
        mermaid_line_map,
        math_line_map,
        image_line_map,
    }
}

/// Run the pulldown-cmark parser, returning true on success or false if it panicked.
fn try_parse(
    source: &str,
    options: Options,
    headings_config: &HeadingsConfig,
    syntax_set: &syntect::parsing::SyntaxSet,
    highlight_theme: &syntect::highlighting::Theme,
    ctx: &mut ParseContext,
) -> bool {
    let source_owned = source.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let parser = Parser::new_ext(&source_owned, options);
        for event in parser {
            match event {
                Event::Start(tag) => ctx.handle_start(tag, headings_config),
                Event::End(tag_end) => {
                    ctx.handle_end(tag_end, headings_config, syntax_set, highlight_theme)
                }
                Event::Text(text) => ctx.handle_text(&text),
                Event::Code(code) => ctx.handle_inline_code(&code),
                Event::SoftBreak | Event::HardBreak => ctx.flush_line(),
                Event::Rule => ctx.handle_rule(),
                Event::TaskListMarker(checked) => ctx.handle_task_marker(checked),
                Event::FootnoteReference(label) => ctx.handle_footnote_ref(&label),
                Event::InlineHtml(html) | Event::Html(html) => ctx.handle_inline_html(&html),
                Event::InlineMath(math) => ctx.handle_inline_math(&math),
                Event::DisplayMath(math) => ctx.handle_display_math(&math),
            }
        }
    }));
    result.is_ok()
}

struct ListState {
    ordered_index: Option<u64>,
}

#[derive(Default)]
struct TableState {
    in_table: bool,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Vec<StyledSpan>>>, // rows -> cells -> spans
    current_cell: Vec<StyledSpan>,
    current_row: Vec<Vec<StyledSpan>>,
    blocks: Vec<crate::document::TableBlock>,
}

#[derive(Default)]
struct CodeBlockState {
    in_code_block: bool,
    lang: Option<String>,
    buffer: String,
    in_mermaid: bool,
}

#[derive(Default)]
struct ParseContext {
    lines: Vec<StyledLine>,
    current_spans: Vec<StyledSpan>,
    style_stack: Vec<SpanStyle>,
    code: CodeBlockState,
    list_stack: Vec<ListState>,
    blockquote_depth: usize,
    headings: Vec<Heading>,
    current_heading_level: Option<usize>,
    heading_text_buf: String,
    links: Vec<Link>,
    current_link_url: Option<String>,
    link_text_buf: String,
    in_item: bool,
    item_paragraph_count: usize,
    mermaid_blocks: Vec<MermaidBlock>,
    math_blocks: Vec<MathBlock>,
    overline_idx: Option<usize>,
    images: Vec<ImageRef>,
    current_image_url: Option<String>,
    image_alt_buf: String,
    table: TableState,
}

const CODE_BG: Color = Color::DarkGray;
const CODE_BORDER: SpanStyle = SpanStyle {
    fg: Some(Color::Gray),
    bg: Some(CODE_BG),
    bold: false,
    italic: false,
    underline: false,
    strikethrough: false,
    dim: true,
};

impl ParseContext {
    fn list_depth(&self) -> usize {
        self.list_stack.len()
    }

    fn handle_start(&mut self, tag: Tag, headings_config: &HeadingsConfig) {
        match tag {
            Tag::Heading { level, .. } => {
                let lvl = level as usize;
                let hstyle = headings_config.for_level(lvl);
                let style = SpanStyle {
                    fg: Some(Color::Cyan),
                    bold: hstyle.bold,
                    dim: hstyle.dim,
                    ..Default::default()
                };
                // Overline decoration
                if let Some(ch) = hstyle.decoration.overline_char() {
                    self.overline_idx = Some(self.lines.len());
                    // Use 0 as placeholder; will be updated in handle_end
                    self.lines.push(StyledLine::decoration(
                        vec![StyledSpan {
                            content: ch.repeat(40),
                            style: SpanStyle {
                                fg: Some(Color::Cyan),
                                dim: hstyle.dim,
                                ..Default::default()
                            },
                            link_url: None,
                        }],
                        0,
                    ));
                } else {
                    self.overline_idx = None;
                }
                self.current_spans.push(StyledSpan {
                    content: format!("{} ", "#".repeat(lvl)),
                    style: style.clone(),
                    link_url: None,
                });
                self.style_stack.push(style);
                self.current_heading_level = Some(lvl);
                self.heading_text_buf.clear();
            }
            Tag::Paragraph => {
                if self.in_item {
                    self.item_paragraph_count += 1;
                    if self.item_paragraph_count > 1 {
                        // Continuation paragraph in loose list — flush and indent
                        self.flush_line();
                        self.push_quote_prefix();
                        let depth = self.list_depth();
                        let indent = "  ".repeat(depth);
                        self.current_spans.push(StyledSpan {
                            content: indent,
                            style: SpanStyle::default(),
                            link_url: None,
                        });
                    }
                    // First paragraph: text continues after marker on same line
                }
                self.style_stack.push(self.current_style());
            }
            Tag::Strong => {
                let mut s = self.current_style();
                s.bold = true;
                s.fg = Some(Color::Yellow);
                self.style_stack.push(s);
            }
            Tag::Emphasis => {
                let mut s = self.current_style();
                s.italic = true;
                s.fg = Some(Color::Green);
                self.style_stack.push(s);
            }
            Tag::Strikethrough => {
                let mut s = self.current_style();
                s.strikethrough = true;
                s.fg = Some(Color::Gray);
                self.style_stack.push(s);
            }
            Tag::CodeBlock(kind) => {
                self.code.buffer.clear();
                self.code.lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string();
                        if l.is_empty() { None } else { Some(l) }
                    }
                    CodeBlockKind::Indented => None,
                };

                if self
                    .code
                    .lang
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case("mermaid"))
                {
                    self.code.in_mermaid = true;
                    self.code.in_code_block = true; // still collect text
                } else {
                    self.code.in_code_block = true;
                    self.flush_line();
                    let mut top_spans = self.make_quote_spans();
                    top_spans.push(StyledSpan {
                        content: match &self.code.lang {
                            Some(lang) => format!(" ┌─ {} {}", lang, "─".repeat(40)),
                            None => format!(" ┌{}", "─".repeat(43)),
                        },
                        style: CODE_BORDER,
                        link_url: None,
                    });
                    self.lines.push(StyledLine::code_border(top_spans, CODE_BG));
                }
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.blockquote_depth += 1;
                let mut s = self.current_style();
                s.fg = Some(Color::Magenta);
                s.italic = true;
                self.style_stack.push(s);
            }
            Tag::List(start) => {
                self.flush_line();
                self.list_stack.push(ListState {
                    ordered_index: start,
                });
            }
            Tag::Item => {
                self.flush_line();
                self.in_item = true;
                self.item_paragraph_count = 0;
                self.push_quote_prefix();
                let depth = self.list_depth();
                let indent = "  ".repeat(depth.saturating_sub(1));

                let marker = if let Some(list) = self.list_stack.last_mut() {
                    if let Some(ref mut idx) = list.ordered_index {
                        let m = format!("{indent}{}. ", idx);
                        *idx += 1;
                        m
                    } else {
                        format!("{indent}• ")
                    }
                } else {
                    "• ".to_string()
                };

                self.current_spans.push(StyledSpan {
                    content: marker,
                    style: SpanStyle {
                        fg: Some(Color::Yellow),
                        ..Default::default()
                    },
                    link_url: None,
                });
            }
            Tag::MetadataBlock(_) => {
                self.style_stack.push(self.current_style());
            }
            Tag::Link { dest_url, .. } => {
                let mut s = self.current_style();
                s.fg = Some(Color::Blue);
                s.underline = true;
                self.style_stack.push(s);
                self.current_link_url = Some(dest_url.to_string());
                self.link_text_buf.clear();
            }
            Tag::Image { dest_url, .. } => {
                self.style_stack.push(self.current_style());
                self.current_image_url = Some(dest_url.to_string());
                self.image_alt_buf.clear();
                self.current_spans.push(StyledSpan {
                    content: "[🖼 ".to_string(),
                    style: SpanStyle {
                        fg: Some(Color::Gray),
                        ..Default::default()
                    },
                    link_url: None,
                });
                let _ = dest_url;
            }
            Tag::Table(alignments) => {
                self.flush_line();
                self.table.in_table = true;
                self.table.alignments = alignments;
                self.table.rows.clear();
            }
            Tag::TableHead => {
                self.table.current_row.clear();
            }
            Tag::TableRow => {
                self.table.current_row.clear();
            }
            Tag::TableCell => {
                self.table.current_cell.clear();
                self.style_stack.push(self.current_style());
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_line();
                self.current_spans.push(StyledSpan {
                    content: format!("[^{}]: ", label),
                    style: SpanStyle {
                        fg: Some(Color::Cyan),
                        dim: true,
                        ..Default::default()
                    },
                    link_url: None,
                });
                self.style_stack.push(SpanStyle {
                    fg: Some(Color::Gray),
                    ..Default::default()
                });
            }
            Tag::HtmlBlock => {
                self.style_stack.push(SpanStyle {
                    fg: Some(Color::Gray),
                    dim: true,
                    ..Default::default()
                });
            }
            Tag::DefinitionList => {
                self.flush_line();
                self.style_stack.push(self.current_style());
            }
            Tag::DefinitionListTitle => {
                self.flush_line();
                self.style_stack.push(SpanStyle {
                    bold: true,
                    fg: Some(Color::Yellow),
                    ..Default::default()
                });
            }
            Tag::DefinitionListDefinition => {
                self.flush_line();
                self.current_spans.push(StyledSpan {
                    content: "  ".to_string(),
                    style: SpanStyle::default(),
                    link_url: None,
                });
                self.style_stack.push(SpanStyle {
                    fg: Some(Color::Gray),
                    ..Default::default()
                });
            }
        }
    }

    fn handle_end(
        &mut self,
        tag_end: TagEnd,
        headings_config: &HeadingsConfig,
        syntax_set: &SyntaxSet,
        highlight_theme: &syntect::highlighting::Theme,
    ) {
        match tag_end {
            TagEnd::Heading(_) => {
                if let Some(level) = self.current_heading_level.take() {
                    let hstyle = headings_config.for_level(level);
                    self.style_stack.pop();
                    self.flush_line();
                    let heading_line_idx = self.lines.len().saturating_sub(1);
                    self.headings.push(Heading {
                        level,
                        title: self.heading_text_buf.clone(),
                        line_idx: heading_line_idx,
                    });
                    self.heading_text_buf.clear();
                    // Fix overline's heading reference
                    if let Some(ol_idx) = self.overline_idx.take()
                        && let Some(ol_line) = self.lines.get_mut(ol_idx)
                    {
                        ol_line.heading_decoration_for = Some(heading_line_idx);
                    }
                    // Underline decoration
                    if let Some(ch) = hstyle.decoration.underline_char() {
                        self.lines.push(StyledLine::decoration(
                            vec![StyledSpan {
                                content: ch.repeat(40),
                                style: SpanStyle {
                                    fg: Some(Color::Cyan),
                                    dim: hstyle.dim,
                                    ..Default::default()
                                },
                                link_url: None,
                            }],
                            heading_line_idx,
                        ));
                    }
                    self.lines.push(StyledLine::empty());
                } else {
                    self.style_stack.pop();
                    self.flush_line();
                    self.lines.push(StyledLine::empty());
                }
            }
            TagEnd::Paragraph => {
                self.style_stack.pop();
                self.flush_line();
                if !self.in_item {
                    self.lines.push(StyledLine::empty());
                }
            }
            TagEnd::CodeBlock => {
                self.code.in_code_block = false;

                if self.code.in_mermaid {
                    self.code.in_mermaid = false;
                    let source = self.code.buffer.clone();
                    let diagram_type = detect_mermaid_type(&source);
                    let line_idx = self.lines.len();
                    self.mermaid_blocks.push(MermaidBlock {
                        source,
                        line_idx,
                        diagram_type: diagram_type.clone(),
                    });
                    // Placeholder display
                    self.flush_line();
                    self.lines.push(StyledLine::with_bg(
                        vec![StyledSpan {
                            content: format!(" 📊 Mermaid: {} ", diagram_type),
                            style: SpanStyle {
                                fg: Some(Color::Cyan),
                                bold: true,
                                ..Default::default()
                            },
                            link_url: None,
                        }],
                        CODE_BG,
                    ));
                    self.lines.push(StyledLine::with_bg(
                        vec![StyledSpan {
                            content: " [diagram rendered inline on supported terminals]"
                                .to_string(),
                            style: SpanStyle {
                                fg: Some(Color::Gray),
                                dim: true,
                                ..Default::default()
                            },
                            link_url: None,
                        }],
                        CODE_BG,
                    ));
                    self.lines.push(StyledLine::empty());
                    self.code.buffer.clear();
                    self.code.lang = None;
                } else {
                    let highlighted = highlight_code(
                        &self.code.buffer,
                        self.code.lang.as_deref(),
                        syntax_set,
                        highlight_theme,
                    );
                    for hl_spans in &highlighted {
                        let mut spans = self.make_quote_spans();
                        spans.push(StyledSpan {
                            content: " │ ".to_string(),
                            style: CODE_BORDER,
                            link_url: None,
                        });
                        spans.extend(hl_spans.iter().cloned());
                        self.lines.push(StyledLine::with_bg(spans, CODE_BG));
                    }
                    let mut bottom_spans = self.make_quote_spans();
                    bottom_spans.push(StyledSpan {
                        content: format!(" └{}", "─".repeat(41)),
                        style: CODE_BORDER,
                        link_url: None,
                    });
                    self.lines
                        .push(StyledLine::code_border(bottom_spans, CODE_BG));
                    self.lines.push(StyledLine::empty());
                    self.code.buffer.clear();
                    self.code.lang = None;
                }
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
                self.style_stack.pop();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.lines.push(StyledLine::empty());
                }
            }
            TagEnd::Item => {
                let was_loose = self.item_paragraph_count > 1;
                self.in_item = false;
                self.style_stack.pop();
                self.flush_line();
                if was_loose {
                    self.lines.push(StyledLine::empty());
                }
            }
            TagEnd::Strikethrough => {
                self.style_stack.pop();
            }
            TagEnd::Link => {
                if let Some(url) = self.current_link_url.take() {
                    let line_idx = self.lines.len();
                    let link_num = self.links.len() + 1;
                    self.links.push(Link {
                        text: self.link_text_buf.clone(),
                        url: url.clone(),
                        line_idx,
                    });
                    // Retroactively set link_url on the preceding link text spans
                    if !self.link_text_buf.is_empty() {
                        let mut remaining = self.link_text_buf.len();
                        for span in self.current_spans.iter_mut().rev() {
                            if span.style.underline && span.link_url.is_none() {
                                span.link_url = Some(url.clone());
                                if span.content.len() >= remaining {
                                    break;
                                }
                                remaining = remaining.saturating_sub(span.content.len());
                            } else {
                                break;
                            }
                        }
                    }
                    self.current_spans.push(StyledSpan {
                        content: format!("[{}]", link_num),
                        style: SpanStyle {
                            fg: Some(Color::Gray),
                            dim: true,
                            ..Default::default()
                        },
                        link_url: None,
                    });
                    self.link_text_buf.clear();
                }
                self.style_stack.pop();
            }
            TagEnd::Image => {
                self.current_spans.push(StyledSpan {
                    content: "]".to_string(),
                    style: SpanStyle {
                        fg: Some(Color::Gray),
                        ..Default::default()
                    },
                    link_url: None,
                });
                if let Some(url) = self.current_image_url.take() {
                    let line_idx = self.lines.len();
                    self.images.push(ImageRef {
                        path: url,
                        alt: self.image_alt_buf.clone(),
                        line_idx,
                    });
                }
                self.style_stack.pop();
            }
            TagEnd::FootnoteDefinition => {
                self.style_stack.pop();
                self.flush_line();
            }
            TagEnd::Table => {
                self.table.in_table = false;
                self.render_table();
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = std::mem::take(&mut self.table.current_row);
                self.table.rows.push(row);
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.table.current_cell);
                self.table.current_row.push(cell);
                self.style_stack.pop();
            }
            TagEnd::DefinitionList => {
                self.style_stack.pop();
                self.lines.push(StyledLine::empty());
            }
            TagEnd::DefinitionListTitle => {
                self.style_stack.pop();
                self.flush_line();
            }
            TagEnd::DefinitionListDefinition => {
                self.style_stack.pop();
                self.flush_line();
            }
            _ => {
                self.style_stack.pop();
            }
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.current_heading_level.is_some() {
            self.heading_text_buf.push_str(text);
        }
        if self.current_image_url.is_some() {
            self.image_alt_buf.push_str(text);
        }
        if self.current_link_url.is_some() {
            self.link_text_buf.push_str(text);
        }
        if self.code.in_code_block {
            self.code.buffer.push_str(text);
            return;
        }
        if self.table.in_table {
            self.table.current_cell.push(StyledSpan {
                content: text.to_string(),
                style: self.current_style(),
                link_url: None,
            });
            return;
        }
        // Inside a link already — don't detect nested URLs
        let detect_urls = self.current_link_url.is_none();

        if self.blockquote_depth > 0 {
            for (i, line) in text.lines().enumerate() {
                if i > 0 {
                    self.flush_line();
                }
                if i == 0 && self.current_spans.is_empty() {
                    self.push_quote_prefix();
                }
                if detect_urls {
                    self.push_text_with_autolinks(line);
                } else {
                    self.current_spans.push(StyledSpan {
                        content: line.to_string(),
                        style: self.current_style(),
                        link_url: None,
                    });
                }
            }
            return;
        }
        if detect_urls {
            self.push_text_with_autolinks(text);
        } else {
            self.current_spans.push(StyledSpan {
                content: text.to_string(),
                style: self.current_style(),
                link_url: None,
            });
        }
    }

    fn handle_inline_code(&mut self, code: &str) {
        let span = StyledSpan {
            content: format!("`{}`", code),
            style: SpanStyle {
                fg: Some(Color::Red),
                bg: Some(Color::DarkGray),
                ..Default::default()
            },
            link_url: None,
        };
        if self.table.in_table {
            self.table.current_cell.push(span);
        } else {
            self.current_spans.push(span);
        }
    }

    fn handle_task_marker(&mut self, checked: bool) {
        let marker = if checked { "☑ " } else { "☐ " };
        self.current_spans.push(StyledSpan {
            content: marker.to_string(),
            style: SpanStyle {
                fg: Some(if checked { Color::Green } else { Color::Gray }),
                ..Default::default()
            },
            link_url: None,
        });
    }

    fn handle_footnote_ref(&mut self, label: &str) {
        self.current_spans.push(StyledSpan {
            content: format!("[^{}]", label),
            style: SpanStyle {
                fg: Some(Color::Cyan),
                bold: true,
                ..Default::default()
            },
            link_url: None,
        });
    }

    fn handle_inline_math(&mut self, math: &str) {
        // Register as MathBlock for SVG rendering in HTML export
        let line_idx = self.lines.len();
        self.math_blocks.push(MathBlock {
            source: math.to_string(),
            line_idx,
            display: false,
        });
        // Terminal fallback: styled text
        self.current_spans.push(StyledSpan {
            content: format!("${math}$"),
            style: SpanStyle {
                fg: Some(Color::Cyan),
                italic: true,
                ..Default::default()
            },
            link_url: None,
        });
    }

    fn handle_display_math(&mut self, math: &str) {
        self.flush_line();
        let line_idx = self.lines.len();
        self.math_blocks.push(MathBlock {
            source: math.to_string(),
            line_idx,
            display: true,
        });
        // Placeholder for terminal display
        self.lines.push(StyledLine::with_bg(
            vec![StyledSpan {
                content: " 📐 Math (display) ".to_string(),
                style: SpanStyle {
                    fg: Some(Color::Cyan),
                    bold: true,
                    ..Default::default()
                },
                link_url: None,
            }],
            CODE_BG,
        ));
        // Show source as fallback text
        for line in math.lines() {
            self.lines.push(StyledLine::with_bg(
                vec![StyledSpan {
                    content: format!("  {line}"),
                    style: SpanStyle {
                        fg: Some(Color::Cyan),
                        italic: true,
                        ..Default::default()
                    },
                    link_url: None,
                }],
                CODE_BG,
            ));
        }
        self.lines.push(StyledLine::empty());
    }

    fn handle_inline_html(&mut self, html: &str) {
        // Render <kbd>...</kbd> with special styling, others as dim text
        let trimmed = html.trim();
        if trimmed.starts_with("<kbd>") || trimmed.starts_with("</kbd>") {
            // Extract content or just show the tag
            if let Some(content) = trimmed
                .strip_prefix("<kbd>")
                .and_then(|s| s.strip_suffix("</kbd>"))
            {
                self.current_spans.push(StyledSpan {
                    content: format!("[{}]", content),
                    style: SpanStyle {
                        fg: Some(Color::Yellow),
                        bold: true,
                        ..Default::default()
                    },
                    link_url: None,
                });
                return;
            }
        }
        // Generic inline HTML: show as dim text
        self.current_spans.push(StyledSpan {
            content: html.to_string(),
            style: SpanStyle {
                fg: Some(Color::Gray),
                dim: true,
                ..Default::default()
            },
            link_url: None,
        });
    }

    fn handle_rule(&mut self) {
        self.flush_line();
        self.lines.push(StyledLine::new(vec![StyledSpan {
            content: "─".repeat(40),
            style: SpanStyle {
                fg: Some(Color::Gray),
                dim: true,
                ..Default::default()
            },
            link_url: None,
        }]));
        self.lines.push(StyledLine::empty());
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            self.lines
                .push(StyledLine::new(std::mem::take(&mut self.current_spans)));
        }
    }

    fn current_style(&self) -> SpanStyle {
        self.style_stack.last().cloned().unwrap_or_default()
    }

    /// Push blockquote prefix markers (▎) to current_spans if inside a blockquote.
    fn push_quote_prefix(&mut self) {
        if self.blockquote_depth > 0 {
            let marker = "▎ ".repeat(self.blockquote_depth);
            self.current_spans.push(StyledSpan {
                content: marker,
                style: SpanStyle {
                    fg: Some(Color::Magenta),
                    ..Default::default()
                },
                link_url: None,
            });
        }
    }

    /// Create a vec of quote prefix spans (for use in lines pushed directly).
    fn make_quote_spans(&self) -> Vec<StyledSpan> {
        if self.blockquote_depth > 0 {
            vec![StyledSpan {
                content: "▎ ".repeat(self.blockquote_depth),
                style: SpanStyle {
                    fg: Some(Color::Magenta),
                    ..Default::default()
                },
                link_url: None,
            }]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HeadingsConfig;

    fn parse(source: &str) -> Document {
        parse_markdown(source, &HeadingsConfig::default(), None)
    }

    #[test]
    fn test_empty_input() {
        let doc = parse("");
        assert!(doc.lines.is_empty() || doc.lines.len() == 1);
    }

    #[test]
    fn test_headings() {
        let doc = parse("# H1\n## H2\n### H3\n");
        assert_eq!(doc.headings.len(), 3);
        assert_eq!(doc.headings[0].level, 1);
        assert_eq!(doc.headings[1].level, 2);
        assert_eq!(doc.headings[2].level, 3);
    }

    #[test]
    fn test_links() {
        let doc = parse("[Example](https://example.com)\n");
        assert!(!doc.links.is_empty());
        assert_eq!(doc.links[0].url, "https://example.com");
    }

    #[test]
    fn test_bare_url_autolink() {
        let doc = parse("Visit https://example.com today.\n");
        assert!(!doc.links.is_empty());
        assert_eq!(doc.links[0].url, "https://example.com");
    }

    #[test]
    fn test_images() {
        let doc = parse("![alt](image.png)\n");
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].path, "image.png");
        assert_eq!(doc.images[0].alt, "alt");
    }

    #[test]
    fn test_mermaid_block() {
        let doc = parse("```mermaid\ngraph TD\n  A-->B\n```\n");
        assert_eq!(doc.mermaid_blocks.len(), 1);
        assert!(doc.mermaid_blocks[0].source.contains("A-->B"));
    }

    #[test]
    fn test_math_inline() {
        let doc = parse("Einstein: $E=mc^2$\n");
        assert!(!doc.math_blocks.is_empty());
        assert!(!doc.math_blocks[0].display);
    }

    #[test]
    fn test_math_display() {
        let doc = parse("$$\nx^2 + y^2 = z^2\n$$\n");
        assert!(!doc.math_blocks.is_empty());
        assert!(doc.math_blocks[0].display);
    }

    #[test]
    fn test_table() {
        let doc = parse("| A | B |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(doc.table_blocks.len(), 1);
        assert_eq!(doc.table_blocks[0].rows.len(), 2); // header + 1 data row
    }

    #[test]
    fn test_code_block() {
        let doc = parse("```rust\nfn main() {}\n```\n");
        // Should have lines with code_bg set
        let code_lines: Vec<_> = doc.lines.iter().filter(|l| l.line_bg.is_some()).collect();
        assert!(!code_lines.is_empty());
    }

    #[test]
    fn test_parser_panic_recovery() {
        // Blockquote + definition list triggers pulldown-cmark panic
        // Parser should recover gracefully
        let doc = parse("> Term\n> : Definition\n\nAfter.\n");
        // Should not panic, and should produce some output
        assert!(!doc.lines.is_empty());
    }
}
