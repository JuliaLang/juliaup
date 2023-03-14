use assert_cmd::Command;

#[test]
fn command_default() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.0")
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.0")
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("--startup-file=no")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout("1.6.0");
}
