use anyhow::{anyhow, bail, Context, Result};
use console::style;
use retry::{
    delay::{jitter, Fibonacci},
    retry, OperationResult,
};
use semver::{BuildMetadata, Version};
use std::path::{Path, PathBuf};
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
                .map(|name| name.ends_with(".app"))
                .unwrap_or(false)
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

    if build_parts.len() != 4 {
        bail!(
            "`{}` is an invalid version specifier: the build part must have four parts.",
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
