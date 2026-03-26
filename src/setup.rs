use std::process::Command;

use anyhow::{Context, Result, bail};

/// Packages that `mdskim setup` can install.
const MERMAID_PACKAGES: &[&str] = &["mermaid"];
const MATH_PACKAGES: &[&str] = &["mathjax-full"];
const PDF_PACKAGES: &[&str] = &["puppeteer-core"];

/// Run `mdskim setup` to install npm dependencies into the data directory.
pub fn run(mermaid: bool, math: bool, pdf: bool) -> Result<()> {
    // Pre-check: node must be available
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !node_ok {
        bail!(
            "Node.js not found.\n\n\
             mdskim setup requires Node.js to install rendering dependencies.\n\
             Install with:  mise use -g node\n\
             Or see:        https://nodejs.org/"
        );
    }

    // Pre-check: npm must be available
    let npm_ok = Command::new("npm")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !npm_ok {
        bail!("npm not found. Ensure Node.js is installed correctly.");
    }

    // Determine which packages to install
    let install_all = !mermaid && !math && !pdf;
    let mut packages: Vec<&str> = Vec::new();
    if install_all || mermaid {
        packages.extend_from_slice(MERMAID_PACKAGES);
    }
    if install_all || math {
        packages.extend_from_slice(MATH_PACKAGES);
    }
    if install_all || pdf {
        packages.extend_from_slice(PDF_PACKAGES);
    }

    if packages.is_empty() {
        eprintln!("Nothing to install.");
        return Ok(());
    }

    let data_dir = crate::util::data_dir();
    crate::util::create_private_dir(&data_dir).context("Failed to create data directory")?;

    // Create package.json if it doesn't exist
    let pkg_json = data_dir.join("package.json");
    if !pkg_json.exists() {
        std::fs::write(
            &pkg_json,
            r#"{"private": true, "description": "mdskim rendering dependencies"}"#,
        )
        .context("Failed to create package.json")?;
    }

    eprintln!("Installing: {}", packages.join(", "));
    eprintln!("Directory:  {}", data_dir.display());

    let output = Command::new("npm")
        .args(["install", "--save"])
        .args(&packages)
        .current_dir(&data_dir)
        .output()
        .context("Failed to run npm install")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("npm install failed:\n{stderr}");
    }

    eprintln!("\nInstalled successfully.");

    // Post-install checks
    if install_all || mermaid || pdf {
        if crate::util::find_chrome().is_some() {
            eprintln!("Chrome/Chromium: found");
        } else {
            eprintln!(
                "\nNote: Chrome/Chromium not found.\n\
                 Mermaid PNG and PDF export require a system browser.\n\
                 Mermaid SVG output works without a browser.\n\
                 Install: brew install --cask google-chrome  (macOS)\n\
                          apt install chromium-browser        (Linux)"
            );
        }
    }

    Ok(())
}
