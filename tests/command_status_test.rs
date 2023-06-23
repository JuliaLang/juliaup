use assert_cmd::Command;

#[test]
fn command_status() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("st")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_HOME", depot_dir.path().join("juliaup"))
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");
}
