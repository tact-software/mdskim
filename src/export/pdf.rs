use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::document;

const HTML2PDF_SCRIPT: &str = include_str!("../../scripts/html2pdf.cjs");
const PDF_TIMEOUT: Duration = Duration::from_secs(60);

/// Generate PDF from a document via puppeteer-core + system Chrome/Chromium.
///
/// Strategy:
/// 1. Render to HTML (full-featured: images, Mermaid SVG, Math SVG)
/// 2. Convert HTML to PDF via headless Chrome
pub fn generate(
    doc: &document::Document,
    output_path: &Path,
    custom_css: Option<&str>,
    base_dir: Option<&Path>,
    no_sandbox: bool,
) -> Result<()> {
    let html = super::to_html(doc, super::ExportTheme::Light, custom_css, base_dir);

    if !crate::util::has_npm_package("puppeteer-core") {
        bail!(
            "puppeteer-core not found.\n\
             Run: mdskim setup --pdf"
        );
    }

    let tmp_dir = std::env::temp_dir().join("mdskim-pdf");
    crate::util::create_private_dir(&tmp_dir).context("Failed to create PDF temp directory")?;

    let unique_id = format!(
        "{}_{}",
        std::process::id(),
        crate::util::content_hash(html.as_bytes())
    );
    let tmp_html = tmp_dir.join(format!("export_{unique_id}.html"));
    std::fs::write(&tmp_html, &html).context("Failed to write temp HTML")?;

    let script_path = tmp_dir.join(format!("html2pdf_{unique_id}.cjs"));
    std::fs::write(&script_path, HTML2PDF_SCRIPT).context("Failed to write html2pdf script")?;

    let mut cmd = Command::new("node");
    cmd.env("NODE_PATH", crate::util::node_modules_dir()).args([
        script_path
            .to_str()
            .context("Script path is not valid UTF-8")?,
        tmp_html
            .to_str()
            .context("Temp HTML path is not valid UTF-8")?,
        output_path
            .to_str()
            .context("Output path is not valid UTF-8")?,
    ]);
    if no_sandbox {
        cmd.arg("--no-sandbox");
    }
    let output = crate::util::run_with_timeout(&mut cmd, PDF_TIMEOUT)
        .context("Failed to execute node. Is node installed?")?;

    let _ = std::fs::remove_file(&tmp_html);
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("PDF generation failed:\n{stderr}");
    }
    Ok(())
}
