use assert_cmd::Command;

#[test]
fn command_status() {
    let mut cmd = Command::cargo_bin("juliaup").unwrap();

    let depot_dir = tempfile::Builder::new().prefix("juliauptest").tempdir().unwrap();

    let assert = cmd
        .arg("status")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert();
    
    assert.success().stdout("Installed Julia channels (default marked with *):\n");
}
