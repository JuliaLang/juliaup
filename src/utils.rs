use anyhow::{anyhow, Context, Result};
use semver::Version;
use std::path::PathBuf;

pub fn get_juliaup_home_path() -> Result<PathBuf> {
    let entry_sep = if std::env::consts::OS == "windows" {';'} else {':'};

    let path = match std::env::var("JULIA_DEPOT_PATH") {
        Ok(val) => {
            let path = PathBuf::from(val.to_string().split(entry_sep).next().unwrap()); // We can unwrap here because even when we split an empty string we should get a first element.

            if !path.is_absolute() {
                return Err(anyhow!("The `JULIA_DEPOT_PATH` environment variable contains a value that resolves to an an invalid path `{}`.", path.display()));
            };

            path
        }
        Err(_) => {
    let path = dirs::home_dir()
        .ok_or(anyhow!(
            "Could not determine the path of the user home directory."
        ))?
        .join(".julia")
        .join("juliaup");

            if !path.is_absolute() {
                return Err(anyhow!(
                    "The system returned an invalid home directory path `{}`.",
                    path.display()
                ));
            };

            path
        }
    };

    Ok(path)
}

pub fn get_juliaupconfig_path() -> Result<PathBuf> {
    let path = get_juliaup_home_path()?.join("juliaup.json");

    Ok(path)
}

pub fn get_arch() -> Result<String> {
    if std::env::consts::ARCH == "x86" {
        return Ok("x86".to_string());
    } else if std::env::consts::ARCH == "x86_64" {
        return Ok("x64".to_string());
    }

    Err(anyhow!("Running on an unknown arch."))
}

pub fn parse_versionstring(value: &String) -> Result<(String, Version)> {
    let parts: Vec<&str> = value.split('~').collect();

    if parts.len() > 2 {
        return Err(anyhow!(format!(
            "`{}` is an invalid version specifier: multiple `~` characters are not allowed.",
            value
        )));
    }

    let version = parts[0];
    let platform = if parts.len() == 2 { parts[1].to_string() } else { get_arch()? };

    let version = Version::parse(version).with_context(|| {
        format!(
            "'{}' was determined to be the semver part of '{}', but failed to parse as a version.",
            version, value
        )
    })?;

    Ok((platform.to_string(), version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versionstring() {
        let s = "1.1.1";
        let (p,v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x64");
        assert_eq!(v, Version::new(1, 1, 1));

        let s = "1.1.1~x86";
        let (p,v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x86");
        assert_eq!(v, Version::new(1, 1, 1));

        let s = "1.1.1~x64";
        let (p,v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x64");
        assert_eq!(v, Version::new(1, 1, 1));

        let s = "1.1.1+0~x64";
        let (p,v) = parse_versionstring(&s.to_owned()).unwrap();
        assert_eq!(p, "x64");
        assert_eq!(v, Version::new(1, 1, 1));
    }
}
