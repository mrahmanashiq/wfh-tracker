use wt_core::{Clipboard, ClipboardError};

/// The system clipboard.
#[derive(Debug, Default)]
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn set_text(&self, text: &str) -> Result<(), ClipboardError> {
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(text))
            .map_err(|e| ClipboardError(e.to_string()))
    }
}
