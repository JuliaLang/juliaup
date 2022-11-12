extern crate itertools;
extern crate semver;
extern crate serde;
extern crate serde_json;

#[cfg(windows)]
extern crate winres;
#[path = "src/jsonstructs_versionsdb.rs"]
mod jsonstructs_versionsdb;

use anyhow::Result;
use serde_json::Value;
use std::env;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // let db = produce_version_db()?;

    // let version_db_path = out_path.join("versionsdb.json");
    // let file = File::create(&version_db_path)?;
    // serde_json::to_writer_pretty(file, &db)?;

    let file =
        File::open(Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("versions.json"))?;
    let data: Value = serde_json::from_reader(file)?;
    let bundled_version = data["JuliaAppPackage"]["BundledJuliaVersion"].to_string();
    let bundled_full_version = data["JuliaAppPackage"]["BundledJuliaSemVersion"].to_string();
    let bundled_version_path = Path::new(&out_path).join("bundled_version.rs");
    std::fs::write(
        &bundled_version_path,
        format!("pub const BUNDLED_JULIA_VERSION: &str = {}; pub const BUNDLED_JULIA_FULL_VERSION: &str = {};", bundled_version, bundled_full_version)
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
        format!(
            "pub const JULIAUP_TARGET: &str = \"{}\";",
            std::env::var("TARGET").unwrap()
        ),
    )
    .unwrap();

    built::write_built_file().expect("Failed to acquire build-time information");

    Ok(())
}
