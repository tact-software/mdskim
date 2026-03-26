use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::util::find_node;

use anyhow::{Context, Result, bail};

const RENDER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

const MATH2SVG_SCRIPT: &str = include_str!("../../scripts/math2svg.cjs");

/// Renders LaTeX math expressions to SVG/PNG via MathJax/node.
pub struct MathRenderer {
    cache_dir: PathBuf,
    svg_cache: HashMap<u64, PathBuf>,
    png_cache: HashMap<u64, PathBuf>,
    node_path: Option<PathBuf>,
    script_path: Option<PathBuf>,
    enabled: bool,
}

impl MathRenderer {
    pub fn is_available(&self) -> bool {
        self.node_path.is_some()
    }

    pub fn new() -> Self {
        let cache_dir = std::env::temp_dir().join("mdskim-math");
        static NODE_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
        let node_path = NODE_PATH.get_or_init(find_node).clone();
        Self {
            cache_dir,
            svg_cache: HashMap::new(),
            png_cache: HashMap::new(),
            node_path,
            script_path: None,
            enabled: true,
        }
    }

    /// Render LaTeX to SVG, returning the path to the SVG file.
    pub fn render_to_svg(&mut self, source: &str, display: bool) -> Result<PathBuf> {
        if !self.enabled {
            bail!("Math rendering disabled (fast mode)");
        }
        let key = hash_source(source, display);
        if let Some(path) = self.svg_cache.get(&key)
            && path.exists()
        {
            return Ok(path.clone());
        }
        if let Some(path) =
            crate::util::disk_cache_lookup(&self.cache_dir, &mut self.svg_cache, key, "svg")
        {
            return Ok(path);
        }
        let output_path = self.render(source, key, display, "svg")?;
        self.svg_cache.insert(key, output_path.clone());
        Ok(output_path)
    }

    /// Render LaTeX to PNG, returning the path to the PNG file.
    /// Uses SVG as intermediate, then converts via resvg or falls back.
    pub fn render_to_png(&mut self, source: &str, display: bool) -> Result<PathBuf> {
        if !self.enabled {
            bail!("Math rendering disabled (fast mode)");
        }
        let key = hash_source(source, display);
        if let Some(path) = self.png_cache.get(&key)
            && path.exists()
        {
            return Ok(path.clone());
        }
        if let Some(path) =
            crate::util::disk_cache_lookup(&self.cache_dir, &mut self.png_cache, key, "png")
        {
            return Ok(path);
        }

        // Render SVG first
        let svg_path = self.render_to_svg(source, display)?;

        // Convert SVG to PNG
        let png_path = self.cache_dir.join(format!("{key}.png"));
        crate::util::svg_to_png(&svg_path, &png_path)?;
        self.png_cache.insert(key, png_path.clone());
        Ok(png_path)
    }

    fn render(&mut self, source: &str, key: u64, display: bool, format: &str) -> Result<PathBuf> {
        let node = self.node_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("node not found. Install Node.js to enable math rendering")
        })?;

        if !crate::util::has_npm_package("mathjax-full") {
            bail!(
                "mathjax-full not found.\n\
                 Run: mdskim setup --math"
            );
        }

        crate::util::create_private_dir(&self.cache_dir)
            .context("Failed to create math cache directory")?;

        // Ensure script is written to cache dir
        let script_path = self.script_path.get_or_insert_with(|| {
            let p = self.cache_dir.join("math2svg.cjs");
            if let Err(e) = std::fs::write(&p, MATH2SVG_SCRIPT) {
                eprintln!("WARN: Failed to write math2svg script: {e}");
            }
            p
        });

        let input_path = self.cache_dir.join(format!("{key}.tex"));
        let output_path = self.cache_dir.join(format!("{key}.{format}"));

        std::fs::File::create(&input_path)
            .and_then(|mut f| f.write_all(source.as_bytes()))
            .context("Failed to write math input file")?;

        let script_str = script_path
            .to_str()
            .context("Script path is not valid UTF-8")?;
        let input_str = input_path
            .to_str()
            .context("Input path is not valid UTF-8")?;
        let output_str = output_path
            .to_str()
            .context("Output path is not valid UTF-8")?;

        let mut args = vec![script_str, input_str, output_str];
        if display {
            args.push("--display");
        }

        let mut cmd = Command::new(node);
        cmd.args(&args);
        cmd.env("NODE_PATH", crate::util::node_modules_dir());

        let output = crate::util::run_with_timeout(&mut cmd, RENDER_TIMEOUT)
            .context("Failed to execute node for math rendering")?;

        let _ = std::fs::remove_file(&input_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Math rendering failed: {}", stderr.trim());
        }

        if !output_path.exists() {
            bail!("Math renderer did not produce output file");
        }

        Ok(output_path)
    }
}

fn hash_source(source: &str, display: bool) -> u64 {
    let mut data = source.as_bytes().to_vec();
    data.push(if display { 1 } else { 0 });
    crate::util::content_hash(&data)
}
