use anyhow::{anyhow, bail, Context, Result};
use semver::{BuildMetadata, Version};
use std::path::PathBuf;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    self, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};
use url::Url;

/// Returns the project directories.
/// Uses the `ProjectDirs` struct from the `directories` crate to define a directory
/// structure for the project with "org" as the organization, "julialang" as the application,
/// and "install" as the qualifier. This is a standard approach for determining data storage paths.
fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("org", "julialang", "install")
}

/// Returns the default data directory for the application.
/// First, it checks the environment variable `JULIAUP_DATA_HOME`.
/// If the environment variable is not set, it falls back to the data directory
/// provided by `project_dirs`, which is typically platform-specific.
/// If both fail, it defaults to a `.data` directory in the current working directory.
pub fn default_data_dir() -> PathBuf {
    std::env::var("JULIAUP_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            project_dirs()
                .map(|dirs| dirs.data_local_dir().to_path_buf())
                .ok_or(())
        })
        .unwrap_or(PathBuf::from(".").join(".data"))
}

/// Initializes logging for the application.
/// Creates the default data directory if it doesn't already exist.
/// A log file named after the package is created in this directory.
/// The `tracing_subscriber` library is then used to configure logging to the file,
/// including file and line numbers in log entries, while disabling ANSI coloring.
/// The log level for specific libraries (`tokio_util`, `hyper`, `reqwest`) is turned off.
pub fn init_logging() -> Result<()> {
    // Get the default data directory
    let directory = default_data_dir();

    // Create the directory (and any missing parent directories) if it doesn't exist
    std::fs::create_dir_all(directory.clone())?;

    // Create a log file named after the package
    let log_file = format!("{}.log", env!("CARGO_PKG_NAME"));
    let log_path = directory.join(log_file);
    let log_file = std::fs::File::create(log_path)?;

    // Set up a logging subscriber that writes to the log file, with specific configurations
    let file_subscriber = tracing_subscriber::fmt::layer()
        .with_file(true) // Include source file name in logs
        .with_line_number(true) // Include line number in logs
        .with_writer(log_file) // Log to the created file
        .with_target(false) // Disable logging target (e.g., module paths)
        .with_ansi(false); // Disable ANSI color codes

    // Initialize the tracing subscriber with filters for specific libraries
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default()) // Add error handling layer
        .with(
            tracing_subscriber::filter::EnvFilter::from_default_env()
                .add_directive("tokio_util=off".parse().unwrap()) // Disable logging for `tokio_util`
                .add_directive("hyper=off".parse().unwrap()) // Disable logging for `hyper`
                .add_directive("reqwest=off".parse().unwrap()), // Disable logging for `reqwest`
        )
        .init();

    Ok(())
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
    return std::process::Command::new(julia_path)
        .arg("-v")
        .stdout(std::process::Stdio::null())
        .spawn()
        .is_ok();
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
