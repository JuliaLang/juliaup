use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn command_list() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("list")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with(" Channel").and(predicate::str::contains("release")))
        .stdout(predicate::str::contains("nightly"))
        .stdout(predicate::str::contains("pr{number}"));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("ls")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with(" Channel").and(predicate::str::contains("release")));
}
