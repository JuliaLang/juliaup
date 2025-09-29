use predicates::boolean::PredicateBooleanExt;
use predicates::str::contains;

mod utils;
use utils::TestEnv;

#[test]
fn channel_selection() {
    let env = TestEnv::new();

    env.juliaup()
        .arg("add")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.7.3")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("add")
        .arg("1.8.5")
        .assert()
        .success()
        .stdout("");

    env.juliaup()
        .arg("default")
        .arg("1.6.7")
        .assert()
        .success()
        .stdout("");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.6.7");

    env.julia()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.8.5");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.7.3");

    env.julia()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.8.5");

    // Now testing incorrect channels

    env.julia()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    env.julia()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr(
            "ERROR: Invalid Juliaup channel `1.7.4` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.\n",
        );

    env.julia()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    // https://github.com/JuliaLang/juliaup/issues/766
    // First enable auto-install in configuration
    env.juliaup()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("true")
        .assert()
        .success();

    // Command line channel selector should auto-install valid channels
    env.julia()
        .arg("+1.8.2")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .success()
        .stdout("1.8.2")
        .stderr(contains(
            "Info: Installing Julia 1.8.2 automatically per juliaup settings...",
        ));

    // https://github.com/JuliaLang/juliaup/issues/820
    // Command line channel selector should auto-install valid channels including nightly
    env.julia()
        .arg("+nightly")
        .arg("-e")
        .arg("print(\"SUCCESS\")") // Use SUCCESS instead of VERSION since nightly version can vary
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .success()
        .stdout("SUCCESS")
        .stderr(contains(
            "Info: Installing Julia nightly automatically per juliaup settings...",
        ));

    // https://github.com/JuliaLang/juliaup/issues/995
    // Reset auto-install to false for this test
    env.juliaup()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("false")
        .assert()
        .success();

    // PR channels that don't exist should not auto-install in non-interactive mode
    env.julia()
        .arg("+pr1")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr(contains("`pr1` is not installed. Please run `juliaup add pr1` to install pull request channel if available."));
}

#[test]
fn auto_install_valid_channel() {
    let env = TestEnv::new();

    // First set up a basic julia installation so juliaup is properly initialized
    env.juliaup()
        .arg("add")
        .arg("1.11")
        .assert()
        .success()
        .stdout("");

    // Enable auto-install for this test
    env.juliaup()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("true")
        .assert()
        .success();

    // Now test auto-installing a valid but not installed channel via command line
    env.julia()
        .arg("+1.10.10")
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.10.10")
        .stderr(contains(
            "Info: Installing Julia 1.10.10 automatically per juliaup settings...",
        ));
}

#[test]
fn no_update_messages_in_non_interactive_mode() {
    let env = TestEnv::new();

    // Set up julia installation
    env.juliaup().arg("add").arg("1.11").assert().success();

    env.juliaup().arg("default").arg("1.11").assert().success();

    // Test that update messages are not shown when using -e flag (non-interactive)
    env.julia()
        .arg("-e")
        .arg("println(\"test\")")
        .assert()
        .success()
        .stderr(
            contains("new version")
                .not()
                .and(contains("juliaup update").not())
                .and(contains("available").not()),
        );

    // Test that update messages are not shown when using --version flag (non-interactive)
    env.julia().arg("--version").assert().success().stderr(
        contains("new version")
            .not()
            .and(contains("juliaup update").not())
            .and(contains("available").not()),
    );
}
