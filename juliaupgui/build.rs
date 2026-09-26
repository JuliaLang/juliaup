// Explorer, the taskbar and the MSI start menu shortcut take the icon from
// the executable; the MSIX package carries its own tile images. See the root
// build script for why the resource is linked this way.
#[cfg(windows)]
const WINRES_CHILD_ENV: &str = "JULIAUPGUI_BUILD_WINRES_CHILD";

fn main() {
    #[cfg(windows)]
    {
        if std::env::var_os(WINRES_CHILD_ENV).is_some() {
            let mut res = winres::WindowsResource::new();
            res.set_icon("../src/icons/juliaup.ico");
            res.compile().unwrap();
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .env(WINRES_CHILD_ENV, "1")
            .output()
            .expect("failed to run the resource compiler child process");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let search = stdout
            .lines()
            .find_map(|l| l.strip_prefix("cargo:rustc-link-search=native="));
        let search = match (output.status.success(), search) {
            (true, Some(search)) => search,
            _ => panic!(
                "resource compilation failed:\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
        let file = if std::env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc" {
            "resource.lib"
        } else {
            "libresource.a"
        };
        println!(
            "cargo:rustc-link-arg-bins={}",
            std::path::Path::new(search).join(file).display()
        );
    }
}
