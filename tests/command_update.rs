mod utils;
use utils::TestEnv;

#[test]
fn command_update() {
    let env = TestEnv::new();

    env.juliaup().arg("update").assert().success().stdout("");

    env.juliaup().arg("up").assert().success().stdout("");
}

#[test]
fn command_update_alias_works() {
    let env = TestEnv::new();

    // First install a Julia version to create an alias to
    env.juliaup().arg("add").arg("1.10.10").assert().success();

    // Create an alias to the installed version
    env.juliaup()
        .arg("link")
        .arg("r")
        .arg("+1.10.10")
        .assert()
        .success();

    // Update the alias - should succeed and update the target
    env.juliaup().arg("update").arg("r").assert().success();
}
