use crate::get_juliaup_target;
#[cfg(feature = "selfupdate")]
use crate::utils::get_juliaup_path;
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
pub struct GlobalPaths {
    pub juliauphome: PathBuf,
    pub juliaupconfig: PathBuf,
    pub lockfile: PathBuf,
    pub versiondb: PathBuf,
    #[cfg(feature = "selfupdate")]
    pub juliaupselfhome: PathBuf,
    #[cfg(feature = "selfupdate")]
    pub juliaupselfconfig: PathBuf,
    #[cfg(feature = "selfupdate")]
    pub juliaupselfbin: PathBuf,
}

fn get_juliaup_home_path() -> Result<PathBuf> {
    match std::env::var("JULIAUP_DEPOT_PATH") {
        Ok(val) => {
            let val = val.trim();

            if val.is_empty() {
                return get_default_juliaup_home_path();
            } else {
                let path = PathBuf::from(val);

                if !path.is_absolute() {
                    return Err(anyhow!("The current value of '{}' for the environment variable JULIAUP_DEPOT_PATH is not an absolute path.", val));
                } else {
                    return Ok(PathBuf::from(val).join("juliaup"));
                }
            }
        }
        Err(_) => return get_default_juliaup_home_path(),
    }
}

/// Return ~/.julia/juliaup, if such a directory can be found
fn get_default_juliaup_home_path() -> Result<PathBuf> {
    let path = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine the path of the user home directory."))?
        .join(".julia")
        .join("juliaup");

    if !path.is_absolute() {
        bail!(
            "The system returned an invalid home directory path `{}`.",
            path.display()
        );
    };
    Ok(path)
}

pub fn get_paths() -> Result<GlobalPaths> {
    let juliauphome = get_juliaup_home_path()?;

    #[cfg(feature = "selfupdate")]
    let my_own_path = get_juliaup_path()?;

    #[cfg(feature = "selfupdate")]
    let juliaupselfbin = my_own_path
        .parent()
        .ok_or_else(|| anyhow!("Could not determine parent."))?
        .to_path_buf();

    let juliaupconfig = juliauphome.join("juliaup.json");

    let versiondb = juliauphome.join(format!("versiondb-{}.json", get_juliaup_target()));

    let lockfile = juliauphome.join(".juliaup-lock");

    #[cfg(feature = "selfupdate")]
    let juliaupselfhome = my_own_path
        .parent()
        .ok_or_else(|| anyhow!("Failed to get path of folder of own executable."))?
        .parent()
        .ok_or_else(|| anyhow!("Failed to get parent path of folder of own executable."))?
        .to_path_buf();

    #[cfg(feature = "selfupdate")]
    let juliaupselfconfig = juliaupselfhome.join("juliaupself.json");

    Ok(GlobalPaths {
        juliauphome,
        juliaupconfig,
        lockfile,
        versiondb,
        #[cfg(feature = "selfupdate")]
        juliaupselfhome,
        #[cfg(feature = "selfupdate")]
        juliaupselfconfig,
        #[cfg(feature = "selfupdate")]
        juliaupselfbin,
    })
}
