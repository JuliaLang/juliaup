use std::env;
use std::path::PathBuf;
use std::path::Path;
use anyhow::{Result};

fn main() -> Result<()> {
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    std::fs::copy(Path::new("build/versiondb/juliaup-versionsdb-winnt-x64.json"), out_path.join("versionsdb.json"))?;

    Ok(())
}
