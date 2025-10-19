use crate::config_file::{load_mut_config_db, save_config_db, JuliaupConfigChannel};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{
    channel_to_name, install_non_db_version, install_version, update_version_db,
};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, Context, Result};
use regex::Regex;

pub fn run_command_add(channel: &str, paths: &GlobalPaths) -> Result<()> {
    // This regex is dynamically compiled, but its runtime is negligible compared to downloading Julia
    if Regex::new(r"^(?:pr\d+|nightly|\d+\.\d+-nightly)(?:~|$)")
        .unwrap()
        .is_match(channel)
    {
        return add_non_db(channel, paths);
    }

    update_version_db(&Some(channel.to_string()), paths)
        .with_context(|| "Failed to update versions db.")?;
    let version_db =
        load_versions_db(paths).with_context(|| "`add` command failed to load versions db.")?;

    let required_version = &version_db
        .available_channels
        .get(channel)
        .ok_or_else(|| {
            anyhow!(
                "'{}' is not a valid Julia version or channel name.",
                &channel
            )
        })?
        .version;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        eprintln!("'{}' is already installed.", &channel);
        return Ok(());
    }

    install_version(required_version, &mut config_file.data, &version_db, paths)?;

    config_file.data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::SystemChannel {
            version: required_version.clone(),
        },
    );

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{}' was installed.",
            channel
        )
    })?;

    #[cfg(not(windows))]
    if create_symlinks {
        create_symlink(
            &JuliaupConfigChannel::SystemChannel {
                version: required_version.clone(),
            },
            &format!("julia-{}", channel),
            paths,
        )?;
    }

    Ok(())
}

fn add_non_db(channel: &str, paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        eprintln!("'{}' is already installed.", &channel);
        return Ok(());
    }

    // Warn about security implications of PR builds
    if let Some(caps) = Regex::new(r"^pr(\d+)").unwrap().captures(channel) {
        let pr_number = &caps[1];
        eprintln!(
            "\nWARNING: Note that unmerged PRs may not have been reviewed for security issues etc."
        );
        eprintln!(
            "Review code at https://github.com/JuliaLang/julia/pull/{}\n",
            pr_number
        );
    }

    let name = channel_to_name(channel)?;
    let config_channel = install_non_db_version(channel, &name, paths)?;

    config_file
        .data
        .installed_channels
        .insert(channel.to_string(), config_channel.clone());

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    save_config_db(&mut config_file).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{channel}' was installed.",
        )
    })?;

    #[cfg(not(windows))]
    if config_file.data.settings.create_channel_symlinks {
        create_symlink(&config_channel, &format!("julia-{}", channel), paths)?;
    }

    // Handle codesigning for PR builds on macOS
    #[cfg(target_os = "macos")]
    if Regex::new(r"^pr\d+").unwrap().is_match(channel) {
        if let Err(e) = codesign_pr_build_if_needed(channel, paths) {
            eprintln!("\nWarning: Codesigning failed: {}", e);
            eprintln!("The Julia binary may not run without manual codesigning.");
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn codesign_pr_build_if_needed(channel: &str, paths: &GlobalPaths) -> Result<()> {
    use std::io::{self, Write};

    eprintln!("\nWARNING: PR builds are not code-signed for macOS.");
    eprintln!("The Julia binary will fail to run unless you codesign it locally.");
    eprint!("\nWould you like to automatically codesign this PR build now? [Y/n]: ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "n" || input == "no" {
        let dir_name = format!("julia-{}", channel);
        eprintln!("\nSkipping codesigning. You can manually codesign later with:");
        eprintln!("  codesign --force --sign - {}/{}/bin/julia", paths.juliauphome.display(), dir_name);
        eprintln!("  codesign --force --sign - {}/{}/lib/libjulia.*.dylib", paths.juliauphome.display(), dir_name);
        return Ok(());
    }

    let julia_dir = paths.juliauphome.join(format!("julia-{}", channel));
    let julia_bin = julia_dir.join("bin").join("julia");

    eprintln!("\nCodesigning Julia binary...");
    codesign_file(&julia_bin)?;

    // Find and codesign libjulia dylib
    eprintln!("Codesigning Julia library...");
    let lib_dir = julia_dir.join("lib");
    if lib_dir.exists() {
        for entry in std::fs::read_dir(&lib_dir)? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
                if name.starts_with("libjulia.") && name.ends_with(".dylib") {
                    codesign_file(&path)?;
                }
            }
        }
    }

    eprintln!("✓ Codesigning completed successfully.");
    Ok(())
}

#[cfg(target_os = "macos")]
fn codesign_file(path: &std::path::Path) -> Result<()> {
    use std::process::Command;

    let status = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(path)
        .status()
        .with_context(|| format!("Failed to execute codesign on {}", path.display()))?;

    if !status.success() {
        return Err(anyhow!("Failed to codesign {}", path.display()));
    }
    Ok(())
}
