/// Shows a desktop notification. Best effort: a failed toast must never stop
/// time tracking, so implementations report problems themselves.
pub trait Notifier {
    fn notify(&self, title: &str, body: &str);
}
