use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use cluFlock::{ExclusiveFlock, FlockLock, SharedFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom, Write};
#[cfg(target_os = "windows")]
use std::mem;
use tempfile::NamedTempFile;

use crate::global_paths::GlobalPaths;

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

fn default_versionsdb_update_interval() -> i64 {
    1440
}

fn is_default_versionsdb_update_interval(i: &i64) -> bool {
    *i == default_versionsdb_update_interval()
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfigVersion {
    #[serde(rename = "Path")]
    pub path: String,
    /// Relative path to the Julia binary (e.g., for .app bundles on macOS).
    /// If None, the binary path is computed at runtime for backward compatibility.
    #[serde(rename = "BinaryPath", skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum JuliaupConfigChannel {
    DirectDownloadChannel {
        #[serde(rename = "Path")]
        path: String,
        #[serde(rename = "Url")]
        url: String,
        #[serde(rename = "LocalETag")]
        local_etag: String,
        #[serde(rename = "ServerETag")]
        server_etag: String,
        #[serde(rename = "Version")]
        version: String,
        /// Relative path to the Julia binary (e.g., for .app bundles on macOS).
        /// If None, the binary path is computed at runtime for backward compatibility.
        #[serde(rename = "BinaryPath", skip_serializing_if = "Option::is_none")]
        binary_path: Option<String>,
    },
    SystemChannel {
        #[serde(rename = "Version")]
        version: String,
    },
    LinkedChannel {
        #[serde(rename = "Command")]
        command: String,
        #[serde(rename = "Args")]
        args: Option<Vec<String>>,
    },
    AliasChannel {
        #[serde(rename = "Target")]
        target: String,
        #[serde(rename = "Args")]
        args: Option<Vec<String>>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupConfigSettings {
    #[serde(
        rename = "CreateChannelSymlinks",
        default,
        skip_serializing_if = "is_default"
    )]
    pub create_channel_symlinks: bool,
    #[serde(
        rename = "VersionsDbUpdateInterval",
        default = "default_versionsdb_update_interval",
        skip_serializing_if = "is_default_versionsdb_update_interval"
    )]
    pub versionsdb_update_interval: i64,
    #[serde(
        rename = "AutoInstallChannels",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_install_channels: Option<bool>,
    #[serde(
        rename = "ManifestVersionDetect",
        default,
        skip_serializing_if = "is_default"
    )]
    pub manifest_version_detect: bool,
}

impl Default for JuliaupConfigSettings {
    fn default() -> Self {
        JuliaupConfigSettings {
            create_channel_symlinks: false,
            versionsdb_update_interval: default_versionsdb_update_interval(),
            auto_install_channels: None,
            manifest_version_detect: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct JuliaupOverride {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Channel")]
    pub channel: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct JuliaupConfig {
    #[serde(rename = "Default")]
    pub default: Option<String>,
    #[serde(rename = "InstalledVersions")]
    pub installed_versions: HashMap<String, JuliaupConfigVersion>,
    #[serde(rename = "InstalledChannels")]
    pub installed_channels: HashMap<String, JuliaupConfigChannel>,
    #[serde(rename = "Settings", default)]
    pub settings: JuliaupConfigSettings,
    #[serde(rename = "Overrides", default)]
    pub overrides: Vec<JuliaupOverride>,
    #[serde(
        rename = "LastVersionDbUpdate",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_version_db_update: Option<DateTime<Utc>>,
}

#[cfg(feature = "selfupdate")]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct JuliaupSelfConfig {
    #[serde(
        rename = "BackgroundSelfUpdateInterval",
        skip_serializing_if = "Option::is_none"
    )]
    pub background_selfupdate_interval: Option<i64>,
    #[serde(
        rename = "StartupSelfUpdateInterval",
        skip_serializing_if = "Option::is_none"
    )]
    pub startup_selfupdate_interval: Option<i64>,
    #[serde(rename = "ModifyPath", default, skip_serializing_if = "is_default")]
    pub modify_path: bool,
    #[serde(rename = "JuliaupChannel", skip_serializing_if = "Option::is_none")]
    pub juliaup_channel: Option<String>,
    #[serde(rename = "LastSelfUpdate", skip_serializing_if = "Option::is_none")]
    pub last_selfupdate: Option<DateTime<Utc>>,
}

pub struct JuliaupConfigFile {
    pub file: File,
    pub lock: FlockLock<File>,
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_file: File,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}

pub struct JuliaupReadonlyConfigFile {
    pub data: JuliaupConfig,
    #[cfg(feature = "selfupdate")]
    pub self_data: JuliaupSelfConfig,
}

/// Acquires a file lock, only printing the "locked by another process" message
/// if the lock cannot be obtained within a short grace period. This avoids
/// spurious messages for the common case where another process holds the lock
/// for just a few milliseconds (e.g. while committing a config change).
///
/// `try_lock` must return the original file (via the lock error) when the lock
/// is currently held, so it can be retried; `wait_lock` performs a blocking
/// acquisition.
fn lock_with_delayed_message<TryFn, WaitFn>(
    file: File,
    try_lock: TryFn,
    wait_lock: WaitFn,
) -> Result<FlockLock<File>>
where
    TryFn: Fn(File) -> std::result::Result<FlockLock<File>, File>,
    WaitFn: FnOnce(File) -> Result<FlockLock<File>>,
{
    use std::time::{Duration, Instant};

    const GRACE_PERIOD: Duration = Duration::from_secs(1);
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    let mut file = file;
    let start = Instant::now();

    loop {
        match try_lock(file) {
            Ok(lock) => return Ok(lock),
            Err(unlocked_file) => {
                file = unlocked_file;
                if start.elapsed() >= GRACE_PERIOD {
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
        }
    }

    eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

    wait_lock(file)
}

pub fn get_read_lock(paths: &GlobalPaths) -> Result<FlockLock<File>> {
    std::fs::create_dir_all(&paths.juliauphome).with_context(|| {
        format!(
            "Could not create juliaup home folder `{}`.",
            paths.juliauphome.display()
        )
    })?;

    let lock_file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lockfile)
    {
        Ok(file) => file,
        Err(e) => {
            return Err(anyhow!(
                "Could not create lockfile `{}`: {}.",
                paths.lockfile.display(),
                e
            ));
        }
    };

    let file_lock = lock_with_delayed_message(
        lock_file,
        |f| SharedFlock::try_lock(f).map_err(|e| e.into()),
        |f| {
            SharedFlock::wait_lock(f)
                .map_err(|e| anyhow!("Failed to acquire shared configuration lock: {}.", e))
        },
    )?;

    Ok(file_lock)
}

/// Reads the configuration from disk without any locking.
fn read_config_db(paths: &GlobalPaths) -> Result<JuliaupReadonlyConfigFile> {
    let v = match std::fs::OpenOptions::new()
        .read(true)
        .open(&paths.juliaupconfig)
    {
        Ok(file) => {
            // A zero-length config file can only be left behind by an
            // interrupted initial setup of an older juliaup version; treat it
            // like a missing file rather than failing to parse it.
            if file.metadata().map(|m| m.len() == 0).unwrap_or(false) {
                JuliaupConfig::default()
            } else {
                let reader = BufReader::new(&file);

                serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Failed to parse configuration file '{:?}' for reading.",
                        paths.juliaupconfig
                    )
                })?
            }
        }
        Err(error) => match error.kind() {
            ErrorKind::NotFound => JuliaupConfig::default(),
            other_error => {
                bail!(
                    "Problem opening the file {:?}: {:?}",
                    paths.juliaupconfig,
                    other_error
                )
            }
        },
    };

    #[cfg(feature = "selfupdate")]
    let selfconfig: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        selfconfig = match std::fs::OpenOptions::new()
            .read(true)
            .open(&paths.juliaupselfconfig)
        {
            Ok(file) => {
                let reader = BufReader::new(&file);

                serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Failed to parse self configuration file '{:?}' for reading.",
                        paths.juliaupselfconfig
                    )
                })?
            }
            Err(error) => match error.kind() {
                ErrorKind::NotFound => JuliaupSelfConfig::default(),
                other_error => bail!(
                    "Could not open self configuration file {:?}: {:?}",
                    paths.juliaupselfconfig,
                    other_error
                ),
            },
        };
    }

    Ok(JuliaupReadonlyConfigFile {
        data: v,
        #[cfg(feature = "selfupdate")]
        self_data: selfconfig,
    })
}

pub fn load_config_db(
    paths: &GlobalPaths,
    existing_lock: Option<&FlockLock<File>>,
) -> Result<JuliaupReadonlyConfigFile> {
    let mut file_lock: Option<FlockLock<File>> = None;

    if existing_lock.is_none() {
        file_lock = Some(get_read_lock(paths)?);
    }

    let result = read_config_db(paths)?;

    if let Some(file_lock) = file_lock {
        file_lock.unlock().with_context(|| {
            format!(
                "Failed to unlock configuration lock file `{}`.",
                paths.lockfile.display()
            )
        })?;
    }

    Ok(result)
}

/// Loads the configuration without acquiring the configuration lock.
///
/// This is safe for readers because every writer replaces `juliaup.json`
/// atomically (temp file + rename, see [`save_config_db`] and
/// [`create_initial_config_file`]), so a reader always observes either the
/// old or the new contents in full, never a partial write. `julialauncher`
/// uses this so that launching Julia can never block on the configuration
/// lock.
pub fn load_config_db_lockfree(paths: &GlobalPaths) -> Result<JuliaupReadonlyConfigFile> {
    read_config_db(paths)
}

/// Atomically replaces `dest` with `temp_file`.
///
/// Do not use `tempfile`'s `persist()` for the config file: on Windows it
/// renames via `MoveFileExW`, which fails while any process has `dest` open —
/// including a lock-free reader that is merely reading the config at that
/// moment. `std::fs::rename` instead renames with
/// `FILE_RENAME_FLAG_POSIX_SEMANTICS` (falling back to `MoveFileExW` only on
/// filesystems that don't support it), so the replacement succeeds while
/// readers still hold the old file open. On Unix both are plain `rename(2)`.
fn persist_atomically(temp_file: NamedTempFile, dest: &std::path::Path) -> Result<()> {
    let temp_path = temp_file.into_temp_path().keep()?;

    if let Err(e) = std::fs::rename(&temp_path, dest) {
        // Clean up the now-orphaned temp file before reporting the error.
        let _ = std::fs::remove_file(&temp_path);
        return Err(e.into());
    }

    Ok(())
}

/// Atomically creates `juliaup.json` with default contents and returns an
/// open read/write handle to it. Writing via a temp file + rename means a
/// lock-free reader never observes an empty or partially written config file.
/// Callers must hold the exclusive configuration lock.
fn create_initial_config_file(paths: &GlobalPaths) -> Result<File> {
    let new_config = JuliaupConfig::default();

    let mut temp_file = NamedTempFile::new_in(&paths.juliauphome).with_context(|| {
        format!(
            "Failed to create temporary config file in directory `{}`.",
            paths.juliauphome.display()
        )
    })?;

    serde_json::to_writer_pretty(&mut temp_file, &new_config).with_context(|| {
        format!(
            "Failed to write initial configuration data for `{}` to temporary file.",
            paths.juliaupconfig.display()
        )
    })?;

    temp_file.flush().with_context(|| {
        format!(
            "Failed to flush initial configuration data for `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    temp_file.as_file().sync_all().with_context(|| {
        format!(
            "Failed to sync initial configuration data for `{}` to disk.",
            paths.juliaupconfig.display()
        )
    })?;

    persist_atomically(temp_file, &paths.juliaupconfig).with_context(|| {
        format!(
            "Failed to persist initial configuration file `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&paths.juliaupconfig)
        .with_context(|| {
            format!(
                "Failed to open juliaup config file `{}` after initial creation.",
                paths.juliaupconfig.display()
            )
        })?;

    Ok(file)
}

pub fn load_mut_config_db(paths: &GlobalPaths) -> Result<JuliaupConfigFile> {
    std::fs::create_dir_all(&paths.juliauphome).with_context(|| {
        format!(
            "Could not create juliaup home folder `{}`.",
            paths.juliauphome.display()
        )
    })?;

    let lock_file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lockfile)
    {
        Ok(file) => file,
        Err(e) => {
            return Err(anyhow!(
                "Could not create lockfile `{}`: {}.",
                paths.lockfile.display(),
                e
            ));
        }
    };

    let file_lock = lock_with_delayed_message(
        lock_file,
        |f| ExclusiveFlock::try_lock(f).map_err(|e| e.into()),
        |f| {
            ExclusiveFlock::wait_lock(f)
                .map_err(|e| anyhow!("Failed to acquire exclusive configuration lock: {}.", e))
        },
    )?;

    // Open without `create` so that an empty config file is never visible to
    // lock-free readers; if the file is missing it is created atomically with
    // its initial contents by `create_initial_config_file`.
    let open_result = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&paths.juliaupconfig);

    let (file, data) = match open_result {
        Ok(mut file) => {
            let stream_len = file.seek(SeekFrom::End(0)).with_context(|| {
                format!(
                    "Failed to determine the length of configuration file `{}`.",
                    paths.juliaupconfig.display()
                )
            })?;

            if stream_len == 0 {
                // An empty config file can only be left behind by an
                // interrupted initial setup of an older juliaup version;
                // replace it with a valid initial config.
                drop(file);
                let file = create_initial_config_file(paths)?;
                (file, JuliaupConfig::default())
            } else {
                file.rewind().with_context(|| {
                    format!(
                        "Failed to rewind existing config file `{}`.",
                        paths.juliaupconfig.display()
                    )
                })?;

                let reader = BufReader::new(&file);

                let data = serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Failed to parse configuration file `{}`.",
                        paths.juliaupconfig.display()
                    )
                })?;

                (file, data)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let file = create_initial_config_file(paths)?;
            (file, JuliaupConfig::default())
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to open juliaup config file `{}`.",
                    paths.juliaupconfig.display()
                )
            });
        }
    };

    #[cfg(feature = "selfupdate")]
    let self_file: File;
    #[cfg(feature = "selfupdate")]
    let self_data: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        self_file = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.juliaupselfconfig)
        {
            Ok(file) => file,
            Err(error) => match error.kind() {
                ErrorKind::NotFound => {
                    let default_config = JuliaupSelfConfig::default();
                    if let Some(parent) = paths.juliaupselfconfig.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!(
                                "Failed to create parent directory for self configuration file `{}`.",
                                paths.juliaupselfconfig.display()
                            )
                        })?;
                    }
                    let mut new_file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&paths.juliaupselfconfig)
                        .with_context(|| {
                            format!(
                                "Failed to create self configuration file `{}`.",
                                paths.juliaupselfconfig.display()
                            )
                        })?;
                    serde_json::to_writer_pretty(&mut new_file, &default_config).with_context(|| {
                        format!(
                            "Failed to write default self configuration to `{}`.",
                            paths.juliaupselfconfig.display()
                        )
                    })?;
                    new_file.rewind().with_context(|| {
                        format!(
                            "Failed to rewind self configuration file `{}` after writing defaults.",
                            paths.juliaupselfconfig.display()
                        )
                    })?;
                    new_file
                }
                other_error => bail!(
                    "Failed to open juliaup self config file `{}`: {:?}",
                    paths.juliaupselfconfig.display(),
                    other_error
                ),
            },
        };

        let reader = BufReader::new(&self_file);

        self_data = serde_json::from_reader(reader).with_context(|| {
            format!(
                "Failed to parse self configuration file '{:?}' for reading.",
                paths.juliaupselfconfig
            )
        })?
    }

    let result = JuliaupConfigFile {
        file,
        lock: file_lock,
        data,
        #[cfg(feature = "selfupdate")]
        self_file,
        #[cfg(feature = "selfupdate")]
        self_data,
    };

    Ok(result)
}

pub fn save_config_db(
    juliaup_config_file: &mut JuliaupConfigFile,
    paths: &crate::global_paths::GlobalPaths,
) -> Result<()> {
    // Write to a temporary file in the same directory as the target file
    // This ensures we're on the same filesystem for atomic rename
    let config_dir = paths.juliaupconfig.parent().ok_or_else(|| {
        anyhow!(
            "Config file path `{}` has no parent directory.",
            paths.juliaupconfig.display()
        )
    })?;

    let mut temp_file = NamedTempFile::new_in(config_dir).with_context(|| {
        format!(
            "Failed to create temporary config file in directory `{}`.",
            config_dir.display()
        )
    })?;

    // Write the configuration data to the temporary file
    serde_json::to_writer_pretty(&mut temp_file, &juliaup_config_file.data).with_context(|| {
        format!(
            "Failed to write configuration data for `{}` to temporary file.",
            paths.juliaupconfig.display()
        )
    })?;

    // Ensure all data is written to disk before persisting
    temp_file.flush().with_context(|| {
        format!(
            "Failed to flush temporary configuration data for `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    temp_file.as_file().sync_all().with_context(|| {
        format!(
            "Failed to sync temporary configuration data for `{}` to disk.",
            paths.juliaupconfig.display()
        )
    })?;

    // On Windows, we must close the file before replacing it
    // On Unix, the file can remain open during atomic rename
    #[cfg(target_os = "windows")]
    {
        let dummy_path = config_dir.join(".dummy");
        // Create a dummy file to replace the handle we need to close
        let dummy = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&dummy_path)
            .with_context(|| {
                format!(
                    "Failed to create dummy file handle `{}`.",
                    dummy_path.display()
                )
            })?;

        // Replace the file handle with the dummy, dropping the old handle
        let _ = mem::replace(&mut juliaup_config_file.file, dummy);
    }

    // Atomically replace the old config file with the new one
    persist_atomically(temp_file, &paths.juliaupconfig).with_context(|| {
        format!(
            "Failed to persist configuration file `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    // Reopen the file to update our file handle (Windows only, since we closed it)
    #[cfg(target_os = "windows")]
    {
        juliaup_config_file.file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.juliaupconfig)
            .with_context(|| {
                format!(
                    "Failed to reopen configuration file `{}` after save.",
                    paths.juliaupconfig.display()
                )
            })?;
    }

    #[cfg(feature = "selfupdate")]
    {
        // Use the same atomic write pattern for the self config file
        let self_config_dir = paths.juliaupselfconfig.parent().ok_or_else(|| {
            anyhow!(
                "Self config file path `{}` has no parent directory.",
                paths.juliaupselfconfig.display()
            )
        })?;

        let mut temp_self_file = NamedTempFile::new_in(self_config_dir).with_context(|| {
            format!(
                "Failed to create temporary self config file in directory `{}`.",
                self_config_dir.display()
            )
        })?;

        serde_json::to_writer_pretty(&mut temp_self_file, &juliaup_config_file.self_data)
            .with_context(|| {
                format!(
                    "Failed to write self configuration data for `{}` to temporary file.",
                    paths.juliaupselfconfig.display()
                )
            })?;

        temp_self_file.flush().with_context(|| {
            format!(
                "Failed to flush temporary self configuration data for `{}`.",
                paths.juliaupselfconfig.display()
            )
        })?;

        temp_self_file.as_file().sync_all().with_context(|| {
            format!(
                "Failed to sync temporary self configuration data for `{}` to disk.",
                paths.juliaupselfconfig.display()
            )
        })?;

        // On Windows, we must close the file before replacing it
        #[cfg(target_os = "windows")]
        {
            let dummy_self_path = self_config_dir.join(".dummy_self");
            // Create a dummy file to replace the handle we need to close
            let dummy = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&dummy_self_path)
                .with_context(|| {
                    format!(
                        "Failed to create dummy self config file handle `{}`.",
                        dummy_self_path.display()
                    )
                })?;

            // Replace the file handle with the dummy, dropping the old handle
            let _ = mem::replace(&mut juliaup_config_file.self_file, dummy);
        }

        persist_atomically(temp_self_file, &paths.juliaupselfconfig).with_context(|| {
            format!(
                "Failed to persist self configuration file `{}`.",
                paths.juliaupselfconfig.display()
            )
        })?;

        // Reopen the self config file (Windows only, since we closed it)
        #[cfg(target_os = "windows")]
        {
            juliaup_config_file.self_file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&paths.juliaupselfconfig)
                .with_context(|| {
                    format!(
                        "Failed to reopen self configuration file `{}` after save.",
                        paths.juliaupselfconfig.display()
                    )
                })?;
        }
    }

    Ok(())
}

#[cfg(all(test, not(feature = "selfupdate")))]
mod tests {
    use super::*;

    fn test_paths(dir: &std::path::Path) -> GlobalPaths {
        GlobalPaths {
            juliauphome: dir.to_path_buf(),
            juliaupconfig: dir.join("juliaup.json"),
            lockfile: dir.join(".juliaup-lock"),
            versiondb: dir.join("versiondb-test.json"),
        }
    }

    #[test]
    fn lockfree_read_of_missing_config_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());

        let config = load_config_db_lockfree(&paths).unwrap();
        assert!(config.data.installed_channels.is_empty());
        assert!(config.data.default.is_none());
    }

    #[test]
    fn lockfree_read_of_empty_config_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        File::create(&paths.juliaupconfig).unwrap();

        let config = load_config_db_lockfree(&paths).unwrap();
        assert!(config.data.installed_channels.is_empty());
    }

    #[test]
    fn initial_config_is_created_atomically_and_parseable() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());

        let config_file = load_mut_config_db(&paths).unwrap();
        assert!(config_file.data.installed_channels.is_empty());

        // The file on disk must already contain the full initial config
        // (atomic create), so a lock-free reader parses it successfully.
        let on_disk = load_config_db_lockfree(&paths).unwrap();
        assert!(on_disk.data == config_file.data);
    }

    #[test]
    fn lockfree_read_does_not_block_on_exclusive_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());

        let mut config_file = load_mut_config_db(&paths).unwrap();
        config_file.data.default = Some("release".to_string());
        save_config_db(&mut config_file, &paths).unwrap();

        // The exclusive lock is still held by `config_file` here; a lock-free
        // read must succeed immediately and see the saved contents.
        let config = load_config_db_lockfree(&paths).unwrap();
        assert_eq!(config.data.default.as_deref(), Some("release"));
    }
}
