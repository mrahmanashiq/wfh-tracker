use wt_core::CallSensor;

/// Where Windows records each app's microphone use (what drives the taskbar
/// mic icon). An app holds the mic right now when its `LastUsedTimeStop` is 0.
#[cfg(windows)]
const MIC_USE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

/// Reads whether Teams holds the microphone: the new Teams (a packaged app,
/// `MSTeams_…`) or classic Teams (`…#Teams.exe` under `NonPackaged`). Only
/// those entries are looked at; no app names or paths are stored or logged.
#[derive(Debug, Default)]
pub struct TeamsCallSensor;

impl CallSensor for TeamsCallSensor {
    #[cfg(windows)]
    fn in_call(&self) -> bool {
        let Ok(root) = windows_registry::CURRENT_USER.open(MIC_USE) else {
            return false;
        };
        let in_use = |parent: &windows_registry::Key, is_teams: fn(&str) -> bool| {
            parent.keys().is_ok_and(|mut names| {
                names.any(|name| {
                    is_teams(&name)
                        && parent.open(&name).is_ok_and(|app| {
                            app.get_u64("LastUsedTimeStart").is_ok_and(|t| t != 0)
                                && app.get_u64("LastUsedTimeStop").is_ok_and(|t| t == 0)
                        })
                })
            })
        };
        in_use(&root, |name| name.starts_with("MSTeams_"))
            || root.open("NonPackaged").is_ok_and(|classic| {
                in_use(&classic, |name| {
                    name.to_ascii_lowercase().ends_with("#teams.exe")
                })
            })
    }

    #[cfg(not(windows))]
    fn in_call(&self) -> bool {
        false
    }
}
