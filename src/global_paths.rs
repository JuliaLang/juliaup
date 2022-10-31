use std::path::PathBuf;
use anyhow::{Result,bail,anyhow, Context};
pub struct GlobalPaths {
    pub juliauphome: PathBuf,
    pub juliaupconfig: PathBuf,
    pub lockfile: PathBuf,
    pub juliaupselfhome: PathBuf,
    #[cfg(feature = "selfupdate")]
    pub juliaupselfconfig: PathBuf,
    #[cfg(feature = "selfupdate")]
    pub juliaupselfbin: PathBuf,
}

fn get_juliaup_home_path() -> Result<PathBuf> {
    let entry_sep = if std::env::consts::OS == "windows" {';'} else {':'};

    let path = match std::env::var("JULIA_DEPOT_PATH") {
        Ok(val) => {
            let path = PathBuf::from(val.to_string().split(entry_sep).next().unwrap()); // We can unwrap here because even when we split an empty string we should get a first element.

            if !path.is_absolute() {
                bail!("The `JULIA_DEPOT_PATH` environment variable contains a value that resolves to an an invalid path `{}`.", path.display());
            };

            path.join("juliaup")
        }
        Err(_) => {
            let path = dirs::home_dir()
            .ok_or(anyhow!(
                "Could not determine the path of the user home directory."
            ))?
            .join(".julia")
            .join("juliaup");

            if !path.is_absolute() {
                bail!(
                    "The system returned an invalid home directory path `{}`.",
                    path.display()
                );
            };

            path
        }
    };

    Ok(path)
}

fn get_juliaup_selfhome_path() -> Result<PathBuf> {
    let entry_sep = if std::env::consts::OS == "windows" {';'} else {':'};

    let selfpath = match std::env::var("JULIAUP_SELF_HOME") {
        Ok(val) => {
            let path = PathBuf::from(val.to_string().split(entry_sep).next().unwrap());
            if !path.is_absolute() { bail!("The `JULIAUP_SELF_HOME` environment variable contains a value that resolves to an an invalid path `{}`.", path.display()); };

            path
        }
        Err(_) => {
            let my_own_path = std::env::current_exe()
                .with_context(|| anyhow!("Could not determine the path of the running exe."))?;
            my_own_path
                .parent()
                .ok_or_else(|| anyhow!("Failed to get path of folder of own executable."))?
                .parent()
                .ok_or_else(|| anyhow!("Failed to get parent path of folder of own executable."))?
                .to_path_buf()
        }
    };

    Ok(selfpath)
}

pub fn get_paths() -> Result<GlobalPaths> {

    let juliauphome = get_juliaup_home_path()?;

    let juliaupconfig = juliauphome.join("juliaup.json");

    let lockfile = juliauphome.join(".juliaup-lock");

    let juliaupselfhome = get_juliaup_selfhome_path()?;

    #[cfg(feature = "selfupdate")]
    let juliaupselfbin = juliaselfhome.join("bin");

    #[cfg(feature = "selfupdate")]
    let juliaupselfconfig = juliaupselfhome
        .join("juliaupself.json");

    Ok(GlobalPaths {
        juliauphome,
        juliaupconfig,
        lockfile,
        juliaupselfhome,
        #[cfg(feature = "selfupdate")]
        juliaupselfconfig,
        #[cfg(feature = "selfupdate")]
        juliaupselfbin,
    })
}
