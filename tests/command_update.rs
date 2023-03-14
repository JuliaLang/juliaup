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
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("up")
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout("");
}
