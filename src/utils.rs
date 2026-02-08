use anyhow::{anyhow, bail, Context, Result};
use console::style;
use retry::{
    delay::{jitter, Fibonacci},
    retry, OperationResult,
};
use semver::{BuildMetadata, Version};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use url::Url;

/// Resolves the Julia binary path, accounting for .app bundles on macOS
#[cfg(target_os = "macos")]
pub fn resolve_julia_binary_path(base_path: &Path) -> Result<PathBuf> {
    // Check if this is a .app bundle installation
    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".app"))
            {
                // This is a DMG installation with .app bundle
                let julia_path = entry
                    .path()
                    .join("Contents")
                    .join("Resources")
                    .join("julia")
                    .join("bin")
                    .join(format!("julia{}", std::env::consts::EXE_SUFFIX));

                if julia_path.exists() {
                    return Ok(julia_path);
                }
            }
        }
    }

    // Fall back to standard path (tarball installation)
    Ok(base_path
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX)))
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_julia_binary_path(base_path: &Path) -> Result<PathBuf> {
    Ok(base_path
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX)))
}

/// Cached result of whether the nightly server supports etag headers.
/// This is used to avoid repeated HTTP requests to check server capabilities.
static NIGHTLY_SERVER_SUPPORTS_ETAG: OnceLock<bool> = OnceLock::new();

/// Checks if the nightly server supports etag headers.
/// This is required for nightly and PR channel support because we use etags
/// to track versions of these builds.
///
/// The result is cached after the first check.
/// If JULIAUP_SERVER equals the default official ones, it works as usual (assumes ETAG support).
/// Otherwise, sends a HEAD check request to verify ETAG support.
#[cfg(not(windows))]
pub fn check_server_supports_nightlies() -> Result<bool> {
    Ok(*NIGHTLY_SERVER_SUPPORTS_ETAG.get_or_init(|| {
        // Check if JULIAUP_SERVER equals the default official one
        let is_default_server = {
            let julia_server = std::env::var("JULIAUP_SERVER")
                .unwrap_or_else(|_| "https://julialang-s3.julialang.org".to_string());

            // Normalize URL (remove trailing slashes for comparison)
            let julia_server_normalized = julia_server.trim_end_matches('/');

            julia_server_normalized == "https://julialang-s3.julialang.org"
        };

        // If using default official servers, assume ETAG support
        if is_default_server {
            return true;
        }

        // For custom servers, check via HEAD request
        let base_url = match get_julianightlies_base_url() {
            Ok(url) => url,
            Err(_) => return false,
        };

        let test_url = match base_url.join("bin/") {
            Ok(url) => url,
            Err(_) => return false,
        };

        let client = reqwest::blocking::Client::new();
        match client.head(test_url.as_str()).send() {
            Ok(response) => {
                let has_etag = response.headers().get("etag").is_some();
                log::debug!("Server etag support check: {}", has_etag);
                has_etag
            }
            Err(e) => {
                log::debug!("Failed to check server etag support: {}", e);
                false
            }
        }
    }))
}

/// Checks if the nightly server supports etag headers.
/// This is required for nightly and PR channel support because we use etags
/// to track versions of these builds.
///
/// The result is cached after the first check.
/// If JULIAUP_SERVER equals the default official ones, it works as usual (assumes ETAG support).
/// Otherwise, sends a HEAD check request to verify ETAG support.
#[cfg(windows)]
pub fn check_server_supports_nightlies() -> Result<bool> {
    use windows::core::HSTRING;
    use windows::Foundation::Uri;
    use windows::Web::Http::HttpClient;
    use windows::Web::Http::HttpMethod;
    use windows::Web::Http::HttpRequestMessage;

    Ok(*NIGHTLY_SERVER_SUPPORTS_ETAG.get_or_init(|| {
        // Check if JULIAUP_SERVER equals the default official one
        let is_default_server = {
            let julia_server = std::env::var("JULIAUP_SERVER")
                .unwrap_or_else(|_| "https://julialang-s3.julialang.org".to_string());

            // Parse and compare URLs properly to handle variations
            match Url::parse(&julia_server) {
                Ok(parsed) => {
                    if let Ok(default_url) = Url::parse("https://julialang-s3.julialang.org") {
                        parsed.scheme() == default_url.scheme()
                            && parsed.host_str() == default_url.host_str()
                            && parsed.path().trim_end_matches('/')
                                == default_url.path().trim_end_matches('/')
                    } else {
                        false
                    }
                }
                Err(_) => {
                    // Fall back to simple string comparison if URL parsing fails
                    let julia_server_normalized = julia_server.trim_end_matches('/');
                    julia_server_normalized == "https://julialang-s3.julialang.org"
                }
            }
        };

        // If using default official servers, assume ETAG support
        if is_default_server {
            return true;
        }

        // For custom servers, check via HEAD request
        let base_url = match get_julianightlies_base_url() {
            Ok(url) => url,
            Err(e) => {
                log::debug!("Failed to get nightly base URL: {}", e);
                return false;
            }
        };

        let test_url = match base_url.join("bin/") {
            Ok(url) => url,
            Err(e) => {
                log::debug!("Failed to join bin/ to base URL: {}", e);
                return false;
            }
        };

        let http_client = match HttpClient::new() {
            Ok(client) => client,
            Err(e) => {
                log::debug!("Failed to create HTTP client: {:?}", e);
                return false;
            }
        };

        let request_uri = match Uri::CreateUri(&HSTRING::from(test_url.as_str())) {
            Ok(uri) => uri,
            Err(e) => {
                log::debug!("Failed to create URI: {:?}", e);
                return false;
            }
        };

        let head_method = match HttpMethod::Head() {
            Ok(m) => m,
            Err(e) => {
                log::debug!("Failed to create HEAD method: {:?}", e);
                return false;
            }
        };

        let request = match HttpRequestMessage::Create(&head_method, &request_uri) {
            Ok(req) => req,
            Err(e) => {
                log::debug!("Failed to create request: {:?}", e);
                return false;
            }
        };

        let response = match http_client.SendRequestAsync(&request) {
            Ok(async_op) => match async_op.join() {
                Ok(resp) => resp,
                Err(e) => {
                    log::debug!("Failed to send HEAD request: {:?}", e);
                    return false;
                }
            },
            Err(e) => {
                log::debug!("Failed to start HEAD request: {:?}", e);
                return false;
            }
        };

        match response.Headers() {
            Ok(headers) => {
                let has_etag = headers.Lookup(&HSTRING::from("ETag")).is_ok();
                log::debug!("Server etag support check: {}", has_etag);
                has_etag
            }
            Err(e) => {
                log::debug!("Failed to get response headers: {:?}", e);
                false
            }
        }
    }))
}

pub fn get_juliaserver_base_url() -> Result<Url> {
    let base_url = if let Ok(val) = std::env::var("JULIAUP_SERVER") {
        if val.ends_with('/') {
            val
        } else {
            format!("{}/", val)
        }
    } else {
        "https://julialang-s3.julialang.org".to_string()
    };

    let parsed_url = Url::parse(&base_url).with_context(|| {
        format!(
            "Failed to parse the value of JULIAUP_SERVER '{}' as a uri.",
            base_url
        )
    })?;

    Ok(parsed_url)
}

pub fn get_julianightlies_base_url() -> Result<Url> {
    let base_url = if let Ok(val) = std::env::var("JULIAUP_NIGHTLY_SERVER") {
        if val.ends_with('/') {
            val
        } else {
            format!("{}/", val)
        }
    } else {
        "https://julialangnightlies-s3.julialang.org".to_string()
    };

    let parsed_url = Url::parse(&base_url).with_context(|| {
        format!(
            "Failed to parse the value of JULIAUP_NIGHTLY_SERVER '{}' as a uri.",
            base_url
        )
    })?;

    Ok(parsed_url)
}

pub fn get_bin_dir() -> Result<PathBuf> {
    let entry_sep = if std::env::consts::OS == "windows" {
        ';'
    } else {
        ':'
    };

    let path = match std::env::var("JULIAUP_BIN_DIR") {
        Ok(val) => {
            let path = PathBuf::from(val.split(entry_sep).next().unwrap()); // We can unwrap here because even when we split an empty string we should get a first element.

            if !path.is_absolute() {
                bail!("The `JULIAUP_BIN_DIR` environment variable contains a value that resolves to an an invalid path `{}`.", path.display());
            };

            path
        }
        Err(_) => {
            let mut path = std::env::current_exe()
                .with_context(|| "Could not determine the path of the running exe.")?
                .parent()
                .ok_or_else(|| anyhow!("Could not determine parent."))?
                .to_path_buf();

            if let Some(home_dir) = dirs::home_dir() {
                if !path.starts_with(&home_dir) {
                    path = home_dir.join(".local").join("bin");

                    if !path.is_absolute() {
                        bail!(
                            "The system returned an invalid home directory path `{}`.",
                            path.display()
                        );
                    };
                }
            }

            path
        }
    };

    Ok(path)
}

pub fn is_valid_julia_path(julia_path: &PathBuf) -> bool {
    std::process::Command::new(julia_path)
        .arg("-v")
        .stdout(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

pub fn get_arch() -> Result<String> {
    if std::env::consts::ARCH == "x86" {
        return Ok("x86".to_string());
    } else if std::env::consts::ARCH == "x86_64" {
        return Ok("x64".to_string());
    } else if std::env::consts::ARCH == "aarch64" {
        return Ok("aarch64".to_string());
    }

    bail!("Running on an unknown arch: {}.", std::env::consts::ARCH)
}

pub fn parse_versionstring(value: &String) -> Result<(String, Version)> {
    let version = Version::parse(value).unwrap();

    let build_parts: Vec<&str> = version.build.split('.').collect();

    if build_parts.len() < 4 {
        bail!(
            "`{}` is an invalid version specifier: the build part must have at least four parts.",
            value
        );
    }

    let version_without_build = semver::Version {
        major: version.major,
        minor: version.minor,
        patch: version.patch,
        pre: version.pre,
        build: BuildMetadata::EMPTY,
    };

    let platform = build_parts[1];

    Ok((platform.to_string(), version_without_build))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versionstring() {
        let s = "1.1.1";
        assert!(parse_versionstring(&s.to_owned()).is_err());

        let s = "1.1.1+0.x86.apple.darwin14";
        let (p, v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x86");
        assert_eq!(v, Version::new(1, 1, 1));

        let s = "1.1.1+0.x64.apple.darwin14";
        let (p, v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x64");
        assert_eq!(v, Version::new(1, 1, 1));

        // FreeBSD has 5 parts in the build metadata
        let s = "1.10.10+0.x64.unknown.freebsd11.1";
        let (p, v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x64");
        assert_eq!(v, Version::new(1, 10, 10));
    }
}

// Message formatting constants and functions
// Match the indent of Pkg.jl style messages
const JULIAUP_STYLE_INDENT: usize = 12; // Width of "Precompiling" in Pkg

/// Color options for styled messages
#[derive(Clone, Copy)]
pub enum JuliaupMessageType {
    Success,
    Error,
    Warning,
    Progress,
}

enum JuliaupStyleColor {
    Green,
    Red,
    Yellow,
    Cyan,
}

impl JuliaupMessageType {
    fn color(&self) -> JuliaupStyleColor {
        match self {
            JuliaupMessageType::Success => JuliaupStyleColor::Green,
            JuliaupMessageType::Progress => JuliaupStyleColor::Cyan,
            JuliaupMessageType::Warning => JuliaupStyleColor::Yellow,
            JuliaupMessageType::Error => JuliaupStyleColor::Red,
        }
    }
}

/// Print a styled message with Pkg.jl-like formatting (right-aligned prefix)
/// Format: "     [action] message"
///
/// # Message Types
/// - **Success**: Completion messages (Configure, Link, Remove, Tidyup) - Green
/// - **Progress**: Active/in-progress operations (Updating, Installing, Creating, Deleting, Checking) - Cyan
/// - **Warning**: Non-critical issues that need attention - Yellow
/// - **Error**: Critical failures - Red
///
pub fn print_juliaup_style(action: &str, message: &str, message_type: JuliaupMessageType) {
    let color = message_type.color();
    let styled_action = match color {
        JuliaupStyleColor::Green => {
            style(format!("{:>width$}", action, width = JULIAUP_STYLE_INDENT))
                .green()
                .bold()
        }
        JuliaupStyleColor::Red => {
            style(format!("{:>width$}", action, width = JULIAUP_STYLE_INDENT))
                .red()
                .bold()
        }
        JuliaupStyleColor::Yellow => {
            style(format!("{:>width$}", action, width = JULIAUP_STYLE_INDENT))
                .yellow()
                .bold()
        }
        JuliaupStyleColor::Cyan => {
            style(format!("{:>width$}", action, width = JULIAUP_STYLE_INDENT))
                .cyan()
                .bold()
        }
    };

    eprintln!("{} {}", styled_action, message);
}

/// Retry a rename with Fibonacci backoff to handle transient permission errors
/// from e.g. antivirus scanners. Similar approach to rustup.
pub fn retry_rename(src: &Path, dest: &Path) -> Result<()> {
    // 20 fib steps from 1 millisecond sums to ~18 seconds
    retry(
        Fibonacci::from_millis(1).map(jitter).take(20),
        || match std::fs::rename(src, dest) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    log::debug!("Retrying rename {} to {}.", src.display(), dest.display());
                    OperationResult::Retry(e)
                }
                _ => OperationResult::Err(e),
            },
        },
    )
    .with_context(|| {
        format!(
            "Failed to rename '{}' to '{}'.",
            src.display(),
            dest.display()
        )
    })
}
