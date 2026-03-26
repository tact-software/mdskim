use std::path::PathBuf;
use std::process::Command;

use crate::config::HeadingsConfig;
use crate::document::Document;
use crate::theme::Theme;

/// Allowed URL schemes for `open_url`. Only these prefixes are permitted
/// to prevent arbitrary scheme execution via the system opener.
const ALLOWED_URL_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];

fn open_url(url: &str) -> anyhow::Result<()> {
    if !ALLOWED_URL_SCHEMES.iter().any(|s| url.starts_with(s)) {
        return Err(anyhow::anyhow!("Blocked URL with disallowed scheme: {url}"));
    }

    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    let cmd = "start";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let cmd = "xdg-open";

    let mut child = Command::new(cmd)
        .arg(url)
        .spawn()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    // Reap child process in background to avoid zombie
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

mod action;
pub use action::*;

mod download;
mod file_manager;
mod link;
mod overlay;
mod prerender;
mod render_cache;
mod search;
mod viewport;
pub(crate) use file_manager::FileManager;
pub(crate) use overlay::OverlayState;
pub(crate) use render_cache::RenderCache;
pub(crate) use search::SearchState;
pub(crate) use viewport::Viewport;

/// Core application state for the Markdown viewer.
pub struct AppState {
    pub(crate) document: Document,
    pub(crate) viewport: Viewport,
    pub(crate) overlay: OverlayState,
    pub(crate) files: FileManager,
    pub(crate) search: SearchState,
    pub(crate) theme: Theme,
    pub(crate) render_cache: RenderCache,
    pub(crate) headings_config: HeadingsConfig,
    pub(crate) syntax_dir: Option<String>,
}

impl AppState {
    pub fn new(
        document: Document,
        file_path: Option<PathBuf>,
        theme: Theme,
        headings_config: HeadingsConfig,
    ) -> Self {
        Self {
            document,
            viewport: Viewport::new(),
            overlay: OverlayState::new(),
            files: FileManager::new(file_path),
            search: SearchState::new(),
            render_cache: RenderCache::new(),
            headings_config,
            theme,
            syntax_dir: None,
        }
    }

    pub fn apply(&mut self, action: Action) {
        // TOC pane focus mode
        if self.overlay.toc_pane_focus {
            match &action {
                Action::ScrollDown(_) => {
                    let max = self.document.headings.len().saturating_sub(1);
                    self.overlay.toc_pane_cursor = (self.overlay.toc_pane_cursor + 1).min(max);
                    return;
                }
                Action::ScrollUp(_) => {
                    self.overlay.toc_pane_cursor = self.overlay.toc_pane_cursor.saturating_sub(1);
                    return;
                }
                Action::TocSelect => {
                    if let Some(h) = self.document.headings.get(self.overlay.toc_pane_cursor) {
                        self.jump_to_line(h.line_idx);
                    }
                    self.overlay.toc_pane_focus = false;
                    return;
                }
                Action::ToggleFold => {
                    // Fold/unfold the heading at toc_pane_cursor
                    if let Some(h) = self.document.headings.get(self.overlay.toc_pane_cursor) {
                        let idx = h.line_idx;
                        if self.viewport.folded_headings.contains(&idx) {
                            self.viewport.folded_headings.remove(&idx);
                        } else {
                            self.viewport.folded_headings.insert(idx);
                        }
                        self.viewport.recompute_hidden_lines(&self.document);
                        self.adjust_cursor_after_fold();
                    }
                    return;
                }
                Action::FoldAll | Action::UnfoldAll => {
                    // Fall through to normal handling
                }
                Action::CloseOverlay | Action::ToggleTocPane => {
                    // Fall through to normal handling
                }
                _ => return,
            }
        }

        // In overlay mode, intercept navigation keys
        if self.overlay.show_links {
            match &action {
                Action::ScrollDown(_) => {
                    let max = self.document.links.len().saturating_sub(1);
                    self.overlay.links_cursor = (self.overlay.links_cursor + 1).min(max);
                    return;
                }
                Action::ScrollUp(_) => {
                    self.overlay.links_cursor = self.overlay.links_cursor.saturating_sub(1);
                    return;
                }
                Action::LinkSelect => {
                    if let Some(link) = self.document.links.get(self.overlay.links_cursor) {
                        self.jump_to_line(link.line_idx);
                    }
                    self.overlay.show_links = false;
                    return;
                }
                _ => {}
            }
        }

        if self.overlay.show_toc {
            match &action {
                Action::ScrollDown(_) => {
                    let max = self.document.headings.len().saturating_sub(1);
                    self.overlay.toc_cursor = (self.overlay.toc_cursor + 1).min(max);
                    return;
                }
                Action::ScrollUp(_) => {
                    self.overlay.toc_cursor = self.overlay.toc_cursor.saturating_sub(1);
                    return;
                }
                Action::TocSelect => {
                    let idx = self.overlay.toc_cursor;
                    if let Some(h) = self.document.headings.get(idx) {
                        self.jump_to_line(h.line_idx);
                    }
                    self.overlay.show_toc = false;
                    return;
                }
                _ => {}
            }
        }

        match action {
            Action::Quit => self.overlay.should_quit = true,
            Action::ScrollDown(n) => self.scroll_down(n),
            Action::ScrollUp(n) => self.scroll_up(n),
            Action::HalfPageDown(count) => {
                let half = self.viewport.height / 2;
                self.scroll_down(half * count);
            }
            Action::HalfPageUp(count) => {
                let half = self.viewport.height / 2;
                self.scroll_up(half * count);
            }
            Action::PageDown(count) => {
                let page = self.viewport.height.saturating_sub(2);
                self.scroll_down(page * count);
            }
            Action::PageUp(count) => {
                let page = self.viewport.height.saturating_sub(2);
                self.scroll_up(page * count);
            }
            Action::GoToTop => {
                self.viewport.scroll_offset = 0;
                self.viewport.cursor_line = 0;
            }
            Action::GoToBottom => {
                self.viewport.scroll_offset = self.viewport.max_scroll(&self.document);
                self.viewport.cursor_line = self.viewport.height.saturating_sub(1);
            }
            Action::GoToLine(n) => {
                self.viewport.scroll_offset = n
                    .saturating_sub(1)
                    .min(self.viewport.max_scroll(&self.document));
                self.viewport.cursor_line = 0;
            }
            Action::ToggleHelp => {
                self.overlay.show_help = !self.overlay.show_help;
                if self.overlay.show_help {
                    self.overlay.show_toc = false;
                    self.overlay.show_links = false;
                }
            }
            Action::CloseOverlay => {
                if self.overlay.toc_pane_focus {
                    self.overlay.toc_pane_focus = false;
                } else if self.overlay.show_links {
                    self.overlay.show_links = false;
                } else if self.overlay.show_toc {
                    self.overlay.show_toc = false;
                } else if self.overlay.show_help {
                    self.overlay.show_help = false;
                } else if !self.search.query.is_empty() {
                    self.search = SearchState::new();
                    self.overlay.status_message = None;
                }
            }
            Action::Resize => {
                self.viewport.scroll_offset = self
                    .viewport
                    .scroll_offset
                    .min(self.viewport.max_scroll(&self.document));
            }
            Action::NextHeading => self.jump_heading(true),
            Action::PrevHeading => self.jump_heading(false),
            Action::ToggleToc => {
                self.overlay.show_toc = !self.overlay.show_toc;
                if self.overlay.show_toc {
                    self.overlay.show_help = false;
                    self.overlay.show_links = false;
                    self.overlay.toc_cursor = self.nearest_heading_idx();
                }
            }
            Action::ToggleTocPane => {
                if !self.overlay.show_toc_pane {
                    self.overlay.show_toc_pane = true;
                    self.overlay.toc_pane_focus = true;
                    self.overlay.toc_pane_cursor = self.nearest_heading_idx();
                } else if self.overlay.toc_pane_focus {
                    self.overlay.toc_pane_focus = false;
                    self.overlay.show_toc_pane = false;
                } else {
                    self.overlay.toc_pane_focus = true;
                    self.overlay.toc_pane_cursor = self.nearest_heading_idx();
                }
            }
            Action::ToggleFold => {
                self.toggle_fold_at_cursor();
            }
            Action::FoldAll => {
                for h in &self.document.headings {
                    self.viewport.folded_headings.insert(h.line_idx);
                }
                self.viewport.recompute_hidden_lines(&self.document);
                self.viewport.scroll_offset = 0;
                self.viewport.cursor_line = 0;
            }
            Action::UnfoldAll => {
                self.viewport.folded_headings.clear();
                self.viewport.recompute_hidden_lines(&self.document);
                self.adjust_cursor_after_fold();
            }
            Action::TocSelect => {
                let idx = self.overlay.toc_cursor;
                if let Some(h) = self.document.headings.get(idx) {
                    self.jump_to_line(h.line_idx);
                    self.overlay.show_toc = false;
                }
            }
            Action::ToggleLinks => {
                self.overlay.show_links = !self.overlay.show_links;
                if self.overlay.show_links {
                    self.overlay.show_help = false;
                    self.overlay.show_toc = false;
                    self.overlay.links_cursor = 0;
                }
            }
            Action::LinkSelect => {
                if let Some(link) = self.document.links.get(self.overlay.links_cursor) {
                    self.jump_to_line(link.line_idx);
                    self.overlay.show_links = false;
                }
            }
            Action::LinkOpen => self.open_link_at_cursor(),
            Action::NextFile => self.switch_file(1),
            Action::PrevFile => self.switch_file(-1),
            Action::Reload => self.reload(),
            Action::EnterSearch => {
                self.search.active_input = true;
                self.search.input_buf.clear();
            }
            Action::SearchUpdate(query) => {
                self.search.input_buf = query.clone();
                self.search.query = query;
                self.search.search(&self.document);
                self.search.jump_to_nearest(self.viewport.scroll_offset);
                self.scroll_to_current_match();
                self.update_search_status();
            }
            Action::SearchSubmit(query) => {
                self.search.active_input = false;
                self.search.query = query;
                self.search.search(&self.document);
                self.search.jump_to_nearest(self.viewport.scroll_offset);
                self.scroll_to_current_match();
                self.update_search_status();
            }
            Action::SearchNext => {
                self.search.jump_to_next();
                self.scroll_to_current_match();
                self.update_search_status();
            }
            Action::SearchPrev => {
                self.search.jump_to_prev();
                self.scroll_to_current_match();
                self.update_search_status();
            }
            Action::ClearSearch => {
                self.search = SearchState::new();
                self.overlay.status_message = None;
            }
        }
    }

    fn scroll_to_current_match(&mut self) {
        if let Some(idx) = self.search.current_match
            && let Some(m) = self.search.matches.get(idx)
        {
            let line = m.line_idx;
            // Count visible lines between scroll_offset and the match line
            let visible_offset = (self.viewport.scroll_offset..line)
                .filter(|i| self.viewport.is_line_visible(*i))
                .count();
            if line < self.viewport.scroll_offset || visible_offset >= self.viewport.height {
                self.viewport.scroll_offset = line.saturating_sub(self.viewport.height / 3);
                self.viewport.scroll_offset = self
                    .viewport
                    .scroll_offset
                    .min(self.viewport.max_scroll(&self.document));
            }
            // Compute cursor_line as the number of visible lines from scroll_offset to line
            let cursor = (self.viewport.scroll_offset..line)
                .filter(|i| self.viewport.is_line_visible(*i))
                .count();
            self.viewport.cursor_line = cursor;
        }
    }

    fn update_search_status(&mut self) {
        if self.search.matches.is_empty() {
            if !self.search.query.is_empty() {
                self.overlay.status_message = Some(format!("/{} [no match]", self.search.query));
            }
        } else if let Some(idx) = self.search.current_match {
            self.overlay.status_message = Some(format!(
                "/{} [{}/{}]",
                self.search.query,
                idx + 1,
                self.search.matches.len()
            ));
        }
    }

    /// Get the absolute document line index at the current cursor position.
    pub fn abs_cursor_line(&self) -> usize {
        self.viewport.abs_cursor_line(&self.document)
    }

    fn scroll_down(&mut self, n: usize) {
        let total = self.document.total_lines();
        let abs_cursor = self.abs_cursor_line();
        let mut new_abs = abs_cursor;
        let mut moved = 0;
        while moved < n && new_abs + 1 < total {
            new_abs += 1;
            if self.viewport.is_line_visible(new_abs) {
                moved += 1;
            }
        }
        // Count visible lines between scroll_offset and new_abs
        let visible_offset = (self.viewport.scroll_offset..=new_abs)
            .filter(|i| self.viewport.is_line_visible(*i))
            .count()
            .saturating_sub(1);
        self.viewport.cursor_line = visible_offset;
        if self.viewport.cursor_line >= self.viewport.height {
            // Need to scroll: find new scroll_offset
            self.viewport.scroll_offset = new_abs;
            self.viewport.cursor_line = 0;
            // Back up scroll_offset to fill viewport
            let mut visible_before = 0;
            while self.viewport.scroll_offset > 0
                && visible_before < self.viewport.height.saturating_sub(1)
            {
                self.viewport.scroll_offset -= 1;
                if self.viewport.is_line_visible(self.viewport.scroll_offset) {
                    visible_before += 1;
                }
            }
            self.viewport.cursor_line = visible_before;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let abs_cursor = self.abs_cursor_line();
        // Find the previous n visible lines before current position
        let mut new_abs = abs_cursor;
        let mut moved = 0;
        while moved < n && new_abs > 0 {
            new_abs -= 1;
            if self.viewport.is_line_visible(new_abs) {
                moved += 1;
            }
        }
        if new_abs < self.viewport.scroll_offset {
            self.viewport.scroll_offset = new_abs;
            self.viewport.cursor_line = 0;
        } else {
            self.viewport.cursor_line = (self.viewport.scroll_offset..=new_abs)
                .filter(|i| self.viewport.is_line_visible(*i))
                .count()
                .saturating_sub(1);
        }
    }

    fn toggle_fold_at_cursor(&mut self) {
        let abs_line = self.abs_cursor_line();

        // If cursor is on a heading decoration (overline), use its heading
        let target_heading = if let Some(line) = self.document.lines.get(abs_line)
            && let Some(heading_idx) = line.heading_decoration_for
        {
            self.document
                .headings
                .iter()
                .find(|h| h.line_idx == heading_idx)
        } else {
            // Find the heading at or before cursor
            self.document
                .headings
                .iter()
                .rev()
                .find(|h| h.line_idx <= abs_line)
        };

        if let Some(h) = target_heading {
            let idx = h.line_idx;
            if self.viewport.folded_headings.contains(&idx) {
                self.viewport.folded_headings.remove(&idx);
            } else {
                self.viewport.folded_headings.insert(idx);
            }
            self.viewport.recompute_hidden_lines(&self.document);
            self.adjust_cursor_after_fold();
        }
    }

    /// After recomputing hidden lines, ensure scroll_offset and cursor_line
    /// point to visible lines and stay within viewport bounds.
    fn adjust_cursor_after_fold(&mut self) {
        let total = self.document.total_lines();
        // Advance scroll_offset to the next visible line if it's hidden
        while self.viewport.scroll_offset < total
            && !self.viewport.is_line_visible(self.viewport.scroll_offset)
        {
            self.viewport.scroll_offset += 1;
        }
        // Clamp cursor_line so it doesn't exceed viewport height
        if self.viewport.cursor_line >= self.viewport.height {
            self.viewport.cursor_line = self.viewport.height.saturating_sub(1);
        }
        // Ensure cursor_line doesn't point past the last visible line in viewport
        let visible_in_viewport = (self.viewport.scroll_offset..total)
            .filter(|i| self.viewport.is_line_visible(*i))
            .take(self.viewport.height)
            .count();
        if visible_in_viewport > 0 {
            self.viewport.cursor_line = self.viewport.cursor_line.min(visible_in_viewport - 1);
        } else {
            self.viewport.cursor_line = 0;
        }
    }

    /// Check if a document line should be visible (not hidden by a fold).
    pub fn is_line_visible(&self, line_idx: usize) -> bool {
        self.viewport.is_line_visible(line_idx)
    }

    fn jump_to_line(&mut self, line: usize) {
        let target = line.min(self.document.total_lines().saturating_sub(1));
        // Check if target is visible in current viewport
        let visible_in_viewport = (self.viewport.scroll_offset
            ..self.viewport.scroll_offset + self.viewport.height * 2)
            .filter(|i| self.viewport.is_line_visible(*i))
            .take(self.viewport.height)
            .any(|i| i == target);
        if !visible_in_viewport {
            // Scroll so target is roughly 1/3 from top
            let mut offset = target;
            let mut count = 0;
            while offset > 0 && count < self.viewport.height / 3 {
                offset -= 1;
                if self.viewport.is_line_visible(offset) {
                    count += 1;
                }
            }
            self.viewport.scroll_offset = offset;
        }
        // Calculate cursor_line as visible lines from scroll_offset to target
        self.viewport.cursor_line = (self.viewport.scroll_offset..=target)
            .filter(|i| self.viewport.is_line_visible(*i))
            .count()
            .saturating_sub(1);
    }

    fn jump_heading(&mut self, forward: bool) {
        if self.document.headings.is_empty() {
            return;
        }
        let abs_cursor = self.abs_cursor_line();
        if forward {
            if let Some(h) = self
                .document
                .headings
                .iter()
                .find(|h| h.line_idx > abs_cursor)
            {
                self.jump_to_line(h.line_idx);
            }
        } else if let Some(h) = self
            .document
            .headings
            .iter()
            .rev()
            .find(|h| h.line_idx < abs_cursor)
        {
            self.jump_to_line(h.line_idx);
        }
    }

    fn nearest_heading_idx(&self) -> usize {
        let abs_cursor = self.abs_cursor_line();
        self.document
            .headings
            .iter()
            .enumerate()
            .rev()
            .find(|(_, h)| h.line_idx <= abs_cursor)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Clear caches and re-render all external resources (Mermaid, math, images).
    /// Called by both `switch_file` and `reload`.
    fn reload_resources(&mut self) {
        self.render_cache.clear();
        self.viewport.recompute_hidden_lines(&self.document);
        self.prerender_mermaid();
        self.prerender_math();
        self.prerender_images();
    }

    fn switch_file(&mut self, delta: i32) {
        if self.files.file_list.len() <= 1 {
            return;
        }
        let new_idx = (self.files.file_index as i32 + delta)
            .rem_euclid(self.files.file_list.len() as i32) as usize;
        let path = self.files.file_list[new_idx].clone();
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                self.document = crate::parser::parse_markdown(
                    &source,
                    &self.headings_config,
                    self.syntax_dir.as_deref(),
                );
                self.files.file_index = new_idx;
                self.files.file_path = Some(path);
                self.viewport.scroll_offset = 0;
                self.viewport.cursor_line = 0;
                self.search = SearchState::new();
                self.overlay.show_toc = false;
                self.overlay.show_links = false;
                self.overlay.show_help = false;
                self.overlay.toc_pane_focus = false;
                self.overlay.toc_cursor = 0;
                self.overlay.links_cursor = 0;
                self.overlay.toc_pane_cursor = 0;
                self.viewport.folded_headings.clear();
                self.reload_resources();
                let display_path = self
                    .files
                    .file_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "[unknown]".to_string());
                self.overlay.status_message = Some(format!(
                    "[{}/{}] {}",
                    new_idx + 1,
                    self.files.file_list.len(),
                    display_path
                ));
            }
            Err(e) => {
                self.overlay.status_message = Some(format!("Failed to open: {e}"));
            }
        }
    }

    fn reload(&mut self) {
        let Some(path) = &self.files.file_path else {
            self.overlay.status_message = Some("Cannot reload: reading from stdin".to_string());
            return;
        };
        match std::fs::read_to_string(path) {
            Ok(source) => {
                let doc = crate::parser::parse_markdown(
                    &source,
                    &self.headings_config,
                    self.syntax_dir.as_deref(),
                );
                self.document = doc;
                self.viewport.scroll_offset = self
                    .viewport
                    .scroll_offset
                    .min(self.viewport.max_scroll(&self.document));
                self.search = SearchState::new();
                self.viewport.folded_headings.clear();
                self.overlay.toc_cursor = 0;
                self.overlay.links_cursor = 0;
                self.overlay.toc_pane_cursor = 0;
                self.reload_resources();
                self.overlay.status_message = Some("Reloaded".to_string());
            }
            Err(e) => {
                self.overlay.status_message = Some(format!("Reload failed: {}", e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HeadingsConfig;
    use crate::test_helpers::make_doc;
    use crate::theme::Theme;

    /// Build an `AppState` from raw markdown with a sensible viewport size.
    fn make_state(md: &str) -> AppState {
        let doc = make_doc(md);
        let mut state = AppState::new(doc, None, Theme::dark(), HeadingsConfig::default());
        state.viewport.height = 10;
        state.viewport.width = 80;
        state
    }

    /// Generate a long markdown string with the given number of plain-text lines.
    fn long_md(n: usize) -> String {
        (0..n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ── Scroll ───────────────────────────────────────────────────────

    #[test]
    fn scroll_down_1() {
        let mut s = make_state(&long_md(30));
        assert_eq!(s.viewport.scroll_offset, 0);
        assert_eq!(s.viewport.cursor_line, 0);
        s.apply(Action::ScrollDown(1));
        // Cursor should have moved down by 1
        assert_eq!(s.abs_cursor_line(), 1);
    }

    #[test]
    fn scroll_down_n_clamps_at_max() {
        let mut s = make_state(&long_md(10));
        let max = s.viewport.max_scroll(&s.document);
        // Scroll far beyond document length
        s.apply(Action::ScrollDown(1000));
        // Should not exceed the last line
        let total = s.document.total_lines();
        assert!(s.abs_cursor_line() < total);
        assert!(s.viewport.scroll_offset <= max);
    }

    #[test]
    fn scroll_up_1() {
        let mut s = make_state(&long_md(30));
        // First scroll down, then scroll up
        s.apply(Action::ScrollDown(5));
        let before = s.abs_cursor_line();
        s.apply(Action::ScrollUp(1));
        assert_eq!(s.abs_cursor_line(), before - 1);
    }

    #[test]
    fn scroll_up_at_top_stays_zero() {
        let mut s = make_state(&long_md(30));
        s.apply(Action::ScrollUp(1));
        assert_eq!(s.viewport.scroll_offset, 0);
        assert_eq!(s.abs_cursor_line(), 0);
    }

    // ── GoTo ─────────────────────────────────────────────────────────

    #[test]
    fn go_to_top() {
        let mut s = make_state(&long_md(30));
        s.apply(Action::ScrollDown(10));
        s.apply(Action::GoToTop);
        assert_eq!(s.viewport.scroll_offset, 0);
        assert_eq!(s.viewport.cursor_line, 0);
    }

    #[test]
    fn go_to_bottom() {
        let mut s = make_state(&long_md(30));
        let max = s.viewport.max_scroll(&s.document);
        s.apply(Action::GoToBottom);
        assert_eq!(s.viewport.scroll_offset, max);
    }

    #[test]
    fn go_to_line() {
        let mut s = make_state(&long_md(30));
        s.apply(Action::GoToLine(5));
        // GoToLine(n) sets scroll_offset to n-1 (clamped) and cursor_line to 0
        assert_eq!(s.viewport.scroll_offset, 4);
        assert_eq!(s.viewport.cursor_line, 0);
    }

    // ── Page / HalfPage ──────────────────────────────────────────────

    #[test]
    fn page_down() {
        let mut s = make_state(&long_md(50));
        s.apply(Action::PageDown(1));
        // PageDown scrolls by height-2 = 8
        let expected_line = s.viewport.height.saturating_sub(2);
        assert_eq!(s.abs_cursor_line(), expected_line);
    }

    #[test]
    fn page_up() {
        let mut s = make_state(&long_md(50));
        s.apply(Action::PageDown(2));
        let before = s.abs_cursor_line();
        s.apply(Action::PageUp(1));
        let page = s.viewport.height.saturating_sub(2);
        assert_eq!(s.abs_cursor_line(), before - page);
    }

    #[test]
    fn half_page_down() {
        let mut s = make_state(&long_md(50));
        s.apply(Action::HalfPageDown(1));
        let half = s.viewport.height / 2;
        assert_eq!(s.abs_cursor_line(), half);
    }

    // ── Fold ─────────────────────────────────────────────────────────

    #[test]
    fn toggle_fold() {
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub";
        let mut s = make_state(md);
        assert!(s.viewport.folded_headings.is_empty());
        // Cursor is at line 0 (H1), toggling should fold it
        s.apply(Action::ToggleFold);
        assert!(!s.viewport.folded_headings.is_empty());
        // Toggle again should unfold
        s.apply(Action::ToggleFold);
        assert!(s.viewport.folded_headings.is_empty());
    }

    #[test]
    fn fold_all() {
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub";
        let mut s = make_state(md);
        s.apply(Action::FoldAll);
        assert_eq!(s.viewport.folded_headings.len(), s.document.headings.len());
        assert_eq!(s.viewport.scroll_offset, 0);
        assert_eq!(s.viewport.cursor_line, 0);
    }

    #[test]
    fn unfold_all() {
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub";
        let mut s = make_state(md);
        s.apply(Action::FoldAll);
        assert!(!s.viewport.folded_headings.is_empty());
        s.apply(Action::UnfoldAll);
        assert!(s.viewport.folded_headings.is_empty());
    }

    // ── Heading navigation ───────────────────────────────────────────

    #[test]
    fn next_heading() {
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub";
        let mut s = make_state(md);
        let h1_line = s.document.headings[0].line_idx;
        let h2_line = s.document.headings[1].line_idx;
        // Position cursor on H1 itself (past any decoration line)
        s.jump_to_line(h1_line);
        assert_eq!(s.abs_cursor_line(), h1_line);
        s.apply(Action::NextHeading);
        // After jumping, cursor should be at H2's line
        assert_eq!(s.abs_cursor_line(), h2_line);
    }

    #[test]
    fn prev_heading() {
        let md = "# H1\n\nparagraph\n\n## H2\n\nsub";
        let mut s = make_state(md);
        let h1_line = s.document.headings[0].line_idx;
        let h2_line = s.document.headings[1].line_idx;
        // Position cursor on H2
        s.jump_to_line(h2_line);
        assert_eq!(s.abs_cursor_line(), h2_line);
        s.apply(Action::PrevHeading);
        assert_eq!(s.abs_cursor_line(), h1_line);
    }

    // ── Quit ─────────────────────────────────────────────────────────

    #[test]
    fn quit() {
        let mut s = make_state("hello");
        assert!(!s.overlay.should_quit);
        s.apply(Action::Quit);
        assert!(s.overlay.should_quit);
    }

    // ── Resize ───────────────────────────────────────────────────────

    #[test]
    fn resize_clamps_scroll_offset() {
        let mut s = make_state(&long_md(30));
        // Manually set scroll_offset beyond max
        s.viewport.scroll_offset = 9999;
        s.apply(Action::Resize);
        let max = s.viewport.max_scroll(&s.document);
        assert!(s.viewport.scroll_offset <= max);
    }
}
