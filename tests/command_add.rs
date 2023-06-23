use assert_cmd::Command;

#[test]
fn command_add() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.4")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("+1.6.4")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout("1.6.4");
}
