extern crate itertools;
extern crate semver;
extern crate serde;
extern crate serde_json;
#[cfg(windows)]
extern crate winres;

use anyhow::Result;
use serde_json::Value;
use std::env;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<()> {
    let target_platform = std::env::var("TARGET").unwrap();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let db_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("versiondb")
        .join(format!("versiondb-{}.json", target_platform));

    let version_db_path = out_path.join("versionsdb.json");
    std::fs::copy(&db_path, &version_db_path).unwrap();

    let file = File::open(&db_path)?;
    let data: Value = serde_json::from_reader(file)?;
    let bundled_version_as_string: String =
        data["AvailableChannels"]["release"]["Version"].to_string();
    let bundled_dbversion_as_string: String = data["Version"].to_string();
    let bundled_version_path = Path::new(&out_path).join("bundled_version.rs");
    std::fs::write(
        &bundled_version_path,
        format!(
            "pub const BUNDLED_JULIA_VERSION: &str = {}; pub const BUNDLED_DB_VERSION: &str = {};",
            bundled_version_as_string, bundled_dbversion_as_string
        ),
    )
    .unwrap();

    #[cfg(windows)]
    {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let mut res = winres::WindowsResource::new();
        res.set_icon(&manifest_dir.join("src/julia.ico").to_string_lossy());

        #[cfg(feature = "winpkgidentityext")]
        res.set_manifest_file(
            &manifest_dir
                .join("deploy/winpkgidentityext/app.manifest")
                .to_string_lossy(),
        );

        // winres's own `compile` links the resource with `rustc-link-lib`,
        // which rustc records in the library and then drags into every
        // dependent binary, including juliaupgui, which has its own resource.
        // Compile it with embed-resource instead, which links it into this
        // package's binaries only.
        let rc = out_path.join("resource.rc");
        res.write_resource_file(&rc).unwrap();
        embed_resource::compile(&rc, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }

    let various_constants_path = Path::new(&out_path).join("various_constants.rs");
    std::fs::write(
        &various_constants_path,
        format!("pub const JULIAUP_TARGET: &str = \"{target_platform}\";"),
    )
    .unwrap();

    built::write_built_file().expect("Failed to acquire build-time information");

    Ok(())
}
