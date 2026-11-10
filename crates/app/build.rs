// Puts the icon into the Windows executable (shown in Explorer and the taskbar).
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.set("ProductName", "git-tabs");
        res.set("FileDescription", "git-tabs");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the icon: {e}");
        }
    }
}
