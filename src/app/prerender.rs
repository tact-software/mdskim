use std::path::PathBuf;

use crate::math::MathRenderer;
use crate::mermaid::MermaidRenderer;

use super::AppState;
use super::download::{download_image, short_error, svg_to_display_png};

impl AppState {
    /// Check availability of external tools (mmdc, node) and show a warning if missing.
    pub fn check_tool_availability(&mut self) {
        let mut missing = Vec::new();
        if !self.document.mermaid_blocks.is_empty() && !MermaidRenderer::new().is_available() {
            missing.push("mmdc (Mermaid)");
        }
        if !self.document.math_blocks.is_empty() && !MathRenderer::new().is_available() {
            missing.push("node (Math)");
        }
        if !missing.is_empty() {
            self.overlay.status_message = Some(format!("Missing tools: {}", missing.join(", ")));
        }
    }

    pub fn prerender_mermaid(&mut self) {
        let total = self.document.mermaid_blocks.len();
        if total == 0 {
            return;
        }

        // Parallel rendering: each thread gets its own MermaidRenderer
        let blocks: Vec<(usize, String)> = self
            .document
            .mermaid_blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.source.clone()))
            .collect();

        let max_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let results: Vec<(usize, Result<std::path::PathBuf, String>)> = blocks
            .chunks(max_threads)
            .flat_map(|chunk| {
                std::thread::scope(|s| {
                    let handles: Vec<_> = chunk
                        .iter()
                        .map(|(i, source)| {
                            let i = *i;
                            s.spawn(move || {
                                let mut renderer = MermaidRenderer::new();
                                match renderer.render_to_png(source) {
                                    Ok(path) => (i, Ok(path)),
                                    Err(e) => (i, Err(e.to_string())),
                                }
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| match h.join() {
                            Ok(result) => result,
                            Err(_) => (0, Err("thread panicked".to_string())),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        let mut successes = 0usize;
        let mut last_err = String::new();
        for (i, result) in results {
            match result {
                Ok(path) => {
                    self.render_cache.mermaid_images.insert(i, path);
                    successes += 1;
                }
                Err(msg) => {
                    self.render_cache
                        .mermaid_errors
                        .insert(i, short_error(&msg));
                    last_err = msg;
                }
            }
        }
        let failures = total - successes;
        if failures > 0 {
            self.overlay.status_message = Some(format!(
                "Mermaid: {successes}/{total} rendered ({})",
                short_error(&last_err)
            ));
        }
    }

    pub fn prerender_math(&mut self) {
        let total = self
            .document
            .math_blocks
            .iter()
            .filter(|b| b.display)
            .count();
        if total == 0 {
            return;
        }

        // Parallel rendering: each thread gets its own MathRenderer
        let blocks: Vec<(usize, String, bool)> = self
            .document
            .math_blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.display)
            .map(|(i, b)| (i, b.source.clone(), b.display))
            .collect();

        let max_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let results: Vec<(usize, Result<std::path::PathBuf, String>)> = blocks
            .chunks(max_threads)
            .flat_map(|chunk| {
                std::thread::scope(|s| {
                    let handles: Vec<_> = chunk
                        .iter()
                        .map(|(i, source, display)| {
                            let i = *i;
                            let display = *display;
                            s.spawn(move || {
                                let mut renderer = MathRenderer::new();
                                match renderer.render_to_png(source, display) {
                                    Ok(path) => (i, Ok(path)),
                                    Err(e) => (i, Err(e.to_string())),
                                }
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| match h.join() {
                            Ok(result) => result,
                            Err(_) => (0, Err("thread panicked".to_string())),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        let mut successes = 0usize;
        let mut last_err = String::new();
        for (i, result) in results {
            match result {
                Ok(path) => {
                    self.render_cache.math_images.insert(i, path);
                    successes += 1;
                }
                Err(msg) => {
                    self.render_cache.math_errors.insert(i, short_error(&msg));
                    last_err = msg;
                }
            }
        }
        let failures = total - successes;
        if failures > 0 {
            let msg = format!(
                "Math: {successes}/{total} rendered ({})",
                short_error(&last_err)
            );
            // Append to existing status_message if present
            if let Some(existing) = &self.overlay.status_message {
                self.overlay.status_message = Some(format!("{existing} | {msg}"));
            } else {
                self.overlay.status_message = Some(msg);
            }
        }
    }

    pub fn prerender_images(&mut self) {
        let total = self.document.images.len();
        if total == 0 {
            return;
        }
        let base_dir = self
            .files
            .file_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let image_cache_dir = std::env::temp_dir().join("mdskim-images");
        let svg_cache_dir = std::env::temp_dir().join("mdskim-svg-cache");
        let mut failures = 0usize;
        for (i, img) in self.document.images.iter().enumerate() {
            let resolved = if img.path.starts_with("http://") || img.path.starts_with("https://") {
                eprintln!("INFO: Downloading remote image: {}", img.path);
                match download_image(&img.path, &image_cache_dir) {
                    Ok(path) => path,
                    Err(_) => {
                        failures += 1;
                        continue;
                    }
                }
            } else {
                let local: PathBuf = if let Some(base) = &base_dir {
                    base.join(&img.path)
                } else {
                    PathBuf::from(&img.path)
                };
                if !local.exists() {
                    failures += 1;
                    continue;
                }
                local
            };

            // Convert SVG to PNG for terminal display
            if resolved
                .extension()
                .is_some_and(|ext: &std::ffi::OsStr| ext.eq_ignore_ascii_case("svg"))
            {
                match svg_to_display_png(&resolved, &svg_cache_dir) {
                    Ok(png_path) => {
                        self.render_cache.image_paths.insert(i, png_path);
                    }
                    Err(_) => {
                        failures += 1;
                    }
                }
            } else {
                self.render_cache.image_paths.insert(i, resolved);
            }
        }
        if failures > 0 {
            let successes = total - failures;
            let msg = format!("Images: {successes}/{total} resolved");
            if let Some(existing) = &self.overlay.status_message {
                self.overlay.status_message = Some(format!("{existing} | {msg}"));
            } else {
                self.overlay.status_message = Some(msg);
            }
        }
    }
}
