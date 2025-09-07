use assert_cmd::Command;
use predicates::boolean::PredicateBooleanExt;

mod utils;
use utils::juliaup_command_tempfile as juliaup_command;

#[test]
fn command_remove() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    juliaup_command(&depot_dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").not());

    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.6.4")
        .assert()
        .success()
        .stdout("");

    juliaup_command(&depot_dir)
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4"));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release")));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release").not()));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("nightly")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("-DEV")));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("nightly")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("-DEV").not()));
}
