//! Gives the Windows executable its icon and file details (Explorer, Start menu).

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icon.ico")
            .set("ProductName", "WFH Tracker")
            .set("FileDescription", "WFH Tracker");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the app icon: {e}");
        }
    }
}
