use assert_cmd::Command;

#[test]
fn command_remove() {
    let depot_dir = tempfile::Builder::new().prefix("juliauptest").tempdir().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()        
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("Installed Julia channels (default marked with *):\n");

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
        .stdout("Installed Julia channels (default marked with *):\n  *  1.6.4\n");

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
        .stdout("Installed Julia channels (default marked with *):\n  *  1.6.4\n     release\n");

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
        .stdout("Installed Julia channels (default marked with *):\n  *  1.6.4\n");
}
