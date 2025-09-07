use predicates::prelude::predicate;

mod utils;
use utils::TestEnv;

#[test]
fn command_add() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.4")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("nightly")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.11-nightly")
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("+1.6.4")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.6.4");

    env.julia()
        .arg("+nightly")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout(
            predicate::str::is_match(
                "^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)-DEV\\.(0|[1-9]\\d*)",
            )
            .unwrap(),
        );
}
