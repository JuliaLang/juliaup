use anyhow::{anyhow, bail, Context, Result};
use console::style;
use semver::{BuildMetadata, Version};
use std::path::PathBuf;
use url::Url;

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

/// Returns the list of Julia environment variables that can be persisted
/// Excludes JULIA_PROJECT as noted in the Julia documentation
pub fn get_julia_environment_variables() -> Vec<&'static str> {
    vec![
        // File Locations
        "JULIA_BINDIR",
        "JULIA_LOAD_PATH",
        "JULIA_DEPOT_PATH",
        "JULIA_HISTORY",
        "JULIA_MAX_NUM_PRECOMPILE_FILES",
        "JULIA_VERBOSE_LINKING",
        // Pkg.jl
        "JULIA_CI",
        "JULIA_NUM_PRECOMPILE_TASKS",
        "JULIA_PKG_DEVDIR",
        "JULIA_PKG_IGNORE_HASHES",
        "JULIA_PKG_OFFLINE",
        "JULIA_PKG_PRECOMPILE_AUTO",
        "JULIA_PKG_SERVER",
        "JULIA_PKG_SERVER_REGISTRY_PREFERENCE",
        "JULIA_PKG_UNPACK_REGISTRY",
        "JULIA_PKG_USE_CLI_GIT",
        "JULIA_PKGRESOLVE_ACCURACY",
        "JULIA_PKG_PRESERVE_TIERED_INSTALLED",
        "JULIA_PKG_GC_AUTO",
        // Network Transport
        "JULIA_NO_VERIFY_HOSTS",
        "JULIA_SSL_NO_VERIFY_HOSTS",
        "JULIA_SSH_NO_VERIFY_HOSTS",
        "JULIA_ALWAYS_VERIFY_HOSTS",
        "JULIA_SSL_CA_ROOTS_PATH",
        // External Applications
        "JULIA_SHELL",
        "JULIA_EDITOR",
        // Parallelization
        "JULIA_CPU_THREADS",
        "JULIA_WORKER_TIMEOUT",
        "JULIA_NUM_THREADS",
        "JULIA_THREAD_SLEEP_THRESHOLD",
        "JULIA_NUM_GC_THREADS",
        "JULIA_IMAGE_THREADS",
        "JULIA_IMAGE_TIMINGS",
        "JULIA_EXCLUSIVE",
        // Garbage Collection
        "JULIA_HEAP_SIZE_HINT",
        // REPL Formatting
        "JULIA_ERROR_COLOR",
        "JULIA_WARN_COLOR",
        "JULIA_INFO_COLOR",
        "JULIA_INPUT_COLOR",
        "JULIA_ANSWER_COLOR",
        "NO_COLOR",
        "FORCE_COLOR",
        // System and Package Image Building
        "JULIA_CPU_TARGET",
        // Debugging and Profiling
        "JULIA_DEBUG",
        "JULIA_PROFILE_PEEK_HEAP_SNAPSHOT",
        "JULIA_TIMING_SUBSYSTEMS",
        "JULIA_GC_NO_GENERATIONAL",
        "JULIA_GC_WAIT_FOR_DEBUGGER",
        "ENABLE_JITPROFILING",
        "ENABLE_GDBLISTENER",
        "JULIA_LLVM_ARGS",
        "JULIA_FALLBACK_REPL",
    ]
}
