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
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("up")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout("");
}
