use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};
use ratatui_image::StatefulImage;

use unicode_width::UnicodeWidthStr;

use crate::app::AppState;
use crate::document::SpanStyle;
use crate::theme::Theme;
use crate::ui::ImageState;

const MAX_MERMAID_IMAGE_HEIGHT: u16 = 15;
const MAX_MATH_IMAGE_HEIGHT: u16 = 8;
const MAX_DOC_IMAGE_HEIGHT: u16 = 10;

/// Calculate image display height based on aspect ratio.
///
/// Uses the image's pixel dimensions and the available terminal width to compute
/// an appropriate height that preserves the aspect ratio. Terminal cells are
/// roughly twice as tall as they are wide, so the width-to-height ratio is
/// adjusted by a factor of 2. Falls back to `max_height` when dimensions
/// cannot be determined.
fn calc_image_height(
    path: &Path,
    available_width: u16,
    max_height: u16,
    dim_cache: &HashMap<PathBuf, (u32, u32)>,
) -> u16 {
    let dims = dim_cache
        .get(path)
        .copied()
        .or_else(|| image::image_dimensions(path).ok());
    if let Some((pw, ph)) = dims
        && pw > 0
        && ph > 0
    {
        // Terminal cells are ~2:1 (height:width in pixels), so divide by 2
        let height = ((ph as f64 / pw as f64) * available_width as f64 / 2.0).ceil() as u16;
        return height.clamp(1, max_height);
    }
    max_height
}
const CURSOR_LINE_BG: ratatui::style::Color = ratatui::style::Color::Rgb(50, 50, 70);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImageCacheKey {
    Mermaid(usize),
    Math(usize),
    Document(usize),
}

enum ImageSource {
    Mermaid(usize),
    Math(usize),
    Document(usize),
}

struct PendingImage {
    source: ImageSource,
    area: Rect,
}

pub fn draw_content(frame: &mut Frame, state: &AppState, area: Rect, img_state: &mut ImageState) {
    img_state.sync_generation(state.render_cache.generation);

    let mermaid_lines = &state.document.mermaid_line_map;
    let math_lines = &state.document.math_line_map;
    let image_lines = &state.document.image_line_map;

    let visible_lines: Vec<(usize, &crate::document::StyledLine)> = state
        .document
        .lines
        .iter()
        .enumerate()
        .skip(state.viewport.scroll_offset)
        .filter(|(idx, _)| state.is_line_visible(*idx))
        .take(area.height as usize)
        .collect();

    let mut pending_images: Vec<PendingImage> = Vec::new();

    // Pass 1: render text lines to buffer, collect image positions
    {
        let buf = frame.buffer_mut();
        let mut skip_lines: usize = 0;
        let mut row_idx: usize = 0;
        let mut i = 0;

        while i < visible_lines.len() {
            let (line_idx, line) = visible_lines[i];

            if skip_lines > 0 {
                skip_lines -= 1;
                i += 1;
                continue;
            }

            let y = area.y + row_idx as u16;
            if y >= area.y + area.height {
                break;
            }

            // Check if this is a mermaid placeholder line
            if let Some(&block_idx) = mermaid_lines.get(&line_idx)
                && let Some(png_path) = state.render_cache.mermaid_images.get(&block_idx)
                && png_path.exists()
                && img_state.supports_images()
            {
                let remaining_rows = (area.y + area.height - y) as usize;
                let desired = calc_image_height(
                    png_path,
                    area.width,
                    MAX_MERMAID_IMAGE_HEIGHT,
                    &img_state.dimension_cache,
                );
                let img_height = remaining_rows.min(desired as usize);
                let img_area = Rect::new(area.x, y, area.width, img_height as u16);
                pending_images.push(PendingImage {
                    source: ImageSource::Mermaid(block_idx),
                    area: img_area,
                });
                row_idx += img_height;
                skip_lines = 1;
                i += 1;
                continue;
            }

            // Check if this is a display math placeholder line
            if let Some(&block_idx) = math_lines.get(&line_idx)
                && let Some(png_path) = state.render_cache.math_images.get(&block_idx)
                && png_path.exists()
                && img_state.supports_images()
            {
                let remaining_rows = (area.y + area.height - y) as usize;
                let desired = calc_image_height(
                    png_path,
                    area.width,
                    MAX_MATH_IMAGE_HEIGHT,
                    &img_state.dimension_cache,
                );
                let img_height = remaining_rows.min(desired as usize);
                let img_area = Rect::new(area.x, y, area.width, img_height as u16);
                pending_images.push(PendingImage {
                    source: ImageSource::Math(block_idx),
                    area: img_area,
                });
                row_idx += img_height;
                // Skip placeholder + source fallback lines + empty line
                let block = &state.document.math_blocks[block_idx];
                skip_lines = block.source.lines().count() + 1;
                i += 1;
                continue;
            }

            // Check if this line is a document image placeholder
            if let Some(&img_idx) = image_lines.get(&line_idx)
                && let Some(img_path) = state.render_cache.image_paths.get(&img_idx)
                && img_state.supports_images()
            {
                let remaining_rows = (area.y + area.height - y) as usize;
                let desired = calc_image_height(
                    img_path,
                    area.width,
                    MAX_DOC_IMAGE_HEIGHT,
                    &img_state.dimension_cache,
                );
                let img_height = remaining_rows.min(desired as usize);
                let img_area = Rect::new(area.x, y, area.width, img_height as u16);
                pending_images.push(PendingImage {
                    source: ImageSource::Document(img_idx),
                    area: img_area,
                });
                row_idx += img_height;
                i += 1;
                continue;
            }

            let row_area = Rect::new(area.x, y, area.width, 1);
            let is_cursor_line =
                row_idx == state.viewport.cursor_line && !state.overlay.toc_pane_focus;

            if is_cursor_line {
                // Cursor line: subtle highlight
                let cursor_style = Style::default().bg(CURSOR_LINE_BG);
                fill_area(buf, row_area, cursor_style);
            } else if let Some(bg) = &line.line_bg {
                let bg_style = Style::default().bg(state.theme.map_color(bg));
                fill_area(buf, row_area, bg_style);
            }

            let highlight_ranges = search_highlight_ranges(state, line_idx);

            let mut x_offset = area.x;
            let mut text_offset: usize = 0;
            for span in &line.spans {
                let style = span_style_to_ratatui(&span.style, &state.theme);
                let remaining = (area.x + area.width).saturating_sub(x_offset) as usize;
                if remaining == 0 {
                    break;
                }
                let text = truncate_to_width(&span.content, remaining);
                if text.is_empty() {
                    text_offset += span.content.len();
                    continue;
                }

                if highlight_ranges.is_empty() {
                    let text_width = text.width() as u16;
                    buf.set_string(x_offset, y, text, style);
                    x_offset += text_width;
                } else {
                    for (ci, ch) in text.char_indices() {
                        let abs_pos = text_offset + ci;
                        let ch_width =
                            unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
                        if x_offset + ch_width > area.x + area.width {
                            break;
                        }
                        let ch_style = if is_highlighted(&highlight_ranges, abs_pos) {
                            Style::default()
                                .fg(state.theme.search_fg)
                                .bg(state.theme.search_bg)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            style
                        };
                        let mut ch_buf = [0u8; 4];
                        buf.set_string(x_offset, y, ch.encode_utf8(&mut ch_buf), ch_style);
                        x_offset += ch_width;
                    }
                }
                text_offset += span.content.len();
            }

            row_idx += 1;
            i += 1;
        }
    }

    // Pass 2: render images via stateful widgets (cache-hit only, no blocking I/O)
    for pending in &pending_images {
        let cache_key = match &pending.source {
            ImageSource::Mermaid(idx) => ImageCacheKey::Mermaid(*idx),
            ImageSource::Math(idx) => ImageCacheKey::Math(*idx),
            ImageSource::Document(idx) => ImageCacheKey::Document(*idx),
        };
        if let Some(proto) = img_state.protocols.get_mut(&cache_key) {
            let image_widget = StatefulImage::default();
            frame.render_stateful_widget(image_widget, pending.area, proto);
        }
    }
}

fn fill_area(buf: &mut Buffer, area: Rect, style: Style) {
    for x in area.x..area.x + area.width {
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_style(style);
            if cell.symbol() == "" {
                cell.set_symbol(" ");
            }
        }
    }
}

fn search_highlight_ranges(state: &AppState, line_idx: usize) -> Vec<(usize, usize)> {
    if state.search.query.is_empty() {
        return Vec::new();
    }
    let matches = &state.search.matches;
    let start = matches.partition_point(|m| m.line_idx < line_idx);
    let end = matches.partition_point(|m| m.line_idx <= line_idx);
    matches[start..end]
        .iter()
        .map(|m| (m.byte_start, m.byte_end))
        .collect()
}

fn is_highlighted(ranges: &[(usize, usize)], byte_pos: usize) -> bool {
    ranges
        .iter()
        .any(|(start, end)| byte_pos >= *start && byte_pos < *end)
}

fn span_style_to_ratatui(style: &SpanStyle, theme: &Theme) -> Style {
    let mut s = Style::default();
    if let Some(fg) = &style.fg {
        s = s.fg(theme.map_color(fg));
    }
    if let Some(bg) = &style.bg {
        s = s.bg(theme.map_color(bg));
    }
    if style.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if style.underline {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if style.strikethrough {
        s = s.add_modifier(Modifier::CROSSED_OUT);
    }
    if style.dim {
        s = s.add_modifier(Modifier::DIM);
    }
    s
}

fn truncate_to_width(s: &str, max_width: usize) -> &str {
    if s.width() <= max_width {
        return s;
    }
    let mut width = 0;
    for (idx, ch) in s.char_indices() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width {
            return &s[..idx];
        }
        width += ch_width;
    }
    s
}
