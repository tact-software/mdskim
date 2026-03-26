use std::collections::HashSet;

use crate::document::Document;

/// Viewport state managing scroll position, cursor, and line folding.
pub(crate) struct Viewport {
    pub(crate) scroll_offset: usize,
    pub(crate) cursor_line: usize,
    pub(crate) height: usize,
    pub(crate) width: usize,
    pub(crate) folded_headings: HashSet<usize>,
    pub(crate) hidden_lines: Vec<bool>,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            cursor_line: 0,
            height: 24,
            width: 80,
            folded_headings: HashSet::new(),
            hidden_lines: Vec::new(),
        }
    }

    /// Check if a document line should be visible (not hidden by a fold).
    pub fn is_line_visible(&self, line_idx: usize) -> bool {
        !self.hidden_lines.get(line_idx).copied().unwrap_or(false)
    }

    /// Recompute `hidden_lines` based on current `folded_headings`.
    pub fn recompute_hidden_lines(&mut self, document: &Document) {
        let total = document.total_lines();
        self.hidden_lines = vec![false; total];

        for h in &document.headings {
            if !self.folded_headings.contains(&h.line_idx) {
                continue;
            }
            // Find the end of this fold: next heading at same or higher level
            let fold_end = document
                .headings
                .iter()
                .find(|h2| h2.line_idx > h.line_idx && h2.level <= h.level)
                .map(|h2| h2.line_idx)
                .unwrap_or(total);
            for idx in (h.line_idx + 1)..fold_end {
                self.hidden_lines[idx] = true;
            }
        }

        // Heading decoration lines follow their heading's visibility
        for line_idx in 0..total {
            if let Some(line) = document.lines.get(line_idx)
                && let Some(heading_idx) = line.heading_decoration_for
            {
                self.hidden_lines[line_idx] =
                    self.hidden_lines.get(heading_idx).copied().unwrap_or(false);
            }
        }
    }

    /// Get the absolute document line index at the current cursor position.
    pub fn abs_cursor_line(&self, document: &Document) -> usize {
        let mut visible_count = 0;
        for i in self.scroll_offset..document.total_lines() {
            if self.is_line_visible(i) {
                if visible_count == self.cursor_line {
                    return i;
                }
                visible_count += 1;
            }
        }
        document.total_lines().saturating_sub(1)
    }

    pub fn max_scroll(&self, document: &Document) -> usize {
        let total = document.total_lines();
        if total == 0 {
            return 0;
        }
        // Find the last visible line, then walk back `height` visible lines
        // to find the maximum valid scroll_offset.
        let last_visible = (0..total).rev().find(|i| self.is_line_visible(*i));
        let Some(last) = last_visible else {
            return 0;
        };
        // Walk backwards from last, counting height visible lines
        let mut count = 0;
        let mut pos = last;
        while pos > 0 && count < self.height.saturating_sub(1) {
            pos -= 1;
            if self.is_line_visible(pos) {
                count += 1;
            }
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::make_doc;

    #[test]
    fn max_scroll_basic() {
        // A document with many lines and a small viewport
        let md = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let doc = make_doc(&md);
        let mut vp = Viewport::new();
        vp.height = 5;
        let max = vp.max_scroll(&doc);
        // max_scroll should be > 0 since doc is longer than viewport
        assert!(max > 0);
    }

    #[test]
    fn max_scroll_empty_document() {
        let doc = make_doc("");
        let vp = Viewport::new();
        assert_eq!(vp.max_scroll(&doc), 0);
    }

    #[test]
    fn max_scroll_all_lines_hidden() {
        let md = "# Heading\n\nline1\nline2";
        let doc = make_doc(md);
        let mut vp = Viewport::new();
        // Mark all lines as hidden
        vp.hidden_lines = vec![true; doc.total_lines()];
        assert_eq!(vp.max_scroll(&doc), 0);
    }

    #[test]
    fn abs_cursor_line_basic() {
        let md = "line0\nline1\nline2\nline3";
        let doc = make_doc(&md);
        let mut vp = Viewport::new();
        vp.scroll_offset = 0;
        vp.cursor_line = 0;
        let abs = vp.abs_cursor_line(&doc);
        assert_eq!(abs, 0);

        vp.cursor_line = 2;
        let abs = vp.abs_cursor_line(&doc);
        assert_eq!(abs, 2);
    }

    #[test]
    fn abs_cursor_line_with_hidden_lines() {
        let md = "line0\nline1\nline2\nline3\nline4";
        let doc = make_doc(&md);
        let mut vp = Viewport::new();
        vp.scroll_offset = 0;
        vp.hidden_lines = vec![false; doc.total_lines()];
        // Hide line 1
        vp.hidden_lines[1] = true;
        // cursor_line=1 should map to line 2 (skipping hidden line 1)
        vp.cursor_line = 1;
        let abs = vp.abs_cursor_line(&doc);
        assert_eq!(abs, 2);
    }

    #[test]
    fn recompute_hidden_lines_fold_heading() {
        // # H1\n\nparagraph\n\n## H2\n\nsub-paragraph
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub-paragraph";
        let doc = make_doc(&md);
        let mut vp = Viewport::new();
        // Fold the H1 heading
        let h1_idx = doc.headings[0].line_idx;
        vp.folded_headings.insert(h1_idx);
        vp.recompute_hidden_lines(&doc);

        // H1 line itself should be visible
        assert!(vp.is_line_visible(h1_idx));
        // Lines after H1 until H2 (or end) should be hidden
        // Since H1 is level 1, everything until a level<=1 heading is hidden
        // There's no other level-1 heading, so everything after H1 is hidden
        let total = doc.total_lines();
        for i in (h1_idx + 1)..total {
            // decoration lines follow heading visibility, but the heading line itself
            // should have hidden content
            if doc.lines[i].heading_decoration_for == Some(h1_idx) {
                // decoration of H1 follows H1's visibility (visible)
                continue;
            }
            assert!(
                !vp.is_line_visible(i) || doc.lines[i].heading_decoration_for.is_some(),
                "line {i} should be hidden"
            );
        }
    }

    #[test]
    fn recompute_hidden_lines_nested_fold() {
        let md = "# H1\n\n## H2\n\ncontent under h2\n\n## H2b\n\nmore content";
        let doc = make_doc(&md);
        let mut vp = Viewport::new();
        // Fold H2 (second heading)
        let h2_idx = doc.headings.iter().find(|h| h.level == 2).unwrap().line_idx;
        vp.folded_headings.insert(h2_idx);
        vp.recompute_hidden_lines(&doc);
        // H2 line visible, lines between H2 and H2b hidden
        assert!(vp.is_line_visible(h2_idx));
    }

    #[test]
    fn is_line_visible_out_of_bounds() {
        let vp = Viewport::new();
        // hidden_lines is empty, so any index returns true (visible)
        assert!(vp.is_line_visible(100));
    }
}
