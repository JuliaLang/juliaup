use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn channel_selection() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.7.3")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.6.7");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.8.5");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.7.3");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.8.5");

    // Now testing incorrect channels

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr(
            "ERROR: Invalid Juliaup channel `1.7.4` from environment variable JULIAUP_CHANNEL. Please run `juliaup list` to get a list of valid channels and versions.\n",
        );

    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6`. Please run `juliaup list` to get a list of valid channels and versions.\n");

    // https://github.com/JuliaLang/juliaup/issues/766
    // First enable auto-install in configuration
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("true")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Command line channel selector should auto-install valid channels
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.2")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .success()
        .stdout("1.8.2")
        .stderr(contains(
            "Info: Installing Julia 1.8.2 automatically per juliaup settings...",
        ));

    // https://github.com/JuliaLang/juliaup/issues/820
    // Command line channel selector should auto-install valid channels including nightly
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+nightly")
        .arg("-e")
        .arg("print(\"SUCCESS\")") // Use SUCCESS instead of VERSION since nightly version can vary
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .success()
        .stdout("SUCCESS")
        .stderr(contains(
            "Info: Installing Julia nightly automatically per juliaup settings...",
        ));

    // https://github.com/JuliaLang/juliaup/issues/995
    // Reset auto-install to false for this test
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("false")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // PR channels that don't exist should not auto-install in non-interactive mode
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+pr1")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr(contains("`pr1` is not installed. Please run `juliaup add pr1` to install pull request channel if available."));
}

#[test]
fn auto_install_valid_channel() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    // First set up a basic julia installation so juliaup is properly initialized
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.11")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    // Enable auto-install for this test
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("config")
        .arg("autoinstallchannels")
        .arg("true")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    // Now test auto-installing a valid but not installed channel via command line
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.10.10")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.10.10")
        .stderr(contains(
            "Info: Installing Julia 1.10.10 automatically per juliaup settings...",
        ));
}
