use super::action::{LinkType, classify_link};
use super::{AppState, open_url};

impl AppState {
    pub(super) fn open_link_at_cursor(&mut self) {
        let url = if self.overlay.show_links {
            self.document
                .links
                .get(self.overlay.links_cursor)
                .map(|l| l.url.clone())
        } else {
            let abs_line = self.abs_cursor_line();
            self.document
                .links
                .iter()
                .find(|l| l.line_idx == abs_line)
                .map(|l| l.url.clone())
        };

        let Some(url) = url else {
            self.overlay.status_message = Some("No link on this line".to_string());
            return;
        };

        self.activate_link(&url);
        if self.overlay.show_links {
            self.overlay.show_links = false;
        }
    }

    pub(super) fn activate_link(&mut self, url: &str) {
        match classify_link(url) {
            LinkType::Anchor(anchor) => {
                let target = anchor.to_lowercase().replace('-', " ");
                let found = self
                    .document
                    .headings
                    .iter()
                    .find(|h| h.title.to_lowercase() == target)
                    .map(|h| (h.line_idx, h.title.clone()));
                if let Some((line_idx, title)) = found {
                    self.jump_to_line(line_idx);
                    self.overlay.status_message = Some(format!("Jumped to: {title}"));
                } else {
                    self.overlay.status_message = Some(format!("Anchor not found: #{anchor}"));
                }
            }
            LinkType::External(url) => {
                if let Err(e) = open_url(&url) {
                    self.overlay.status_message = Some(format!("Open failed: {e}"));
                } else {
                    self.overlay.status_message = Some(format!("Opened: {url}"));
                }
            }
            LinkType::Email(addr) => {
                if let Err(e) = open_url(&format!("mailto:{addr}")) {
                    self.overlay.status_message = Some(format!("Open failed: {e}"));
                } else {
                    self.overlay.status_message = Some(format!("Mail to: {addr}"));
                }
            }
        }
    }
}
