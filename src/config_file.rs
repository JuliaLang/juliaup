use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use cluFlock::{ExclusiveFlock, FlockLock, SharedFlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, ErrorKind, Seek, SeekFrom, Write};
#[cfg(target_os = "windows")]
use std::mem;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::NamedTempFile;

use crate::global_paths::GlobalPaths;

/// How often we re-check the lock while waiting for another process to release it.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Information about the process currently holding the exclusive configuration
/// lock. This is written to a sidecar file next to the lock file when the
/// exclusive lock is taken, and removed when it is released. It lets a waiting
/// process detect a lock that was orphaned by a crashed process (for example on
/// network filesystems where the OS does not reliably release `flock` locks) and
/// reclaim it instead of waiting forever.
#[derive(Serialize, Deserialize)]
struct LockMetadata {
    #[serde(rename = "Pid")]
    pid: u32,
    #[serde(rename = "Hostname")]
    hostname: String,
    #[serde(rename = "Started")]
    started: DateTime<Utc>,
}

#[derive(Clone, Copy)]
enum LockKind {
    Shared,
    Exclusive,
}

fn lock_metadata_path(paths: &GlobalPaths) -> PathBuf {
    paths.lockfile.with_extension("meta")
}

fn current_hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Records the current process as the lock holder. Best-effort: failing to write
/// the metadata only disables stale-lock detection, it does not affect the lock
/// itself, so callers ignore the result.
fn write_lock_metadata(paths: &GlobalPaths) -> Result<()> {
    let meta = LockMetadata {
        pid: std::process::id(),
        hostname: current_hostname(),
        started: Utc::now(),
    };

    let path = lock_metadata_path(paths);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .with_context(|| format!("Failed to open lock metadata file `{}`.", path.display()))?;

    serde_json::to_writer(&mut file, &meta)
        .with_context(|| format!("Failed to write lock metadata file `{}`.", path.display()))?;
    file.flush()
        .with_context(|| format!("Failed to flush lock metadata file `{}`.", path.display()))?;

    Ok(())
}

fn read_lock_metadata(paths: &GlobalPaths) -> Option<LockMetadata> {
    let file = OpenOptions::new()
        .read(true)
        .open(lock_metadata_path(paths))
        .ok()?;
    serde_json::from_reader(BufReader::new(file)).ok()
}

/// A lock is considered stale only when we can be confident its holder is gone:
/// the metadata must have been written on this same machine and the recorded
/// process must no longer be running. We never reason about liveness across
/// hosts, so a lock taken on another machine (shared network filesystem) is
/// always treated as live.
fn lock_is_stale(meta: &LockMetadata) -> bool {
    meta.hostname == current_hostname() && !process_is_alive(meta.pid)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    use nix::errno::Errno;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    // Signal 0 performs error checking without sending a signal.
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => true,
        Err(Errno::ESRCH) => false,
        // EPERM means the process exists but we may not signal it; any other
        // error is treated conservatively as "alive" so we never break a lock
        // that might still be held.
        Err(_) => true,
    }
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, FALSE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const STILL_ACTIVE: u32 = 259;
    // HRESULT form of ERROR_INVALID_PARAMETER (process id does not exist).
    const E_INVALID_PARAMETER: i32 = 0x8007_0057u32 as i32;

    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) {
            Ok(handle) => {
                let mut code: u32 = 0;
                let alive = match GetExitCodeProcess(handle, &mut code) {
                    Ok(()) => code == STILL_ACTIVE,
                    Err(_) => true,
                };
                let _ = CloseHandle(handle);
                alive
            }
            // Only a non-existent process id reliably yields ERROR_INVALID_PARAMETER.
            // Access-denied and other errors mean the process exists, so assume alive.
            Err(e) => e.code().0 != E_INVALID_PARAMETER,
        }
    }
}

fn open_lock_file(paths: &GlobalPaths) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lockfile)
        .map_err(|e| {
            anyhow!(
                "Could not create lockfile `{}`: {}.",
                paths.lockfile.display(),
                e
            )
        })
}

fn try_lock(file: File, kind: LockKind) -> std::result::Result<FlockLock<File>, File> {
    match kind {
        LockKind::Shared => SharedFlock::try_lock(file).map_err(|e| e.into()),
        LockKind::Exclusive => ExclusiveFlock::try_lock(file).map_err(|e| e.into()),
    }
}

/// Acquires the configuration lock, waiting for any other process to release it.
/// If the holder is detected to be a dead process on this machine, the lock is
/// reclaimed rather than waited on indefinitely.
fn acquire_lock(paths: &GlobalPaths, kind: LockKind) -> Result<FlockLock<File>> {
    let mut file = open_lock_file(paths)?;

    file = match try_lock(file, kind) {
        Ok(lock) => return Ok(lock),
        Err(file) => file,
    };

    eprintln!("Juliaup configuration is locked by another process, waiting for it to unlock.");

    let mut announced_stale = false;
    loop {
        if let Some(meta) = read_lock_metadata(paths) {
            if lock_is_stale(&meta) {
                if !announced_stale {
                    eprintln!(
                        "Detected a stale lock previously held by process {} which is no longer running; reclaiming it.",
                        meta.pid
                    );
                    announced_stale = true;
                }

                // Drop our handle, remove the orphaned lock file and its
                // metadata, then start over against a fresh lock file.
                drop(file);
                let _ = std::fs::remove_file(&paths.lockfile);
                let _ = std::fs::remove_file(lock_metadata_path(paths));
                file = open_lock_file(paths)?;
            }
        }

        match try_lock(file, kind) {
            Ok(lock) => return Ok(lock),
            Err(returned_file) => {
                file = returned_file;
                std::thread::sleep(LOCK_POLL_INTERVAL);
            }
        }
    }
}

/// Holds the exclusive configuration lock and removes the holder metadata when
/// dropped, so a subsequent waiter never mistakes our cleanly-released lock for
/// a stale one.
pub struct ConfigLock {
    lock: Option<FlockLock<File>>,
    meta_path: PathBuf,
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        // Remove the metadata before releasing the OS lock.
        let _ = std::fs::remove_file(&self.meta_path);
        // Dropping the inner FlockLock releases the OS-level lock.
        self.lock = None;
    }
}

fn get_write_lock(paths: &GlobalPaths) -> Result<ConfigLock> {
    let lock = acquire_lock(paths, LockKind::Exclusive)?;

    // Best-effort: recording the holder enables stale-lock detection, but a
    // failure here (e.g. a read-only metadata path) must not block the lock.
    if let Err(e) = write_lock_metadata(paths) {
        log::debug!("Could not record lock holder metadata: {e}");
    }

    Ok(ConfigLock {
        lock: Some(lock),
        meta_path: lock_metadata_path(paths),
    })
}

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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone)]
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
    pub lock: ConfigLock,
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

pub fn get_read_lock(paths: &GlobalPaths) -> Result<FlockLock<File>> {
    std::fs::create_dir_all(&paths.juliauphome).with_context(|| {
        format!(
            "Could not create juliaup home folder `{}`.",
            paths.juliauphome.display()
        )
    })?;

    acquire_lock(paths, LockKind::Shared)
}

pub fn load_config_db(
    paths: &GlobalPaths,
    existing_lock: Option<&FlockLock<File>>,
) -> Result<JuliaupReadonlyConfigFile> {
    let mut file_lock: Option<FlockLock<File>> = None;

    if existing_lock.is_none() {
        file_lock = Some(get_read_lock(paths)?);
    }

    let v = match std::fs::OpenOptions::new()
        .read(true)
        .open(&paths.juliaupconfig)
    {
        Ok(file) => {
            let reader = BufReader::new(&file);

            serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse configuration file '{:?}' for reading.",
                    paths.juliaupconfig
                )
            })?
        }
        Err(error) => match error.kind() {
            ErrorKind::NotFound => JuliaupConfig {
                default: None,
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings::default(),
                last_version_db_update: None,
            },
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
            Err(error) => bail!(
                "Could not open self configuration file {:?}: {:?}",
                paths.juliaupselfconfig,
                error
            ),
        };
    }

    if let Some(file_lock) = file_lock {
        file_lock.unlock().with_context(|| {
            format!(
                "Failed to unlock configuration lock file `{}`.",
                paths.lockfile.display()
            )
        })?;
    }

    Ok(JuliaupReadonlyConfigFile {
        data: v,
        #[cfg(feature = "selfupdate")]
        self_data: selfconfig,
    })
}

pub fn load_mut_config_db(paths: &GlobalPaths) -> Result<JuliaupConfigFile> {
    std::fs::create_dir_all(&paths.juliauphome).with_context(|| {
        format!(
            "Could not create juliaup home folder `{}`.",
            paths.juliauphome.display()
        )
    })?;

    let file_lock = get_write_lock(paths)?;

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.juliaupconfig)
        .with_context(|| {
            format!(
                "Failed to open juliaup config file `{}`.",
                paths.juliaupconfig.display()
            )
        })?;

    let stream_len = file.seek(SeekFrom::End(0)).with_context(|| {
        format!(
            "Failed to determine the length of configuration file `{}`.",
            paths.juliaupconfig.display()
        )
    })?;

    let data = match stream_len {
        0 => {
            let new_config = JuliaupConfig {
                default: None,
                installed_versions: HashMap::new(),
                installed_channels: HashMap::new(),
                overrides: Vec::new(),
                settings: JuliaupConfigSettings::default(),
                last_version_db_update: None,
            };

            serde_json::to_writer_pretty(&file, &new_config).with_context(|| {
                format!(
                    "Failed to write configuration file `{}`.",
                    paths.juliaupconfig.display()
                )
            })?;

            file.sync_all().with_context(|| {
                format!(
                    "Failed to sync configuration file `{}` to disk.",
                    paths.juliaupconfig.display()
                )
            })?;

            file.rewind().with_context(|| {
                format!(
                    "Failed to rewind config file `{}` after initial write.",
                    paths.juliaupconfig.display()
                )
            })?;

            new_config
        }
        _ => {
            file.rewind().with_context(|| {
                format!(
                    "Failed to rewind existing config file `{}`.",
                    paths.juliaupconfig.display()
                )
            })?;

            let reader = BufReader::new(&file);

            serde_json::from_reader(reader).with_context(|| {
                format!(
                    "Failed to parse configuration file `{}`.",
                    paths.juliaupconfig.display()
                )
            })?
        }
    };

    #[cfg(feature = "selfupdate")]
    let self_file: File;
    #[cfg(feature = "selfupdate")]
    let self_data: JuliaupSelfConfig;
    #[cfg(feature = "selfupdate")]
    {
        self_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&paths.juliaupselfconfig)
            .with_context(|| {
                format!(
                    "Failed to open juliaup self config file `{}`.",
                    paths.juliaupselfconfig.display()
                )
            })?;

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
    temp_file.persist(&paths.juliaupconfig).with_context(|| {
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

        temp_self_file
            .persist(&paths.juliaupselfconfig)
            .with_context(|| {
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
