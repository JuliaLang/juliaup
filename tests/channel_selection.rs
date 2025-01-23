use assert_cmd::Command;

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
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.8.2")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: `1.8.2` is not installed. Please run `juliaup add 1.8.2` to install channel or version.\n");

    // https://github.com/JuliaLang/juliaup/issues/820
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+nightly")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_CHANNEL", "1.7.4")
        .assert()
        .failure()
        .stderr("ERROR: `nightly` is not installed. Please run `juliaup add nightly` to install channel or version.\n");

    // https://github.com/JuliaLang/juliaup/issues/995
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
        .stderr("ERROR: `pr1` is not installed. Please run `juliaup add pr1` to install pull request channel if available.\n");
}

#[test]
fn manifest_version_selection() {
    let depot_dir = assert_fs::TempDir::new().unwrap();

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.6.2")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.10.1")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("add")
        .arg("1.11.1")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("default")
        .arg("1.11.1")
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
        .stdout("1.11.1");

    let proj1_dir = assert_fs::TempDir::new().unwrap();

    // We are adding and then removing a package here to force generation of an actual Project.toml
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.10.1")
        .arg("-e")
        .arg("using Pkg; Pkg.activate(\".\"); Pkg.add(\"StringBuilders\"); Pkg.rm(\"StringBuilders\"); print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj1_dir)
        .assert()
        .success()
        .stdout("1.10.1");

    // First we try this with the feature disabled
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj1_dir)
        .assert()
        .success()
        .stdout("1.11.1");

    // Now we enable the feature
    Command::cargo_bin("juliaup")
        .unwrap()
        .arg("config")
        .arg("featuremanifestsupport")
        .arg("true")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success()
        .stdout("");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj1_dir)
        .assert()
        .success()
        .stdout("1.10.1");

    // TODO This currently fails, but it shouldn't
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj1_dir)
        .assert()
        .success()
        .stdout("1.10.1");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=@.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj1_dir)
        .assert()
        .success()
        .stdout("1.10.1");

    let sub_dir1 = &proj1_dir.path().join("subdir1");
    std::fs::create_dir(&sub_dir1).unwrap();

    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&sub_dir1)
        .assert()
        .success()
        .stdout("1.11.1");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&sub_dir1)
        .assert()
        .success()
        .stdout("1.10.1");

    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=@.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&sub_dir1)
        .assert()
        .success()
        .stdout("1.10.1");

    // Now we try with a Julia version that generates schema v1 manifests
    let proj2_dir = assert_fs::TempDir::new().unwrap();

    // We are adding and then removing a package here to force generation of an actual Project.toml
    Command::cargo_bin("julia")
        .unwrap()
        .arg("+1.6.2")
        .arg("-e")
        .arg("using Pkg; Pkg.activate(\".\"); Pkg.add(\"StringBuilders\"); Pkg.rm(\"StringBuilders\"); print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj2_dir)
        .assert()
        .success()
        .stdout("1.6.2");

    // It shouldn't pick up the version from the manifest, as it isn't stored in the manifest
    Command::cargo_bin("julia")
        .unwrap()
        .arg("--project=.")
        .arg("-e")
        .arg("print(VERSION)")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .current_dir(&proj2_dir)
        .assert()
        .success()
        .stdout("1.11.1");
}
