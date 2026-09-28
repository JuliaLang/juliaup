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

/// Link the winres resource into this package's binaries only.
///
/// `WindowsResource::compile` prints `cargo:rustc-link-lib=resource`, which
/// rustc records in the library target and then links into every dependent
/// binary as well, so juliaupgui.exe used to carry juliaup's icon and version
/// block and could not have a resource of its own. winres has no API to
/// compile without printing those lines, so run the compile in a child copy
/// of this build script, read the output directory it reports, and link the
/// file with `rustc-link-arg-bins` instead.
#[cfg(windows)]
const WINRES_CHILD_ENV: &str = "JULIAUP_BUILD_WINRES_CHILD";

#[cfg(windows)]
fn windows_resource() -> winres::WindowsResource {
    let mut res = winres::WindowsResource::new();
    res.set_icon("src/julia.ico");

    #[cfg(feature = "winpkgidentityext")]
    res.set_manifest_file("deploy/winpkgidentityext/app.manifest");

    res
}

#[cfg(windows)]
fn link_resource_into_bins() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(WINRES_CHILD_ENV, "1")
        .output()
        .expect("failed to run the resource compiler child process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let search = stdout
        .lines()
        .find_map(|l| l.strip_prefix("cargo:rustc-link-search=native="));
    let search = match (output.status.success(), search) {
        (true, Some(search)) => search,
        _ => panic!(
            "resource compilation failed:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        ),
    };
    // winres names the output after the toolchain: rc.exe writes a .res it
    // calls resource.lib, windres output is archived into libresource.a.
    let file = if std::env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc" {
        "resource.lib"
    } else {
        "libresource.a"
    };
    println!(
        "cargo:rustc-link-arg-bins={}",
        Path::new(search).join(file).display()
    );
}

fn main() -> Result<()> {
    #[cfg(windows)]
    if std::env::var_os(WINRES_CHILD_ENV).is_some() {
        windows_resource().compile().unwrap();
        return Ok(());
    }

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
    link_resource_into_bins();

    let various_constants_path = Path::new(&out_path).join("various_constants.rs");
    std::fs::write(
        &various_constants_path,
        format!("pub const JULIAUP_TARGET: &str = \"{target_platform}\";"),
    )
    .unwrap();

    built::write_built_file().expect("Failed to acquire build-time information");

    Ok(())
}
