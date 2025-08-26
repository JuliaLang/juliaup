use clap::{Arg, ArgAction, Command};
use crate::cli_styles::get_styles;

pub fn julia_cli() -> Command {
    Command::new("julia")
        .about("")
        .override_usage("julia [switches] -- [programfile] [args...]")
        .styles(get_styles())
        .after_help(
"Settings marked '($)' may trigger package precompilation"
        )
        // Display version information
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .help("Display version information")
                .action(ArgAction::SetTrue)
        )
        // Help
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help("Print command-line options (this message)")
                .action(ArgAction::SetTrue)
        )
        // Help hidden
        .arg(
            Arg::new("help-hidden")
                .long("help-hidden")
                .help("Print uncommon options not shown by `-h`")
                .action(ArgAction::SetTrue)
        )
        // Project
        .arg(
            Arg::new("project")
                .long("project")
                .value_name("{<dir>|@temp|@.|@script[<rel>]}")
                .help("Set <dir> as the active project/environment. Or, create a temporary environment with `@temp`. The default @. option will search through parent directories until a Project.toml or JuliaProject.toml file is found. @script is similar, but searches up from the programfile or a path relative to programfile.")
                .action(ArgAction::Set)
                .num_args(0..=1)
        )
        // Sysimage
        .arg(
            Arg::new("sysimage")
                .short('J')
                .long("sysimage")
                .value_name("file")
                .help("Start up with the given system image file")
                .action(ArgAction::Set)
        )
        // Home
        .arg(
            Arg::new("home")
                .short('H')
                .long("home")
                .value_name("dir")
                .help("Set location of `julia` executable")
                .action(ArgAction::Set)
        )
        // Startup file
        .arg(
            Arg::new("startup-file")
                .long("startup-file")
                .value_name("yes|no")
                .help("Load `JULIA_DEPOT_PATH/config/startup.jl`; if `JULIA_DEPOT_PATH` environment variable is unset, load `~/.julia/config/startup.jl`")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // Handle signals
        .arg(
            Arg::new("handle-signals")
                .long("handle-signals")
                .value_name("yes|no")
                .help("Enable or disable Julia's default signal handlers")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // Sysimage native code
        .arg(
            Arg::new("sysimage-native-code")
                .long("sysimage-native-code")
                .value_name("yes|no")
                .help("Use native code from system image if available")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // Compiled modules
        .arg(
            Arg::new("compiled-modules")
                .long("compiled-modules")
                .value_name("yes|no|existing|strict")
                .help("Enable or disable incremental precompilation of modules. The `existing` option allows use of existing compiled modules that were previously precompiled, but disallows creation of new precompile files. The `strict` option is similar, but will error if no precompile file is found.")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "existing", "strict"])
                .default_value("yes")
        )
        // Pkgimages
        .arg(
            Arg::new("pkgimages")
                .long("pkgimages")
                .value_name("yes|no|existing")
                .help("Enable or disable usage of native code caching in the form of pkgimages. The `existing` option allows use of existing pkgimages but disallows creation of new ones ($)")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "existing"])
                .default_value("yes")
        )
        // Eval
        .arg(
            Arg::new("eval")
                .short('e')
                .long("eval")
                .value_name("expr")
                .help("Evaluate <expr>")
                .action(ArgAction::Set)
                .allow_hyphen_values(true)
        )
        // Print
        .arg(
            Arg::new("print")
                .short('E')
                .long("print")
                .value_name("expr")
                .help("Evaluate <expr> and display the result")
                .action(ArgAction::Set)
                .allow_hyphen_values(true)
        )
        // Module (NEW!)
        .arg(
            Arg::new("module")
                .short('m')
                .long("module")
                .value_name("Package")
                .help("Run entry point of `Package` (`@main` function) with `args'.")
                .action(ArgAction::Set)
                .allow_hyphen_values(true)
        )
        // Load
        .arg(
            Arg::new("load")
                .short('L')
                .long("load")
                .value_name("file")
                .help("Load <file> immediately on all processors")
                .action(ArgAction::Set)
        )
        // Threads
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .value_name("{auto|N[,auto|M]}")
                .help("Enable N[+M] threads; N threads are assigned to the `default` threadpool, and if M is specified, M threads are assigned to the `interactive` threadpool; `auto` tries to infer a useful default number of threads to use but the exact behavior might change in the future. Currently sets N to the number of CPUs assigned to this Julia process based on the OS-specific affinity assignment interface if supported (Linux and Windows) or to the number of CPU threads if not supported (MacOS) or if process affinity is not configured, and sets M to 1.")
                .action(ArgAction::Set)
        )
        // GC threads
        .arg(
            Arg::new("gcthreads")
                .long("gcthreads")
                .value_name("N[,M]")
                .help("Use N threads for the mark phase of GC and M (0 or 1) threads for the concurrent sweeping phase of GC. N is set to the number of compute threads and M is set to 0 if unspecified.")
                .action(ArgAction::Set)
        )
        // Procs
        .arg(
            Arg::new("procs")
                .short('p')
                .long("procs")
                .value_name("{N|auto}")
                .help("Integer value N launches N additional local worker processes `auto` launches as many workers as the number of local CPU threads (logical cores).")
                .action(ArgAction::Set)
        )
        // Machine file
        .arg(
            Arg::new("machine-file")
                .long("machine-file")
                .value_name("file")
                .help("Run processes on hosts listed in <file>")
                .action(ArgAction::Set)
        )
        // Interactive
        .arg(
            Arg::new("interactive")
                .short('i')
                .long("interactive")
                .help("Interactive mode; REPL runs and `isinteractive()` is true.")
                .action(ArgAction::SetTrue)
        )
        // Quiet
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Quiet startup: no banner, suppress REPL warnings")
                .action(ArgAction::SetTrue)
        )
        // Banner
        .arg(
            Arg::new("banner")
                .long("banner")
                .value_name("yes|no|short|auto")
                .help("Enable or disable startup banner")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "short", "auto"])
                .default_value("auto")
        )
        // Color
        .arg(
            Arg::new("color")
                .long("color")
                .value_name("yes|no|auto")
                .help("Enable or disable color text")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "auto"])
                .default_value("auto")
        )
        // History file
        .arg(
            Arg::new("history-file")
                .long("history-file")
                .value_name("yes|no")
                .help("Load or save history")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // Depwarn
        .arg(
            Arg::new("depwarn")
                .long("depwarn")
                .value_name("yes|no|error")
                .help("Enable or disable syntax and method deprecation warnings (`error` turns warnings into errors)")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "error"])
                .default_value("no")
        )
        // Warn overwrite
        .arg(
            Arg::new("warn-overwrite")
                .long("warn-overwrite")
                .value_name("yes|no")
                .help("Enable or disable method overwrite warnings")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("no")
        )
        // Warn scope
        .arg(
            Arg::new("warn-scope")
                .long("warn-scope")
                .value_name("yes|no")
                .help("Enable or disable warning for ambiguous top-level scope")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // CPU target
        .arg(
            Arg::new("cpu-target")
                .short('C')
                .long("cpu-target")
                .value_name("target")
                .help("Limit usage of CPU features up to <target>; set to `help` to see the available options")
                .action(ArgAction::Set)
        )
        // Optimize
        .arg(
            Arg::new("optimize")
                .short('O')
                .long("optimize")
                .value_name("0|1|2|3")
                .help("Set the optimization level (level 3 if `-O` is used without a level) ($)")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_value("2")
                .default_missing_value("3")
                .value_parser(["0", "1", "2", "3"])
        )
        // Min optlevel
        .arg(
            Arg::new("min-optlevel")
                .long("min-optlevel")
                .value_name("0|1|2|3")
                .help("Set a lower bound on the optimization level")
                .action(ArgAction::Set)
                .value_parser(["0", "1", "2", "3"])
                .default_value("0")
        )
        // Debug info
        .arg(
            Arg::new("debug-info")
                .short('g')
                .long("debug-info")
                .value_name("0|1|2")
                .help("Set the level of debug info generation (level 2 if `-g` is used without a level) ($)")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_value("1")
                .default_missing_value("2")
                .value_parser(["0", "1", "2"])
        )
        // Inline
        .arg(
            Arg::new("inline")
                .long("inline")
                .value_name("yes|no")
                .help("Control whether inlining is permitted, including overriding @inline declarations")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("yes")
        )
        // Check bounds
        .arg(
            Arg::new("check-bounds")
                .long("check-bounds")
                .value_name("yes|no|auto")
                .help("Emit bounds checks always, never, or respect @inbounds declarations ($)")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "auto"])
                .default_value("auto")
        )
        // Math mode
        .arg(
            Arg::new("math-mode")
                .long("math-mode")
                .value_name("ieee|user")
                .help("Always follow `ieee` floating point semantics or respect `@fastmath` declarations")
                .action(ArgAction::Set)
                .value_parser(["ieee", "user"])
                .default_value("user")
        )
        // Code coverage
        .arg(
            Arg::new("code-coverage")
                .long("code-coverage")
                .value_name("none|user|all")
                .help("Count executions of source lines (omitting setting is equivalent to `user`)")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_value("none")
                .default_missing_value("user")
        )
        // Code coverage with path
        .arg(
            Arg::new("code-coverage-path")
                .long("code-coverage")
                .value_name("@<path>")
                .help("Count executions but only in files that fall under the given file path/directory. The `@` prefix is required to select this option. A `@` with no path will track the current directory.")
                .action(ArgAction::Set)
                .conflicts_with("code-coverage")
                .hide(true)
        )
        // Code coverage tracefile
        .arg(
            Arg::new("code-coverage-tracefile")
                .long("code-coverage")
                .value_name("tracefile.info")
                .help("Append coverage information to the LCOV tracefile (filename supports format tokens)")
                .action(ArgAction::Set)
                .conflicts_with_all(&["code-coverage", "code-coverage-path"])
                .hide(true)
        )
        // Track allocation
        .arg(
            Arg::new("track-allocation")
                .long("track-allocation")
                .value_name("none|user|all")
                .help("Count bytes allocated by each source line (omitting setting is equivalent to `user`)")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_value("none")
                .default_missing_value("user")
        )
        // Track allocation with path
        .arg(
            Arg::new("track-allocation-path")
                .long("track-allocation")
                .value_name("@<path>")
                .help("Count bytes but only in files that fall under the given file path/directory. The `@` prefix is required to select this option. A `@` with no path will track the current directory.")
                .action(ArgAction::Set)
                .conflicts_with("track-allocation")
                .hide(true)
        )
        // Bug report
        .arg(
            Arg::new("bug-report")
                .long("bug-report")
                .value_name("KIND")
                .help("Launch a bug report session. It can be used to start a REPL, run a script, or evaluate expressions. It first tries to use BugReporting.jl installed in current environment and fallbacks to the latest compatible BugReporting.jl if not. For more information, see --bug-report=help.")
                .action(ArgAction::Set)
        )
        // Heap size hint
        .arg(
            Arg::new("heap-size-hint")
                .long("heap-size-hint")
                .value_name("<size>[<unit>]")
                .help("Forces garbage collection if memory usage is higher than the given value. The value may be specified as a number of bytes, optionally in units of: B, K (kibibytes), M (mebibytes), G (gibibytes), T (tebibytes), or % (percentage of physical memory).")
                .action(ArgAction::Set)
        )
        // ========= HIDDEN OPTIONS =========
        // Compile
        .arg(
            Arg::new("compile")
                .long("compile")
                .value_name("yes|no|all|min")
                .help("Enable or disable JIT compiler, or request exhaustive or minimal compilation")
                .action(ArgAction::Set)
                .value_parser(["yes", "no", "all", "min"])
                .default_value("yes")
                .hide(true)
        )
        // Output-o
        .arg(
            Arg::new("output-o")
                .long("output-o")
                .value_name("name")
                .help("Generate an object file (including system image data)")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Output-ji
        .arg(
            Arg::new("output-ji")
                .long("output-ji")
                .value_name("name")
                .help("Generate a system image data file (.ji)")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Strip metadata
        .arg(
            Arg::new("strip-metadata")
                .long("strip-metadata")
                .help("Remove docstrings and source location info from system image")
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        // Strip IR
        .arg(
            Arg::new("strip-ir")
                .long("strip-ir")
                .help("Remove IR (intermediate representation) of compiled functions")
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        // Experimental (NEW!)
        .arg(
            Arg::new("experimental")
                .long("experimental")
                .help("Enable the use of experimental (alpha) features")
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        // Output unopt bc
        .arg(
            Arg::new("output-unopt-bc")
                .long("output-unopt-bc")
                .value_name("name")
                .help("Generate unoptimized LLVM bitcode (.bc)")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Output bc
        .arg(
            Arg::new("output-bc")
                .long("output-bc")
                .value_name("name")
                .help("Generate LLVM bitcode (.bc)")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Output asm
        .arg(
            Arg::new("output-asm")
                .long("output-asm")
                .value_name("name")
                .help("Generate an assembly file (.s)")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Output incremental
        .arg(
            Arg::new("output-incremental")
                .long("output-incremental")
                .value_name("yes|no")
                .help("Generate an incremental output file (rather than complete)")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("no")
                .hide(true)
        )
        // Timeout for safepoint straggler (NEW!)
        .arg(
            Arg::new("timeout-for-safepoint-straggler")
                .long("timeout-for-safepoint-straggler")
                .value_name("seconds")
                .help("If this value is set, then we will dump the backtrace for a thread that fails to reach a safepoint within the specified time")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Trace compile
        .arg(
            Arg::new("trace-compile")
                .long("trace-compile")
                .value_name("{stderr|name}")
                .help("Print precompile statements for methods compiled during execution or save to stderr or a path. Methods that were recompiled are printed in yellow or with a trailing comment if color is not supported")
                .action(ArgAction::Set)
                .hide(true)
        )
        // Trace compile timing (NEW!)
        .arg(
            Arg::new("trace-compile-timing")
                .long("trace-compile-timing")
                .help("If --trace-compile is enabled show how long each took to compile in ms")
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        // Task metrics (NEW!)
        .arg(
            Arg::new("task-metrics")
                .long("task-metrics")
                .value_name("yes|no")
                .help("Enable collection of per-task timing data.")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("no")
                .hide(true)
        )
        // Image codegen
        .arg(
            Arg::new("image-codegen")
                .long("image-codegen")
                .help("Force generate code in imaging mode")
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        // Permalloc pkgimg
        .arg(
            Arg::new("permalloc-pkgimg")
                .long("permalloc-pkgimg")
                .value_name("yes|no")
                .help("Copy the data section of package images into memory")
                .action(ArgAction::Set)
                .value_parser(["yes", "no"])
                .default_value("no")
                .hide(true)
        )
        // Trim (NEW!)
        .arg(
            Arg::new("trim")
                .long("trim")
                .value_name("no|safe|unsafe|unsafe-warn")
                .help("Build a sysimage including only code provably reachable from methods marked by calling `entrypoint`. In unsafe mode, the resulting binary might be missing needed code and can throw errors. With unsafe-warn warnings will be printed for dynamic call sites that might lead to such errors. In safe mode compile-time errors are given instead.")
                .action(ArgAction::Set)
                .value_parser(["no", "safe", "unsafe", "unsafe-warn"])
                .default_value("no")
                .hide(true)
        )
        // Positional arguments
        .arg(
            Arg::new("programfile")
                .help("Julia script to execute")
                .action(ArgAction::Set)
                .index(1)
        )
        .arg(
            Arg::new("args")
                .help("Arguments to pass to the Julia script")
                .action(ArgAction::Append)
                .index(2)
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
        )
}