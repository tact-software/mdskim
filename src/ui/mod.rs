mod draw_content;

use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) use draw_content::ImageCacheKey;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color as RatColor, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use crate::app::{AppState, RenderCache};
use crate::input::InputHandler;

const TOC_PANE_MAX_WIDTH: u16 = 30;
const HELP_DIALOG_WIDTH: u16 = 36;
const TOC_DIALOG_WIDTH: u16 = 50;
const LINKS_DIALOG_MAX_WIDTH: u16 = 80;

/// Holds stateful image protocols for rendering mermaid diagrams.
pub struct ImageState {
    pub picker: Option<Picker>,
    pub protocols: HashMap<ImageCacheKey, StatefulProtocol>,
    pub generation: u64,
    pub dimension_cache: HashMap<PathBuf, (u32, u32)>,
}

impl ImageState {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            picker,
            protocols: HashMap::new(),
            generation: 0,
            dimension_cache: HashMap::new(),
        }
    }

    pub fn supports_images(&self) -> bool {
        self.picker.is_some()
    }

    pub fn sync_generation(&mut self, new_gen: u64) {
        if self.generation != new_gen {
            self.protocols.clear();
            self.dimension_cache.clear();
            self.generation = new_gen;
        }
    }

    /// Pre-load image protocols for all resolved image paths in the render cache.
    /// This runs `image::open()` and `picker.new_resize_protocol()` eagerly so that
    /// the draw loop only needs a cache lookup (no blocking I/O).
    pub fn preload_protocols(&mut self, cache: &RenderCache) {
        if self.picker.is_none() {
            return;
        }

        self.sync_generation(cache.generation);

        // Collect keys and paths that need loading to avoid borrow conflicts.
        let to_load: Vec<(ImageCacheKey, std::path::PathBuf)> = cache
            .mermaid_images
            .iter()
            .map(|(&idx, p)| (ImageCacheKey::Mermaid(idx), p.clone()))
            .chain(
                cache
                    .math_images
                    .iter()
                    .map(|(&idx, p)| (ImageCacheKey::Math(idx), p.clone())),
            )
            .chain(
                cache
                    .image_paths
                    .iter()
                    .map(|(&idx, p)| (ImageCacheKey::Document(idx), p.clone())),
            )
            .filter(|(key, _)| !self.protocols.contains_key(key))
            .collect();

        // Borrow picker and protocols as disjoint fields to satisfy the borrow checker.
        let picker = self.picker.as_mut().unwrap();
        let protocols = &mut self.protocols;
        let dim_cache = &mut self.dimension_cache;
        for (key, path) in to_load {
            // Cache image dimensions before opening (cheaper than a full decode)
            if !dim_cache.contains_key(&path)
                && let Ok(dims) = image::image_dimensions(&path)
            {
                dim_cache.insert(path.clone(), dims);
            }
            if let Ok(dyn_img) = image::open(&path) {
                let proto = picker.new_resize_protocol(dyn_img);
                protocols.insert(key, proto);
            }
        }
    }
}

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    input: &InputHandler,
    img_state: &mut ImageState,
) {
    let has_search_bar = input.search_mode;
    let bottom_height = if has_search_bar { 2 } else { 1 };

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(bottom_height)])
        .split(frame.area());

    let main_area = chunks[0];
    let bottom_area = chunks[1];

    // Split horizontally for TOC side pane
    let (toc_pane_area, content_area) = if state.overlay.show_toc_pane {
        let toc_width = TOC_PANE_MAX_WIDTH.min(main_area.width / 3);
        let h_chunks = Layout::horizontal([Constraint::Length(toc_width), Constraint::Min(1)])
            .split(main_area);
        (Some(h_chunks[0]), h_chunks[1])
    } else {
        (None, main_area)
    };

    state.viewport.height = content_area.height as usize;
    state.viewport.width = content_area.width as usize;

    if let Some(toc_area) = toc_pane_area {
        draw_toc_pane(frame, state, toc_area);
    }

    draw_content::draw_content(frame, state, content_area, img_state);

    if has_search_bar {
        let bar_chunks =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(bottom_area);
        draw_search_bar(frame, state, input, bar_chunks[0]);
        draw_status_bar(frame, state, bar_chunks[1]);
    } else {
        draw_status_bar(frame, state, bottom_area);
    }

    if state.overlay.show_links {
        draw_links(frame, state, frame.area());
    } else if state.overlay.show_toc {
        draw_toc(frame, state, frame.area());
    } else if state.overlay.show_help {
        draw_help(frame, frame.area());
    }
}

fn draw_search_bar(frame: &mut Frame, state: &AppState, input: &InputHandler, area: Rect) {
    let white = state.theme.map_color(&crate::document::Color::White);
    let line = Line::from(vec![
        Span::styled(
            "/",
            Style::default()
                .fg(RatColor::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(input.search_query.as_str(), Style::default().fg(white)),
        Span::styled("█", Style::default().fg(RatColor::Gray)),
    ]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    let total = state.document.total_lines();
    let current = state.viewport.scroll_offset + 1;
    let percent = if total == 0 {
        100
    } else {
        ((state.viewport.scroll_offset + state.viewport.height).min(total) * 100) / total
    };

    let file_name = state
        .files
        .file_name()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "[stdin]".to_string());
    let position = format!(" {}/{} ({}%) ", current, total, percent);

    let status_style = Style::default()
        .fg(state.theme.status_fg)
        .bg(state.theme.status_bg);

    let left = Span::styled(
        format!(" {} ", file_name),
        status_style.add_modifier(Modifier::BOLD),
    );

    let black = state.theme.map_color(&crate::document::Color::Black);
    let help_hint = Span::styled(
        " ? help ",
        Style::default().fg(RatColor::DarkGray).bg(black),
    );

    let status_msg = if let Some(msg) = &state.overlay.status_message {
        Span::styled(
            format!(" {} ", msg),
            Style::default().fg(RatColor::Yellow).bg(black),
        )
    } else {
        Span::raw("")
    };

    let right = Span::styled(position, status_style);

    let left_len = file_name.len() + 2;
    let help_len = 8;
    let status_len = state
        .overlay
        .status_message
        .as_ref()
        .map_or(0, |m| m.len() + 2);
    let right_len = right.content.len();
    let fill_len =
        (area.width as usize).saturating_sub(left_len + help_len + status_len + right_len);
    let fill = Span::styled(" ".repeat(fill_len), Style::default().bg(black));

    let line = Line::from(vec![left, help_hint, status_msg, fill, right]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            " Keybindings ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(RatColor::Cyan),
        )),
        Line::from(""),
        help_line("j/↓", "Scroll down"),
        help_line("k/↑", "Scroll up"),
        help_line("d", "Half page down"),
        help_line("u", "Half page up"),
        help_line("PgDn/C-f", "Page down"),
        help_line("PgUp/C-b", "Page up"),
        help_line("gg", "Go to top"),
        help_line("G", "Go to bottom"),
        help_line("10j", "Scroll down 10 lines"),
        help_line("/", "Search"),
        help_line("n/N", "Next/prev match"),
        help_line("]/[", "Next/prev heading"),
        help_line("t", "Table of contents"),
        help_line("s", "TOC side pane (focus)"),
        help_line("z", "Fold/unfold section"),
        help_line("Z", "Fold all sections"),
        help_line("U", "Unfold all sections"),
        help_line("l", "Link list"),
        help_line("o", "Open link on cursor line"),
        help_line("r", "Reload file"),
        help_line("C-n/C-p", "Next/prev file"),
        help_line("?", "Toggle help"),
        help_line("q", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? or Esc to close ",
            Style::default().fg(RatColor::DarkGray),
        )),
    ];

    let help_height = help_text.len() as u16 + 2;
    let help_width = HELP_DIALOG_WIDTH;
    let x = area.width.saturating_sub(help_width) / 2;
    let y = area.height.saturating_sub(help_height) / 2;
    let help_area = Rect::new(
        x,
        y,
        help_width.min(area.width),
        help_height.min(area.height),
    );

    frame.render_widget(Clear, help_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RatColor::Cyan))
        .title(" Help ");
    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, help_area);
}

fn draw_toc(frame: &mut Frame, state: &AppState, area: Rect) {
    let headings = &state.document.headings;
    if headings.is_empty() {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, h) in headings.iter().enumerate() {
        let indent = "  ".repeat(h.level.saturating_sub(1));
        let prefix = if i == state.overlay.toc_cursor {
            "▸ "
        } else {
            "  "
        };
        let style = if i == state.overlay.toc_cursor {
            Style::default()
                .fg(RatColor::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            let white = state.theme.map_color(&crate::document::Color::White);
            Style::default().fg(white)
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}{}", prefix, indent, h.title),
            style,
        )));
    }

    let toc_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let toc_width = TOC_DIALOG_WIDTH.min(area.width.saturating_sub(4));
    let x = area.width.saturating_sub(toc_width) / 2;
    let y = area.height.saturating_sub(toc_height) / 2;
    let toc_area = Rect::new(x, y, toc_width, toc_height);

    frame.render_widget(Clear, toc_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RatColor::Cyan))
        .title(" Table of Contents ");

    let inner_height = toc_height.saturating_sub(2) as usize;
    let scroll = if state.overlay.toc_cursor >= inner_height {
        state.overlay.toc_cursor - inner_height + 1
    } else {
        0
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(paragraph, toc_area);
}

fn draw_links(frame: &mut Frame, state: &AppState, area: Rect) {
    let links = &state.document.links;
    if links.is_empty() {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, link) in links.iter().enumerate() {
        let prefix = if i == state.overlay.links_cursor {
            "▸ "
        } else {
            "  "
        };
        let style = if i == state.overlay.links_cursor {
            Style::default()
                .fg(RatColor::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(&link.text, style),
            Span::styled(
                format!("  {}", link.url),
                Style::default().fg(RatColor::DarkGray),
            ),
        ]));
    }

    let list_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let list_width = (area.width.saturating_sub(4)).min(LINKS_DIALOG_MAX_WIDTH);
    let x = area.width.saturating_sub(list_width) / 2;
    let y = area.height.saturating_sub(list_height) / 2;
    let list_area = Rect::new(x, y, list_width, list_height);

    frame.render_widget(Clear, list_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RatColor::Cyan))
        .title(" Links (Enter=jump, o=open, Esc=close) ");

    let inner_height = list_height.saturating_sub(2) as usize;
    let scroll = if state.overlay.links_cursor >= inner_height {
        state.overlay.links_cursor - inner_height + 1
    } else {
        0
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(paragraph, list_area);
}

fn draw_toc_pane(frame: &mut Frame, state: &AppState, area: Rect) {
    let headings = &state.document.headings;
    let abs_cursor = state.abs_cursor_line();

    // Find the nearest heading at or before cursor
    let active_idx = headings
        .iter()
        .enumerate()
        .rev()
        .find(|(_, h)| h.line_idx <= abs_cursor)
        .map(|(i, _)| i);

    let mut lines: Vec<Line> = Vec::new();
    for (i, h) in headings.iter().enumerate() {
        let indent = "  ".repeat(h.level.saturating_sub(1));
        let is_active = active_idx == Some(i);
        let is_focus_cursor = state.overlay.toc_pane_focus && i == state.overlay.toc_pane_cursor;
        let black = state.theme.map_color(&crate::document::Color::Black);
        let style = if is_focus_cursor {
            Style::default()
                .fg(black)
                .bg(RatColor::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(RatColor::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(RatColor::DarkGray)
        };
        let is_folded = state.viewport.folded_headings.contains(&h.line_idx);
        let marker = if is_folded { "▶ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{indent}{}", h.title),
            style,
        )));
    }

    // Scroll to keep active heading visible
    // RIGHT border only, no top/bottom padding
    let inner_height = area.height as usize;
    let scroll_target = if state.overlay.toc_pane_focus {
        state.overlay.toc_pane_cursor
    } else {
        active_idx.unwrap_or(0)
    };
    let scroll = if scroll_target >= inner_height {
        scroll_target - inner_height + 1
    } else {
        0
    };

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(RatColor::DarkGray))
        .title(" TOC ");
    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(paragraph, area);
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:<10}", key),
            Style::default().fg(RatColor::Yellow),
        ),
        Span::raw(desc),
    ])
}
