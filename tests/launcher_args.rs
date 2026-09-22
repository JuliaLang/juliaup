use juliaup::launcher_args::{extract_auto_instantiate, is_ci_value, starts_repl, AutoInstantiate};

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

#[test]
fn test_auto_instantiate_parse() {
    assert_eq!(AutoInstantiate::parse("none"), Ok(AutoInstantiate::None));
    assert_eq!(AutoInstantiate::parse("julia"), Ok(AutoInstantiate::Julia));
    assert_eq!(AutoInstantiate::parse("pkg"), Ok(AutoInstantiate::Pkg));
    assert_eq!(AutoInstantiate::parse(" all "), Ok(AutoInstantiate::All));
    assert!(AutoInstantiate::parse("yes").is_err());

    assert!(AutoInstantiate::All.includes_julia() && AutoInstantiate::All.includes_pkg());
    assert!(AutoInstantiate::Julia.includes_julia() && !AutoInstantiate::Julia.includes_pkg());
    assert!(!AutoInstantiate::Pkg.includes_julia() && AutoInstantiate::Pkg.includes_pkg());
    assert!(!AutoInstantiate::None.includes_julia() && !AutoInstantiate::None.includes_pkg());
}

#[test]
fn test_extract_auto_instantiate() {
    let extract = |parts: &[&str]| extract_auto_instantiate(&args(parts)).unwrap();

    // Not given
    assert_eq!(
        extract(&["--project", "-e", "1"]),
        (args(&["--project", "-e", "1"]), None)
    );

    // Bare flag means all, and is removed
    assert_eq!(
        extract(&["--auto-instantiate", "--project"]),
        (args(&["--project"]), Some(AutoInstantiate::All))
    );

    // With a value, after a +channel
    assert_eq!(
        extract(&[
            "+1.10",
            "--project=.",
            "--auto-instantiate=julia",
            "script.jl"
        ]),
        (
            args(&["+1.10", "--project=.", "script.jl"]),
            Some(AutoInstantiate::Julia)
        )
    );

    // The last flag wins
    assert_eq!(
        extract(&["--auto-instantiate=pkg", "--auto-instantiate=none"]),
        (args(&[]), Some(AutoInstantiate::None))
    );

    // Arguments of the Julia program are left alone
    assert_eq!(
        extract(&["script.jl", "--auto-instantiate"]),
        (args(&["script.jl", "--auto-instantiate"]), None)
    );
    assert_eq!(
        extract(&["-e", "print(ARGS)", "--", "--auto-instantiate=pkg"]),
        (
            args(&["-e", "print(ARGS)", "--", "--auto-instantiate=pkg"]),
            None
        )
    );

    // Option values are left alone
    assert_eq!(
        extract(&["--eval", "--auto-instantiate"]),
        (args(&["--eval", "--auto-instantiate"]), None)
    );

    // Invalid values are an error
    assert!(extract_auto_instantiate(&args(&["--auto-instantiate=yes"])).is_err());
}
