use std::sync::Mutex;

use cargo_packager_updater::semver::Version;
use cargo_packager_updater::url::Url;
use cargo_packager_updater::{
    Config, Update, UpdaterBuilder, WindowsConfig, WindowsUpdateInstallMode,
};
use wt_core::{AvailableUpdate, UpdateError, Updater};

use crate::PlatformError;

/// Updates from a release manifest (e.g. on GitHub Releases). Every download
/// is checked against the signing public key before it may be installed.
pub struct ReleaseUpdater {
    current: String,
    config: Config,
    found: Mutex<Option<Found>>,
}

/// The update found by the last check, and its bytes once downloaded.
struct Found {
    update: Update,
    bytes: Option<Vec<u8>>,
}

impl ReleaseUpdater {
    pub fn new(current: &str, manifest_url: &str, public_key: &str) -> Result<Self, PlatformError> {
        let endpoint = Url::parse(manifest_url).map_err(update_error)?;
        Ok(Self {
            current: current.to_owned(),
            config: Config {
                endpoints: vec![endpoint],
                pubkey: public_key.trim().to_owned(),
                windows: Some(WindowsConfig {
                    installer_args: None,
                    // A progress bar only; the installer relaunches the app when done.
                    install_mode: Some(WindowsUpdateInstallMode::Passive),
                }),
            },
            found: Mutex::new(None),
        })
    }

    fn found(&self) -> Result<std::sync::MutexGuard<'_, Option<Found>>, UpdateError> {
        self.found
            .lock()
            .map_err(|_| UpdateError("the updater is in a bad state".to_owned()))
    }
}

impl Updater for ReleaseUpdater {
    fn current_version(&self) -> &str {
        &self.current
    }

    fn check(&self) -> Result<Option<AvailableUpdate>, UpdateError> {
        let current = Version::parse(&self.current).map_err(|e| UpdateError(e.to_string()))?;
        let update = UpdaterBuilder::new(current, self.config.clone())
            .build()
            .and_then(|u| u.check())
            .map_err(|e| UpdateError(e.to_string()))?;
        let available = update.as_ref().map(|u| AvailableUpdate {
            version: u.version.clone(),
        });
        *self.found()? = update.map(|update| Found {
            update,
            bytes: None,
        });
        Ok(available)
    }

    fn download(&self) -> Result<(), UpdateError> {
        let mut found = self.found()?;
        let found = found
            .as_mut()
            .ok_or_else(|| UpdateError("no update to download".to_owned()))?;
        // `download` verifies the signature before returning the bytes.
        found.bytes = Some(
            found
                .update
                .download()
                .map_err(|e| UpdateError(e.to_string()))?,
        );
        Ok(())
    }

    fn install_and_relaunch(&self) -> Result<(), UpdateError> {
        let Found { update, bytes } = self
            .found()?
            .take()
            .ok_or_else(|| UpdateError("no update has been downloaded".to_owned()))?;
        let bytes = bytes
            .ok_or_else(|| UpdateError("the update hasn't finished downloading".to_owned()))?;
        // Starts the installer with `/P /R` (passive, relaunch) and exits this process.
        update
            .install(bytes)
            .map_err(|e| UpdateError(e.to_string()))
    }
}

fn update_error(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Update(e.to_string())
}
