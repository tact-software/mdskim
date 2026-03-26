use crate::document::{Color, Link, StyledSpan};

use super::ParseContext;

impl ParseContext {
    /// Split text on bare URLs and push as spans, auto-linking detected URLs.
    pub(super) fn push_text_with_autolinks(&mut self, text: &str) {
        let mut remaining = text;
        while let Some(start) = remaining
            .find("https://")
            .or_else(|| remaining.find("http://"))
        {
            // Push text before the URL
            if start > 0 {
                self.current_spans.push(StyledSpan {
                    content: remaining[..start].to_string(),
                    style: self.current_style(),
                    link_url: None,
                });
            }
            // Find end of URL (stop at whitespace, closing paren/bracket, or common punctuation at end)
            let url_start = &remaining[start..];
            let end = url_start
                .find(|c: char| c.is_whitespace() || c == '>' || c == '<')
                .unwrap_or(url_start.len());
            let mut url = &url_start[..end];
            // Strip trailing punctuation that is likely not part of the URL
            while url.ends_with(['.', ',', ';', ':', '!', '?', ')', ']']) {
                url = &url[..url.len() - 1];
            }
            let url_len = url.len();

            let mut link_style = self.current_style();
            link_style.fg = Some(Color::Blue);
            link_style.underline = true;
            self.current_spans.push(StyledSpan {
                content: url.to_string(),
                style: link_style,
                link_url: Some(url.to_string()),
            });
            self.links.push(Link {
                url: url.to_string(),
                text: url.to_string(),
                line_idx: self.lines.len(),
            });

            remaining = &remaining[start + url_len..];
        }
        // Push any remaining text
        if !remaining.is_empty() {
            self.current_spans.push(StyledSpan {
                content: remaining.to_string(),
                style: self.current_style(),
                link_url: None,
            });
        }
    }
}
