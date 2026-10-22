/// Puts text on the system clipboard.
pub trait Clipboard {
    fn set_text(&self, text: &str) -> Result<(), ClipboardError>;
}

#[derive(Debug, thiserror::Error)]
#[error("clipboard: {0}")]
pub struct ClipboardError(pub String);
