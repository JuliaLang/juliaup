mod utils;
use utils::juliaup_command_tempfile as juliaup_command;

#[test]
fn command_update() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    juliaup_command(&depot_dir)
        .arg("update")
        .assert()
        .success()
        .stdout("");

    juliaup_command(&depot_dir)
        .arg("up")
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
    juliaup_command(&depot_dir)
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    // Create an alias to the installed version
    juliaup_command(&depot_dir)
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .assert()
        .success();

    // Update the alias - should succeed and update the target
    juliaup_command(&depot_dir)
        .arg("update")
        .arg("r")
        .assert()
        .success();
}
