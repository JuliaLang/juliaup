fn main() {
    // Explorer, the taskbar and the MSI start menu shortcut take the icon
    // from the executable; the MSIX package carries its own tile images.
    #[cfg(windows)]
    {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let mut res = winres::WindowsResource::new();
        res.set_icon(
            &manifest_dir
                .join("../src/icons/juliaup.ico")
                .to_string_lossy(),
        );
        let rc = out_dir.join("resource.rc");
        res.write_resource_file(&rc).unwrap();
        // See the root build script for why this is not `res.compile()`.
        embed_resource::compile(&rc, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
