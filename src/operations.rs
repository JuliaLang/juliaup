use crate::config_file::load_mut_config_db;
use crate::config_file::save_config_db;
use crate::config_file::JuliaupConfig;
use crate::config_file::JuliaupConfigApplication;
use crate::config_file::JuliaupConfigChannel;
use crate::config_file::JuliaupConfigVersion;
use crate::get_bundled_dbversion;
use crate::get_bundled_julia_version;
use crate::get_juliaup_target;
use crate::global_paths::GlobalPaths;
use crate::jsonstructs_versionsdb::JuliaupVersionDB;
use crate::utils::get_bin_dir;
use crate::utils::get_julianightlies_base_url;
use crate::utils::get_juliaserver_base_url;
use anyhow::{anyhow, bail, Context, Result};
use bstr::ByteSlice;
use bstr::ByteVec;
use console::style;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use indoc::formatdoc;
use semver::Version;
use std::io::BufReader;
use std::io::Seek;
use std::io::Write;
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use std::{
    io::Read,
    path::{Component::Normal, Path, PathBuf},
};
use tar::Archive;
use tempfile::Builder;
use url::Url;

fn unpack_sans_parent<R, P>(mut archive: Archive<R>, dst: P, levels_to_skip: usize) -> Result<()>
where
    R: Read,
    P: AsRef<Path>,
{
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path: PathBuf = entry
            .path()?
            .components()
            .skip(levels_to_skip) // strip top-level directory
            .filter(|c| matches!(c, Normal(_))) // prevent traversal attacks TODO We should actually abort if we come across a non-standard path element
            .collect();
        entry.unpack(dst.as_ref().join(path))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn download_extract_sans_parent(
    url: &str,
    target_path: &Path,
    levels_to_skip: usize,
) -> Result<String> {
    log::debug!("Downloading from url `{}`.", url);
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let content_length = response.content_length();

    let pb = match content_length {
        Some(content_length) => ProgressBar::new(content_length),
        None => ProgressBar::new_spinner(),
    };

    pb.set_prefix("  Downloading:");
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.cyan.bold} [{bar}] {bytes}/{total_bytes} eta: {eta}")
            .unwrap()
            .progress_chars("=> "),
    );

    let last_modified = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let response_with_pb = pb.wrap_read(response);

    let tar = GzDecoder::new(response_with_pb);
    let archive = Archive::new(tar);
    unpack_sans_parent(archive, target_path, levels_to_skip)
        .with_context(|| format!("Failed to extract downloaded file from url `{}`.", url))?;

    Ok(last_modified)
}

#[cfg(windows)]
struct DataReaderWrap(windows::Storage::Streams::DataReader);

#[cfg(windows)]
impl std::io::Read for DataReaderWrap {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut bytes =
            self.0
                .LoadAsync(buf.len() as u32)
                .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?
                .get()
                .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))? as usize;
        bytes = bytes.min(buf.len());
        self.0
            .ReadBytes(&mut buf[0..bytes])
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))
            .map(|_| bytes)
    }
}

#[cfg(windows)]
pub fn download_extract_sans_parent(
    url: &str,
    target_path: &Path,
    levels_to_skip: usize,
) -> Result<String> {
    use windows::core::HSTRING;

    let http_client =
        windows::Web::Http::HttpClient::new().with_context(|| "Failed to create HttpClient.")?;

    let request_uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url))
        .with_context(|| "Failed to convert url string to Uri.")?;

    let http_response = http_client
        .GetAsync(&request_uri)
        .with_context(|| "Failed to initiate download.")?
        .get()
        .with_context(|| "Failed to complete async download operation.")?;

    http_response
        .EnsureSuccessStatusCode()
        .with_context(|| "HTTP download reported error status code.")?;

    let last_modified = http_response
        .Headers()
        .unwrap()
        .Lookup(&HSTRING::from("etag"))
        .unwrap()
        .to_string();

    let http_response_content = http_response
        .Content()
        .with_context(|| "Failed to obtain content from http response.")?;

    let response_stream = http_response_content
        .ReadAsInputStreamAsync()
        .with_context(|| "Failed to initiate get input stream from response")?
        .get()
        .with_context(|| "Failed to obtain input stream from http response")?;

    let reader = windows::Storage::Streams::DataReader::CreateDataReader(&response_stream)
        .with_context(|| "Failed to create DataReader.")?;

    reader
        .SetInputStreamOptions(windows::Storage::Streams::InputStreamOptions::ReadAhead)
        .with_context(|| "Failed to set input stream options.")?;

    let mut content_length: u64 = 0;
    let pb = if http_response_content.TryComputeLength(&mut content_length)? {
        ProgressBar::new(content_length)
    } else {
        ProgressBar::new_spinner()
    };

    pb.set_prefix("  Downloading:");
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.cyan.bold} [{bar}] {bytes}/{total_bytes} eta: {eta}")
            .unwrap()
            .progress_chars("=> "),
    );

    let response_with_pb = pb.wrap_read(DataReaderWrap(reader));

    let tar = GzDecoder::new(response_with_pb);
    let archive = Archive::new(tar);
    unpack_sans_parent(archive, target_path, levels_to_skip)
        .with_context(|| format!("Failed to extract downloaded file from url `{}`.", url))?;

    Ok(last_modified)
}

#[cfg(not(windows))]
pub fn download_file(
    url: &str,
    target_path: &Path,
    filename: &str
) -> Result<()> {
    log::debug!("Downloading from url `{}`.", url);
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let mut file = std::fs::File::create(target_path.join(filename))?;
    let mut content =  std::io::Cursor::new(response.bytes().unwrap());
    std::io::copy(&mut content, &mut file)?;

    Ok(())
}

#[cfg(windows)]
pub fn download_file(
    url: &str,
    target_path: &Path,
    filename: &str
) -> Result<String> {
    use windows::{core::HSTRING, Storage::FileAccessMode};

    let http_client =
        windows::Web::Http::HttpClient::new().with_context(|| "Failed to create HttpClient.")?;

    let request_uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url))
        .with_context(|| "Failed to convert url string to Uri.")?;

    let http_response = http_client
        .GetAsync(&request_uri)
        .with_context(|| "Failed to initiate download.")?
        .get()
        .with_context(|| "Failed to complete async download operation.")?;

    http_response
        .EnsureSuccessStatusCode()
        .with_context(|| "HTTP download reported error status code.")?;

    let last_modified = http_response
        .Headers()
        .unwrap()
        .Lookup(&HSTRING::from("etag"))
        .unwrap()
        .to_string();

    let http_response_content = http_response
        .Content()
        .with_context(|| "Failed to obtain content from http response.")?;

    let folder = windows::Storage::StorageFolder::GetFolderFromPathAsync(&HSTRING::from(target_path)).unwrap().get().unwrap();
    let file = folder.CreateFileAsync(&HSTRING::from(filename), windows::Storage::CreationCollisionOption::ReplaceExisting).unwrap().get().unwrap();

    let stream = file.OpenAsync(FileAccessMode::ReadWrite).unwrap().get().unwrap();

    http_response_content.WriteToStreamAsync(&stream).unwrap().get().unwrap();

    Ok(last_modified)
}

#[cfg(not(windows))]
pub fn download_juliaup_version(url: &str) -> Result<Version> {
    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download from url `{}`.", url))?
        .text()?;

    let trimmed_response = response.trim();

    let version = Version::parse(trimmed_response).with_context(|| {
        format!(
            "`download_juliaup_version` failed to parse `{}` as a valid semversion.",
            trimmed_response
        )
    })?;

    Ok(version)
}

#[cfg(not(windows))]
pub fn download_versiondb(url: &str, path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("Failed to open or create version db file at {:?}", path))?;
    let mut buf: Vec<u8> = vec![];
    response.copy_to(&mut buf)?;
    file.write_all(buf.as_slice())
        .with_context(|| "Failed to write content into version db file.")?;

    Ok(())
}

#[cfg(windows)]
pub fn download_juliaup_version(url: &str) -> Result<Version> {
    let http_client =
        windows::Web::Http::HttpClient::new().with_context(|| "Failed to create HttpClient.")?;

    let request_uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url))
        .with_context(|| "Failed to convert url string to Uri.")?;

    let response = http_client
        .GetStringAsync(&request_uri)
        .with_context(|| "Failed on http_client.GetStringAsync")?
        .get()
        .with_context(|| "Failed on http_client.GetStringAsync.get")?
        .to_string();

    let trimmed_response = response.trim();

    let version = Version::parse(trimmed_response).with_context(|| {
        format!(
            "`download_juliaup_version` failed to parse `{}` as a valid semversion.",
            trimmed_response
        )
    })?;

    Ok(version)
}

#[cfg(windows)]
pub fn download_versiondb(url: &str, path: &Path) -> Result<()> {
    let http_client =
        windows::Web::Http::HttpClient::new().with_context(|| "Failed to create HttpClient.")?;

    let request_uri = windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url))
        .with_context(|| "Failed to convert url string to Uri.")?;

    let response = http_client
        .GetStringAsync(&request_uri)
        .with_context(|| "Failed to download version db step 1.")?
        .get()
        .with_context(|| "Failed to download version db step 2.")?
        .to_string();

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .with_context(|| format!("Failed to open or create version db file at {:?}", path))?;

    file.write_all(response.as_bytes())
        .with_context(|| "Failed to write content into version db file.")?;

    Ok(())
}

pub fn install_version(
    fullversion: &String,
    config_data: &mut JuliaupConfig,
    version_db: &JuliaupVersionDB,
    paths: &GlobalPaths,
) -> Result<()> {
    // Return immediately if the version is already installed.
    if config_data.installed_versions.contains_key(fullversion) {
        return Ok(());
    }

    // TODO At some point we could put this behind a conditional compile, we know
    // that we don't ship a bundled version for some platforms.
    let full_version_string_of_bundled_version = get_bundled_julia_version();
    let my_own_path = std::env::current_exe()?;
    let path_of_bundled_version = my_own_path
        .parent()
        .unwrap() // unwrap OK because we can't get a path that does not have a parent
        .join("BundledJulia");

    let child_target_foldername = format!("julia-{}", fullversion);
    let target_path = paths.juliauphome.join(&child_target_foldername);
    std::fs::create_dir_all(target_path.parent().unwrap())?;

    if fullversion == full_version_string_of_bundled_version && path_of_bundled_version.exists() {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        options.content_only = true;
        fs_extra::dir::copy(path_of_bundled_version, target_path, &options)?;
    } else {
        let juliaupserver_base =
            get_juliaserver_base_url().with_context(|| "Failed to get Juliaup server base URL.")?;

        let download_url_path = &version_db
            .available_versions
            .get(fullversion)
            .ok_or_else(|| {
                anyhow!(
                    "Failed to find download url in versions db for '{}'.",
                    fullversion
                )
            })?
            .url_path;

        let download_url = juliaupserver_base
            .join(download_url_path)
            .with_context(|| {
                format!(
                    "Failed to construct a valid url from '{}' and '{}'.",
                    juliaupserver_base, download_url_path
                )
            })?;

        eprintln!(
            "{} Julia {}",
            style("Installing").green().bold(),
            fullversion
        );

        download_extract_sans_parent(download_url.as_ref(), &target_path, 1)?;
    }

    let mut rel_path = PathBuf::new();
    rel_path.push(".");
    rel_path.push(&child_target_foldername);

    config_data.installed_versions.insert(
        fullversion.clone(),
        JuliaupConfigVersion {
            path: rel_path.to_string_lossy().into_owned(),
        },
    );

    Ok(())
}

// which nightly arch to default to when simply using the `nightly` channel
pub fn default_nightly_arch() -> Result<String> {
    if cfg!(target_arch = "aarch64") {
        Ok("aarch64".to_string())
    } else if cfg!(target_arch = "x86_64") {
        Ok("x64".to_string())
    } else if cfg!(target_arch = "x86") {
        Ok("x86".to_string())
    } else {
        bail!("Unsupported architecture for nightly channel.")
    }
}

// which nightly archs are compatible with the current system, for `juliaup list` purposes
pub fn compatible_nightly_archs() -> Result<Vec<String>> {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            Ok(vec!["x64".to_string()])
        } else if cfg!(target_arch = "aarch64") {
            // Rosetta 2 can execute x86_64 binaries
            Ok(vec!["aarch64".to_string(), "x64".to_string()])
        } else {
            bail!("Unsupported architecture for nightly channel on macOS.")
        }
    } else if cfg!(target_arch = "x86") {
        Ok(vec!["x86".to_string()])
    } else if cfg!(target_arch = "x86_64") {
        // x86_64 can execute x86 binaries
        Ok(vec!["x86".to_string(), "x64".to_string()])
    } else if cfg!(target_arch = "aarch64") {
        Ok(vec!["aarch64".to_string()])
    } else {
        bail!("Unsupported architecture for nightly channel.")
    }
}

// Identify the unversioned name of a nightly (e.g., `latest-macos-x86_64`) for a channel
pub fn identify_nightly(channel: &String) -> Result<String> {
    let arch = if channel == "nightly" {
        default_nightly_arch()?
    } else {
        let parts: Vec<&str> = channel.splitn(2, '~').collect();
        if parts.len() != 2 {
            bail!("Invalid nightly channel name '{}'.", channel)
        }
        parts[1].to_string()
    };

    let name = {
        #[cfg(target_os = "macos")]
        if arch == "x64" {
            "latest-macos-x86_64"
        } else if arch == "aarch64" {
            "latest-macos-aarch64"
        } else {
            bail!("Unsupported architecture for nightly channel on macOS.")
        }

        #[cfg(target_os = "windows")]
        if arch == "x64" {
            "latest-win64"
        } else if arch == "x86" {
            "latest-win32"
        } else {
            bail!("Unsupported architecture for nightly channel on Windows.")
        }

        #[cfg(target_os = "linux")]
        if arch == "x64" {
            "latest-linux-x86_64"
        } else if arch == "x86" {
            "latest-linux-i686"
        } else if arch == "aarch64" {
            "latest-linux-aarch64"
        } else {
            bail!("Unsupported architecture for nightly channel on Linux.")
        }
    };

    Ok(name.to_string())
}

pub fn install_from_url(
    url: &Url,
    path: &PathBuf,
    paths: &GlobalPaths,
) -> Result<crate::config_file::JuliaupConfigChannel> {
    // Download and extract into a temporary directory
    let temp_dir = Builder::new()
        .prefix("julia-temp-")
        .tempdir_in(&paths.juliauphome)
        .expect("Failed to create temporary directory");

    let download_result = download_extract_sans_parent(url.as_ref(), &temp_dir.path(), 1);

    let server_etag = match download_result {
        Ok(last_updated) => last_updated,
        Err(e) => {
            std::fs::remove_dir_all(temp_dir.into_path())?;
            bail!("Failed to download and extract nightly: {}", e);
        }
    };

    // Query the actual version
    let julia_path = temp_dir
        .path()
        .join("bin")
        .join(format!("julia{}", std::env::consts::EXE_SUFFIX));
    let julia_process = std::process::Command::new(julia_path.clone())
        .arg("--startup-file=no")
        .arg("-e")
        .arg("print(VERSION)")
        .output()
        .with_context(|| {
            format!(
                "Failed to execute Julia binary at `{}`.",
                julia_path.display()
            )
        })?;
    let julia_version = String::from_utf8(julia_process.stdout)?;

    // Move into the final location
    let target_path = paths.juliauphome.join(&path);
    if target_path.exists() {
        std::fs::remove_dir_all(&target_path)?;
    }
    std::fs::rename(temp_dir.into_path(), &target_path)?;

    Ok(JuliaupConfigChannel::DirectDownloadChannel {
        path: path.to_string_lossy().into_owned(),
        url: url.to_string().to_owned(), // TODO Use proper URL
        local_etag: server_etag.clone(), // TODO Use time stamp of HTTPS response
        server_etag: server_etag,
        version: julia_version,
    })
}

pub fn install_nightly(
    channel: &str,
    name: &String,
    paths: &GlobalPaths,
) -> Result<crate::config_file::JuliaupConfigChannel> {
    // Determine the download URL
    let download_url_base = get_julianightlies_base_url()?;
    let download_url_path = match name.as_str() {
        "latest-macos-x86_64" => Ok("bin/macos/x86_64/julia-latest-macos-x86_64.tar.gz"),
        "latest-macos-aarch64" => Ok("bin/macos/aarch64/julia-latest-macos-aarch64.tar.gz"),
        "latest-win64" => Ok("bin/winnt/x64/julia-latest-win64.tar.gz"),
        "latest-win32" => Ok("bin/winnt/x86/julia-latest-win32.tar.gz"),
        "latest-linux-x86_64" => Ok("bin/linux/x86_64/julia-latest-linux-x86_64.tar.gz"),
        "latest-linux-i686" => Ok("bin/linux/i686/julia-latest-linux-i686.tar.gz"),
        "latest-linux-aarch64" => Ok("bin/linux/aarch64/julia-latest-linux-aarch64.tar.gz"),
        _ => Err(anyhow!("Unknown nightly.")),
    }?;
    let download_url = download_url_base.join(download_url_path).with_context(|| {
        format!(
            "Failed to construct a valid url from '{}' and '{}'.",
            download_url_base, download_url_path
        )
    })?;

    let child_target_foldername = format!("julia-{}", channel);

    let mut rel_path = PathBuf::new();
    rel_path.push(".");
    rel_path.push(&child_target_foldername);

    eprintln!("{} Julia {}", style("Installing").green().bold(), name);

    let res = install_from_url(&download_url, &rel_path, paths)?;

    Ok(res)
}

pub fn garbage_collect_versions(
    config_data: &mut JuliaupConfig,
    paths: &GlobalPaths,
) -> Result<()> {
    let mut versions_to_uninstall: Vec<String> = Vec::new();
    for (installed_version, detail) in &config_data.installed_versions {
        if config_data.installed_channels.iter().all(|j| match &j.1 {
            JuliaupConfigChannel::SystemChannel { version } => version != installed_version,
            JuliaupConfigChannel::LinkedChannel {
                command: _,
                args: _,
            } => true,
            JuliaupConfigChannel::DirectDownloadChannel {
                path: _,
                url: _,
                local_etag: _,
                server_etag: _,
                version: _,
            } => true,
        }) && config_data.installed_apps.iter().all(|j| match &j.1 {
            JuliaupConfigApplication::DirectDownloadApplication { path: _, url: _, local_etag: _, server_etag: _, version, execution_aliases: _ } => version != installed_version
        } ) {
            let path_to_delete = paths.juliauphome.join(&detail.path);
            let display = path_to_delete.display();

            if std::fs::remove_dir_all(&path_to_delete).is_err() {
                eprintln!("WARNING: Failed to delete {}. You can try to delete at a later point by running `juliaup gc`.", display)
            }
            versions_to_uninstall.push(installed_version.clone());
        }
    }

    for i in versions_to_uninstall {
        config_data.installed_versions.remove(&i);
    }

    Ok(())
}

fn _remove_symlink(symlink_path: &Path) -> Result<()> {
    std::fs::create_dir_all(symlink_path.parent().unwrap())?;

    if symlink_path.exists() {
        std::fs::remove_file(symlink_path)?;
    }

    Ok(())
}

pub fn remove_symlink(symlink_name: &String) -> Result<()> {
    let symlink_path = get_bin_dir()
        .with_context(|| "Failed to retrieve binary directory while trying to remove a symlink.")?
        .join(symlink_name);

    eprintln!(
        "{} {}.",
        style("Deleting symlink").cyan().bold(),
        symlink_name
    );

    _remove_symlink(&symlink_path)?;

    Ok(())
}

#[cfg(not(windows))]
pub fn create_symlink(
    channel: &JuliaupConfigChannel,
    symlink_name: &String,
    paths: &GlobalPaths,
) -> Result<()> {
    let symlink_folder = get_bin_dir()
        .with_context(|| "Failed to retrieve binary directory while trying to create a symlink.")?;

    let symlink_path = symlink_folder.join(symlink_name);

    _remove_symlink(&symlink_path)?;

    match channel {
        JuliaupConfigChannel::SystemChannel { version } => {
            let child_target_foldername = format!("julia-{}", version);

            let target_path = paths.juliauphome.join(&child_target_foldername);

            eprintln!(
                "{} {} for Julia {}.",
                style("Creating symlink").cyan().bold(),
                symlink_name,
                version
            );

            std::os::unix::fs::symlink(target_path.join("bin").join("julia"), &symlink_path)
                .with_context(|| {
                    format!(
                        "failed to create symlink `{}`.",
                        symlink_path.to_string_lossy()
                    )
                })?;
        }
        JuliaupConfigChannel::DirectDownloadChannel {
            path,
            url: _,
            local_etag: _,
            server_etag: _,
            version,
        } => {
            let target_path = paths.juliauphome.join(path);

            eprintln!(
                "{} {} for Julia {}.",
                style("Creating symlink").cyan().bold(),
                symlink_name,
                version
            );

            std::os::unix::fs::symlink(target_path.join("bin").join("julia"), &symlink_path)
                .with_context(|| {
                    format!(
                        "failed to create symlink `{}`.",
                        symlink_path.to_string_lossy()
                    )
                })?;
        }
        JuliaupConfigChannel::LinkedChannel { command, args } => {
            let formatted_command = match args {
                Some(x) => format!("{} {}", command, x.join(" ")),
                None => command.clone(),
            };

            eprintln!(
                "{} {} for `{}`",
                style("Creating shim").cyan().bold(),
                symlink_name,
                formatted_command
            );

            std::fs::write(
                &symlink_path,
                format!(
                    r#"#!/bin/sh
{} "$@"
"#,
                    formatted_command,
                ),
            )
            .with_context(|| {
                format!(
                    "failed to create shim `{}`.",
                    symlink_path.to_string_lossy()
                )
            })?;

            // set as executable
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&symlink_path, perms).with_context(|| {
                format!(
                    "failed to change permissions for shim `{}`.",
                    symlink_path.to_string_lossy()
                )
            })?;
        }
    };

    if let Ok(path) = std::env::var("PATH") {
        if !path.split(':').any(|p| Path::new(p) == symlink_folder) {
            eprintln!(
                "Symlink {} added in {}. Add this directory to the system PATH to make the command available in your shell.",
                &symlink_name, symlink_folder.display(),
            );
        }
    }

    Ok(())
}

#[cfg(windows)]
pub fn create_symlink(_: &JuliaupConfigChannel, _: &String, _paths: &GlobalPaths) -> Result<()> {
    Ok(())
}

#[cfg(feature = "selfupdate")]
pub fn install_background_selfupdate(interval: i64) -> Result<()> {
    use itertools::Itertools;
    use std::process::Stdio;

    let own_exe_path = std::env::current_exe()
        .with_context(|| "Could not determine the path of the running exe.")?;

    let my_own_path = own_exe_path.to_str().unwrap();

    match std::env::var("WSL_DISTRO_NAME") {
        // This is the WSL case, where we schedule a Windows task to do the update
        Ok(val) => {
            std::process::Command::new("schtasks.exe")
                .args([
                    "/create",
                    "/sc",
                    "minute",
                    "/mo",
                    &interval.to_string(),
                    "/tn",
                    &format!("Juliaup self update for WSL {} distribution", val),
                    "/f",
                    "/it",
                    "/tr",
                    &format!("wsl --distribution {} {} self update", val, my_own_path),
                ])
                .output()
                .with_context(|| "Failed to create new Windows task for juliaup.")?;
        }
        Err(_e) => {
            let output = std::process::Command::new("crontab")
                .args(["-l"])
                .output()
                .with_context(|| "Failed to retrieve crontab configuration.")?;

            let new_crontab_content = String::from_utf8(output.stdout)?
                .lines()
                .filter(|x| !x.contains("4c79c12db1d34bbbab1f6c6f838f423f"))
                .chain([
                    &format!(
                        "*/{} * * * * {} 4c79c12db1d34bbbab1f6c6f838f423f",
                        interval, my_own_path
                    ),
                    "",
                ])
                .join("\n");

            let mut child = std::process::Command::new("crontab")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut child_stdin = child.stdin.take().unwrap();

            child_stdin.write_all(new_crontab_content.as_bytes())?;

            // Close stdin to finish and avoid indefinite blocking
            drop(child_stdin);

            child.wait_with_output()?;
        }
    };

    Ok(())
}

#[cfg(feature = "selfupdate")]
pub fn uninstall_background_selfupdate() -> Result<()> {
    use itertools::Itertools;
    use std::process::Stdio;

    match std::env::var("WSL_DISTRO_NAME") {
        // This is the WSL case, where we schedule a Windows task to do the update
        Ok(val) => {
            std::process::Command::new("schtasks.exe")
                .args([
                    "/delete",
                    "/tn",
                    &format!("Juliaup self update for WSL {} distribution", val),
                    "/f",
                ])
                .output()
                .with_context(|| "Failed to remove Windows task for juliaup.")?;
        }
        Err(_e) => {
            let output = std::process::Command::new("crontab")
                .args(["-l"])
                .output()
                .with_context(|| "Failed to remove cron task.")?;

            let new_crontab_content = String::from_utf8(output.stdout)?
                .lines()
                .filter(|x| !x.contains("4c79c12db1d34bbbab1f6c6f838f423f"))
                .chain([""])
                .join("\n");

            let mut child = std::process::Command::new("crontab")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut child_stdin = child.stdin.take().unwrap();

            child_stdin.write_all(new_crontab_content.as_bytes())?;

            // Close stdin to finish and avoid indefinite blocking
            drop(child_stdin);

            child.wait_with_output()?;
        }
    };

    Ok(())
}

const S_MARKER: &[u8] = b"# >>> juliaup initialize >>>";
const E_MARKER: &[u8] = b"# <<< juliaup initialize <<<";
const HEADER: &[u8] = b"\n\n# !! Contents within this block are managed by juliaup !!\n\n";

fn get_shell_script_juliaup_content(bin_path: &Path, path: &Path) -> Result<Vec<u8>> {
    let mut result: Vec<u8> = Vec::new();

    let bin_path_str = match bin_path.to_str() {
        Some(s) => s,
        None =>  bail!("Could not create UTF-8 string from passed-in binary application path. Currently only valid UTF-8 paths are supported"),
    };

    result.extend_from_slice(S_MARKER);
    result.extend_from_slice(HEADER);
    if path.file_name().unwrap() == ".zshrc" {
        append_zsh_content(&mut result, bin_path_str);
    } else {
        append_sh_content(&mut result, bin_path_str);
    }
    result.extend_from_slice(b"\n");
    result.extend_from_slice(E_MARKER);

    Ok(result)
}

fn append_zsh_content(buf: &mut Vec<u8>, path_str: &str) {
    // zsh specific syntax for path extension
    let content = formatdoc!(
        "
            path=('{}' $path)
            export PATH
        ",
        path_str
    );

    buf.extend_from_slice(content.as_bytes());
}

fn append_sh_content(buf: &mut Vec<u8>, path_str: &str) {
    // If the variable is already contained in $PATH, do nothing
    // Otherwise prepend it to path
    // ${PATH:+:${PATH}} => Only append :$PATH if $PATH is set
    let content = formatdoc!(
        "
            case \":$PATH:\" in
                *:{0}:*)
                    ;;

                *)
                    export PATH={0}${{PATH:+:${{PATH}}}}
                    ;;
            esac
        ",
        path_str
    );
    buf.extend_from_slice(content.as_bytes());
}

fn match_markers(buffer: &[u8]) -> Result<Option<(usize, usize)>> {
    let start_marker = buffer.find(S_MARKER);
    let end_marker = buffer.find(E_MARKER);

    // This ensures exactly one opening and one closing marker exists
    let (start_marker, end_marker) = match (start_marker, end_marker) {
        (Some(sidx), Some(eidx)) => {
            if sidx != buffer.rfind(S_MARKER).unwrap() || eidx != buffer.rfind(E_MARKER).unwrap() {
                bail!("Found multiple startup script sections from juliaup.");
            }
            (sidx, eidx)
        }
        (None, None) => {
            return Ok(None);
        }
        (_, None) => {
            bail!("Found an opening marker but no end marker of juliaup section.");
        }
        (None, _) => {
            bail!("Found an opening marker but no end marker of juliaup section.");
        }
    };

    Ok(Some((start_marker, end_marker + E_MARKER.len())))
}

fn add_path_to_specific_file(bin_path: &Path, path: &Path) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .with_context(|| format!("Failed to open file {}.", path.display()))?;

    let mut buffer: Vec<u8> = Vec::new();

    file.read_to_end(&mut buffer)
        .with_context(|| format!("Failed to read data from file {}.", path.display()))?;

    let existing_code_pos = match_markers(&buffer).with_context(|| {
        format!(
            "Error occured while searching juliaup shell startup script section in {}",
            path.display()
        )
    })?;

    let new_content = get_shell_script_juliaup_content(bin_path, &path).with_context(|| {
        format!(
            "Error occured while generating juliaup shell startup script section for {}",
            path.display()
        )
    })?;

    match existing_code_pos {
        Some(pos) => {
            buffer.replace_range(pos.0..pos.1, &new_content);
        }
        None => {
            buffer.extend_from_slice(b"\n");
            buffer.extend_from_slice(&new_content);
            buffer.extend_from_slice(b"\n");
        }
    };

    file.rewind().unwrap();

    file.set_len(0).unwrap();

    file.write_all(&buffer).unwrap();

    file.sync_all().unwrap();

    Ok(())
}

fn remove_path_from_specific_file(path: &Path) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mut buffer: Vec<u8> = Vec::new();

    file.read_to_end(&mut buffer)?;

    let existing_code_pos = match_markers(&buffer).with_context(|| {
        format!(
            "Error occured while searching juliaup shell startup script section in {}",
            path.display()
        )
    })?;

    if let Some(pos) = existing_code_pos {
        buffer.replace_range(pos.0..pos.1, "");

        file.rewind().unwrap();

        file.set_len(0).unwrap();

        file.write_all(&buffer).unwrap();

        file.sync_all().unwrap();
    }

    Ok(())
}

pub fn find_shell_scripts_to_be_modified(add_case: bool) -> Result<Vec<PathBuf>> {
    let home_dir = dirs::home_dir().unwrap();

    let paths_to_test: Vec<PathBuf> = vec![
        home_dir.join(".bashrc"),
        home_dir.join(".profile"),
        home_dir.join(".bash_profile"),
        home_dir.join(".bash_login"),
        home_dir.join(".zshrc"),
    ];

    let result = paths_to_test
        .iter()
        .filter(
            |p| {
                p.exists()
                    || (add_case
                        && p.file_name().unwrap() == ".zshrc"
                        && std::env::consts::OS == "macos")
            }, // On MacOS, always edit .zshrc as that is the default shell, but only when we add things
        )
        .cloned()
        .collect();
    Ok(result)
}

pub fn add_binfolder_to_path_in_shell_scripts(bin_path: &Path) -> Result<()> {
    let paths = find_shell_scripts_to_be_modified(true)?;

    paths.into_iter().for_each(|p| {
        add_path_to_specific_file(bin_path, &p).unwrap();
    });
    Ok(())
}

pub fn remove_binfolder_from_path_in_shell_scripts() -> Result<()> {
    let paths = find_shell_scripts_to_be_modified(false)?;

    paths.into_iter().for_each(|p| {
        remove_path_from_specific_file(&p).unwrap();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_markers_none_without_markers() {
        let inp: &[u8] = b"Some input\n";
        let res = match_markers(inp);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn match_markers_returns_correct_indices() {
        let mut inp: Vec<u8> = Vec::new();
        let start_bytes = b"Some random bytes.";
        let middle_bytes = b"More bytes.";
        let end_bytes = b"Final bytes.";
        inp.extend_from_slice(start_bytes);
        inp.extend_from_slice(S_MARKER);
        inp.extend_from_slice(middle_bytes);
        inp.extend_from_slice(E_MARKER);
        inp.extend_from_slice(end_bytes);

        // Verify Ok(Some(..)) is returned
        let res = match_markers(&inp);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(res.is_some());
        let (sidx, eidx) = res.unwrap();

        // Verify correct positions
        assert_eq!(sidx, start_bytes.len());
        let expected_eidx =
            start_bytes.len() + S_MARKER.len() + middle_bytes.len() + E_MARKER.len();
        assert_eq!(eidx, expected_eidx);
    }

    #[test]
    fn match_markers_returns_err_without_start() {
        let mut inp: Vec<u8> = Vec::new();
        let start_bytes = b"Some random bytes.";
        let middle_bytes = b"More bytes.";
        let end_bytes = b"Final bytes.";
        inp.extend_from_slice(start_bytes);
        inp.extend_from_slice(middle_bytes);
        inp.extend_from_slice(E_MARKER);
        inp.extend_from_slice(end_bytes);

        // Verify Err(..) is returned
        let res = match_markers(&inp);
        assert!(res.is_err());
    }

    #[test]
    fn match_markers_returns_err_without_end() {
        let mut inp: Vec<u8> = Vec::new();
        let start_bytes = b"Some random bytes.";
        let middle_bytes = b"More bytes.";
        let end_bytes = b"Final bytes.";
        inp.extend_from_slice(start_bytes);
        inp.extend_from_slice(S_MARKER);
        inp.extend_from_slice(middle_bytes);
        inp.extend_from_slice(end_bytes);

        // Verify Err(..) is returned
        let res = match_markers(&inp);
        assert!(res.is_err());
    }

    #[test]
    fn match_markers_returns_err_with_multiple_start() {
        let mut inp: Vec<u8> = Vec::new();
        let start_bytes = b"Some random bytes.";
        let middle_bytes = b"More bytes.";
        let end_bytes = b"Final bytes.";
        inp.extend_from_slice(S_MARKER);
        inp.extend_from_slice(start_bytes);
        inp.extend_from_slice(S_MARKER);
        inp.extend_from_slice(middle_bytes);
        inp.extend_from_slice(E_MARKER);
        inp.extend_from_slice(end_bytes);

        // Verify Err(..) is returned
        let res = match_markers(&inp);
        assert!(res.is_err());
    }

    #[test]
    fn match_markers_returns_err_with_multiple_end() {
        let mut inp: Vec<u8> = Vec::new();
        let start_bytes = b"Some random bytes.";
        let middle_bytes = b"More bytes.";
        let end_bytes = b"Final bytes.";
        inp.extend_from_slice(start_bytes);
        inp.extend_from_slice(S_MARKER);
        inp.extend_from_slice(middle_bytes);
        inp.extend_from_slice(E_MARKER);
        inp.extend_from_slice(end_bytes);
        inp.extend_from_slice(E_MARKER);

        // Verify Err(..) is returned
        let res = match_markers(&inp);
        assert!(res.is_err());
    }
}

pub fn update_version_db(paths: &GlobalPaths) -> Result<()> {
    let mut config_file = load_mut_config_db(paths).with_context(|| {
        "`run_command_update_version_db` command failed to load configuration db."
    })?;

    #[cfg(feature = "selfupdate")]
    let juliaup_channel = match &config_file.self_data.juliaup_channel {
        Some(juliaup_channel) => juliaup_channel.to_string(),
        None => "release".to_string(),
    };

    // TODO Figure out how we can learn about the correctn Juliaup channel here
    #[cfg(not(feature = "selfupdate"))]
    let juliaup_channel = "release".to_string();

    let juliaupserver_base =
        get_juliaserver_base_url().with_context(|| "Failed to get Juliaup server base URL.")?;

    let dbversion_url_path = match juliaup_channel.as_str() {
        "release" => "juliaup/RELEASECHANNELDBVERSION",
        "releasepreview" => "juliaup/RELEASEPREVIEWCHANNELDBVERSION",
        "dev" => "juliaup/DEVCHANNELDBVERSION",
        _ => bail!(
            "Juliaup is configured to a channel named '{}' that does not exist.",
            &juliaup_channel
        ),
    };

    let dbversion_url = juliaupserver_base
        .join(dbversion_url_path)
        .with_context(|| {
            format!(
                "Failed to construct a valid url from '{}' and '{}'.",
                juliaupserver_base, dbversion_url_path
            )
        })?;

    let online_dbversion = download_juliaup_version(&dbversion_url.to_string())
        .with_context(|| "Failed to download current version db version.")?;

    let direct_download_etags = download_direct_download_etags(&mut config_file.data)?;

    for (channel, etag) in direct_download_etags {
        let channel_data = config_file.data.installed_channels.get(&channel).unwrap();

        match channel_data {
            JuliaupConfigChannel::DirectDownloadChannel {
                path,
                url,
                local_etag,
                server_etag: _,
                version,
            } => {
                config_file.data.installed_channels.insert(
                    channel,
                    JuliaupConfigChannel::DirectDownloadChannel {
                        path: path.clone(),
                        url: url.clone(),
                        local_etag: local_etag.clone(),
                        server_etag: etag,
                        version: version.clone(),
                    },
                );
            }
            _ => {}
        }
    }

    config_file.data.last_version_db_update = Some(chrono::Utc::now());

    save_config_db(&mut config_file).with_context(|| "Failed to save configuration file.")?;

    let bundled_dbversion = get_bundled_dbversion()
        .with_context(|| "Failed to determine the bundled version db version.")?;

    let local_dbversion = match std::fs::OpenOptions::new()
        .read(true)
        .open(&paths.versiondb)
    {
        Ok(file) => {
            let reader = BufReader::new(&file);

            if let Ok(versiondb) =
                serde_json::from_reader::<BufReader<&std::fs::File>, JuliaupVersionDB>(reader)
            {
                if let Ok(version) = semver::Version::parse(&versiondb.version) {
                    Some(version)
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(_) => None,
    };

    if online_dbversion > bundled_dbversion {
        if local_dbversion.is_none() || online_dbversion > local_dbversion.unwrap() {
            let onlineversiondburl = juliaupserver_base
                .join(&format!(
                    "juliaup/versiondb/versiondb-{}-{}.json",
                    online_dbversion,
                    get_juliaup_target()
                ))
                .with_context(|| "Failed to construct URL for version db download.")?;

            download_versiondb(&onlineversiondburl.to_string(), &paths.versiondb).with_context(
                || {
                    format!(
                        "Failed to download new version db from {}.",
                        onlineversiondburl
                    )
                },
            )?;
        }
    } else if local_dbversion.is_some() {
        // If the bundled version is up-to-date we can delete any cached version db json file
        let _ = std::fs::remove_file(&paths.versiondb);
    }

    Ok(())
}

#[cfg(windows)]
fn download_direct_download_etags(
    config_data: &mut JuliaupConfig,
) -> Result<Vec<(String, String)>> {
    use windows::core::HSTRING;
    use windows::Web::Http::HttpMethod;
    use windows::Web::Http::HttpRequestMessage;

    let http_client =
        windows::Web::Http::HttpClient::new().with_context(|| "Failed to create HttpClient.")?;

    let requests: Vec<_> = config_data
        .installed_channels
        .iter()
        .filter_map(|(channel_name, channel)| {
            if let JuliaupConfigChannel::DirectDownloadChannel {
                path: _,
                url,
                local_etag: _,
                server_etag: _,
                version: _,
            } = channel
            {
                let request_uri =
                    windows::Foundation::Uri::CreateUri(&windows::core::HSTRING::from(url))
                        .with_context(|| "Failed to convert url string to Uri.")
                        .unwrap();

                let request =
                    HttpRequestMessage::Create(&HttpMethod::Head().unwrap(), &request_uri).unwrap();

                let request = http_client.SendRequestAsync(&request).unwrap();

                Some((channel_name, request))
            } else {
                None
            }
        })
        .collect();

    let requests: Vec<_> = requests
        .into_iter()
        .map(|(channel_name, request)| {
            (
                channel_name.clone(),
                request
                    .get()
                    .unwrap()
                    .Headers()
                    .unwrap()
                    .Lookup(&HSTRING::from("etag"))
                    .unwrap()
                    .to_string(),
            )
        })
        .collect();

    Ok(requests)
}

#[cfg(not(windows))]
fn download_direct_download_etags(
    config_data: &mut JuliaupConfig,
) -> Result<Vec<(String, String)>> {
    let client = reqwest::blocking::Client::new();

    let requests: Vec<_> = config_data
        .installed_channels
        .iter()
        .filter_map(|(channel_name, channel)| {
            if let JuliaupConfigChannel::DirectDownloadChannel {
                path: _,
                url,
                local_etag: _,
                server_etag: _,
                version: _,
            } = channel
            {
                let etag = client
                    .head(url)
                    .send()
                    .unwrap()
                    .headers()
                    .get("etag")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();

                Some((channel_name.clone(), etag))
            } else {
                None
            }
        })
        .collect();

    Ok(requests)
}
