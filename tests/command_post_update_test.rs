#![cfg(not(windows))]

mod utils;
use utils::TestEnv;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

fn juliaup(exe: &Path, env: &TestEnv) -> Command {
    let mut cmd = Command::new(exe);
    env.apply_env(&mut cmd);
    cmd
}

/// Regression test for #1513: a self-update swaps the entire bin directory,
/// which removes the `julia` -> `julialauncher` symlink (not part of the
/// juliaup tarball) and left users with "command not found: julia". The
/// `_post-update` hook must restore the symlink so `julia` keeps working.
#[test]
fn post_update_restores_julia_symlink() {
    let env = TestEnv::new();
    let bin = assert_fs::TempDir::new().unwrap();

    // Place real juliaup + julialauncher binaries in an isolated bin directory.
    // The post-update hook keys off the running exe's parent, so running this
    // copy makes it operate here rather than in the cargo target dir.
    // The `julialauncher` bin requires a build feature and isn't compiled by
    // default; in the dev build the launcher binary is named `julia`, so copy
    // that to act as the `julialauncher` the installed product would have.
    let juliaup_exe = bin.path().join("juliaup");
    let julialauncher_exe = bin.path().join("julialauncher");
    fs::copy(cargo_bin("juliaup"), &juliaup_exe).unwrap();
    fs::copy(cargo_bin("julia"), &julialauncher_exe).unwrap();

    // Install and default a real Julia so the launcher has something to run.
    juliaup(&juliaup_exe, &env)
        .args(["add", "1.12.0"])
        .assert()
        .success();
    juliaup(&juliaup_exe, &env)
        .args(["default", "1.12.0"])
        .assert()
        .success();

    // Create the `julia` -> `julialauncher` symlink as the installer does, then
    // delete it to reproduce the directory swap that wipes it during self-update.
    let julia = bin.path().join("julia");
    symlink(&julialauncher_exe, &julia).unwrap();
    fs::remove_file(&julia).unwrap();
    assert!(
        julia.symlink_metadata().is_err(),
        "precondition: julia symlink should be absent before post-update"
    );

    // The post-update hook should restore the symlink.
    juliaup(&juliaup_exe, &env)
        .arg("_post-update")
        .assert()
        .success();

    assert!(
        julia.symlink_metadata().is_ok(),
        "julia symlink should be restored by post-update"
    );
    assert_eq!(fs::read_link(&julia).unwrap(), julialauncher_exe);

    // And julia must actually run via the restored symlink.
    let mut julia_cmd = Command::new(&julia);
    env.apply_env(&mut julia_cmd);
    julia_cmd
        .args(["-e", "print(VERSION)"])
        .assert()
        .success()
        .stdout("1.12.0");
}
