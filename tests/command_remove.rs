use assert_cmd::Command;
use predicates::boolean::PredicateBooleanExt;

#[test]
fn command_remove() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").not());

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.4")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4"));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release")));

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("remove")
        .arg("release")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.6.4").and(predicates::str::contains("release").not()));
}
