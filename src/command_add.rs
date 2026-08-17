use crate::config_file::{
    load_config_db, load_mut_config_db, save_config_db, JuliaupConfig, JuliaupConfigChannel,
};
use crate::global_paths::GlobalPaths;
#[cfg(not(windows))]
use crate::operations::create_symlink;
use crate::operations::{
    channel_to_name, commit_version_install, download_version_to_temp, install_non_db_version,
    update_version_db,
};
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use crate::versions_file::load_versions_db;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use tempfile::TempDir;

#[derive(Debug, PartialEq)]
enum AddChannelOutcome {
    Installed,
    AlreadyInstalled,
}

/// Commits a downloaded database version after re-checking the channel under
/// the exclusive configuration lock.
///
/// A concurrent system-channel install only wins when it selected the same
/// version. If it selected a different version, keep the completed download
/// and move the channel to the version this `add` resolved before downloading.
/// Non-system channels are explicit name claims and are never overwritten.
fn commit_downloaded_channel(
    channel: &str,
    required_version: &str,
    downloaded: TempDir,
    config_data: &mut JuliaupConfig,
    paths: &GlobalPaths,
) -> Result<AddChannelOutcome> {
    match config_data.installed_channels.get(channel) {
        Some(JuliaupConfigChannel::SystemChannel { version }) if version != required_version => {}
        Some(_) => return Ok(AddChannelOutcome::AlreadyInstalled),
        None => {}
    }

    commit_version_install(downloaded, required_version, config_data, paths)?;

    config_data.installed_channels.insert(
        channel.to_string(),
        JuliaupConfigChannel::SystemChannel {
            version: required_version.to_string(),
        },
    );

    Ok(AddChannelOutcome::Installed)
}

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
                channel
            )
        })?
        .version;

    // Check whether the channel is already installed before downloading. This
    // read only briefly takes a shared lock, which is released immediately.
    {
        let config_file = load_config_db(paths, None)
            .with_context(|| "`add` command failed to load configuration data.")?;

        if config_file.data.installed_channels.contains_key(channel) {
            eprintln!("'{}' is already installed.", channel);
            return Ok(());
        }
    }

    // Download and extract the version without holding the configuration lock,
    // so concurrent juliaup processes (and the launcher) are not blocked.
    let downloaded = download_version_to_temp(required_version, &version_db, paths)?;

    // Re-acquire the exclusive lock to commit the installation.
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if commit_downloaded_channel(
        channel,
        required_version,
        downloaded,
        &mut config_file.data,
        paths,
    )? == AddChannelOutcome::AlreadyInstalled
    {
        eprintln!("'{}' is already installed.", channel);
        return Ok(());
    }

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    #[cfg(not(windows))]
    let create_symlinks = config_file.data.settings.create_channel_symlinks;

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{}' was installed at `{}`.",
            channel,
            paths.juliaupconfig.display()
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

    print_juliaup_style(
        "Add",
        &format!("Installed Julia channel '{}'", channel),
        JuliaupMessageType::Success,
    );

    Ok(())
}

fn add_non_db(channel: &str, paths: &GlobalPaths) -> Result<()> {
    // Check whether the channel is already installed before downloading. This
    // read only briefly takes a shared lock, which is released immediately.
    {
        let config_file = load_config_db(paths, None)
            .with_context(|| "`add` command failed to load configuration data.")?;

        if config_file.data.installed_channels.contains_key(channel) {
            eprintln!("'{}' is already installed.", channel);
            return Ok(());
        }
    }

    // Warn about security implications of PR builds
    if let Some(caps) = Regex::new(r"^pr(\d+)").unwrap().captures(channel) {
        let pr_number = &caps[1];
        eprintln!(
            "\nWARNING: Note that unmerged PRs may not have been reviewed for security issues etc."
        );
        eprintln!(
            "         Review code at https://github.com/JuliaLang/julia/pull/{}\n",
            pr_number
        );
    }

    // Download and extract the version without holding the configuration lock.
    let name = channel_to_name(channel)?;
    let (config_channel, _used_dmg) = install_non_db_version(channel, &name, paths)?;

    // Re-acquire the exclusive lock to commit the installation.
    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`add` command failed to load configuration data.")?;

    if config_file.data.installed_channels.contains_key(channel) {
        eprintln!("'{}' is already installed.", channel);
        return Ok(());
    }

    config_file
        .data
        .installed_channels
        .insert(channel.to_string(), config_channel.clone());

    if config_file.data.default.is_none() {
        config_file.data.default = Some(channel.to_string());
    }

    save_config_db(&mut config_file, paths).with_context(|| {
        format!(
            "Failed to save configuration file from `add` command after '{channel}' was installed at `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    #[cfg(not(windows))]
    if config_file.data.settings.create_channel_symlinks {
        create_symlink(&config_channel, &format!("julia-{}", channel), paths)?;
    }

    print_juliaup_style(
        "Add",
        &format!("Installed Julia channel '{}'", channel),
        JuliaupMessageType::Success,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_file::JuliaupConfigVersion;
    use std::path::Path;
    use tempfile::Builder;

    fn test_paths(dir: &Path) -> GlobalPaths {
        GlobalPaths {
            juliauphome: dir.to_path_buf(),
            juliaupconfig: dir.join("juliaup.json"),
            lockfile: dir.join(".juliaup-lock"),
            versiondb: dir.join("versiondb-test.json"),
            #[cfg(feature = "selfupdate")]
            juliaupselfhome: dir.to_path_buf(),
            #[cfg(feature = "selfupdate")]
            juliaupselfconfig: dir.join("juliaupself.json"),
            #[cfg(feature = "selfupdate")]
            juliaupselfbin: dir.to_path_buf(),
        }
    }

    fn downloaded_install(dir: &Path, marker: &str) -> Result<TempDir> {
        let downloaded = Builder::new().prefix("julia-temp-").tempdir_in(dir)?;
        std::fs::create_dir_all(downloaded.path().join("bin"))?;
        std::fs::write(downloaded.path().join("bin/julia"), marker)?;
        Ok(downloaded)
    }

    fn installed_version(path: &str) -> JuliaupConfigVersion {
        JuliaupConfigVersion {
            path: path.to_string(),
            binary_path: None,
        }
    }

    #[test]
    fn concurrent_different_system_version_commits_download() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let paths = test_paths(dir.path());
        let downloaded = downloaded_install(dir.path(), "downloaded")?;
        let mut config = JuliaupConfig::default();

        config.installed_versions.insert(
            "1.10.11+0.test".to_string(),
            installed_version("./julia-1.10.11+0.test"),
        );
        config.installed_channels.insert(
            "1.10".to_string(),
            JuliaupConfigChannel::SystemChannel {
                version: "1.10.11+0.test".to_string(),
            },
        );

        let outcome =
            commit_downloaded_channel("1.10", "1.10.12+0.test", downloaded, &mut config, &paths)?;

        assert_eq!(outcome, AddChannelOutcome::Installed);
        assert!(config.installed_versions.contains_key("1.10.12+0.test"));
        assert!(matches!(
            config.installed_channels.get("1.10"),
            Some(JuliaupConfigChannel::SystemChannel { version })
                if version == "1.10.12+0.test"
        ));
        assert_eq!(
            std::fs::read_to_string(paths.juliauphome.join("julia-1.10.12+0.test/bin/julia"))?,
            "downloaded"
        );
        Ok(())
    }

    #[test]
    fn concurrent_same_system_version_discards_download() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let paths = test_paths(dir.path());
        let downloaded = downloaded_install(dir.path(), "duplicate")?;
        let downloaded_path = downloaded.path().to_path_buf();
        let target = paths.juliauphome.join("julia-1.10.12+0.test/bin");
        std::fs::create_dir_all(&target)?;
        std::fs::write(target.join("julia"), "existing")?;

        let mut config = JuliaupConfig::default();
        config.installed_versions.insert(
            "1.10.12+0.test".to_string(),
            installed_version("./julia-1.10.12+0.test"),
        );
        config.installed_channels.insert(
            "1.10".to_string(),
            JuliaupConfigChannel::SystemChannel {
                version: "1.10.12+0.test".to_string(),
            },
        );

        let outcome =
            commit_downloaded_channel("1.10", "1.10.12+0.test", downloaded, &mut config, &paths)?;

        assert_eq!(outcome, AddChannelOutcome::AlreadyInstalled);
        assert!(!downloaded_path.exists());
        assert_eq!(std::fs::read_to_string(target.join("julia"))?, "existing");
        Ok(())
    }

    fn assert_concurrent_explicit_channel_is_preserved(
        explicit_channel: JuliaupConfigChannel,
    ) -> Result<()> {
        let dir = tempfile::tempdir()?;
        let paths = test_paths(dir.path());
        let mut config = JuliaupConfig::default();
        config
            .installed_channels
            .insert("1.10".to_string(), explicit_channel.clone());
        let downloaded = downloaded_install(dir.path(), "downloaded")?;
        let downloaded_path = downloaded.path().to_path_buf();

        let outcome =
            commit_downloaded_channel("1.10", "1.10.12+0.test", downloaded, &mut config, &paths)?;

        assert_eq!(outcome, AddChannelOutcome::AlreadyInstalled);
        assert!(!downloaded_path.exists());
        assert!(config.installed_channels.get("1.10") == Some(&explicit_channel));
        assert!(!config.installed_versions.contains_key("1.10.12+0.test"));
        Ok(())
    }

    #[test]
    fn concurrent_explicit_channels_preserve_name_claim() -> Result<()> {
        assert_concurrent_explicit_channel_is_preserved(JuliaupConfigChannel::LinkedChannel {
            command: "/custom/julia".to_string(),
            args: None,
        })?;
        assert_concurrent_explicit_channel_is_preserved(
            JuliaupConfigChannel::DirectDownloadChannel {
                path: "./julia-1.10".to_string(),
                url: "https://example.com/julia.tar.gz".to_string(),
                local_etag: "etag".to_string(),
                server_etag: "etag".to_string(),
                version: "1.10.99-DEV".to_string(),
                binary_path: None,
            },
        )
    }
}
