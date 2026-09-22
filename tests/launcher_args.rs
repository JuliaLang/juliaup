use juliaup::launcher_args::{is_ci_value, starts_repl};

fn args(parts: &[&str]) -> Vec<String> {
    std::iter::once("julia")
        .chain(parts.iter().copied())
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn test_starts_repl() {
    // Plain REPL
    assert!(starts_repl(&args(&[])));
    assert!(starts_repl(&args(&["+1.10"])));
    assert!(starts_repl(&args(&["--project=.", "-t", "4"])));
    assert!(starts_repl(&args(&["--project", "-q"])));
    assert!(starts_repl(&args(&["-J", "sys.so"])));
    assert!(starts_repl(&args(&["--"])));

    // Scripts, expressions and stdin
    assert!(!starts_repl(&args(&["script.jl"])));
    assert!(!starts_repl(&args(&["+1.10", "script.jl"])));
    assert!(!starts_repl(&args(&["--project=.", "script"])));
    assert!(!starts_repl(&args(&["-e", "1+1"])));
    assert!(!starts_repl(&args(&["-e1+1"])));
    assert!(!starts_repl(&args(&["--eval=1+1"])));
    assert!(!starts_repl(&args(&["-E", "1+1"])));
    assert!(!starts_repl(&args(&["--print", "1+1"])));
    assert!(!starts_repl(&args(&["-"])));
    assert!(!starts_repl(&args(&["--", "script.jl"])));

    // -i forces interactive mode
    assert!(starts_repl(&args(&["-i", "script.jl"])));
    assert!(starts_repl(&args(&["-i", "-e", "1+1"])));

    // --version and --help never start a REPL
    assert!(!starts_repl(&args(&["--version"])));
    assert!(!starts_repl(&args(&["-v"])));
    assert!(!starts_repl(&args(&["-h"])));
    assert!(!starts_repl(&args(&["-i", "--help"])));

    // Values of options are not scripts
    assert!(starts_repl(&args(&["--sysimage", "sys.so"])));
    assert!(starts_repl(&args(&["-t", "auto"])));
}

#[test]
fn test_is_ci_value() {
    assert!(!is_ci_value(None));
    assert!(!is_ci_value(Some("")));
    assert!(!is_ci_value(Some("0")));
    assert!(!is_ci_value(Some("false")));
    assert!(!is_ci_value(Some("FALSE")));
    assert!(is_ci_value(Some("true")));
    assert!(is_ci_value(Some("1")));
    assert!(is_ci_value(Some("yes")));
}
