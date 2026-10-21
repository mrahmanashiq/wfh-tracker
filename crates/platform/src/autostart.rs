use auto_launch::{AutoLaunch, AutoLaunchBuilder, WindowsEnableMode};
use wt_core::{Autostart, AutostartError};

use crate::{MINIMIZED_FLAG, PlatformError};

/// Start-at-login for the current user only (Windows `HKCU\...\Run`), so it
/// never needs admin rights.
pub struct LoginAutostart(AutoLaunch);

impl LoginAutostart {
    pub fn for_current_exe() -> Result<Self, PlatformError> {
        let exe = std::env::current_exe()?;
        let exe = exe.to_str().ok_or(PlatformError::NonUtf8Path)?.to_owned();
        AutoLaunchBuilder::new()
            .set_app_name("WFH Tracker")
            .set_app_path(&exe)
            .set_args(&[MINIMIZED_FLAG])
            .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
            .build()
            .map(Self)
            .map_err(|e| PlatformError::Autostart(e.to_string()))
    }
}

impl Autostart for LoginAutostart {
    fn is_enabled(&self) -> Result<bool, AutostartError> {
        self.0
            .is_enabled()
            .map_err(|e| AutostartError(e.to_string()))
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), AutostartError> {
        if enabled == self.is_enabled()? {
            return Ok(());
        }
        let result = if enabled {
            self.0.enable()
        } else {
            self.0.disable()
        };
        result.map_err(|e| AutostartError(e.to_string()))
    }
}
