extern crate itertools;
extern crate semver;
extern crate serde;
extern crate serde_json;
#[cfg(windows)]
extern crate winres;
#[path = "src/jsonstructs_versionsdb.rs"]
mod jsonstructs_versionsdb;

use anyhow::Result;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::path::Path;
use serde_json::Value;

fn main() -> Result<()> {
    let target_platform = std::env::var("TARGET").unwrap();

    let rust_pl_2_julia_pl = HashMap::from([
        ("x86_64-pc-windows-msvc", "x86_64-w64-mingw32"),
        ("x86_64-apple-darwin", "x86_64-apple-darwin14"),
        ("x86_64-unknown-linux-gnu", "x86_64-linux-gnu"),
        ("i686-pc-windows-msvc", "i686-w64-mingw32"),
        ("i686-unknown-linux-gnu", "i686-linux-gnu"),
        ("aarch64-unknown-linux-gnu", "aarch64-linux-gnu"),
        ("aarch64-apple-darwin", "aarch64-apple-darwin14")
    ]);
    let julia_pl = rust_pl_2_julia_pl.get(target_platform.as_str()).unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let db_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("versiondb").join(format!("versiondb-{}.json", julia_pl));

    let version_db_path = out_path.join("versionsdb.json");
    std::fs::copy(&db_path, &version_db_path).unwrap();

    let file = File::open(&db_path)?;
    let data: Value = serde_json::from_reader(file)?;
    let bundled_version_as_string: String = data["AvailableChannels"]["release"]["Version"].to_string();
    let bundled_version_path = Path::new(&out_path).join("bundled_version.rs");
    std::fs::write(
        &bundled_version_path,
        format!("pub const BUNDLED_JULIA_VERSION: &str = {};", bundled_version_as_string)
    ).unwrap();

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("src/julia.ico");
        res.compile().unwrap();
    }

    let various_constants_path = Path::new(&out_path).join("various_constants.rs");
    std::fs::write(
        &various_constants_path,
        format!("pub const JULIAUP_TARGET: &str = \"{}\";", &target_platform)
    ).unwrap();

    built::write_built_file().expect("Failed to acquire build-time information");
    
    Ok(())
}
