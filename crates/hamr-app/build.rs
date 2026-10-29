//! Embeds the app icon in the Windows executable, so Explorer and the taskbar show it.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/hamr.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/hamr.ico");
        // A missing resource compiler should not stop a developer build; the release
        // workflow checks the icon is there.
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
