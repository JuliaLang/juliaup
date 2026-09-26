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

static CUSTOM_SERVER_WARNING_SHOWN: OnceLock<()> = OnceLock::new();
static CUSTOM_NIGHTLY_SERVER_WARNING_SHOWN: OnceLock<()> = OnceLock::new();
static CUSTOM_PR_SERVER_WARNING_SHOWN: OnceLock<()> = OnceLock::new();

/// Returns `true` if the URL points at a loopback host (localhost / 127.0.0.1 /
/// ::1). Plain HTTP is permitted for these because the traffic never leaves the
/// machine; this is used by the integration tests that spin up a local mock
/// server, and by anyone pointing juliaup at a loopback mirror.
pub(crate) fn is_loopback_http(url: &Url) -> bool {
    url.scheme() == "http"
        && matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
        )
}

pub fn get_juliaserver_base_url() -> Result<Url> {
    let (base_url, is_custom) = if let Ok(val) = std::env::var("JULIAUP_SERVER") {
        let url = if val.ends_with('/') {
            val
        } else {
            format!("{}/", val)
        };
        (url, true)
    } else {
        ("https://julialang-s3.julialang.org".to_string(), false)
    };

    let parsed_url = Url::parse(&base_url).with_context(|| {
        format!(
            "Failed to parse the value of JULIAUP_SERVER '{}' as a uri.",
            base_url
        )
    })?;

    if parsed_url.scheme() != "https" && !is_loopback_http(&parsed_url) {
        bail!("The value of JULIAUP_SERVER '{}' must use HTTPS.", base_url);
    }

    if is_custom {
        CUSTOM_SERVER_WARNING_SHOWN.get_or_init(|| {
            print_juliaup_style(
                "Info",
                &format!("Using custom server '{}' (JULIAUP_SERVER).", parsed_url),
                JuliaupMessageType::Progress,
            );
        });
    }

    Ok(parsed_url)
}

pub fn get_julianightlies_base_url() -> Result<Url> {
    let (base_url, is_custom) = if let Ok(val) = std::env::var("JULIAUP_NIGHTLY_SERVER") {
        let url = if val.ends_with('/') {
            val
        } else {
            format!("{}/", val)
        };
        (url, true)
    } else {
        (
            "https://julialangnightlies-s3.julialang.org".to_string(),
            false,
        )
    };

    let parsed_url = Url::parse(&base_url).with_context(|| {
        format!(
            "Failed to parse the value of JULIAUP_NIGHTLY_SERVER '{}' as a uri.",
            base_url
        )
    })?;

    if parsed_url.scheme() != "https" && !is_loopback_http(&parsed_url) {
        bail!(
            "The value of JULIAUP_NIGHTLY_SERVER '{}' must use HTTPS.",
            base_url
        );
    }

    if is_custom {
        CUSTOM_NIGHTLY_SERVER_WARNING_SHOWN.get_or_init(|| {
            print_juliaup_style(
                "Info",
                &format!(
                    "Using custom nightly server '{}' (JULIAUP_NIGHTLY_SERVER).",
                    parsed_url
                ),
                JuliaupMessageType::Progress,
            );
        });
    }

    Ok(parsed_url)
}

/// Base URL of the bucket that CI stages pull request builds to, keyed by the
/// head commit sha of the PR (see JuliaCI/julia-buildkite#544). The builds
/// stored there are ephemeral and expire roughly 90 days after CI uploads
/// them.
pub fn get_juliaprs_base_url() -> Result<Url> {
    let (base_url, is_custom) = if let Ok(val) = std::env::var("JULIAUP_PR_SERVER") {
        let url = if val.ends_with('/') {
            val
        } else {
            format!("{}/", val)
        };
        (url, true)
    } else {
        (
            "https://julialang-ephemeral-pr.s3.amazonaws.com".to_string(),
            false,
        )
    };

    let parsed_url = Url::parse(&base_url).with_context(|| {
        format!(
            "Failed to parse the value of JULIAUP_PR_SERVER '{}' as a uri.",
            base_url
        )
    })?;

    if parsed_url.scheme() != "https" && !is_loopback_http(&parsed_url) {
        bail!(
            "The value of JULIAUP_PR_SERVER '{}' must use HTTPS.",
            base_url
        );
    }

    if is_custom {
        CUSTOM_PR_SERVER_WARNING_SHOWN.get_or_init(|| {
            print_juliaup_style(
                "Info",
                &format!(
                    "Using custom PR server '{}' (JULIAUP_PR_SERVER).",
                    parsed_url
                ),
                JuliaupMessageType::Progress,
            );
        });
    }

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
    use std::sync::Mutex;

    // Env-var mutation is process-global; serialise tests that set/remove it.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn juliaup_server_rejects_http() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("JULIAUP_SERVER", "http://evil.example.com");
        let result = get_juliaserver_base_url();
        std::env::remove_var("JULIAUP_SERVER");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must use HTTPS"));
    }

    #[test]
    fn juliaup_nightly_server_rejects_http() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("JULIAUP_NIGHTLY_SERVER", "http://evil.example.com");
        let result = get_julianightlies_base_url();
        std::env::remove_var("JULIAUP_NIGHTLY_SERVER");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must use HTTPS"));
    }

    #[test]
    fn juliaup_pr_server_rejects_http() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("JULIAUP_PR_SERVER", "http://evil.example.com");
        let result = get_juliaprs_base_url();
        std::env::remove_var("JULIAUP_PR_SERVER");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must use HTTPS"));
    }

    #[test]
    fn juliaup_server_accepts_https() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("JULIAUP_SERVER", "https://mirror.example.com");
        let result = get_juliaserver_base_url();
        std::env::remove_var("JULIAUP_SERVER");
        assert!(result.is_ok());
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
