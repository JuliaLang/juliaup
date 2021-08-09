use crate::config_file::JuliaupConfigChannel;
use crate::operations::install_version;
use crate::versions_file::JuliaupVersionDB;
use crate::config_file::JuliaupConfig;
use crate::operations::garbage_collect_versions;
use crate::config_file::{load_config_db, save_config_db};
use crate::versions_file::load_versions_db;
use anyhow::{Context, Result,anyhow};

async fn update_channel(config_db: &mut JuliaupConfig, channel: &String, version_db: &JuliaupVersionDB) -> Result<()> {
    let current_version = 
        config_db.installed_channels.get(channel).ok_or(anyhow!("asdf"))?;

    let should_version = version_db.available_channels.get(channel).ok_or(anyhow!("asdf"))?;

	if should_version.version != current_version.version {
		install_version(&should_version.version, config_db, version_db)
            .await
            .with_context(|| format!("Failed to install '{}' while updating channel '{}'.", should_version.version, channel))?;

        config_db.installed_channels.insert(
            channel.clone(),
            JuliaupConfigChannel {
                version: should_version.version.clone(),
            },
        );
    }

    Ok(())
}

pub async fn run_command_update(channel: Option<String>) -> Result<()> {
    let version_db =
        load_versions_db().with_context(|| "`update` command failed to load versions db.")?;

    let mut config_data = load_config_db()
        .with_context(|| "`update` command failed to load configuration file.")?;

    match channel {
        None => {
            for (k,_) in config_data.installed_channels.clone() {
                // TODO Check for linked channel
                // if haskey(i[2], "Version") {
                update_channel(&mut config_data, &k, &version_db).await?;
                // }
            }

        },
        Some(channel) => {
            if !config_data.installed_channels.contains_key(&channel) {
                return Err(anyhow!("'{}' cannot be updated because it is currently not installed.", channel));
            }

            // TODO Check for alinked channel
            update_channel(&mut config_data, &channel, &version_db).await?;
        }
    };

    garbage_collect_versions(&mut config_data)?;

    save_config_db(&config_data)
        .with_context(|| "`update` command failed to save configuration db.")?;

    Ok(())
}
