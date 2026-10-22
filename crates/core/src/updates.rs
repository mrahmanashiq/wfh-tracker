/// A newer version than the one running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub version: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct UpdateError(pub String);

/// Finds, fetches and installs new versions of the app. Calls may block on
/// the network, so callers run them off the UI thread.
pub trait Updater: Send + Sync {
    fn current_version(&self) -> &str;
    /// Looks for a newer version than the running one.
    fn check(&self) -> Result<Option<AvailableUpdate>, UpdateError>;
    /// Downloads and verifies the update found by the last [`Self::check`].
    fn download(&self) -> Result<(), UpdateError>;
    /// Installs the downloaded update and starts the new version. On success
    /// the running app exits, so this only returns on failure (or in fakes).
    fn install_and_relaunch(&self) -> Result<(), UpdateError>;
}
