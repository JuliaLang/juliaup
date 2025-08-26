use assert_cmd::Command;
use predicates::prelude::*;

fn test_shell_completion(shell: &str, expected_patterns: &[&str]) {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    let mut cmd = Command::cargo_bin("juliaup")
        .unwrap()
        .arg("completions")
        .arg(shell)
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .assert()
        .success();

    for pattern in expected_patterns {
        cmd = cmd.stdout(predicate::str::contains(*pattern));
    }
}

#[test]
fn completions_bash() {
    test_shell_completion("bash", &["_juliaup()", "complete -F _juliaup"]);
}

#[test]
fn completions_zsh() {
    test_shell_completion("zsh", &["#compdef juliaup", "_juliaup()"]);
}

#[test]
fn completions_fish() {
    test_shell_completion("fish", &["complete -c juliaup", "-n \"__fish"]);
}

#[test]
fn completions_powershell() {
    test_shell_completion("power-shell", &["Register-ArgumentCompleter", "juliaup"]);
}

#[test]
fn completions_elvish() {
    test_shell_completion("elvish", &["edit:completion:arg-completer", "juliaup"]);
}

#[test]
fn completions_nushell() {
    test_shell_completion(
        "nushell",
        &["module completions", "export extern juliaup", "export use completions"],
    );
}
