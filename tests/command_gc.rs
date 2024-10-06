use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn command_gc() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("julib")
        .arg("julib")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("julic")
        .arg("julic")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\n")
                .count(5)
                .and(predicate::str::contains("julic"))
                .and(predicate::str::contains("julib")),
        );

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("gc")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\n")
                .count(5)
                .and(predicate::str::contains("julic"))
                .and(predicate::str::contains("julib")),
        );

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("gc")
        .arg("--prune-linked")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\n")
                .count(3)
                .and(predicate::str::contains("julic").not())
                .and(predicate::str::contains("julib").not()),
        );
}
