use std::io::Read;
use std::path::{Path, PathBuf};

/// Maximum size for downloaded images (50 MB).
const MAX_IMAGE_DOWNLOAD_SIZE: u64 = 50 * 1024 * 1024;

pub(super) fn is_private_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    // Strip surrounding brackets for IPv6
    let bare = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()          // 127.0.0.0/8
                    || v4.is_private()     // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                    || v4.is_link_local()  // 169.254.0.0/16
                    || v4.is_unspecified() // 0.0.0.0
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()          // ::1
                    || v6.is_unspecified() // ::
                    // fc00::/7 (unique local addresses)
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
            }
        }
    } else {
        false
    }
}

pub(super) fn extract_host(url: &str) -> Option<&str> {
    let after_scheme = url.find("://").map(|i| &url[i + 3..])?;
    // Skip userinfo (user:pass@)
    let after_userinfo = after_scheme
        .find('@')
        .map(|i| &after_scheme[i + 1..])
        .unwrap_or(after_scheme);
    // IPv6 bracket notation: http://[::1]/path
    if after_userinfo.starts_with('[') {
        let end = after_userinfo.find(']')?;
        let host = &after_userinfo[..end + 1];
        if host.len() <= 2 { None } else { Some(host) }
    } else {
        // Host ends at '/', ':', or '?'
        let end = after_userinfo
            .find(['/', ':', '?'])
            .unwrap_or(after_userinfo.len());
        let host = &after_userinfo[..end];
        if host.is_empty() { None } else { Some(host) }
    }
}

pub(super) fn short_error(msg: &str) -> String {
    msg.lines().next().unwrap_or(msg).chars().take(80).collect()
}

pub(super) fn download_image(url: &str, cache_dir: &Path) -> anyhow::Result<PathBuf> {
    // SSRF protection: block requests to private/internal networks
    if let Some(host) = extract_host(url)
        && is_private_host(host)
    {
        anyhow::bail!(
            "requests to private/internal addresses are blocked: {}",
            host
        );
    }

    // DNS rebinding protection: resolve hostname and check resolved IPs before connecting
    if let Some(host) = extract_host(url) {
        let bare = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        // Only check non-IP hostnames (IPs are already checked above)
        if bare.parse::<std::net::IpAddr>().is_err() {
            use std::net::ToSocketAddrs;
            let addr_str = format!("{bare}:0");
            if let Ok(addrs) = addr_str.to_socket_addrs() {
                for addr in addrs {
                    let ip_str = addr.ip().to_string();
                    if is_private_host(&ip_str) {
                        anyhow::bail!(
                            "DNS resolved to private/internal address ({}): {}",
                            ip_str,
                            host
                        );
                    }
                }
            }
        }
    }

    crate::util::create_private_dir(cache_dir)?;
    let hash = crate::util::content_hash(url.as_bytes());
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|s| s.rsplit('.').next())
        .unwrap_or("png");
    let ext = if ["png", "jpg", "jpeg", "gif", "webp", "svg"].contains(&ext) {
        ext
    } else {
        "png"
    };
    let cached = cache_dir.join(format!("{hash}.{ext}"));
    if cached.exists() {
        return Ok(cached);
    }

    // Disable automatic redirects to enforce SSRF checks on each hop
    let agent: ureq::Agent = ureq::config::Config::builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into();

    let max_redirects = 10;
    let mut current_url = url.to_string();
    let mut response = None;
    for _ in 0..=max_redirects {
        let resp = agent.get(&current_url).call()?;
        let status = resp.status().as_u16();
        if (301..=303).contains(&status) || status == 307 || status == 308 {
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let Some(next_url) = location else {
                anyhow::bail!("redirect without Location header");
            };
            // SSRF check on redirect target
            if let Some(host) = extract_host(&next_url)
                && is_private_host(host)
            {
                anyhow::bail!("redirect to private/internal address blocked: {}", host);
            }
            current_url = next_url;
            continue;
        }
        response = Some(resp);
        break;
    }
    let Some(response) = response else {
        anyhow::bail!("too many redirects");
    };

    let mut body = response.into_body();
    let mut reader = body.as_reader().take(MAX_IMAGE_DOWNLOAD_SIZE + 1);
    let mut file = std::fs::File::create(&cached)?;
    let written = std::io::copy(&mut reader, &mut file)?;
    if written > MAX_IMAGE_DOWNLOAD_SIZE {
        let _ = std::fs::remove_file(&cached);
        anyhow::bail!(
            "image download exceeds size limit ({} bytes)",
            MAX_IMAGE_DOWNLOAD_SIZE
        );
    }
    Ok(cached)
}

pub(super) fn svg_to_display_png(svg_path: &Path, cache_dir: &Path) -> anyhow::Result<PathBuf> {
    crate::util::create_private_dir(cache_dir)?;
    let hash = crate::util::content_hash(svg_path.to_string_lossy().as_bytes());
    let png_path = cache_dir.join(format!("{hash}.png"));
    if png_path.exists() {
        return Ok(png_path);
    }
    crate::util::svg_to_png(svg_path, &png_path)?;
    Ok(png_path)
}
