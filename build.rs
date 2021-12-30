extern crate itertools;
extern crate semver;
extern crate serde;
extern crate winres;
extern crate serde_json;
#[path = "src/jsonstructs_versionsdb.rs"]
mod jsonstructs_versionsdb;

use anyhow::Result;
use itertools::Itertools;
use jsonstructs_versionsdb::{JuliaupVersionDB, JuliaupVersionDBChannel, JuliaupVersionDBVersion};
use semver::Version;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::path::Path;
use serde_json::Value;

fn produce_version_db() -> Result<JuliaupVersionDB> {
    let mut original_available_versions: Vec<Version> = Vec::new();

    let lts_version = Version::parse("1.6.5")?;
    let beta_version = Version::parse("1.7.1")?;
    let rc_version = Version::parse("1.7.1")?;
    let nightly_version = Version::parse("1.8.0-latest")?;

    original_available_versions.push(Version::parse("0.7.0")?);
    original_available_versions.push(Version::parse("1.0.0")?);
    original_available_versions.push(Version::parse("1.0.1")?);
    original_available_versions.push(Version::parse("1.0.2")?);
    original_available_versions.push(Version::parse("1.0.3")?);
    original_available_versions.push(Version::parse("1.0.4")?);
    original_available_versions.push(Version::parse("1.0.5")?);
    original_available_versions.push(Version::parse("1.1.0")?);
    original_available_versions.push(Version::parse("1.1.1")?);
    original_available_versions.push(Version::parse("1.2.0")?);
    original_available_versions.push(Version::parse("1.3.0")?);
    original_available_versions.push(Version::parse("1.3.1")?);
    original_available_versions.push(Version::parse("1.4.0")?);
    original_available_versions.push(Version::parse("1.4.1")?);
    original_available_versions.push(Version::parse("1.4.2")?);
    original_available_versions.push(Version::parse("1.5.0")?);
    original_available_versions.push(Version::parse("1.5.1")?);
    original_available_versions.push(Version::parse("1.5.2")?);
    original_available_versions.push(Version::parse("1.5.3")?);
    original_available_versions.push(Version::parse("1.5.4")?);
    original_available_versions.push(Version::parse("1.6.0")?);
    original_available_versions.push(Version::parse("1.6.1")?);
    original_available_versions.push(Version::parse("1.6.2")?);
    original_available_versions.push(Version::parse("1.6.3")?);
    original_available_versions.push(Version::parse("1.6.4")?);
    original_available_versions.push(Version::parse("1.6.5")?);
    original_available_versions.push(Version::parse("1.7.0-beta1")?);
    original_available_versions.push(Version::parse("1.7.0-beta2")?);
    original_available_versions.push(Version::parse("1.7.0-beta3")?);
    original_available_versions.push(Version::parse("1.7.0-beta4")?);
    original_available_versions.push(Version::parse("1.7.0-rc1")?);
    original_available_versions.push(Version::parse("1.7.0-rc2")?);
    original_available_versions.push(Version::parse("1.7.0-rc3")?);
    original_available_versions.push(Version::parse("1.7.0")?);
    original_available_versions.push(Version::parse("1.7.1")?);

    let mut db = JuliaupVersionDB {
        available_versions: HashMap::new(),
        available_channels: HashMap::new(),
    };

    for v in &original_available_versions {
        add_version(&mut db, v, false)?;
    }

    original_available_versions.push(nightly_version.clone());

    add_version(&mut db, &nightly_version, true)?;

    let minor_channels = &original_available_versions
        .iter()
        .filter(|&v| v.pre == semver::Prerelease::EMPTY)
        .into_grouping_map_by(|&v| (v.major, v.minor))
        .max();

    let major_channels = &original_available_versions
        .iter()
        .filter(|&v| v.pre == semver::Prerelease::EMPTY)
        .into_grouping_map_by(|&v| v.major)
        .max();

    for ((major, minor), v) in minor_channels {
        add_channels(&mut db, &format!("{}.{}", major, minor), v)?;
    }

    for (major, v) in major_channels {
        add_channels(&mut db, &major.to_string(), v)?;
    }

    let release_version = &original_available_versions
        .iter()
        .filter(|&v| v.pre == semver::Prerelease::EMPTY)
        .max()
        .unwrap();

    add_channels(&mut db, &"release", &release_version)?;
    add_channels(&mut db, &"lts",     &lts_version)?;
    add_channels(&mut db, &"beta",    &beta_version)?;
    add_channels(&mut db, &"rc",      &rc_version)?;
    add_channels(&mut db, &"nightly", &nightly_version)?;

    Ok(db)
}

fn add_version(
    db: &mut JuliaupVersionDB,
    v: &Version,
    nightly: bool,
) -> Result<()> {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH")?;
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;

    let middle_part = if nightly {
        "julia-latest".to_string()
    } else {
        format!("{}.{}/julia-{}", v.major, v.minor, v)
    };

    if target_os == "windows" && target_arch == "x86_64" {
        db.available_versions.insert(
            format!("{}+0~x64", v),
            JuliaupVersionDBVersion {
                url_path: format!("bin/winnt/x64/{}-win64.tar.gz", middle_part),
                nightly: nightly,
            }
        );
        db.available_versions.insert(
            format!("{}+0~x86", v),
            JuliaupVersionDBVersion {
                url_path: format!("bin/winnt/x86/{}-win32.tar.gz", middle_part),
                nightly: nightly,
            }
        );
    } else if target_os == "windows" && target_arch == "x86" {
        db.available_versions.insert(
            format!("{}+0~x86", v),
            JuliaupVersionDBVersion {
                url_path: format!("bin/winnt/x86/{}-win32.tar.gz", middle_part),
                nightly: nightly,
            }
        );
    } else if target_os == "linux" {
        if nightly {
            if target_arch == "x86_64" {
                db.available_versions.insert(
                    format!("{}+0~x64", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x64/{}-linux64.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
                db.available_versions.insert(
                    format!("{}+0~x86", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x86/{}-linux32.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else if target_arch == "x86" {
                db.available_versions.insert(
                    format!("{}+0~x86", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x86/{}-linux-i686.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else if target_arch == "aarch64" {
                db.available_versions.insert(
                    format!("{}+0~aarch64", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/aarch64/{}-linuxaarch64.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else {
                eprintln!("Nightly builds not available for this platform.");
            }
        } else {
            if target_arch == "x86_64" {
                db.available_versions.insert(
                    format!("{}+0~x64", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x64/{}-linux-x86_64.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
                db.available_versions.insert(
                    format!("{}+0~x86", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x86/{}-linux-i686.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else if target_arch == "x86" {
                db.available_versions.insert(
                    format!("{}+0~x86", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/x86/{}-linux-i686.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else if target_arch == "aarch64" {
                db.available_versions.insert(
                    format!("{}+0~aarch64", v),
                    JuliaupVersionDBVersion {
                        url_path: format!("bin/linux/aarch64/{}-linux-aarch64.tar.gz", middle_part),
                        nightly: nightly,
                    }
                );
            } else {
                panic!("Building on this platform is currently not supported.")
            }
        }
    } else if target_os == "macos" && target_arch == "x86_64"{
        db.available_versions.insert(
            format!("{}+0~x64", v),
            JuliaupVersionDBVersion {
                url_path: format!("bin/mac/x64/{}-mac64.tar.gz", middle_part),
                nightly: nightly,
            }
        );
    } else if target_os == "macos" && target_arch == "aarch64"{
        db.available_versions.insert(
            format!("{}+0~x64", v),
            JuliaupVersionDBVersion {
                url_path: format!("bin/mac/x64/{}-mac64.tar.gz", middle_part),
                nightly: nightly,
            }
        );

        if v >= &Version::new(1,7, 0) {
            if !nightly {
                db.available_versions.insert(
                    format!("{}+0~aarch64", v),
                    JuliaupVersionDBVersion {
                    url_path: format!("bin/mac/aarch64/{}-macaarch64.tar.gz", middle_part),
                    nightly: nightly,
                }
                );
            } else {
                eprintln!("Nightly builds not available for this platform.");
            }
        }

    } else {
        panic!("Building on this platform is currently not supported.")
    }

    add_channels(db, &v.to_string(), v)?;

    Ok(())
}

fn add_channels(
    db: &mut JuliaupVersionDB,
    name: &str,
    v: &Version,
) -> Result<()> {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH")?;
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;

    if target_arch == "x86_64" {
        db.available_channels.insert(
            format!("{}", name),
            JuliaupVersionDBChannel {
                version: format!("{}+0~x64", v),
            },
        );
        db.available_channels.insert(
            format!("{}~x64", name),
            JuliaupVersionDBChannel {
                version: format!("{}+0~x64", v),
            },
        );
        db.available_channels.insert(
            format!("{}~x86", name),
            JuliaupVersionDBChannel {
                version: format!("{}+0~x86", v),
            },
        );
    } else if target_arch == "x86" {
        db.available_channels.insert(
            format!("{}", name),
            JuliaupVersionDBChannel {
                version: format!("{}+0~x86", v),
            },
        );
        db.available_channels.insert(
            format!("{}~x86", name),
            JuliaupVersionDBChannel {
                version: format!("{}+0~x86", v),
            },
        );
    } else if target_arch == "aarch64" {
        if target_os == "windows" {
            db.available_channels.insert(
                format!("{}", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~x86", v),
                },
            );
            db.available_channels.insert(
                format!("{}~x64", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~x64", v),
                },
            );
            db.available_channels.insert(
                format!("{}~x86", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~x86", v),
                },
            );
        } else if target_os == "linux" {
            db.available_channels.insert(
                format!("{}", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~aarch64", v),
                },
            );
            db.available_channels.insert(
                format!("{}~aarch64", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~aarch64", v),
                },
            );
        } else if target_os == "macos" {
            db.available_channels.insert(
                format!("{}", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~x64", v),
                },
            );
            db.available_channels.insert(
                format!("{}~x64", name),
                JuliaupVersionDBChannel {
                    version: format!("{}+0~x64", v),
                },
            );
            if v >= &Version::new(1,7, 0) {
                db.available_channels.insert(
                    format!("{}~aarch64", name),
                    JuliaupVersionDBChannel {
                        version: format!("{}+0~aarch64", v),
                    },
                );
            }
        } else {
            panic!("Building for this platform is currently not supported.");
        }
    } else {
        panic!("Building on this platform is currently not supported.")
    }

    Ok(())
}

fn main() -> Result<()> {
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let db = produce_version_db()?;

    let version_db_path = out_path.join("versionsdb.json");
    let file = File::create(&version_db_path)?;
    serde_json::to_writer_pretty(file, &db)?;

    let file = File::open(Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("versions.json"))?;
    let data: Value = serde_json::from_reader(file)?;
    let bundled_version: String = data["JuliaAppPackage"]["BundledJuliaVersion"].to_string();
    let bundled_full_version: String = data["JuliaAppPackage"]["BundledJuliaSemVersion"].to_string();
    let bundled_version_path = Path::new(&out_path).join("bundled_version.rs");
    std::fs::write(
        &bundled_version_path,
        format!("pub const BUNDLED_JULIA_VERSION: &str = {}; pub const BUNDLED_JULIA_FULL_VERSION: &str = {};", bundled_version, bundled_full_version)
    ).unwrap();

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("src/julia.ico");
        res.compile().unwrap();
    }

    let various_constants_path = Path::new(&out_path).join("various_constants.rs");
    std::fs::write(
        &various_constants_path,
        format!("pub const JULIAUP_TARGET: &str = \"{}\";", std::env::var("TARGET").unwrap())
    ).unwrap();

    built::write_built_file().expect("Failed to acquire build-time information");
    
    Ok(())
}
