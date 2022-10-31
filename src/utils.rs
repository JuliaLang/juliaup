use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use std::path::PathBuf;
use url::Url;

pub fn get_juliaserver_base_url() -> Result<Url> {
    let base_url = if let Ok(val) = std::env::var("JULIAUP_SERVER") { 
        if val.ends_with("/") {val} else {format!("{}/", val)}
     } else {
        "https://julialang-s3.julialang.org".to_string() 
    };

    let parsed_url = Url::parse(&base_url)
        .with_context(|| format!("Failed to parse the value of JULIAUP_SERVER '{}' as a uri.", base_url))?;

    Ok(parsed_url)
}

pub fn get_bin_dir() -> Result<PathBuf> {
    let entry_sep = if std::env::consts::OS == "windows" {';'} else {':'};

    let path = match std::env::var("JULIAUP_SELF_HOME") {
        Ok(val) => {
            let path = PathBuf::from(val.to_string().split(entry_sep).next().unwrap()); // We can unwrap here because even when we split an empty string we should get a first element.

            if !path.is_absolute() {
                bail!("The `JULIAUP_SELF_HOME` environment variable contains a value that resolves to an an invalid path `{}`.", path.display());
            };

            path.join("bin")
        }
        Err(_) => {
            let path = std::env::current_exe()
                .with_context(|| "Could not determine the path of the running exe.")?
                .parent()
                .ok_or_else(|| anyhow!("Could not determine parent."))?
                .to_path_buf();

            path
        },
    };

    Ok(path)
}

pub fn get_arch() -> Result<String> {
    if std::env::consts::ARCH == "x86" {
        return Ok("x86".to_string());
    } else if std::env::consts::ARCH == "x86_64" {
        return Ok("x64".to_string());
    } else if std::env::consts::ARCH == "aarch64" {
        return Ok("aarch64".to_string());
    }

    bail!("Running on an unknown arch: {}.", std::env::consts::ARCH)
}

pub fn parse_versionstring(value: &String) -> Result<(String, Version)> {
    let parts: Vec<&str> = value.split('~').collect();

    if parts.len() > 2 {
        bail!(
            "`{}` is an invalid version specifier: multiple `~` characters are not allowed.",
            value
        );
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
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "x86_64" => "x64",
            "aarch64" => "aarch64",
            _ => ""
        };
        assert_eq!(p, arch);
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
        assert_eq!(v, Version::parse("1.1.1+0").unwrap());
    }
}
