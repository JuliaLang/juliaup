use assert_cmd::Command;

#[test]
fn channel_selection() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.7.3")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.8.5")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.6.7")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.6.7");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("1.8.5");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.7.3");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("+1.8.5")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.3")
        .assert()
        .success()
        .stdout("1.8.5");

    // Now testing incorrect channels

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6` at command line.\n");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.7.4` in environment variable JULIAUP_CHANNEL.\n");

    Command::cargo_bin("julialauncher")
        .unwrap()
        .arg("+1.8.6")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: Invalid Juliaup channel `1.8.6` at command line.\n");
}
