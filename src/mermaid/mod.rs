use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::util::find_node;

use anyhow::{Context, Result, bail};

const RENDER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

const MERMAID_RENDER_SCRIPT: &str = include_str!("../../scripts/mermaid_render.cjs");

/// Renders Mermaid diagrams to SVG/PNG via node + mermaid_render.cjs.
pub struct MermaidRenderer {
    cache_dir: PathBuf,
    svg_cache: HashMap<u64, PathBuf>,
    png_cache: HashMap<u64, PathBuf>,
    node_path: Option<PathBuf>,
    script_path: Option<PathBuf>,
    enabled: bool,
}

impl MermaidRenderer {
    pub fn is_available(&self) -> bool {
        self.node_path.is_some() && crate::util::has_npm_package("mermaid")
    }

    pub fn new() -> Self {
        let cache_dir = std::env::temp_dir().join("mdskim-mermaid");
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

    /// Render mermaid source to SVG, returning the path to the SVG file.
    pub fn render_to_svg(&mut self, source: &str) -> Result<PathBuf> {
        if !self.enabled {
            bail!("Mermaid rendering disabled (fast mode)");
        }
        let hash = hash_source(source);
        if let Some(path) = self.svg_cache.get(&hash)
            && path.exists()
        {
            return Ok(path.clone());
        }
        if let Some(path) =
            crate::util::disk_cache_lookup(&self.cache_dir, &mut self.svg_cache, hash, "svg")
        {
            return Ok(path);
        }
        let output_path = self.render(source, hash, "svg")?;
        self.svg_cache.insert(hash, output_path.clone());
        Ok(output_path)
    }

    /// Render mermaid source to PNG, returning the path to the PNG file.
    pub fn render_to_png(&mut self, source: &str) -> Result<PathBuf> {
        if !self.enabled {
            bail!("Mermaid rendering disabled (fast mode)");
        }
        let hash = hash_source(source);
        if let Some(path) = self.png_cache.get(&hash)
            && path.exists()
        {
            return Ok(path.clone());
        }
        if let Some(path) =
            crate::util::disk_cache_lookup(&self.cache_dir, &mut self.png_cache, hash, "png")
        {
            return Ok(path);
        }
        let output_path = self.render(source, hash, "png")?;
        self.png_cache.insert(hash, output_path.clone());
        Ok(output_path)
    }

    fn render(&mut self, source: &str, hash: u64, format: &str) -> Result<PathBuf> {
        let node = self.node_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("node not found. Install Node.js to enable Mermaid rendering")
        })?;

        if !crate::util::has_npm_package("mermaid") {
            bail!("mermaid not found. Run: mdskim setup --mermaid");
        }

        crate::util::create_private_dir(&self.cache_dir)
            .context("Failed to create mermaid cache directory")?;

        // Ensure script is written to cache dir
        let script_path = self.script_path.get_or_insert_with(|| {
            let p = self.cache_dir.join("mermaid_render.cjs");
            if let Err(e) = std::fs::write(&p, MERMAID_RENDER_SCRIPT) {
                eprintln!("WARN: Failed to write mermaid_render script: {e}");
            }
            p
        });

        let input_path = self.cache_dir.join(format!("{hash}.mmd"));
        let output_path = self.cache_dir.join(format!("{hash}.{format}"));

        std::fs::File::create(&input_path)
            .and_then(|mut f| f.write_all(source.as_bytes()))
            .context("Failed to write mermaid input file")?;

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

        let chrome_path = crate::util::find_chrome();
        let chrome_str;
        if let Some(ref chrome) = chrome_path {
            chrome_str = chrome
                .to_str()
                .context("Chrome path is not valid UTF-8")?
                .to_string();
            args.push("--chrome");
            args.push(&chrome_str);
        }

        let mut cmd = Command::new(node);
        cmd.args(&args);
        cmd.env("NODE_PATH", crate::util::node_modules_dir());

        let output = crate::util::run_with_timeout(&mut cmd, RENDER_TIMEOUT)
            .context("Failed to execute node for mermaid rendering")?;

        let _ = std::fs::remove_file(&input_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let msg = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            bail!("Mermaid rendering failed: {}", msg);
        }

        if !output_path.exists() {
            bail!("Mermaid renderer did not produce output file");
        }

        Ok(output_path)
    }
}

fn hash_source(source: &str) -> u64 {
    crate::util::content_hash(source.as_bytes())
}
