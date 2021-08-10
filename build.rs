extern crate winres;

use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    std::fs::copy(
        Path::new("build/versiondb/juliaup-versionsdb-winnt-x64.json"),
        out_path.join("versionsdb.json"),
    )
    .context("Failed to copy version DB.")?;

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("src/julia.ico");
        res.compile().unwrap();
    }

    Ok(())
}
