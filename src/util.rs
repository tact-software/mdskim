use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Return the mdskim data directory (`$XDG_DATA_HOME/mdskim` or `~/.local/share/mdskim`).
/// This is where `mdskim setup` installs npm dependencies.
pub fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("mdskim");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/mdskim");
    }
    PathBuf::from(".mdskim")
}

/// Return the `node_modules` directory inside the mdskim data directory.
pub fn node_modules_dir() -> PathBuf {
    data_dir().join("node_modules")
}

/// Check if a specific npm package is installed in the mdskim data directory.
pub fn has_npm_package(package: &str) -> bool {
    node_modules_dir().join(package).exists()
}

/// Find a system Chrome/Chromium executable for puppeteer-core.
pub fn find_chrome() -> Option<PathBuf> {
    // Environment variable override (highest priority)
    for var in ["CHROME_PATH", "PUPPETEER_EXECUTABLE_PATH"] {
        if let Ok(p) = std::env::var(var) {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    let candidates = [
        // macOS
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        // Linux
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
    ];

    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Find node binary: check PATH, then mise which.
pub fn find_node() -> Option<PathBuf> {
    if let Ok(output) = Command::new("node").arg("--version").output()
        && output.status.success()
    {
        return Some(PathBuf::from("node"));
    }

    if let Ok(output) = Command::new("mise").args(["which", "node"]).output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    None
}

/// Compute a hash of the given bytes using `DefaultHasher` (SipHash).
///
/// Note: This is NOT cryptographically secure and may produce collisions.
/// Suitable only for cache-key purposes where collisions are tolerable.
pub fn content_hash(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Run a command with a timeout. Returns the output or an error if the process
/// exceeds the timeout (the process is killed in that case).
pub fn run_with_timeout(
    cmd: &mut Command,
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn process")?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map_or_else(Vec::new, |mut s| {
                    let mut buf = Vec::new();
                    let _ = std::io::Read::read_to_end(&mut s, &mut buf);
                    buf
                });
                let stderr = child.stderr.take().map_or_else(Vec::new, |mut s| {
                    let mut buf = Vec::new();
                    let _ = std::io::Read::read_to_end(&mut s, &mut buf);
                    buf
                });
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("Process timed out after {}s", timeout.as_secs());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Create directory with mode 700 (owner-only) on Unix.
pub fn create_private_dir(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(path)
    }
}

/// Look up a cached file on disk. If `cache_dir/"{key}.{ext}"` exists, insert it
/// into `mem_cache` and return the path. Otherwise return `None`.
pub fn disk_cache_lookup(
    cache_dir: &std::path::Path,
    mem_cache: &mut std::collections::HashMap<u64, PathBuf>,
    key: u64,
    ext: &str,
) -> Option<PathBuf> {
    let cached = cache_dir.join(format!("{key}.{ext}"));
    if cached.exists() {
        mem_cache.insert(key, cached.clone());
        Some(cached)
    } else {
        None
    }
}

/// Convert SVG to PNG using the resvg crate (no external tools needed).
pub fn svg_to_png(svg_path: &Path, png_path: &Path) -> Result<()> {
    let svg_data = std::fs::read(svg_path).context("Failed to read SVG file")?;
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&svg_data, &opt).context("Failed to parse SVG")?;

    let size = tree.size();
    let scale = 2.0;
    let width = (size.width() * scale) as u32;
    let height = (size.height() * scale) as u32;
    if width == 0 || height == 0 {
        bail!("SVG has zero dimensions");
    }

    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).context("Failed to create pixmap")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap
        .save_png(png_path)
        .context("Failed to save PNG file")?;
    Ok(())
}

/// Remove cache files older than 7 days from mdskim temp directories.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_same_input_same_hash() {
        let data = b"hello world";
        assert_eq!(content_hash(data), content_hash(data));
    }

    #[test]
    fn content_hash_different_input_different_hash() {
        assert_ne!(content_hash(b"hello"), content_hash(b"world"));
    }

    #[test]
    fn content_hash_empty_input() {
        // Empty input should produce a consistent hash
        let h1 = content_hash(b"");
        let h2 = content_hash(b"");
        assert_eq!(h1, h2);
    }

    #[test]
    fn data_dir_returns_path_with_mdskim() {
        let dir = data_dir();
        assert!(
            dir.to_string_lossy().contains("mdskim"),
            "data_dir should contain 'mdskim': {:?}",
            dir
        );
    }

    #[test]
    fn has_npm_package_nonexistent() {
        // A package that definitely doesn't exist
        assert!(!has_npm_package("__nonexistent_package_12345__"));
    }

    #[test]
    fn find_node_returns_option() {
        // Just check it doesn't panic; result depends on environment
        let _ = find_node();
    }

    #[test]
    fn node_modules_dir_under_data_dir() {
        let nm = node_modules_dir();
        let dd = data_dir();
        assert!(
            nm.starts_with(&dd),
            "node_modules_dir should be under data_dir"
        );
    }

    #[test]
    fn disk_cache_lookup_missing_file() {
        let dir = std::env::temp_dir().join("mdskim-test-cache-nonexistent");
        let mut cache = std::collections::HashMap::new();
        let result = disk_cache_lookup(&dir, &mut cache, 12345, "png");
        assert!(result.is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn disk_cache_lookup_existing_file() {
        let dir = std::env::temp_dir().join("mdskim-test-cache-lookup");
        let _ = std::fs::create_dir_all(&dir);
        let key: u64 = 99999;
        let file_path = dir.join(format!("{key}.txt"));
        std::fs::write(&file_path, "test").unwrap();

        let mut cache = std::collections::HashMap::new();
        let result = disk_cache_lookup(&dir, &mut cache, key, "txt");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), file_path);
        assert!(cache.contains_key(&key));

        // cleanup
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir(&dir);
    }
}

pub fn evict_old_cache() {
    let max_age = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let cache_dirs = [
        "mdskim-mermaid",
        "mdskim-math",
        "mdskim-images",
        "mdskim-svg-cache",
    ];
    let tmp = std::env::temp_dir();
    for dir_name in &cache_dirs {
        let dir = tmp.join(dir_name);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if let Ok(modified) = meta.modified()
                && modified.elapsed().unwrap_or_default() > max_age
                && meta.is_file()
                && !meta.file_type().is_symlink()
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
