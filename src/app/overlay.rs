/// State for overlay UI panels (help, TOC, links).
#[derive(Default)]
pub(crate) struct OverlayState {
    pub(crate) show_help: bool,
    pub(crate) show_toc: bool,
    pub(crate) show_toc_pane: bool,
    pub(crate) toc_pane_focus: bool,
    pub(crate) toc_pane_cursor: usize,
    pub(crate) toc_cursor: usize,
    pub(crate) show_links: bool,
    pub(crate) links_cursor: usize,
    pub(crate) should_quit: bool,
    pub(crate) status_message: Option<String>,
}

impl OverlayState {
    pub fn new() -> Self {
        Self::default()
    }
}
