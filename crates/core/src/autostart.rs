/// Start-at-login switch, implemented per OS.
pub trait Autostart {
    fn is_enabled(&self) -> Result<bool, AutostartError>;
    fn set_enabled(&self, enabled: bool) -> Result<(), AutostartError>;
}

#[derive(Debug, thiserror::Error)]
#[error("start at login: {0}")]
pub struct AutostartError(pub String);
