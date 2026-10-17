//! Gives the Windows executable its icon and version details.

fn main() {
    println!("cargo:rerun-if-changed=brand/ico/overlay.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("brand/ico/overlay.ico")
            .set("ProductName", "Overlay")
            .set("FileDescription", "Overlay")
            .set("LegalCopyright", "");
        if let Err(e) = res.compile() {
            // A missing resource compiler should not stop the build; the app just has no file icon.
            println!("cargo:warning=Could not embed the icon: {e}");
        }
    }
}
