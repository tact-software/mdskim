/// User action dispatched from input handler to application state.
#[derive(Debug)]
pub enum Action {
    Quit,
    ScrollDown(usize),
    ScrollUp(usize),
    HalfPageDown(usize),
    HalfPageUp(usize),
    PageDown(usize),
    PageUp(usize),
    GoToTop,
    GoToBottom,
    GoToLine(usize),
    ToggleHelp,
    CloseOverlay,
    Resize,
    EnterSearch,
    SearchUpdate(String),
    SearchSubmit(String),
    SearchNext,
    SearchPrev,
    ClearSearch,
    NextHeading,
    PrevHeading,
    ToggleToc,
    TocSelect,
    ToggleLinks,
    LinkSelect,
    ToggleTocPane,
    ToggleFold,
    FoldAll,
    UnfoldAll,
    LinkOpen,
    NextFile,
    PrevFile,
    Reload,
}

pub(crate) enum LinkType {
    Anchor(String),
    External(String),
    Email(String),
}

pub(crate) fn classify_link(url: &str) -> LinkType {
    let trimmed = url.trim();
    if let Some(anchor) = trimmed.strip_prefix('#') {
        LinkType::Anchor(anchor.to_string())
    } else if trimmed.starts_with("mailto:") {
        LinkType::Email(
            trimmed
                .strip_prefix("mailto:")
                .unwrap_or(trimmed)
                .to_string(),
        )
    } else if trimmed.contains('@') && !trimmed.contains('/') {
        LinkType::Email(trimmed.to_string())
    } else {
        LinkType::External(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_link_anchor() {
        let result = classify_link("#section");
        assert!(matches!(result, LinkType::Anchor(s) if s == "section"));
    }

    #[test]
    fn classify_link_external_https() {
        let result = classify_link("https://example.com");
        assert!(matches!(result, LinkType::External(s) if s == "https://example.com"));
    }

    #[test]
    fn classify_link_mailto() {
        let result = classify_link("mailto:user@example.com");
        assert!(matches!(result, LinkType::Email(s) if s == "user@example.com"));
    }

    #[test]
    fn classify_link_bare_email() {
        let result = classify_link("user@example.com");
        assert!(matches!(result, LinkType::Email(s) if s == "user@example.com"));
    }

    #[test]
    fn classify_link_empty_string() {
        let result = classify_link("");
        assert!(matches!(result, LinkType::External(s) if s.is_empty()));
    }
}
