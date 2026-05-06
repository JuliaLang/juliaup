use anyhow::Result;

mod app;

fn main() -> Result<()> {
    human_panic::setup_panic!(
        human_panic::Metadata::new("Juliaup GUI", env!("CARGO_PKG_VERSION"))
            .support("https://github.com/JuliaLang/juliaup")
    );

    let paths = juliaup::global_paths::get_paths()?;
    app::run(paths)
}
