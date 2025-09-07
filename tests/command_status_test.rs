mod utils;
use utils::TestEnv;

#[test]
fn command_status() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("status")
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");

    env.juliaup()
        .arg("st")
        .assert()
        .success()
        .stdout(" Default  Channel  Version  Update \n-----------------------------------\n");
}
