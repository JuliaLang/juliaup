// use std::fs::File;
// use std::io::BufReader;
// use crate::utils::get_juliaup_home_path;
use crate::jsonstructs_versionsdb::{
    JuliaupVersionDB, JuliaupVersionDBChannel, JuliaupVersionDBVersion,
};
use crate::utils::get_juliaserver_base_url;
use anyhow::Result;
use itertools::Itertools;
use semver;
use semver::Prerelease;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use ureq;

pub type JsonVersion = HashMap<String, JsonVersionValue>;

#[derive(Serialize, Deserialize)]
pub struct JsonVersionValue {
    #[serde(rename = "files")]
    files: Vec<JFile>,

    #[serde(rename = "stable")]
    stable: bool,
}

#[derive(Serialize, Deserialize)]
pub struct JFile {
    #[serde(rename = "url")]
    url: String,

    #[serde(rename = "triplet")]
    triplet: Triplet,

    #[serde(rename = "kind")]
    kind: Kind,

    #[serde(rename = "arch")]
    arch: Arch,

    #[serde(rename = "sha256")]
    sha256: String,

    #[serde(rename = "size")]
    size: i64,

    #[serde(rename = "version")]
    version: String,

    #[serde(rename = "os")]
    os: Os,

    #[serde(rename = "extension")]
    extension: Extension,

    #[serde(rename = "asc")]
    asc: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum Arch {
    #[serde(rename = "aarch64")]
    Aarch64,

    #[serde(rename = "armv7l")]
    Armv7L,

    #[serde(rename = "i686")]
    I686,

    #[serde(rename = "powerpc64le")]
    Powerpc64Le,

    #[serde(rename = "x86_64")]
    X8664,
}

#[derive(Serialize, Deserialize)]
pub enum Extension {
    #[serde(rename = "dmg")]
    Dmg,

    #[serde(rename = "exe")]
    Exe,

    #[serde(rename = "tar.gz")]
    TarGz,

    #[serde(rename = "zip")]
    Zip,
}

#[derive(Serialize, Deserialize)]
pub enum Kind {
    #[serde(rename = "archive")]
    Archive,

    #[serde(rename = "installer")]
    Installer,
}

#[derive(Serialize, Deserialize)]
pub enum Os {
    #[serde(rename = "freebsd")]
    Freebsd,

    #[serde(rename = "linux")]
    Linux,

    #[serde(rename = "mac")]
    Mac,

    #[serde(rename = "winnt")]
    Winnt,
}

#[derive(Serialize, Deserialize)]
pub enum Triplet {
    #[serde(rename = "aarch64-apple-darwin14")]
    Aarch64AppleDarwin14,

    #[serde(rename = "aarch64-linux-gnu")]
    Aarch64LinuxGnu,

    #[serde(rename = "armv7l-linux-gnueabihf")]
    Armv7LLinuxGnueabihf,

    #[serde(rename = "i686-linux-gnu")]
    I686LinuxGnu,

    #[serde(rename = "i686-w64-mingw32")]
    I686W64Mingw32,

    #[serde(rename = "powerpc64le-linux-gnu")]
    Powerpc64LeLinuxGnu,

    #[serde(rename = "x86_64-apple-darwin14")]
    X8664AppleDarwin14,

    #[serde(rename = "x86_64-linux-gnu")]
    X8664LinuxGnu,

    #[serde(rename = "x86_64-linux-musl")]
    X8664LinuxMusl,

    #[serde(rename = "x86_64-unknown-freebsd11.1")]
    X8664UnknownFreebsd111,

    #[serde(rename = "x86_64-w64-mingw32")]
    X8664W64Mingw32,
}

fn channel_names(v: &Version, target_os: &str, target_arch: &str) -> Vec<(String, String)> {
    match (target_os, target_arch) {
        (_, "x86_64") => vec![
            (format!("{}", v), format!("{}+0.x64", v)),
            (format!("{}~x64", v), format!("{}+0.x64", v)),
            (format!("{}~x86", v), format!("{}+0.x86", v)),
        ],
        (_, "x86") => vec![
            (format!("{}", v), format!("{}+0.x86", v)),
            (format!("{}~x86", v), format!("{}+0.x86", v)),
        ],
        ("windows", "aarch64") => vec![
            (format!("{}", v), format!("{}+0.x86", v)),
            (format!("{}~x64", v), format!("{}+0.x64", v)),
            (format!("{}~x86", v), format!("{}+0.x86", v)),
        ],
        ("linux", "aarch64") => vec![
            (format!("{}", v), format!("{}+0.aarch64", v)),
            (format!("{}~aarch64", v), format!("{}+0.aarch64", v)),
        ],
        ("macos", "aarch64") => vec![],
        (_, _) => panic!("Building on this platform is currently not supported."),
    }
}

fn major_channel_names(v: &Version, target_os: &str, target_arch: &str) -> Vec<(String, String)> {
    match (target_os, target_arch) {
        (_, "x86_64") => vec![
            (format!("{}", v.major), format!("{}+0.x64", v)),
            (format!("{}~x64", v.major), format!("{}+0.x64", v)),
            (format!("{}~x86", v.major), format!("{}+0.x86", v)),
        ],
        (_, "x86") => vec![
            (format!("{}", v.major), format!("{}+0.x86", v)),
            (format!("{}~x86", v.major), format!("{}+0.x86", v)),
        ],
        ("windows", "aarch64") => vec![
            (format!("{}", v.major), format!("{}+0.x86", v)),
            (format!("{}~x64", v.major), format!("{}+0.x64", v)),
            (format!("{}~x86", v.major), format!("{}+0.x86", v)),
        ],
        ("linux", "aarch64") => vec![
            (format!("{}", v.major), format!("{}+0.aarch64", v)),
            (format!("{}~aarch64", v.major), format!("{}+0.aarch64", v)),
        ],
        ("macos", "aarch64") => vec![],
        (_, _) => panic!("Building on this platform is currently not supported."),
    }
}
fn minor_channel_names(v: &Version, target_os: &str, target_arch: &str) -> Vec<(String, String)> {
    match (target_os, target_arch) {
        (_, "x86_64") => vec![
            (format!("{}.{}", v.major, v.minor), format!("{}+0.x64", v)),
            (
                format!("{}.{}~x64", v.major, v.minor),
                format!("{}+0.x64", v),
            ),
            (
                format!("{}.{}~x86", v.major, v.minor),
                format!("{}+0.x86", v),
            ),
        ],
        (_, "x86") => vec![
            (format!("{}.{}", v.major, v.minor), format!("{}+0.x86", v)),
            (
                format!("{}.{}~x86", v.major, v.minor),
                format!("{}+0.x86", v),
            ),
        ],
        ("windows", "aarch64") => vec![
            (format!("{}.{}", v.major, v.minor), format!("{}+0.x86", v)),
            (
                format!("{}.{}~x64", v.major, v.minor),
                format!("{}+0.x64", v),
            ),
            (
                format!("{}.{}~x86", v.major, v.minor),
                format!("{}+0.x86", v),
            ),
        ],
        ("linux", "aarch64") => vec![
            (
                format!("{}.{}", v.major, v.minor),
                format!("{}+0.aarch64", v),
            ),
            (
                format!("{}.{}~aarch64", v.major, v.minor),
                format!("{}+0.aarch64", v),
            ),
        ],
        ("macos", "aarch64") => vec![],
        (_, _) => panic!("Building on this platform is currently not supported."),
    }
}

fn release_channel_names(
    target_os: &str,
    target_arch: &str,
    release_version: &Version,
    beta_version: &Version,
    lts_version: &Version,
    rc_version: &Version,
) -> Vec<(String, String)> {
    match (target_os, target_arch) {
        (_, "x86_64") => vec![
            ("release".to_string(), format!("{}+0.x64", release_version)),
            (
                "release~x64".to_string(),
                format!("{}+0.x64", release_version),
            ),
            (
                "release~x86".to_string(),
                format!("{}+0.x86", release_version),
            ),
            ("lts".to_string(), format!("{}+0.x64", lts_version)),
            ("lts~x64".to_string(), format!("{}+0.x64", lts_version)),
            ("lts~x86".to_string(), format!("{}+0.x86", lts_version)),
            ("beta".to_string(), format!("{}+0.x64", beta_version)),
            ("beta~x64".to_string(), format!("{}+0.x64", beta_version)),
            ("beta~x86".to_string(), format!("{}+0.x86", beta_version)),
            ("rc".to_string(), format!("{}+0.x64", rc_version)),
            ("rc~x64".to_string(), format!("{}+0.x64", rc_version)),
            ("rc~x86".to_string(), format!("{}+0.x86", rc_version)),
        ],
        (_, "x86") => vec![
            ("release".to_string(), format!("{}+0.x86", release_version)),
            (
                "release~x86".to_string(),
                format!("{}+0.x86", release_version),
            ),
            ("lts".to_string(), format!("{}+0.x86", lts_version)),
            ("lts~x86".to_string(), format!("{}+0.x86", lts_version)),
            ("beta".to_string(), format!("{}+0.x86", beta_version)),
            ("beta~x86".to_string(), format!("{}+0.x86", beta_version)),
            ("rc".to_string(), format!("{}+0.x86", rc_version)),
            ("rc~x86".to_string(), format!("{}+0.x86", rc_version)),
        ],
        ("windows", "aarch64") => vec![
            ("release".to_string(), format!("{}+0.x64", release_version)),
            (
                "release~x64".to_string(),
                format!("{}+0.x64", release_version),
            ),
            ("lts".to_string(), format!("{}+0.x64", lts_version)),
            ("lts~x64".to_string(), format!("{}+0.x64", lts_version)),
            ("beta".to_string(), format!("{}+0.x64", beta_version)),
            ("beta~x64".to_string(), format!("{}+0.x64", beta_version)),
            ("rc".to_string(), format!("{}+0.x64", rc_version)),
            ("rc~x64".to_string(), format!("{}+0.x64", rc_version)),
        ],
        ("linux", "aarch64") => vec![
            (
                "release".to_string(),
                format!("{}+0.aarch64", release_version),
            ),
            (
                "release~aarch64".to_string(),
                format!("{}+0.aarch64", release_version),
            ),
            ("lts".to_string(), format!("{}+0.aarch64", lts_version)),
            (
                "lts~aarch64".to_string(),
                format!("{}+0.aarch64", lts_version),
            ),
            ("beta".to_string(), format!("{}+0.aarch64", beta_version)),
            (
                "beta~aarch64".to_string(),
                format!("{}+0.aarch64", beta_version),
            ),
            ("rc".to_string(), format!("{}+0.aarch64", rc_version)),
            (
                "rc~aarch64".to_string(),
                format!("{}+0.aarch64", rc_version),
            ),
        ],
        ("macos", "aarch64") => vec![
            (
                "release".to_string(),
                format!("{}+0.aarch64", release_version),
            ),
            (
                "release~x64".to_string(),
                format!("{}+0.x64", release_version),
            ),
            (
                "release~aarch64".to_string(),
                format!("{}+0.aarch64", release_version),
            ),
            ("lts".to_string(), format!("{}+0.x64", lts_version)),
            ("lts~x64".to_string(), format!("{}+0.x64", lts_version)),
            ("beta".to_string(), format!("{}+0.aarch64", beta_version)),
            ("beta~x64".to_string(), format!("{}+0.x64", beta_version)),
            (
                "beta~aarch64".to_string(),
                format!("{}+0.aarch64", beta_version),
            ),
            ("rc".to_string(), format!("{}+0.aarch64", rc_version)),
            ("rc~x64".to_string(), format!("{}+0.x64", rc_version)),
            (
                "rc~aarch64".to_string(),
                format!("{}+0.aarch64", rc_version),
            ),
        ],
        (_, _) => panic!("Building on this platform is currently not supported."),
    }
}

fn os_version_names(v: &Version, target_os: &str, target_arch: &str) -> Vec<(String, String)> {
    match (target_os, target_arch) {
        ("windows", "x86_64") => vec![
            (
                format!("{}+0.x64", v),
                format!(
                    "bin/winnt/x64/{}.{}/julia-{}-win64.tar.gz",
                    v.major, v.minor, v
                ),
            ),
            (
                format!("{}+0.x86", v),
                format!(
                    "bin/winnt/x86/{}.{}/julia-{}-win32.tar.gz",
                    v.major, v.minor, v
                ),
            ),
        ],
        ("windows", "x86") => vec![(
            format!("{}+0.x86", v),
            format!(
                "bin/winnt/x86/{}.{}/julia-{}-win32.tar.gz",
                v.major, v.minor, v
            ),
        )],
        ("linux", "x86_64") => vec![
            (
                format!("{}+0.x64", v),
                format!(
                    "bin/linux/x64/{}.{}/julia-{}-linux-x86_64.tar.gz",
                    v.major, v.minor, v
                ),
            ),
            (
                format!("{}+0.x86", v),
                format!(
                    "bin/linux/x86/{}.{}/julia-{}-linux-i686.tar.gz",
                    v.major, v.minor, v
                ),
            ),
        ],
        ("linux", "x86") => vec![(
            format!("{}+0.x86", v),
            format!(
                "bin/linux/x86/{}.{}/julia-{}-linux-i686.tar.gz",
                v.major, v.minor, v
            ),
        )],
        ("linux", "aarch64") => vec![(
            format!("{}+0.aarch64", v),
            format!(
                "bin/linux/aarch64/{}.{}/julia-{}-linux-aarch64.tar.gz",
                v.major, v.minor, v
            ),
        )],
        ("macos", "x86_64") => vec![(
            format!("{}+0.x64", v),
            format!(
                "bin/mac/x64/{}.{}/julia-{}-mac64.tar.gz",
                v.major, v.minor, v
            ),
        )],
        ("macos", "aarch64") => vec![(
            format!("{}+0.x64", v),
            format!(
                "bin/mac/x64/{}.{}/julia-{}-mac64.tar.gz",
                v.major, v.minor, v
            ),
        )],
        // ...
        (_, _) => panic!("Building on this platform is currently not supported."),
    }
}

pub fn load_versions_db() -> Result<JuliaupVersionDB> {
    let lts_version = Version::parse("1.6.7")?;

    let mut original_available_versions: Vec<Version> = Vec::new();
    let up_server_json_url = get_juliaserver_base_url()?.join("./bin/versions.json");
    let json_versions: JsonVersion = ureq::get(up_server_json_url?.as_str())
        .call()?
        .into_json()?;
    for (k, _) in json_versions {
        original_available_versions.push(Version::parse(k.as_str())?);
    }
    let mut db = JuliaupVersionDB {
        available_versions: HashMap::new(),
        available_channels: HashMap::new(),
    };
    let target_os = env::consts::OS;
    let target_arch = env::consts::ARCH;

    for v in &original_available_versions {
        let name_urls = os_version_names(v, target_os, target_arch);
        for (version_name, version_url) in name_urls {
            db.available_versions.insert(
                version_name,
                JuliaupVersionDBVersion {
                    url_path: version_url,
                },
            );
        }

        if target_os == "macos"
            && target_arch == "aarch64"
            && v >= &Version::new(1, 7, 0)
            && v != &Version::new(1, 7, 3)
        {
            db.available_versions.insert(
                format!("{}+0.aarch64", v),
                JuliaupVersionDBVersion {
                    url_path: format!(
                        "bin/mac/aarch64/{}.{}/julia-{}-macaarch64.tar.gz",
                        v.major, v.minor, v
                    ),
                },
            );
        }

        let channel_names = channel_names(v, target_os, target_arch);
        for (channel_key, channel_val) in channel_names {
            db.available_channels.insert(
                channel_key,
                JuliaupVersionDBChannel {
                    version: channel_val,
                },
            );
        }
        if target_arch == "aarch64" && target_os == "macos" {
            if v >= &(Version {
                major: 1,
                minor: 8,
                patch: 0,
                pre: Prerelease::new("rc3").unwrap(),
                build: semver::BuildMetadata::EMPTY,
            }) {
                db.available_channels.insert(
                    format!("{}", v),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            } else {
                db.available_channels.insert(
                    format!("{}", v),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.x64", v),
                    },
                );
            }
            db.available_channels.insert(
                format!("{}~x64", v),
                JuliaupVersionDBChannel {
                    version: format!("{}+0.x64", v),
                },
            );
            if v >= &Version::new(1, 7, 0) && v != &Version::new(1, 7, 3) {
                db.available_channels.insert(
                    format!("{}~aarch64", v),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            }
        }
    }

    let minor_channels = &original_available_versions
        .iter()
        .into_grouping_map_by(|&v| (v.major, v.minor))
        .max();

    let major_channels = &original_available_versions
        .iter()
        .filter(|&v| v.pre == semver::Prerelease::EMPTY)
        .into_grouping_map_by(|&v| v.major)
        .max();

    for ((major, minor), v) in minor_channels {
        let minor_channel_names = minor_channel_names(v, target_os, target_arch);
        for (channel_key, channel_val) in minor_channel_names {
            db.available_channels.insert(
                channel_key,
                JuliaupVersionDBChannel {
                    version: channel_val,
                },
            );
        }
        if target_arch == "aarch64" && target_os == "macos" {
            if v >= &&(Version {
                major: 1,
                minor: 8,
                patch: 0,
                pre: Prerelease::new("rc3").unwrap(),
                build: semver::BuildMetadata::EMPTY,
            }) {
                db.available_channels.insert(
                    format!("{}.{}", major, minor),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            } else {
                db.available_channels.insert(
                    format!("{}.{}", major, minor),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.x64", v),
                    },
                );
            }
            db.available_channels.insert(
                format!("{}.{}~x64", major, minor),
                JuliaupVersionDBChannel {
                    version: format!("{}+0.x64", v),
                },
            );

            if v == &&Version::new(1, 7, 3) {
                // This is a hack because there is no aarch64 for 1.7.3, so we fall back to the 1.7.2 version
                db.available_channels.insert(
                    format!("{}.{}~aarch64", major, minor),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", Version::new(1, 7, 2)),
                    },
                );
            } else if v >= &&Version::new(1, 7, 0) {
                db.available_channels.insert(
                    format!("{}.{}~aarch64", major, minor),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            }
        }
    }

    for (major, v) in major_channels {
        let major_channel_names = major_channel_names(v, target_os, target_arch);
        for (channel_key, channel_val) in major_channel_names {
            db.available_channels.insert(
                channel_key,
                JuliaupVersionDBChannel {
                    version: channel_val,
                },
            );
        }
        if target_arch == "aarch64" && target_os == "macos" {
            if v >= &&(Version {
                major: 1,
                minor: 8,
                patch: 0,
                pre: Prerelease::new("rc3").unwrap(),
                build: semver::BuildMetadata::EMPTY,
            }) {
                db.available_channels.insert(
                    format!("{}", major),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            } else {
                db.available_channels.insert(
                    format!("{}", major),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.x64", v),
                    },
                );
            }
            db.available_channels.insert(
                format!("{}~x64", major),
                JuliaupVersionDBChannel {
                    version: format!("{}+0.x64", v),
                },
            );

            if v == &&Version::new(1, 7, 3) {
                // This is a hack because there is no aarch64 for 1.7.3, so we fall back to the 1.7.2 version
                db.available_channels.insert(
                    format!("{}~aarch64", major),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", Version::new(1, 7, 2)),
                    },
                );
            } else if v >= &&Version::new(1, 7, 0) {
                db.available_channels.insert(
                    format!("{}~aarch64", major),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0.aarch64", v),
                    },
                );
            }
        }
    }
    let release_version = &original_available_versions
        .iter()
        .filter(|&v| v.pre == semver::Prerelease::EMPTY)
        .max()
        .unwrap();

    let beta_version_max = &original_available_versions
        .iter()
        .filter(|&v| v.pre.as_str().contains("beta"))
        .max()
        .unwrap();

    let rc_version_max = &original_available_versions
        .iter()
        .filter(|&v| v.pre.as_str().contains("rc"))
        .max()
        .unwrap();
    let beta_version = release_version.max(beta_version_max);
    let rc_version = release_version.max(rc_version_max);

    let release_channel_names = release_channel_names(
        target_os,
        target_arch,
        release_version,
        beta_version,
        &lts_version,
        rc_version,
    );
    for (channel_key, channel_val) in release_channel_names {
        db.available_channels.insert(
            channel_key,
            JuliaupVersionDBChannel {
                version: channel_val,
            },
        );
    }
    // TODO Reenable once we have an automatic way to update this file
    // let version_db_path =
    //     get_juliaup_home_path()
    //         .with_context(|| "Failed to determine versions db file path.")?
    //         .join("juliaup-versionsdb-winnt-x64.json");

    // let file = File::open(&version_db_path);

    // if let Ok(file) = file {
    //     let reader = BufReader::new(file);

    //     let db: JuliaupVersionDB = serde_json::from_reader(reader)
    //         .with_context(|| format!("Failed to parse version db at '{}'.", version_db_path.display()))?;

    //     return Ok(db);
    // }

    // let file = File::create(&vendored_db)?;
    // serde_json::to_writer_pretty(file, &db)?;

    Ok(db)
}
