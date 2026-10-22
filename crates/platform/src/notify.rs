use wt_core::Notifier;

/// Windows toasts / macOS notification centre.
#[derive(Debug, Default)]
pub struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn notify(&self, title: &str, body: &str) {
        let (title, body) = (title.to_owned(), body.to_owned());
        // Showing a toast can block briefly; keep it off the UI thread.
        std::thread::spawn(move || {
            let shown = notify_rust::Notification::new()
                .appname("WFH Tracker")
                .summary(&title)
                .body(&body)
                .show();
            if let Err(e) = shown {
                eprintln!("wfh-tracker: notification failed: {e}");
            }
        });
    }
}
