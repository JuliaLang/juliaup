use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

fn test_shell_completion(shell: &str, expected_patterns: &[&str]) {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    let mut cmd = cargo_bin_cmd!("juliaup")
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
    test_shell_completion(
        "bash",
        &[
            "_juliaup()",
            "complete -F _juliaup",
            "_julia_channel_completions",
            "juliaup _list-channels",
            "complete -o default -F _julia_channel_completions julia",
        ],
    );
}

#[test]
fn completions_zsh() {
    test_shell_completion(
        "zsh",
        &[
            "#compdef juliaup",
            "_juliaup()",
            "_julia_channel",
            "juliaup _list-channels",
            "compdef _julia_channel julia",
        ],
    );
}

#[test]
fn completions_fish() {
    test_shell_completion(
        "fish",
        &[
            "complete -c juliaup",
            "-n \"__fish",
            "complete -c julia",
            "juliaup _list-channels",
        ],
    );
}

#[test]
fn completions_powershell() {
    test_shell_completion(
        "power-shell",
        &[
            "Register-ArgumentCompleter",
            "juliaup",
            "CommandName julia",
            "juliaup _list-channels",
        ],
    );
}

#[test]
fn completions_elvish() {
    test_shell_completion(
        "elvish",
        &[
            "edit:completion:arg-completer",
            "juliaup",
            "arg-completer[julia]",
            "juliaup _list-channels",
        ],
    );
}

#[test]
fn completions_nushell() {
    test_shell_completion(
        "nushell",
        &[
            "module completions",
            "export extern juliaup",
            "export use completions",
            "julia_channels",
            "juliaup _list-channels",
        ],
    );
}

#[test]
fn list_channels_outputs_sorted() {
    let depot_dir = tempfile::Builder::new()
        .prefix("juliauptest")
        .tempdir()
        .unwrap();

    let output = cargo_bin_cmd!("juliaup")
        .arg("_list-channels")
        .env("JULIA_DEPOT_PATH", depot_dir.path())
        .env("JULIAUP_DEPOT_PATH", depot_dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted, "Channel names should be sorted");
}
