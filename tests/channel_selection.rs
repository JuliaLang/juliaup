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
            "Installing Julia 1.8.2 automatically per juliaup settings",
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
            "Installing Julia nightly automatically per juliaup settings",
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
            "Installing Julia 1.10.10 automatically per juliaup settings",
        ));
}

fn write_project(project_dir: &std::path::PathBuf, manifest_version: &str, compat: Option<&str>) {
    std::fs::create_dir_all(project_dir).unwrap();

    std::fs::write(
        project_dir.join("Project.toml"),
        format!(
            r#"
name = "AutoProject"
uuid = "00000000-0000-0000-0000-000000000001"
version = "0.1.0"

{}
"#,
            compat
                .map(|c| format!("[compat]\njulia = \"{}\"", c))
                .unwrap_or_default()
        ),
    )
    .unwrap();

    std::fs::write(
        project_dir.join("Manifest.toml"),
        format!(
            r#"
julia_version = "{}"
"#,
            manifest_version
        ),
    )
    .unwrap();
}

fn install_channel(env: &TestEnv, channel: &str) {
    env.juliaup().arg("add").arg(channel).assert().success();
}

#[test]
fn end_to_end_manifest_selection() {
    let env = TestEnv::new();
    install_channel(&env, "1.8.2");

    let project_dir = env.depot_path().join("manifest_project");
    write_project(&project_dir, "1.8.2", None);

    env.julia()
        .arg(format!(
            "--project={}",
            project_dir.as_os_str().to_string_lossy()
        ))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.8.2");
}

// https://github.com/JuliaLang/juliaup/issues/1464
// If "1.8" is installed (providing Julia 1.8.5), a manifest requesting "1.8.5"
// should reuse the already-installed version without prompting to install channel "1.8.5".
#[test]
fn manifest_reuses_version_from_other_channel() {
    let env = TestEnv::new();

    install_channel(&env, "1.8");

    // Enable manifest version detection (defaults to false)
    env.juliaup()
        .arg("config")
        .arg("manifestversiondetect")
        .arg("true")
        .assert()
        .success();

    let project_dir = env.depot_path().join("manifest_reuse_project");
    write_project(&project_dir, "1.8.5", None);

    env.julia()
        .arg(format!(
            "--project={}",
            project_dir.as_os_str().to_string_lossy()
        ))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.8.5")
        .stderr("");
}

// A project whose manifest records a Julia version that is not installed fails
// in non-interactive mode with an actionable message, and malformed or unknown
// manifests are errors instead of silently falling back to the default channel.
#[test]
fn manifest_version_errors() {
    let env = TestEnv::new();

    install_channel(&env, "1.8.5");

    env.juliaup()
        .arg("config")
        .arg("manifestversiondetect")
        .arg("true")
        .assert()
        .success();

    let project_arg = |dir: &std::path::Path| format!("--project={}", dir.to_string_lossy());

    // Required version is not installed
    let missing_dir = env.depot_path().join("missing_project");
    write_project(&missing_dir, "1.8.4", None);
    env.julia()
        .arg(project_arg(&missing_dir))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .failure()
        .stdout("")
        .stderr(contains(
            "ERROR: This project requires Julia 1.8.4, which is not installed.",
        ))
        .stderr(contains("juliaup add 1.8.4"))
        .stderr(contains("juliaup config autoinstallchannels true"));

    // Malformed manifest
    let malformed_dir = env.depot_path().join("malformed_project");
    write_project(&malformed_dir, "1.8.5", None);
    std::fs::write(malformed_dir.join("Manifest.toml"), "julia_version = ").unwrap();
    env.julia()
        .arg(project_arg(&malformed_dir))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .failure()
        .stdout("")
        .stderr(contains(
            "ERROR: Failed to determine the Julia version for the active project",
        ))
        .stderr(contains("Manifest.toml"));

    // A release version that doesn't exist, even after refreshing the versions db
    let unknown_dir = env.depot_path().join("unknown_project");
    write_project(&unknown_dir, "1.8.99", None);
    env.julia()
        .arg(project_arg(&unknown_dir))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .failure()
        .stdout("")
        .stderr(contains("Julia 1.8.99 recorded in"))
        .stderr(contains("is not a known Julia release"));

    // A project directory without a project file falls back to the default channel
    env.julia()
        .arg(project_arg(&env.depot_path().join("no_such_project")))
        .arg("-e")
        .arg("print(VERSION)")
        .assert()
        .success()
        .stdout("1.8.5");
}
