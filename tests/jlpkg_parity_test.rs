use assert_cmd::Command;
use tempfile::TempDir;

fn jlpkg() -> Command {
    Command::cargo_bin("jlpkg").unwrap()
}

fn julia() -> Command {
    Command::new("julia")
}

fn setup_test_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    std::fs::write(
        temp_dir.path().join("Project.toml"),
        r#"name = "TestProject""#,
    )
    .unwrap();
    temp_dir
}

/// Compare jlpkg and julia pkg"..." outputs for various commands
/// We strip ANSI codes and normalize paths for comparison
fn normalize_output(s: &str) -> String {
    // Remove ANSI escape codes
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    let s = re.replace_all(s, "");

    // Remove warning about REPL mode (julia might show it)
    // and registry initialization messages (jlpkg might show them on first run)
    let lines: Vec<&str> = s
        .lines()
        .filter(|line| !line.contains("REPL mode is intended for interactive use"))
        .filter(|line| !line.contains("@ Pkg.REPLMode"))
        .filter(|line| !line.contains("Installing known registries"))
        .filter(|line| !line.contains("Added `General` registry"))
        .collect();

    lines.join("\n").trim().to_string()
}

#[test]
fn test_status_parity() {
    let temp_dir = setup_test_project();

    // Run with jlpkg
    let jlpkg_output = jlpkg()
        .current_dir(&temp_dir)
        .arg("status")
        .output()
        .unwrap();

    // Run with julia
    let julia_output = julia()
        .current_dir(&temp_dir)
        .args(&["--project=.", "--color=yes", "--startup-file=no", "-e"])
        .arg("using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"status\")")
        .output()
        .unwrap();

    assert_eq!(
        normalize_output(&String::from_utf8_lossy(&jlpkg_output.stdout)),
        normalize_output(&String::from_utf8_lossy(&julia_output.stdout))
    );
}

#[test]
fn test_help_subcommands() {
    // Test that help works for all subcommands
    let subcommands = vec![
        "add",
        "build",
        "compat",
        "develop",
        "free",
        "gc",
        "generate",
        "instantiate",
        "pin",
        "precompile",
        "remove",
        "registry",
        "resolve",
        "status",
        "test",
        "update",
        "why",
    ];

    for cmd in subcommands {
        // Test basic help
        jlpkg().args(&[cmd, "--help"]).assert().success();

        // Test help with Julia flags before
        jlpkg()
            .args(&["--project=/tmp", cmd, "--help"])
            .assert()
            .success();

        // Test help with multiple Julia flags
        jlpkg()
            .args(&["--threads=4", "--color=no", cmd, "--help"])
            .assert()
            .success();
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_command_parity_with_flags() {
    let temp_dir = setup_test_project();

    // Test various commands - the args are the same as the pkg command string
    let test_commands = vec![
        vec!["status", "--manifest"],
        vec!["status", "--outdated"],
        vec!["gc", "--all"],
        vec!["build"],
        vec!["resolve"],
        vec!["precompile"],
        vec!["registry", "status"],
    ];

    for cmd_args in test_commands {
        let pkg_cmd = cmd_args.join(" ");
        // Run with jlpkg
        let jlpkg_output = jlpkg()
            .current_dir(&temp_dir)
            .args(&cmd_args)
            .output()
            .unwrap();

        // Run with julia
        let julia_output = julia()
            .current_dir(&temp_dir)
            .args(&["--project=.", "--color=yes", "--startup-file=no", "-e"])
            .arg(format!("using Pkg; isdefined(Pkg.REPLMode, :PRINTED_REPL_WARNING) && (Pkg.REPLMode.PRINTED_REPL_WARNING[] = true); Pkg.REPLMode.pkgstr(\"{}\")", pkg_cmd))
            .output()
            .unwrap();

        assert_eq!(
            normalize_output(&String::from_utf8_lossy(&jlpkg_output.stdout)),
            normalize_output(&String::from_utf8_lossy(&julia_output.stdout)),
            "Mismatch for command: {:?}",
            cmd_args
        );

        // Also check stderr is similar (both should have no errors for these commands)
        assert_eq!(
            jlpkg_output.status.success(),
            julia_output.status.success(),
            "Status mismatch for command: {:?}",
            cmd_args
        );
    }
}

#[test]
fn test_julia_flags_passthrough() {
    let temp_dir = setup_test_project();

    // Test that various Julia flags are passed through correctly
    let flag_tests = vec![
        vec!["--threads=2", "status"],
        vec!["--project=/tmp", "status"],
        vec!["--color=no", "status"],
        vec!["--startup-file=yes", "status"],
    ];

    for args in flag_tests {
        // Just ensure the command succeeds - we can't easily test the flags are applied
        // without more complex setup, but at least we know they don't break parsing
        jlpkg()
            .current_dir(&temp_dir)
            .args(&args)
            .assert()
            .success();
    }
}

#[test]
fn test_complex_package_specs() {
    let temp_dir = setup_test_project();

    // Test various package specification formats are accepted
    // We don't actually add them (would require network), just test parsing
    let specs = vec![
        vec!["add", "--help"],                // Should show help, not try to add
        vec!["add", "JSON@0.21.1", "--help"], // Help should take precedence
        vec!["develop", "/path/to/package", "--help"],
        vec![
            "add",
            "https://github.com/JuliaLang/Example.jl#master",
            "--help",
        ],
    ];

    for args in specs {
        jlpkg()
            .current_dir(&temp_dir)
            .args(&args)
            .assert()
            .success()
            .stdout(predicates::str::contains("help"));
    }
}
