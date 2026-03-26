use pulldown_cmark::Alignment;
use unicode_width::UnicodeWidthStr;

use crate::document::{Color, SpanStyle, StyledLine, StyledSpan};

use super::ParseContext;

pub(super) fn pad_cell(text: &str, width: usize, alignment: Option<&Alignment>) -> String {
    let text_len = UnicodeWidthStr::width(text);
    if text_len >= width {
        return text.to_string();
    }
    let padding = width - text_len;
    match alignment {
        Some(Alignment::Center) => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
        Some(Alignment::Right) => {
            format!("{}{}", " ".repeat(padding), text)
        }
        _ => {
            format!("{}{}", text, " ".repeat(padding))
        }
    }
}

impl ParseContext {
    pub(super) fn render_table(&mut self) {
        if self.table.rows.is_empty() {
            return;
        }

        // Save table data for semantic HTML export
        let start_line_idx = self.lines.len();
        let table_block = crate::document::TableBlock {
            start_line_idx,
            end_line_idx: 0, // will be updated after rendering
            alignments: self.table.alignments.clone(),
            rows: self.table.rows.clone(),
        };
        let table_block_idx = self.table.blocks.len();
        self.table.blocks.push(table_block);

        let num_cols = self.table.rows.iter().map(|r| r.len()).max().unwrap_or(0);

        // Calculate column widths
        let mut col_widths = vec![0usize; num_cols];
        for row in &self.table.rows {
            for (col_idx, cell) in row.iter().enumerate() {
                let w: usize = cell
                    .iter()
                    .map(|s| UnicodeWidthStr::width(s.content.as_str()))
                    .sum();
                if col_idx < col_widths.len() {
                    col_widths[col_idx] = col_widths[col_idx].max(w);
                }
            }
        }

        let border_style = SpanStyle {
            fg: Some(Color::Gray),
            dim: true,
            ..Default::default()
        };

        let row_count = self.table.rows.len();

        // Top border (heavy)
        let top = format!(
            "┏{}┓",
            col_widths
                .iter()
                .map(|w| "━".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("┯")
        );
        self.lines.push(StyledLine::new(vec![StyledSpan {
            content: top,
            style: border_style.clone(),
            link_url: None,
        }]));

        for (row_idx, row) in self.table.rows.iter().enumerate() {
            // Data row
            let mut spans = Vec::new();
            spans.push(StyledSpan {
                content: "┃".to_string(),
                style: border_style.clone(),
                link_url: None,
            });
            for (col_idx, cell) in row.iter().enumerate() {
                let cell_text: String = cell.iter().map(|s| s.content.as_str()).collect();
                let width = col_widths.get(col_idx).copied().unwrap_or(0);
                let padded = pad_cell(&cell_text, width, self.table.alignments.get(col_idx));

                let cell_style = if row_idx == 0 {
                    SpanStyle {
                        fg: Some(Color::Cyan),
                        bold: true,
                        ..Default::default()
                    }
                } else if let Some(first) = cell.first() {
                    first.style.clone()
                } else {
                    SpanStyle::default()
                };

                spans.push(StyledSpan {
                    content: format!(" {} ", padded),
                    style: cell_style,
                    link_url: None,
                });
                // Inner column separator: thin │, outer: heavy ┃
                let sep = if col_idx == row.len() - 1 || col_idx == num_cols - 1 {
                    "┃"
                } else {
                    "│"
                };
                spans.push(StyledSpan {
                    content: sep.to_string(),
                    style: border_style.clone(),
                    link_url: None,
                });
            }
            // Pad missing columns
            for col_idx in row.len()..num_cols {
                let width = col_widths.get(col_idx).copied().unwrap_or(0);
                spans.push(StyledSpan {
                    content: format!(" {} ", " ".repeat(width)),
                    style: SpanStyle::default(),
                    link_url: None,
                });
                let sep = if col_idx == num_cols - 1 {
                    "┃"
                } else {
                    "│"
                };
                spans.push(StyledSpan {
                    content: sep.to_string(),
                    style: border_style.clone(),
                    link_url: None,
                });
            }
            self.lines.push(StyledLine::new(spans));

            // Separator after header (heavy) or between rows (thin)
            if row_idx == 0 {
                let sep = format!(
                    "┣{}┫",
                    col_widths
                        .iter()
                        .map(|w| "━".repeat(w + 2))
                        .collect::<Vec<_>>()
                        .join("┿")
                );
                self.lines.push(StyledLine::new(vec![StyledSpan {
                    content: sep,
                    style: border_style.clone(),
                    link_url: None,
                }]));
            } else if row_idx < row_count - 1 {
                let sep = format!(
                    "┠{}┨",
                    col_widths
                        .iter()
                        .map(|w| "─".repeat(w + 2))
                        .collect::<Vec<_>>()
                        .join("┼")
                );
                self.lines.push(StyledLine::new(vec![StyledSpan {
                    content: sep,
                    style: border_style.clone(),
                    link_url: None,
                }]));
            }
        }

        // Bottom border (heavy)
        let bottom = format!(
            "┗{}┛",
            col_widths
                .iter()
                .map(|w| "━".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("┷")
        );
        self.lines.push(StyledLine::new(vec![StyledSpan {
            content: bottom,
            style: border_style,
            link_url: None,
        }]));
        self.lines.push(StyledLine::empty());

        // Update end_line_idx for the table block
        self.table.blocks[table_block_idx].end_line_idx = self.lines.len() - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_cell_left_alignment() {
        let result = pad_cell("abc", 6, Some(&Alignment::Left));
        assert_eq!(result, "abc   ");
    }

    #[test]
    fn pad_cell_center_alignment() {
        let result = pad_cell("abc", 7, Some(&Alignment::Center));
        assert_eq!(result, "  abc  ");
    }

    #[test]
    fn pad_cell_right_alignment() {
        let result = pad_cell("abc", 6, Some(&Alignment::Right));
        assert_eq!(result, "   abc");
    }

    #[test]
    fn pad_cell_default_alignment() {
        // None alignment should behave like Left
        let result = pad_cell("abc", 6, None);
        assert_eq!(result, "abc   ");
    }

    #[test]
    fn pad_cell_text_equals_width() {
        let result = pad_cell("abc", 3, Some(&Alignment::Left));
        assert_eq!(result, "abc");
    }

    #[test]
    fn pad_cell_text_exceeds_width() {
        let result = pad_cell("abcdef", 3, Some(&Alignment::Left));
        assert_eq!(result, "abcdef");
    }

    #[test]
    fn pad_cell_unicode_width() {
        // CJK characters are 2 columns wide each
        let result = pad_cell("あ", 6, Some(&Alignment::Left));
        // "あ" is width 2, so 4 spaces of padding
        assert_eq!(result, "あ    ");
    }

    #[test]
    fn pad_cell_unicode_center() {
        let result = pad_cell("あ", 6, Some(&Alignment::Center));
        // width 2, padding 4, left=2, right=2
        assert_eq!(result, "  あ  ");
    }

    #[test]
    fn pad_cell_unicode_right() {
        let result = pad_cell("あ", 6, Some(&Alignment::Right));
        assert_eq!(result, "    あ");
    }
}
