mod utils;
use utils::TestEnv;

#[test]
fn command_default() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.0")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("default")
        .arg("1.6.0")
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.6.0");
}
