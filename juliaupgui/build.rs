fn main() {
    // Explorer, the taskbar and the MSI start menu shortcut take the icon
    // from the executable; the MSIX package carries its own tile images.
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../src/icons/juliaup.ico");
        res.compile().unwrap();
    }
}
