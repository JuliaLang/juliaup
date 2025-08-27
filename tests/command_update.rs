use assert_cmd::Command;

#[test]
fn command_update() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("update")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("up")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");
}

#[test]
fn command_update_alias_works() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    // First install a Julia version to create an alias to
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Create an alias to the installed version
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Update the alias - should succeed and update the target
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("update")
        .arg("r")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();
}
