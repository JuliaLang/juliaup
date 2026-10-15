#[cfg(not(windows))]
use predicates::prelude::*;

#[cfg(not(windows))]
mod utils;

#[cfg(not(windows))]
use utils::TestEnv;

#[cfg(not(windows))]
#[test]
fn channelsymlinks_reports_destination_path() {
    let env = TestEnv::new();
    let bin_dir = assert_fs::TempDir::new().unwrap();
    let expected_symlink = bin_dir.path().join("julia-release");

    env.juliaup()
        .arg("add")
        .arg("release")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .env("JULIAUP_BIN_DIR", bin_dir.path())
        .arg("config")
        .arg("channelsymlinks")
        .arg("true")
        .assert()
        .success()
        .stderr(
            predicate::str::contains(format!(
                "Creating symlink julia-release at {}",
                expected_symlink.display()
            ))
            .and(predicate::str::contains(
                "Configure Property 'channelsymlinks' set to 'true'",
            )),
        );

    assert!(expected_symlink.exists());
}
