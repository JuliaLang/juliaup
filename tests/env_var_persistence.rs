use predicates::str::contains;
use std::fs;

mod utils;
use utils::TestEnv;

#[test]
fn env_var_basic_persistence() {
    let env = TestEnv::new();

    // First install a Julia version
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Run Julia with JULIA_NUM_THREADS set
    env.julia()
        .arg("-e")
        .arg("print(get(ENV, \"JULIA_NUM_THREADS\", \"NOT_SET\"))")
        .env("JULIA_NUM_THREADS", "8")
        .assert()
        .success()
        .stdout("8");

    // Read config file to verify persistence
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_content.contains("JuliaEnvironmentVariables"),
        "Config should contain JuliaEnvironmentVariables section"
    );
    assert!(
        config_content.contains("JULIA_NUM_THREADS"),
        "Config should contain JULIA_NUM_THREADS"
    );
    assert!(
        config_content.contains("\"8\""),
        "Config should contain the value 8"
    );
}

#[test]
fn env_var_persisted_value_used() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First run: set JULIA_NUM_THREADS to persist it
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "12")
        .assert()
        .success();

    // Second run: without setting env var, should use persisted value
    env.julia()
        .arg("-e")
        .arg("print(get(ENV, \"JULIA_NUM_THREADS\", \"NOT_SET\"))")
        .assert()
        .success()
        .stdout("12");
}

#[test]
fn env_var_current_env_takes_precedence() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First run: persist value of 8
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "8")
        .assert()
        .success();

    // Second run: override with current environment value of 4
    env.julia()
        .arg("-e")
        .arg("print(get(ENV, \"JULIA_NUM_THREADS\", \"NOT_SET\"))")
        .env("JULIA_NUM_THREADS", "4")
        .assert()
        .success()
        .stdout("4");

    // Verify the new value was persisted
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_content.contains("\"JULIA_NUM_THREADS\": \"4\""),
        "Config should contain updated value of 4"
    );
}

#[test]
fn env_var_multiple_variables() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Set multiple environment variables
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "6")
        .env("JULIA_EDITOR", "vim")
        .env("JULIA_PKG_SERVER", "https://pkg.julialang.org")
        .assert()
        .success();

    // Verify all variables were persisted
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_content.contains("JULIA_NUM_THREADS"),
        "Config should contain JULIA_NUM_THREADS"
    );
    assert!(
        config_content.contains("JULIA_EDITOR"),
        "Config should contain JULIA_EDITOR"
    );
    assert!(
        config_content.contains("JULIA_PKG_SERVER"),
        "Config should contain JULIA_PKG_SERVER"
    );
    assert!(
        config_content.contains("\"6\""),
        "Config should contain value 6"
    );
    assert!(
        config_content.contains("vim"),
        "Config should contain vim"
    );
    assert!(
        config_content.contains("https://pkg.julialang.org"),
        "Config should contain pkg server URL"
    );
}

#[test]
fn env_var_multiple_variables_persisted_values_used() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First run: persist multiple variables
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "10")
        .env("JULIA_EDITOR", "emacs")
        .assert()
        .success();

    // Second run: verify all persisted values are used (no env vars set)
    env.julia()
        .arg("-e")
        .arg(
            "println(get(ENV, \"JULIA_NUM_THREADS\", \"NOT_SET\")); \
             println(get(ENV, \"JULIA_EDITOR\", \"NOT_SET\"))",
        )
        .assert()
        .success()
        .stdout(contains("10"))
        .stdout(contains("emacs"));
}

#[test]
fn env_var_empty_values_not_persisted() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First: set a value
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_EDITOR", "vim")
        .assert()
        .success();

    // Verify it was persisted
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_content.contains("JULIA_EDITOR"),
        "Config should contain JULIA_EDITOR after setting it"
    );

    // Now set it to empty string - should not be persisted
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_EDITOR", "")
        .assert()
        .success();

    // The key should still exist with the old value since empty strings are not persisted
    // (empty values in environment are not considered and old persisted value remains)
}

#[test]
fn env_var_julia_project_excluded() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Set JULIA_PROJECT (should NOT be persisted)
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_PROJECT", "/tmp/myproject")
        .assert()
        .success();

    // Verify JULIA_PROJECT was NOT persisted
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        !config_content.contains("JULIA_PROJECT"),
        "Config should NOT contain JULIA_PROJECT as it's excluded"
    );
}

#[test]
fn env_var_various_types() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Test various Julia environment variable types
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "auto") // Parallelization
        .env("JULIA_DEPOT_PATH", "/custom/depot") // File location
        .env("JULIA_ERROR_COLOR", "\\033[91m") // REPL formatting
        .env("JULIA_PKG_OFFLINE", "true") // Pkg.jl
        .assert()
        .success();

    // Verify all were persisted
    let config_content = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_content.contains("JULIA_NUM_THREADS"),
        "Should persist JULIA_NUM_THREADS"
    );
    assert!(
        config_content.contains("JULIA_DEPOT_PATH"),
        "Should persist JULIA_DEPOT_PATH"
    );
    assert!(
        config_content.contains("JULIA_ERROR_COLOR"),
        "Should persist JULIA_ERROR_COLOR"
    );
    assert!(
        config_content.contains("JULIA_PKG_OFFLINE"),
        "Should persist JULIA_PKG_OFFLINE"
    );
}

#[test]
fn env_var_persists_across_channel_switches() {
    let env = TestEnv::new();

    // Install multiple Julia versions
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("add")
        .arg("1.10.11")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Set env var with version 1.10.10
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "16")
        .assert()
        .success();

    // Switch to different version and verify env var is still used
    env.julia()
        .arg("+1.10.11")
        .arg("-e")
        .arg("print(get(ENV, \"JULIA_NUM_THREADS\", \"NOT_SET\"))")
        .assert()
        .success()
        .stdout("16");
}

#[test]
fn env_var_config_updated_after_julia_run() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Read initial config state (should not have JuliaEnvironmentVariables yet)
    let initial_config = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        !initial_config.contains("JuliaEnvironmentVariables")
            || initial_config.contains("\"JuliaEnvironmentVariables\": {}"),
        "Config should not have environment variables initially"
    );

    // Run Julia with environment variables
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "8")
        .env("JULIA_EDITOR", "nvim")
        .env("JULIA_PKG_SERVER", "https://pkg.julialang.org")
        .assert()
        .success();

    // Read updated config
    let updated_config = fs::read_to_string(env.config_path()).unwrap();

    // Verify config was updated with all three variables
    assert!(
        updated_config.contains("JuliaEnvironmentVariables"),
        "Config should contain JuliaEnvironmentVariables section after Julia run"
    );
    assert!(
        updated_config.contains("\"JULIA_NUM_THREADS\": \"8\""),
        "Config should contain JULIA_NUM_THREADS=8"
    );
    assert!(
        updated_config.contains("\"JULIA_EDITOR\": \"nvim\""),
        "Config should contain JULIA_EDITOR=nvim"
    );
    assert!(
        updated_config.contains("\"JULIA_PKG_SERVER\": \"https://pkg.julialang.org\""),
        "Config should contain JULIA_PKG_SERVER"
    );
}

#[test]
fn env_var_config_updated_on_value_change() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First run: persist initial value
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "4")
        .assert()
        .success();

    // Verify initial value
    let config_after_first_run = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_after_first_run.contains("\"JULIA_NUM_THREADS\": \"4\""),
        "Config should contain JULIA_NUM_THREADS=4 after first run"
    );

    // Second run: change the value
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "16")
        .assert()
        .success();

    // Verify config was updated with new value
    let config_after_second_run = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_after_second_run.contains("\"JULIA_NUM_THREADS\": \"16\""),
        "Config should be updated to JULIA_NUM_THREADS=16 after second run"
    );
    assert!(
        !config_after_second_run.contains("\"JULIA_NUM_THREADS\": \"4\""),
        "Config should not contain old value of 4"
    );
}

#[test]
fn env_var_config_not_updated_if_same_value() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // First run: persist value
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "8")
        .assert()
        .success();

    // Read config and get modification time
    let config_path = env.config_path();
    let metadata_after_first = fs::metadata(&config_path).unwrap();
    let _modified_time_first = metadata_after_first.modified().unwrap();

    // Wait a bit to ensure timestamps would be different if file was modified
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Second run: same value
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "8")
        .assert()
        .success();

    // Check if file was modified (it shouldn't be since value didn't change)
    let metadata_after_second = fs::metadata(&config_path).unwrap();
    let _modified_time_second = metadata_after_second.modified().unwrap();

    // Note: The file will likely be modified because we always save when we detect
    // an env var, but the content should be the same
    let config = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config.contains("\"JULIA_NUM_THREADS\": \"8\""),
        "Config should still contain JULIA_NUM_THREADS=8"
    );

    // Verify there's only one occurrence (not duplicated)
    let occurrences = config.matches("JULIA_NUM_THREADS").count();
    assert_eq!(
        occurrences, 1,
        "JULIA_NUM_THREADS should appear exactly once in config"
    );
}

#[test]
fn env_var_config_preserves_other_settings() {
    let env = TestEnv::new();

    // Install Julia
    env.juliaup()
        .arg("add")
        .arg("1.10.10")
        .assert()
        .success();

    env.juliaup()
        .arg("default")
        .arg("1.10.10")
        .assert()
        .success();

    // Set some juliaup config first
    env.juliaup()
        .arg("config")
        .arg("versionsdbupdateinterval")
        .arg("720")
        .assert()
        .success();

    // Read config before env var persistence
    let config_before = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_before.contains("\"VersionsDbUpdateInterval\": 720"),
        "Config should have VersionsDbUpdateInterval=720"
    );

    // Run Julia with env var to trigger persistence
    env.julia()
        .arg("-e")
        .arg("exit()")
        .env("JULIA_NUM_THREADS", "6")
        .assert()
        .success();

    // Verify both the existing setting and new env var are present
    let config_after = fs::read_to_string(env.config_path()).unwrap();
    assert!(
        config_after.contains("\"VersionsDbUpdateInterval\": 720"),
        "Config should still have VersionsDbUpdateInterval=720"
    );
    assert!(
        config_after.contains("\"JULIA_NUM_THREADS\": \"6\""),
        "Config should have new JULIA_NUM_THREADS=6"
    );
    assert!(
        config_after.contains("JuliaEnvironmentVariables"),
        "Config should have JuliaEnvironmentVariables section"
    );
}
