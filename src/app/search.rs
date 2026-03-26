use crate::document::Document;

/// State for incremental text search across the document.
#[derive(Default)]
pub(crate) struct SearchState {
    pub(crate) query: String,
    pub(crate) matches: Vec<SearchMatch>,
    pub(crate) current_match: Option<usize>,
    pub(crate) active_input: bool,
    pub(crate) input_buf: String,
}

pub(crate) struct SearchMatch {
    pub(crate) line_idx: usize,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn search(&mut self, doc: &Document) {
        self.matches.clear();
        self.current_match = None;
        if self.query.is_empty() {
            return;
        }
        let query_lower = self.query.to_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();
        for (line_idx, text) in doc.plain_lines.iter().enumerate() {
            // Match on the original text using char-by-char case-insensitive comparison
            // to keep byte offsets aligned with the original text.
            let chars: Vec<(usize, char)> = text.char_indices().collect();
            let mut ci = 0;
            while ci + query_chars.len() <= chars.len() {
                let matched = chars[ci..ci + query_chars.len()]
                    .iter()
                    .zip(&query_chars)
                    .all(|((_, c), q)| c.to_lowercase().eq(std::iter::once(*q)));
                if matched {
                    let byte_start = chars[ci].0;
                    let byte_end = if ci + query_chars.len() < chars.len() {
                        chars[ci + query_chars.len()].0
                    } else {
                        text.len()
                    };
                    self.matches.push(SearchMatch {
                        line_idx,
                        byte_start,
                        byte_end,
                    });
                    ci += query_chars.len();
                } else {
                    ci += 1;
                }
            }
        }
        if !self.matches.is_empty() {
            self.current_match = Some(0);
        }
    }

    pub fn jump_to_next(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        });
    }

    pub fn jump_to_prev(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(0) => self.matches.len() - 1,
            Some(i) => i - 1,
            None => self.matches.len() - 1,
        });
    }

    pub fn jump_to_nearest(&mut self, scroll_offset: usize) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }
        let idx = self
            .matches
            .iter()
            .position(|m| m.line_idx >= scroll_offset)
            .unwrap_or(0);
        self.current_match = Some(idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::make_doc;

    #[test]
    fn search_basic_match() {
        let doc = make_doc("hello world\nfoo bar\nhello again");
        let mut s = SearchState::new();
        s.query = "hello".to_string();
        s.search(&doc);
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.matches[0].line_idx, 0);
        assert_eq!(s.matches[1].line_idx, 2);
        assert_eq!(s.current_match, Some(0));
    }

    #[test]
    fn search_case_insensitive() {
        let doc = make_doc("Hello World\nHELLO again");
        let mut s = SearchState::new();
        s.query = "hello".to_string();
        s.search(&doc);
        assert_eq!(s.matches.len(), 2);
    }

    #[test]
    fn search_empty_query_no_match() {
        let doc = make_doc("hello world");
        let mut s = SearchState::new();
        s.query = String::new();
        s.search(&doc);
        assert!(s.matches.is_empty());
        assert_eq!(s.current_match, None);
    }

    #[test]
    fn search_multibyte_japanese() {
        let doc = make_doc("日本語のテスト\nこんにちは世界");
        let mut s = SearchState::new();
        s.query = "こんにちは".to_string();
        s.search(&doc);
        assert_eq!(s.matches.len(), 1);
    }

    #[test]
    fn jump_to_next_wraps_around() {
        let doc = make_doc("aaa\nbbb\naaa");
        let mut s = SearchState::new();
        s.query = "aaa".to_string();
        s.search(&doc);
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.current_match, Some(0));
        s.jump_to_next();
        assert_eq!(s.current_match, Some(1));
        s.jump_to_next();
        assert_eq!(s.current_match, Some(0)); // wrap
    }

    #[test]
    fn jump_to_prev_wraps_around() {
        let doc = make_doc("aaa\nbbb\naaa");
        let mut s = SearchState::new();
        s.query = "aaa".to_string();
        s.search(&doc);
        assert_eq!(s.current_match, Some(0));
        s.jump_to_prev();
        assert_eq!(s.current_match, Some(1)); // wrap to last
    }

    #[test]
    fn jump_to_next_no_matches_is_noop() {
        let mut s = SearchState::new();
        s.jump_to_next();
        assert_eq!(s.current_match, None);
    }

    #[test]
    fn jump_to_prev_no_matches_is_noop() {
        let mut s = SearchState::new();
        s.jump_to_prev();
        assert_eq!(s.current_match, None);
    }

    #[test]
    fn jump_to_nearest_at_start() {
        let doc = make_doc("aaa\nbbb\naaa\nccc\naaa");
        let mut s = SearchState::new();
        s.query = "aaa".to_string();
        s.search(&doc);
        // jump_to_nearest at offset 0 should find first match
        s.jump_to_nearest(0);
        assert_eq!(s.current_match, Some(0));
    }

    #[test]
    fn jump_to_nearest_past_all_matches_wraps_to_zero() {
        let doc = make_doc("aaa\nbbb");
        let mut s = SearchState::new();
        s.query = "aaa".to_string();
        s.search(&doc);
        // offset beyond all matches -> unwrap_or(0)
        s.jump_to_nearest(999);
        assert_eq!(s.current_match, Some(0));
    }

    #[test]
    fn jump_to_nearest_empty_matches() {
        let mut s = SearchState::new();
        s.jump_to_nearest(0);
        assert_eq!(s.current_match, None);
    }
}
