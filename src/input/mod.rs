use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use crate::app::Action;
use crate::config::Keymap;

/// Input state machine for Vim-like key sequences.
pub struct InputHandler {
    count_buf: String,
    pending_g: bool,
    pending_g_since: Option<std::time::Instant>,
    pub(crate) search_mode: bool,
    pub(crate) search_query: String,
    keymap: Keymap,
}

/// Timeout for multi-key sequences like `gg` (in milliseconds).
const PENDING_KEY_TIMEOUT_MS: u64 = 500;

/// Snapshot of overlay mode flags passed from AppState each frame.
pub struct InputMode {
    pub toc_mode: bool,
    pub links_mode: bool,
    pub toc_pane_focus: bool,
}

impl InputHandler {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            count_buf: String::new(),
            pending_g: false,
            pending_g_since: None,
            search_mode: false,
            search_query: String::new(),
            keymap,
        }
    }

    pub fn poll(&mut self, mode: &InputMode) -> anyhow::Result<Option<Action>> {
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => return Ok(self.handle_key(key, mode)),
                Event::Resize(_, _) => return Ok(Some(Action::Resize)),
                _ => {}
            }
        }

        // Expire pending_g based on its own Instant-based timeout
        if self.pending_g
            && let Some(since) = self.pending_g_since
            && since.elapsed() >= Duration::from_millis(PENDING_KEY_TIMEOUT_MS)
        {
            self.pending_g = false;
            self.pending_g_since = None;
            self.count_buf.clear();
        }

        Ok(None)
    }

    fn handle_key(&mut self, key: KeyEvent, mode: &InputMode) -> Option<Action> {
        if self.search_mode {
            return self.handle_search_key(key);
        }
        if mode.toc_pane_focus {
            return self.handle_toc_pane_key(key);
        }
        if mode.toc_mode || mode.links_mode {
            return self.handle_overlay_key(key, mode);
        }
        self.handle_normal_key(key)
    }

    /// Resolve a key to an action via the shared keymap, ignoring count.
    fn resolve_key_via_keymap(&mut self, key: KeyEvent) -> Option<Action> {
        if let KeyCode::Char(c) = key.code {
            let key_str = c.to_string();
            if let Some(action_name) = self.keymap.get_action(&key_str).map(str::to_string) {
                return self.resolve_action(&action_name, None);
            }
        }
        None
    }

    fn handle_toc_pane_key(&mut self, key: KeyEvent) -> Option<Action> {
        // Enter is context-specific (not in keymap)
        if key.code == KeyCode::Enter {
            return Some(Action::TocSelect);
        }
        // Special keys
        if key.code == KeyCode::Down {
            return Some(Action::ScrollDown(1));
        }
        if key.code == KeyCode::Up {
            return Some(Action::ScrollUp(1));
        }
        if key.code == KeyCode::Esc {
            return Some(Action::CloseOverlay);
        }
        // Delegate to shared keymap for character keys
        let action = self.resolve_key_via_keymap(key);
        // Filter: only allow actions that make sense in toc pane
        match &action {
            Some(
                Action::ScrollDown(_)
                | Action::ScrollUp(_)
                | Action::ToggleFold
                | Action::FoldAll
                | Action::UnfoldAll
                | Action::ToggleTocPane
                | Action::Quit
                | Action::CloseOverlay,
            ) => action,
            _ => None,
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent, mode: &InputMode) -> Option<Action> {
        // Enter is context-specific
        if key.code == KeyCode::Enter {
            return if mode.toc_mode {
                Some(Action::TocSelect)
            } else {
                Some(Action::LinkSelect)
            };
        }
        // Special keys
        if key.code == KeyCode::Down {
            return Some(Action::ScrollDown(1));
        }
        if key.code == KeyCode::Up {
            return Some(Action::ScrollUp(1));
        }
        if key.code == KeyCode::Esc {
            return Some(Action::CloseOverlay);
        }
        // Delegate to shared keymap
        let action = self.resolve_key_via_keymap(key);
        // Filter: only allow actions that make sense in overlays
        match &action {
            Some(
                Action::ScrollDown(_)
                | Action::ScrollUp(_)
                | Action::CloseOverlay
                | Action::Quit
                | Action::ToggleToc
                | Action::ToggleLinks
                | Action::LinkOpen,
            ) => action,
            _ => None,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Enter => {
                self.search_mode = false;
                let query = self.search_query.clone();
                if query.is_empty() {
                    return Some(Action::ClearSearch);
                }
                Some(Action::SearchSubmit(query))
            }
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_query.clear();
                Some(Action::ClearSearch)
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                Some(Action::SearchUpdate(self.search_query.clone()))
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                Some(Action::SearchUpdate(self.search_query.clone()))
            }
            _ => None,
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Option<Action> {
        use KeyCode::*;

        // Handle 'g' pending state
        if self.pending_g {
            self.pending_g = false;
            self.pending_g_since = None;
            if key.code == Char('g') {
                return Some(Action::GoToLine(self.take_count().unwrap_or(1)));
            }
            self.count_buf.clear();
        }

        // Accumulate digit prefix
        if let Char(c) = key.code
            && c.is_ascii_digit()
            && key.modifiers == KeyModifiers::NONE
            && !(c == '0' && self.count_buf.is_empty())
        {
            self.count_buf.push(c);
            return None;
        }

        let count = self.take_count();

        // Ctrl-key combos (not in keymap)
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, Char('c')) => return Some(Action::Quit),
            (KeyModifiers::CONTROL, Char('f')) => {
                return Some(Action::PageDown(count.unwrap_or(1)));
            }
            (KeyModifiers::CONTROL, Char('b')) => return Some(Action::PageUp(count.unwrap_or(1))),
            (KeyModifiers::CONTROL, Char('n')) => return Some(Action::NextFile),
            (KeyModifiers::CONTROL, Char('p')) => return Some(Action::PrevFile),
            _ => {}
        }

        // Special keys
        match key.code {
            Down => return Some(Action::ScrollDown(count.unwrap_or(1))),
            Up => return Some(Action::ScrollUp(count.unwrap_or(1))),
            PageDown => return Some(Action::PageDown(count.unwrap_or(1))),
            PageUp => return Some(Action::PageUp(count.unwrap_or(1))),
            Home => return Some(Action::GoToTop),
            End => return Some(Action::GoToBottom),
            Esc => return Some(Action::CloseOverlay),
            _ => {}
        }

        // Keymap-based lookup for character keys
        if let Char(c) = key.code {
            let key_str = c.to_string();
            if let Some(action_name) = self.keymap.get_action(&key_str).map(str::to_string) {
                return self.resolve_action(&action_name, count);
            }
        }

        None
    }

    fn resolve_action(&mut self, action_name: &str, count: Option<usize>) -> Option<Action> {
        match action_name {
            "quit" => Some(Action::Quit),
            "scroll_down" => Some(Action::ScrollDown(count.unwrap_or(1))),
            "scroll_up" => Some(Action::ScrollUp(count.unwrap_or(1))),
            "half_page_down" => Some(Action::HalfPageDown(count.unwrap_or(1))),
            "half_page_up" => Some(Action::HalfPageUp(count.unwrap_or(1))),
            "page_down" => Some(Action::PageDown(count.unwrap_or(1))),
            "page_up" => Some(Action::PageUp(count.unwrap_or(1))),
            "go_to_top_pending" => {
                self.pending_g = true;
                self.pending_g_since = Some(std::time::Instant::now());
                None
            }
            "go_to_bottom" => {
                if let Some(n) = count {
                    Some(Action::GoToLine(n))
                } else {
                    Some(Action::GoToBottom)
                }
            }
            "search" => {
                self.search_mode = true;
                self.search_query.clear();
                Some(Action::EnterSearch)
            }
            "search_next" => Some(Action::SearchNext),
            "search_prev" => Some(Action::SearchPrev),
            "next_heading" => Some(Action::NextHeading),
            "prev_heading" => Some(Action::PrevHeading),
            "toggle_toc" => Some(Action::ToggleToc),
            "toggle_links" => Some(Action::ToggleLinks),
            "open_link" => Some(Action::LinkOpen),
            "reload" => Some(Action::Reload),
            "next_file" => Some(Action::NextFile),
            "prev_file" => Some(Action::PrevFile),
            "toggle_help" => Some(Action::ToggleHelp),
            "toggle_toc_pane" => Some(Action::ToggleTocPane),
            "toggle_fold" => Some(Action::ToggleFold),
            "fold_all" => Some(Action::FoldAll),
            "unfold_all" => Some(Action::UnfoldAll),
            "go_to_top" => Some(Action::GoToTop),
            _ => None,
        }
    }

    fn take_count(&mut self) -> Option<usize> {
        if self.count_buf.is_empty() {
            return None;
        }
        let n = self.count_buf.parse::<usize>().ok();
        self.count_buf.clear();
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{KeybindingsConfig, Keymap};

    fn make_handler() -> InputHandler {
        let keymap = Keymap::from_config(&KeybindingsConfig::default());
        InputHandler::new(keymap)
    }

    #[test]
    fn resolve_action_quit() {
        let mut h = make_handler();
        assert!(matches!(h.resolve_action("quit", None), Some(Action::Quit)));
    }

    #[test]
    fn resolve_action_scroll_down_default_count() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("scroll_down", None),
            Some(Action::ScrollDown(1))
        ));
    }

    #[test]
    fn resolve_action_scroll_down_with_count() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("scroll_down", Some(5)),
            Some(Action::ScrollDown(5))
        ));
    }

    #[test]
    fn resolve_action_scroll_up() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("scroll_up", None),
            Some(Action::ScrollUp(1))
        ));
    }

    #[test]
    fn resolve_action_half_page_down() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("half_page_down", Some(3)),
            Some(Action::HalfPageDown(3))
        ));
    }

    #[test]
    fn resolve_action_half_page_up() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("half_page_up", None),
            Some(Action::HalfPageUp(1))
        ));
    }

    #[test]
    fn resolve_action_go_to_bottom_no_count() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("go_to_bottom", None),
            Some(Action::GoToBottom)
        ));
    }

    #[test]
    fn resolve_action_go_to_bottom_with_count() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("go_to_bottom", Some(10)),
            Some(Action::GoToLine(10))
        ));
    }

    #[test]
    fn resolve_action_search_sets_search_mode() {
        let mut h = make_handler();
        assert!(!h.search_mode);
        let result = h.resolve_action("search", None);
        assert!(matches!(result, Some(Action::EnterSearch)));
        assert!(h.search_mode);
    }

    #[test]
    fn resolve_action_toggle_fold() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("toggle_fold", None),
            Some(Action::ToggleFold)
        ));
    }

    #[test]
    fn resolve_action_unknown_returns_none() {
        let mut h = make_handler();
        assert!(h.resolve_action("nonexistent_action", None).is_none());
    }

    #[test]
    fn resolve_action_go_to_top_pending_sets_flag() {
        let mut h = make_handler();
        assert!(!h.pending_g);
        let result = h.resolve_action("go_to_top_pending", None);
        assert!(result.is_none()); // returns None, sets pending state
        assert!(h.pending_g);
    }

    #[test]
    fn resolve_action_count_propagated_to_page_down() {
        let mut h = make_handler();
        assert!(matches!(
            h.resolve_action("page_down", Some(3)),
            Some(Action::PageDown(3))
        ));
    }

    #[test]
    fn handle_search_key_char_input() {
        let mut h = make_handler();
        h.search_mode = true;
        h.search_query.clear();
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let action = h.handle_search_key(key);
        assert!(matches!(action, Some(Action::SearchUpdate(ref q)) if q == "a"));
        assert_eq!(h.search_query, "a");
    }

    #[test]
    fn handle_search_key_backspace() {
        let mut h = make_handler();
        h.search_mode = true;
        h.search_query = "abc".to_string();
        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        let action = h.handle_search_key(key);
        assert!(matches!(action, Some(Action::SearchUpdate(ref q)) if q == "ab"));
        assert_eq!(h.search_query, "ab");
    }

    #[test]
    fn handle_search_key_enter_submits() {
        let mut h = make_handler();
        h.search_mode = true;
        h.search_query = "test".to_string();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = h.handle_search_key(key);
        assert!(matches!(action, Some(Action::SearchSubmit(ref q)) if q == "test"));
        assert!(!h.search_mode);
    }

    #[test]
    fn handle_search_key_enter_empty_clears() {
        let mut h = make_handler();
        h.search_mode = true;
        h.search_query.clear();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = h.handle_search_key(key);
        assert!(matches!(action, Some(Action::ClearSearch)));
    }

    #[test]
    fn handle_search_key_esc_clears() {
        let mut h = make_handler();
        h.search_mode = true;
        h.search_query = "test".to_string();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = h.handle_search_key(key);
        assert!(matches!(action, Some(Action::ClearSearch)));
        assert!(!h.search_mode);
        assert!(h.search_query.is_empty());
    }

    #[test]
    fn resolve_action_all_named_actions() {
        let mut h = make_handler();
        let actions = [
            "search_next",
            "search_prev",
            "next_heading",
            "prev_heading",
            "toggle_toc",
            "toggle_links",
            "open_link",
            "reload",
            "next_file",
            "prev_file",
            "toggle_help",
            "toggle_toc_pane",
            "fold_all",
            "unfold_all",
        ];
        for name in actions {
            assert!(
                h.resolve_action(name, None).is_some(),
                "action '{name}' should resolve to Some"
            );
        }
    }
}
