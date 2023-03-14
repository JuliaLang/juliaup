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
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("st")
        .env("JULIAUP_HOME", depot_dir.path())
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");
}
