use anyhow::Result;

fn main() -> Result<std::process::ExitCode> {
    juliaup::julia_launcher::main_impl()
}