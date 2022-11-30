use std::path::PathBuf;
use anyhow::{Result,bail,anyhow};
#[cfg(feature = "selfupdate")]
use anyhow::Context;
use crate::get_juliaup_target;
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
    let entry_sep = if std::env::consts::OS == "windows" {';'} else {':'};

    match std::env::var("JULIA_DEPOT_PATH") {
        Ok(val) => {
            // Note: Docs on JULIA_DEPOT_PATH states that if it exists but is empty, it should
            // be interpreted as an empty array. This code instead interprets it as a 1-element
            // array of the default path.
            // We interpret it differently, because while Julia may work without a DEPOT_PATH,
            // Juliaup does not currently, since it must check for new versions.
            let mut paths = Vec::<PathBuf>::new();
            for segment in val.split(entry_sep) {
                // Empty segments resolve to the default first value of
                // DEPOT_PATH
                let segment_path = if segment.is_empty() {
                    get_default_juliaup_home_path()?
                } else {
                    PathBuf::from(segment.to_string())
                };
                paths.push(segment_path);
            }

            // First, we try to find any directory which already is initialized by
            // Juliaup.
            for path in paths.iter() {
                let subpath = path.join("juliaup").join("juliaup.json");
                if subpath.is_file() {
                    return Ok(path.join("juliaup"));
                }
            }
            // If such a file does not exist, we pick the first segment in JULIA_DEPOT_PATH.
            // This is guaranteed to be nonempty due to the properties of str::split.
            let first_path = paths.iter().next().unwrap();
            if !first_path.is_dir() {
                bail!("The `JULIA_DEPOT_PATH` environment variable contains a value that resolves to an invalid directory `{}`.", first_path.display());
            }
            return Ok(first_path.join("juliaup"));
        }
        Err(_) => return get_default_juliaup_home_path(),
    }
}

/// Return ~/.julia/juliaup, if such a directory can be found
fn get_default_juliaup_home_path() -> Result<PathBuf> {
    let path = dirs::home_dir()
        .ok_or_else(|| anyhow!(
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
    Ok(path)
}

pub fn get_paths() -> Result<GlobalPaths> {
    let juliauphome = get_juliaup_home_path()?;

    #[cfg(feature = "selfupdate")]
    let my_own_path = std::env::current_exe()
        .with_context(|| "Could not determine the path of the running exe.")?;

    #[cfg(feature = "selfupdate")]
    let juliaupselfbin = my_own_path.parent()
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
    let juliaupselfconfig = juliaupselfhome
        .join("juliaupself.json");

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
