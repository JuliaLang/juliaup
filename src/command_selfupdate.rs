use crate::global_paths::GlobalPaths;
use crate::operations::update_version_db;
#[cfg(feature = "selfupdate")]
use crate::utils::{print_juliaup_style, JuliaupMessageType};
use anyhow::{Context, Result};

#[cfg(feature = "selfupdate")]
struct UpdateNotification {
    id: &'static str,
    message: &'static str,
}

#[cfg(feature = "selfupdate")]
static NOTIFICATIONS: &[UpdateNotification] = &[
    UpdateNotification {
        id: "v1.20.0-manifest-detect",
        message: "Note: As of juliaup v1.20.0 launching julia into a specific environment will automatically detect if a manifest exists and launch the julia version that the manifest was resolved by. Disable this via `juliaup config manifestversiondetect false`",
    },
];

#[cfg(feature = "selfupdate")]
fn display_update_notification(old_version: &str, new_version: &str) {
    print_juliaup_style(
        "Updated",
        &format!(
            "juliaup to {}: https://github.com/JuliaLang/juliaup/releases/tag/v{}",
            new_version, new_version
        ),
        JuliaupMessageType::Success,
    );
}

#[cfg(feature = "selfupdate")]
fn get_notifications_to_show(shown_notifications: &[String]) -> Vec<&'static UpdateNotification> {
    NOTIFICATIONS
        .iter()
        .filter(|notif| !shown_notifications.contains(&notif.id.to_string()))
        .collect()
}

#[cfg(feature = "selfupdate")]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    use crate::config_file::{load_mut_config_db, save_config_db};
    use crate::operations::{download_extract_sans_parent, download_juliaup_version};
    use crate::utils::get_juliaserver_base_url;
    use crate::{get_juliaup_target, get_own_version};
    use anyhow::{anyhow, bail};

    update_version_db(&None, paths).with_context(|| "Failed to update versions db.")?;

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

    let version = download_juliaup_version(version_url.as_ref())?;

    config_file.self_data.last_selfupdate = Some(chrono::Utc::now());

    let old_version = get_own_version().unwrap();
    let is_upgrade = version != old_version;
    let should_show_notification = is_upgrade
        && config_file
            .self_data
            .last_update_notification_version
            .as_ref()
            .map_or(true, |last_notified| last_notified != &version);

    save_config_db(&mut config_file).with_context(|| "Failed to save configuration file.")?;

    if version == old_version {
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

        let my_own_path = std::env::current_exe()
            .with_context(|| "Could not determine the path of the running exe.")?;

        let my_own_folder = my_own_path
            .parent()
            .ok_or_else(|| anyhow!("Could not determine parent."))?;

        eprintln!(
            "Found new version {} on channel {}.",
            version, juliaup_channel
        );

        download_extract_sans_parent(new_juliaup_url.as_ref(), my_own_folder, 0)?;
        eprintln!("Updated Juliaup to version {}.", version);

        if should_show_notification {
            display_update_notification(&old_version, &version);

            let mut config_file = load_mut_config_db(paths)
                .with_context(|| "Failed to load configuration db for notification update.")?;

            // Get notifications that should be shown for this upgrade
            let notifications_to_show =
                get_notifications_to_show(&config_file.self_data.shown_notifications);

            // Display custom notifications
            if !notifications_to_show.is_empty() {
                for notif in &notifications_to_show {
                    eprintln!("{}", notif.message);
                    eprintln!();
                }

                // Mark these notifications as shown
                for notif in notifications_to_show {
                    config_file
                        .self_data
                        .shown_notifications
                        .push(notif.id.to_string());
                }
            }

            config_file.self_data.last_update_notification_version = Some(version.clone());
            save_config_db(&mut config_file)
                .with_context(|| "Failed to save notification version.")?;
        }
    }

    Ok(())
}

#[cfg(feature = "windowsstore")]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    use windows::{
        core::Interface,
        Win32::{System::Console::GetConsoleWindow, UI::Shell::IInitializeWithWindow},
    };

    update_version_db(&None, paths).with_context(|| "Failed to update versions db.")?;

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
        .join()
        .with_context(|| {
            "get on the return of GetAppAndOptionalStorePackageUpdatesAsync failed."
        })?;

    if updates
        .Size()
        .with_context(|| "Call to Size on update results failed.")?
        > 0
    {
        eprintln!("An update is available.");

        let download_operation = update_manager
            .RequestDownloadAndInstallStorePackageUpdatesAsync(&updates)
            .with_context(|| "Call to RequestDownloadAndInstallStorePackageUpdatesAsync failed.")?;

        download_operation.join().with_context(|| {
            "get on result from RequestDownloadAndInstallStorePackageUpdatesAsync failed."
        })?;
        // This code will not be reached if the user opts to install updates
    } else {
        eprintln!("No updates available.");
    }

    Ok(())
}

#[cfg(not(any(feature = "windowsstore", feature = "selfupdate")))]
pub fn run_command_selfupdate(paths: &GlobalPaths) -> Result<()> {
    update_version_db(&None, paths).with_context(|| "Failed to update versions db.")?;
    Ok(())
}
