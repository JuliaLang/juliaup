use assert_cmd::Command;

#[test]
fn command_add() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.4")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

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
        .arg("add")
        .arg("1.11-nightly")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.6.4")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.6.4");

    // Disable this test for now, as it makes us depend on a working nightly build of Julia
    // Command::cargo_bin("julia")
    //     .unwrap()
    //     .arg("+nightly")
    //     .arg("-e")
    //     .arg("print(VERSION)")
    //     .env("JULIA_DEPOT_PATH", depot_dir.path())
    //     .env("JULIAUP_DEPOT_PATH", depot_dir.path())
    //     .assert()
    //     .success()
    //     .stdout(
    //         predicate::str::is_match(
    //             "^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)-DEV\\.(0|[1-9]\\d*)",
    //         )
    //         .unwrap(),
    //     );
}
