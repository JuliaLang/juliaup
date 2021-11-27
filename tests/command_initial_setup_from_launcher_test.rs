use assert_cmd::Command;
use std::path::Path;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn command_initial_setup() {
    let mut cmd = Command::cargo_bin("juliaup").unwrap();

    let depot_dir = assert_fs::TempDir::new().unwrap();

    depot_dir.child(Path::new("juliaup")).assert(predicate::path::missing());

    let assert = cmd
        .arg("46029ef5-0b73-4a71-bff3-d0d05de42aac")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert();
    
    assert.success().stdout("");

    depot_dir.child(Path::new("juliaup").join("juliaup.json")).assert(predicate::path::exists());
}
