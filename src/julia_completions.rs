use crate::cli_styles::get_styles;
use clap::{Arg, ArgAction, Command};

pub fn julia_cli() -> Command {
    julia_cli_impl(false)
}

pub fn julia_cli_with_hidden() -> Command {
    // Show ONLY the hidden options - filter to just hidden args
    julia_cli_impl(true)
}

// NOTE: This was last generated against the Julia 1.12.0 help menu
fn julia_cli_impl(only_hidden: bool) -> Command {
    let mut cmd = Command::new("julia")
        .about("")
        .override_usage("julia [+channel] [options] -- [programfile] [args...]")
        .styles(get_styles())
        .disable_help_flag(true);

    // Normal (non-hidden) args
    let normal_args = vec![
        // Display version information
        Arg::new("version")
            .short('v')
            .long("version")
            .help("Display version information")
            .action(ArgAction::SetTrue),
        // Help
        Arg::new("help")
            .short('h')
            .long("help")
            .help("Print command-line options (this message)")
            .action(ArgAction::SetTrue),
        // Help hidden
        Arg::new("help-hidden")
            .long("help-hidden")
            .help("Print uncommon options not shown by `-h`")
            .action(ArgAction::SetTrue),
        // Project
        Arg::new("project")
            .long("project")
            .value_name("{<dir>|@temp|@.|@script[<rel>]}")
            .help("Set <dir> as the active project/environment. Or, create a temporary environment with `@temp`. The default @. option will search through parent directories until a Project.toml or JuliaProject.toml file is found. @script is similar, but searches up from the programfile or a path relative to programfile.")
            .action(ArgAction::Set)
            .num_args(0..=1),
        // Sysimage
        Arg::new("sysimage")
            .short('J')
            .long("sysimage")
            .value_name("file")
            .help("Start up with the given system image file")
            .action(ArgAction::Set),
        // Home
        Arg::new("home")
            .short('H')
            .long("home")
            .value_name("dir")
            .help("Set location of `julia` executable")
            .action(ArgAction::Set),
        // Startup file
        Arg::new("startup-file")
            .long("startup-file")
            .value_name("yes*|no")
            .help("Load `JULIA_DEPOT_PATH/config/startup.jl`; if `JULIA_DEPOT_PATH` environment variable is unset, load `~/.julia/config/startup.jl`")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Handle signals
        Arg::new("handle-signals")
            .long("handle-signals")
            .value_name("yes*|no")
            .help("Enable or disable Julia's default signal handlers")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Sysimage native code
        Arg::new("sysimage-native-code")
            .long("sysimage-native-code")
            .value_name("yes*|no")
            .help("Use native code from system image if available")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Compiled modules
        Arg::new("compiled-modules")
            .long("compiled-modules")
            .value_name("yes*|no|existing|strict")
            .help("Enable or disable incremental precompilation of modules. The `existing` option allows use of existing compiled modules that were previously precompiled, but disallows creation of new precompile files. The `strict` option is similar, but will error if no precompile file is found.")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "existing", "strict"])
            .hide_possible_values(true),
        // Pkgimages
        Arg::new("pkgimages")
            .long("pkgimages")
            .value_name("yes*|no|existing")
            .help("Enable or disable usage of native code caching in the form of pkgimages. The `existing` option allows use of existing pkgimages but disallows creation of new ones ($)\n\nNote: Settings marked '($)' may trigger package precompilation")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "existing"])
            .hide_possible_values(true),
        // Eval
        Arg::new("eval")
            .short('e')
            .long("eval")
            .value_name("expr")
            .help("Evaluate <expr>")
            .action(ArgAction::Set)
            .allow_hyphen_values(true),
        // Print
        Arg::new("print")
            .short('E')
            .long("print")
            .value_name("expr")
            .help("Evaluate <expr> and display the result")
            .action(ArgAction::Set)
            .allow_hyphen_values(true),
        // Module
        Arg::new("module")
            .short('m')
            .long("module")
            .value_name("Package")
            .help("Run entry point of `Package` (`@main` function) with `args'.")
            .action(ArgAction::Set)
            .allow_hyphen_values(true),
        // Load
        Arg::new("load")
            .short('L')
            .long("load")
            .value_name("file")
            .help("Load <file> immediately on all processors")
            .action(ArgAction::Set),
        // Threads
        Arg::new("threads")
            .short('t')
            .long("threads")
            .value_name("{auto|N[,auto|M]}")
            .help("Enable N[+M] threads; N threads are assigned to the `default` threadpool, and if M is specified, M threads are assigned to the `interactive` threadpool; `auto` tries to infer a useful default number of threads to use but the exact behavior might change in the future. Currently sets N to the number of CPUs assigned to this Julia process based on the OS-specific affinity assignment interface if supported (Linux and Windows) or to the number of CPU threads if not supported (MacOS) or if process affinity is not configured, and sets M to 1.")
            .action(ArgAction::Set),
        // GC threads
        Arg::new("gcthreads")
            .long("gcthreads")
            .value_name("N[,M]")
            .help("Use N threads for the mark phase of GC and M (0 or 1) threads for the concurrent sweeping phase of GC. N is set to the number of compute threads and M is set to 0 if unspecified.")
            .action(ArgAction::Set),
        // Procs
        Arg::new("procs")
            .short('p')
            .long("procs")
            .value_name("{N|auto}")
            .help("Integer value N launches N additional local worker processes 'auto' launches as many workers as the number of local CPU threads (logical cores).")
            .action(ArgAction::Set),
        // Machine file
        Arg::new("machine-file")
            .long("machine-file")
            .value_name("file")
            .help("Run processes on hosts listed in <file>")
            .action(ArgAction::Set),
        // Interactive
        Arg::new("interactive")
            .short('i')
            .long("interactive")
            .help("Interactive mode; REPL runs and `isinteractive()` is true.")
            .action(ArgAction::SetTrue),
        // Quiet
        Arg::new("quiet")
            .short('q')
            .long("quiet")
            .help("Quiet startup: no banner, suppress REPL warnings")
            .action(ArgAction::SetTrue),
        // Banner
        Arg::new("banner")
            .long("banner")
            .value_name("yes|no|short|auto*")
            .help("Enable or disable startup banner")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "short", "auto"])
            .hide_possible_values(true),
        // Color
        Arg::new("color")
            .long("color")
            .value_name("yes|no|auto*")
            .help("Enable or disable color text")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "auto"])
            .hide_possible_values(true),
        // History file
        Arg::new("history-file")
            .long("history-file")
            .value_name("yes*|no")
            .help("Load or save history")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Depwarn
        Arg::new("depwarn")
            .long("depwarn")
            .value_name("yes|no*|error")
            .help("Enable or disable syntax and method deprecation warnings (`error` turns warnings into errors)")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "error"])
            .hide_possible_values(true),
        // Warn overwrite
        Arg::new("warn-overwrite")
            .long("warn-overwrite")
            .value_name("yes|no*")
            .help("Enable or disable method overwrite warnings")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Warn scope
        Arg::new("warn-scope")
            .long("warn-scope")
            .value_name("yes*|no")
            .help("Enable or disable warning for ambiguous top-level scope")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // CPU target
        Arg::new("cpu-target")
            .short('C')
            .long("cpu-target")
            .value_name("target")
            .help("Limit usage of CPU features up to <target>; set to `help` to see the available options")
            .action(ArgAction::Set),
        // Optimize
        Arg::new("optimize")
            .short('O')
            .long("optimize")
            .value_name("0|1|2*|3")
            .help("Set the optimization level (level 3 if `-O` is used without a level) ($)")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .default_missing_value("3")
            .value_parser(["0", "1", "2", "3"])
            .hide_possible_values(true),
        // Min optlevel
        Arg::new("min-optlevel")
            .long("min-optlevel")
            .value_name("0*|1|2|3")
            .help("Set a lower bound on the optimization level")
            .action(ArgAction::Set)
            .value_parser(["0", "1", "2", "3"])
            .hide_possible_values(true),
        // Debug info
        Arg::new("debug-info")
            .short('g')
            .long("debug-info")
            .value_name("[{0|1*|2}]")
            .help("Set the level of debug info generation (level 2 if `-g` is used without a level) ($)")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .default_missing_value("2")
            .value_parser(["0", "1", "2"])
            .hide_possible_values(true),
        // Inline
        Arg::new("inline")
            .long("inline")
            .value_name("yes*|no")
            .help("Control whether inlining is permitted, including overriding @inline declarations")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Check bounds
        Arg::new("check-bounds")
            .long("check-bounds")
            .value_name("yes|no|auto*")
            .help("Emit bounds checks always, never, or respect @inbounds declarations ($)")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "auto"])
            .hide_possible_values(true),
        // Math mode
        Arg::new("math-mode")
            .long("math-mode")
            .value_name("ieee|user*")
            .help("Always follow `ieee` floating point semantics or respect `@fastmath` declarations")
            .action(ArgAction::Set)
            .value_parser(["ieee", "user"])
            .hide_possible_values(true),
        // Code coverage
        Arg::new("code-coverage")
            .long("code-coverage")
            .value_name("none*|user|all")
            .help("Count executions of source lines (omitting setting is equivalent to `user`)")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .default_missing_value("user"),
        // Track allocation
        Arg::new("track-allocation")
            .long("track-allocation")
            .value_name("none*|user|all")
            .help("Count bytes allocated by each source line (omitting setting is equivalent to `user`)")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .default_missing_value("user"),
        // Bug report
        Arg::new("bug-report")
            .long("bug-report")
            .value_name("KIND")
            .help("Launch a bug report session. It can be used to start a REPL, run a script, or evaluate expressions. It first tries to use BugReporting.jl installed in current environment and fallbacks to the latest compatible BugReporting.jl if not. For more information, see --bug-report=help.")
            .action(ArgAction::Set),
        // Help raw (pass through to Julia's native help)
        Arg::new("help-raw")
            .long("help-raw")
            .help("Print Julia's native help menu (bypasses this formatted help)")
            .action(ArgAction::SetTrue),
        // Help hidden raw (pass through to Julia's native hidden help)
        Arg::new("help-hidden-raw")
            .long("help-hidden-raw")
            .help("Print Julia's native hidden help menu (bypasses this formatted help)")
            .action(ArgAction::SetTrue),
        // Generate completions
        Arg::new("generate-completions")
            .long("generate-completions")
            .value_name("bash|zsh|fish|elvish|powershell|nushell")
            .help("Generate shell completions for the specified shell")
            .action(ArgAction::Set)
            .value_parser(["bash", "zsh", "fish", "elvish", "powershell", "nushell"])
            .hide_possible_values(true),
        // Heap size hint
        Arg::new("heap-size-hint")
            .long("heap-size-hint")
            .value_name("<size>[<unit>]")
            .help("Forces garbage collection if memory usage is higher than the given value. The value may be specified as a number of bytes, optionally in units of: B, K (kibibytes), M (mebibytes), G (gibibytes), T (tebibytes), or % (percentage of physical memory).")
            .action(ArgAction::Set),
    ];

    // Hidden args
    let hidden_args = vec![
        // Code coverage with path
        Arg::new("code-coverage-path")
            .long("code-coverage")
            .value_name("@<path>")
            .help("Count executions but only in files that fall under the given file path/directory. The `@` prefix is required to select this option. A `@` with no path will track the current directory.")
            .action(ArgAction::Set)
            .conflicts_with("code-coverage"),
        // Code coverage tracefile
        Arg::new("code-coverage-tracefile")
            .long("code-coverage")
            .value_name("tracefile.info")
            .help("Append coverage information to the LCOV tracefile (filename supports format tokens)")
            .action(ArgAction::Set)
            .conflicts_with_all(&["code-coverage", "code-coverage-path"]),
        // Track allocation with path
        Arg::new("track-allocation-path")
            .long("track-allocation")
            .value_name("@<path>")
            .help("Count bytes but only in files that fall under the given file path/directory. The `@` prefix is required to select this option. A `@` with no path will track the current directory.")
            .action(ArgAction::Set)
            .conflicts_with("track-allocation"),
        // Compile
        Arg::new("compile")
            .long("compile")
            .value_name("yes*|no|all|min")
            .help("Enable or disable JIT compiler, or request exhaustive or minimal compilation")
            .action(ArgAction::Set)
            .value_parser(["yes", "no", "all", "min"])
            .hide_possible_values(true),
        // Output-o
        Arg::new("output-o")
            .long("output-o")
            .value_name("name")
            .help("Generate an object file (including system image data)")
            .action(ArgAction::Set),
        // Output-ji
        Arg::new("output-ji")
            .long("output-ji")
            .value_name("name")
            .help("Generate a system image data file (.ji)")
            .action(ArgAction::Set),
        // Strip metadata
        Arg::new("strip-metadata")
            .long("strip-metadata")
            .help("Remove docstrings and source location info from system image")
            .action(ArgAction::SetTrue),
        // Strip IR
        Arg::new("strip-ir")
            .long("strip-ir")
            .help("Remove IR (intermediate representation) of compiled functions")
            .action(ArgAction::SetTrue),
        // Experimental
        Arg::new("experimental")
            .long("experimental")
            .help("Enable the use of experimental (alpha) features")
            .action(ArgAction::SetTrue),
        // Output unopt bc
        Arg::new("output-unopt-bc")
            .long("output-unopt-bc")
            .value_name("name")
            .help("Generate unoptimized LLVM bitcode (.bc)")
            .action(ArgAction::Set),
        // Output bc
        Arg::new("output-bc")
            .long("output-bc")
            .value_name("name")
            .help("Generate LLVM bitcode (.bc)")
            .action(ArgAction::Set),
        // Output asm
        Arg::new("output-asm")
            .long("output-asm")
            .value_name("name")
            .help("Generate an assembly file (.s)")
            .action(ArgAction::Set),
        // Output incremental
        Arg::new("output-incremental")
            .long("output-incremental")
            .value_name("yes|no*")
            .help("Generate an incremental output file (rather than complete)")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Timeout for safepoint straggler
        Arg::new("timeout-for-safepoint-straggler")
            .long("timeout-for-safepoint-straggler")
            .value_name("seconds")
            .help("If this value is set, then we will dump the backtrace for a thread that fails to reach a safepoint within the specified time")
            .action(ArgAction::Set),
        // Trace compile
        Arg::new("trace-compile")
            .long("trace-compile")
            .value_name("{stderr|name}")
            .help("Print precompile statements for methods compiled during execution or save to stderr or a path. Methods that were recompiled are printed in yellow or with a trailing comment if color is not supported")
            .action(ArgAction::Set),
        // Trace compile timing
        Arg::new("trace-compile-timing")
            .long("trace-compile-timing")
            .help("If --trace-compile is enabled show how long each took to compile in ms")
            .action(ArgAction::SetTrue),
        // Task metrics
        Arg::new("task-metrics")
            .long("task-metrics")
            .value_name("yes|no*")
            .help("Enable collection of per-task timing data.")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Image codegen
        Arg::new("image-codegen")
            .long("image-codegen")
            .help("Force generate code in imaging mode")
            .action(ArgAction::SetTrue),
        // Permalloc pkgimg
        Arg::new("permalloc-pkgimg")
            .long("permalloc-pkgimg")
            .value_name("yes|no*")
            .help("Copy the data section of package images into memory")
            .action(ArgAction::Set)
            .value_parser(["yes", "no"])
            .hide_possible_values(true),
        // Trim
        Arg::new("trim")
            .long("trim")
            .value_name("no*|safe|unsafe|unsafe-warn")
            .help("Build a sysimage including only code provably reachable from methods marked by calling `entrypoint`. In unsafe mode, the resulting binary might be missing needed code and can throw errors. With unsafe-warn warnings will be printed for dynamic call sites that might lead to such errors. In safe mode compile-time errors are given instead.")
            .action(ArgAction::Set)
            .value_parser(["no", "safe", "unsafe", "unsafe-warn"])
            .hide_possible_values(true),
    ];

    // Add normal args (hide them if only_hidden is true)
    for arg in normal_args {
        cmd = cmd.arg(arg.hide(only_hidden));
    }

    // Add hidden args (hide them if only_hidden is false, i.e., show when true)
    for arg in hidden_args {
        cmd = cmd.arg(arg.hide(!only_hidden));
    }

    // Add positional arguments
    cmd = cmd
        .arg(
            Arg::new("programfile")
                .help("Julia script to execute")
                .action(ArgAction::Set)
                .index(1),
        )
        .arg(
            Arg::new("args")
                .help("Arguments to pass to the Julia script")
                .action(ArgAction::Append)
                .index(2)
                .trailing_var_arg(true)
                .allow_hyphen_values(true),
        );

    cmd
}
