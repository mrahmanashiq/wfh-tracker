//! Shows one desktop notification the way the app does, then waits so the
//! toast has time to appear: `cargo run -p wt-platform --example notify_test`

use wt_core::Notifier;
use wt_platform::DesktopNotifier;

fn main() {
    let dir = std::env::temp_dir().join("wfh-tracker-notify-test");
    let _ = std::fs::create_dir_all(&dir);
    if let Err(e) = wt_platform::register_notifications(&dir) {
        println!("registration failed: {e}");
    }
    DesktopNotifier.notify(
        "WFH Tracker test",
        "If you can read this, notifications work.",
    );
    std::thread::sleep(std::time::Duration::from_secs(8));
    println!("sent");
}
