use crate::global_paths::GlobalPaths;
use crate::operations::update_version_db;
use anyhow::{Context, Result};

#[cfg(feature = "selfupdate")]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db};
    use crate::operations::{download_extract_sans_parent, download_juliaup_version};
    use crate::utils::get_juliaserver_base_url;
    use crate::{get_juliaup_target, get_own_version};
    use anyhow::{anyhow, bail};

    update_version_db(paths).with_context(|| "Failed to update versions db.")?;

    let mut config_file = load_mut_config_db(paths)
        .with_context(|| "`selfupdate` command failed to load configuration db.")?;

    let juliaup_channel = match &config_file.self_data.juliaup_channel {
        Some(juliaup_channel) => juliaup_channel.to_string(),
        None => "release".to_string(),
    };

    let juliaupserver_base =
        get_juliaserver_base_url().with_context(|| "Failed to get Juliaup server base URL.")?;

    let version_url_path = match juliaup_channel.as_str() {
        "release" => "juliaup/RELEASECHANNELVERSION",
        "releasepreview" => "juliaup/RELEASEPREVIEWCHANNELVERSION",
        "dev" => "juliaup/DEVCHANNELVERSION",
        _ => bail!(
            "Juliaup is configured to a channel named '{}' that does not exist.",
            &juliaup_channel
        ),
    };

    eprintln!("Checking for self-updates");

    let version_url = juliaupserver_base.join(version_url_path).with_context(|| {
        format!(
            "Failed to construct a valid url from '{}' and '{}'.",
            juliaupserver_base, version_url_path
        )
    })?;

    let version = download_juliaup_version(&version_url.to_string())?;

    config_file.self_data.last_selfupdate = Some(chrono::Utc::now());

    save_config_db(&mut config_file).with_context(|| "Failed to save configuration file.")?;

    if version == get_own_version().unwrap() {
        eprintln!(
            "Juliaup unchanged on channel '{}' - {}",
            juliaup_channel, version
        );
    } else {
        let juliaup_target = get_juliaup_target();

        let juliaupserver_base =
            get_juliaserver_base_url().with_context(|| "Failed to get Juliaup server base URL.")?;

        let download_url_path =
            format!("juliaup/bin/juliaup-{}-{}.tar.gz", version, juliaup_target);

        let new_juliaup_url = juliaupserver_base
            .join(&download_url_path)
            .with_context(|| {
                format!(
                    "Failed to construct a valid url from '{}' and '{}'.",
                    juliaupserver_base, download_url_path
                )
            })?;

        eprintln!(
            "Found new version {} on channel {}.",
            version, juliaup_channel
        );

        download_extract_sans_parent(&new_juliaup_url.to_string(), &paths.juliaupselfbin, 0)?;
        eprintln!("Updated Juliaup to version {}.", version);
    }

    Ok(())
}

#[cfg(feature = "windowsstore")]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    use windows::{
        core::Interface,
        Win32::{System::Console::GetConsoleWindow, UI::Shell::IInitializeWithWindow},
    };

    update_version_db(paths).with_context(|| "Failed to update versions db.")?;

    let update_manager = windows::Services::Store::StoreContext::GetDefault()
        .with_context(|| "Failed to get the store context.")?;

    let interop: IInitializeWithWindow = update_manager
        .cast()
        .with_context(|| "Failed to cast the store context to IInitializeWithWindow.")?;

    unsafe {
        let x = GetConsoleWindow();

        interop
            .Initialize(x)
            .with_context(|| "Call to IInitializeWithWindow.Initialize failed.")?;
    }

    let updates = update_manager
        .GetAppAndOptionalStorePackageUpdatesAsync()
        .with_context(|| "Call to GetAppAndOptionalStorePackageUpdatesAsync failed.")?
        .get()
        .with_context(|| {
            "get on the return of GetAppAndOptionalStorePackageUpdatesAsync failed."
        })?;

    if updates
        .Size()
        .with_context(|| "Call to Size on update results failed.")?
        > 0
    {
        println!("An update is available.");

        let download_operation = update_manager
            .RequestDownloadAndInstallStorePackageUpdatesAsync(&updates)
            .with_context(|| "Call to RequestDownloadAndInstallStorePackageUpdatesAsync failed.")?;

        download_operation.get().with_context(|| {
            "get on result from RequestDownloadAndInstallStorePackageUpdatesAsync failed."
        })?;
        // This code will not be reached if the user opts to install updates
    } else {
        println!("No updates available.");
    }

    Ok(())
}

#[cfg(not(any(feature = "windowsstore", feature = "selfupdate")))]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    update_version_db(paths).with_context(|| "Failed to update versions db.")?;
    Ok(())
}
