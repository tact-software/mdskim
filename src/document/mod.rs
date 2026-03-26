use std::collections::HashMap;

/// Represents a parsed Markdown document as styled lines ready for display.
pub struct Document {
    pub(crate) lines: Vec<StyledLine>,
    pub(crate) headings: Vec<Heading>,
    pub(crate) links: Vec<Link>,
    pub(crate) mermaid_blocks: Vec<MermaidBlock>,
    pub(crate) math_blocks: Vec<MathBlock>,
    pub(crate) images: Vec<ImageRef>,
    pub(crate) table_blocks: Vec<TableBlock>,
    /// Pre-computed plain text for each line (spans joined), used by incremental search.
    pub(crate) plain_lines: Vec<String>,
    /// line_idx → block index for mermaid blocks.
    pub(crate) mermaid_line_map: HashMap<usize, usize>,
    /// line_idx → block index for display math blocks.
    pub(crate) math_line_map: HashMap<usize, usize>,
    /// line_idx → image index for document images.
    pub(crate) image_line_map: HashMap<usize, usize>,
}

/// A table preserved for semantic HTML export.
pub struct TableBlock {
    /// Line index of the first table line in `Document.lines`.
    pub(crate) start_line_idx: usize,
    /// Line index of the last table line (inclusive).
    pub(crate) end_line_idx: usize,
    /// Column alignments.
    pub(crate) alignments: Vec<pulldown_cmark::Alignment>,
    /// Rows of cells; first row is the header. Each cell contains styled spans.
    pub(crate) rows: Vec<Vec<Vec<StyledSpan>>>,
}

pub struct ImageRef {
    pub(crate) path: String,
    pub(crate) alt: String,
    pub(crate) line_idx: usize,
}

pub struct MermaidBlock {
    pub(crate) source: String,
    pub(crate) line_idx: usize,
    pub(crate) diagram_type: String,
}

pub struct MathBlock {
    pub(crate) source: String,
    pub(crate) line_idx: usize,
    pub(crate) display: bool,
}

pub struct Link {
    pub(crate) text: String,
    pub(crate) url: String,
    pub(crate) line_idx: usize,
}

pub struct Heading {
    pub(crate) level: usize,
    pub(crate) title: String,
    pub(crate) line_idx: usize,
}

/// A single line that may contain multiple styled spans.
#[derive(Clone, Debug)]
pub struct StyledLine {
    pub(crate) spans: Vec<StyledSpan>,
    /// Background color applied to the entire line (fills remaining width).
    pub(crate) line_bg: Option<Color>,
    /// If set, this line is a heading decoration belonging to the heading at this line_idx.
    pub(crate) heading_decoration_for: Option<usize>,
    /// If true, this line is a code block border (┌─, └─) — terminal-only.
    pub(crate) is_code_border: bool,
}

#[derive(Clone, Debug, Default)]
pub struct StyledSpan {
    pub(crate) content: String,
    pub(crate) style: SpanStyle,
    /// URL for link spans (used in HTML export to generate `<a>` tags).
    pub(crate) link_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SpanStyle {
    pub(crate) fg: Option<Color>,
    pub(crate) bg: Option<Color>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
    pub(crate) dim: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Color {
    Cyan,
    Green,
    Yellow,
    Magenta,
    Red,
    Blue,
    Gray,
    DarkGray,
    White,
    Black,
    Rgb(u8, u8, u8),
}

impl StyledLine {
    pub fn new(spans: Vec<StyledSpan>) -> Self {
        Self {
            spans,
            line_bg: None,
            heading_decoration_for: None,
            is_code_border: false,
        }
    }

    pub fn with_bg(spans: Vec<StyledSpan>, bg: Color) -> Self {
        Self {
            spans,
            line_bg: Some(bg),
            heading_decoration_for: None,
            is_code_border: false,
        }
    }

    pub fn code_border(spans: Vec<StyledSpan>, bg: Color) -> Self {
        Self {
            spans,
            line_bg: Some(bg),
            heading_decoration_for: None,
            is_code_border: true,
        }
    }

    pub fn decoration(spans: Vec<StyledSpan>, heading_line_idx: usize) -> Self {
        Self {
            spans,
            line_bg: None,
            heading_decoration_for: Some(heading_line_idx),
            is_code_border: false,
        }
    }

    pub fn empty() -> Self {
        Self {
            spans: vec![],
            line_bg: None,
            heading_decoration_for: None,
            is_code_border: false,
        }
    }
}

impl Document {
    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }
}
