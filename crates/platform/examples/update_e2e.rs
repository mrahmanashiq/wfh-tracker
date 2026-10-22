//! End-to-end check of the update path against the real GitHub release:
//! manifest → download → signature check → (optionally) install + relaunch.
//!
//! ```text
//! cargo run -p wt-platform --example update_e2e -- 0.0.1             # check + download
//! cargo run -p wt-platform --example update_e2e -- 0.0.1 <pubkey>    # with another key: must fail
//! cargo run -p wt-platform --example update_e2e -- 0.0.1 --install   # also install and relaunch
//! ```

use std::process::ExitCode;

use wt_core::Updater;
use wt_platform::ReleaseUpdater;

const MANIFEST: &str =
    "https://github.com/mrahmanashiq/wfh-tracker/releases/latest/download/latest.json";
const PUBLIC_KEY: &str = include_str!("../../../packaging/updater.key.pub");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pretend = args.first().map_or("0.0.1", String::as_str);
    let install = args.iter().any(|a| a == "--install");
    let key = match args.get(1).filter(|a| *a != "--install") {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(k) => k,
            Err(e) => return fail(&format!("reading {path}: {e}")),
        },
        None => PUBLIC_KEY.to_owned(),
    };

    let updater = match ReleaseUpdater::new(pretend, MANIFEST, &key) {
        Ok(u) => u,
        Err(e) => return fail(&e.to_string()),
    };
    println!("pretending to run {pretend}");
    let found = match updater.check() {
        Ok(Some(found)) => found,
        Ok(None) => return fail("no update offered"),
        Err(e) => return fail(&format!("check: {e}")),
    };
    println!("CHECK ok: offered {}", found.version);
    if let Err(e) = updater.download() {
        return fail(&format!("download: {e}"));
    }
    println!("DOWNLOAD ok: signature verified");

    if install {
        println!("INSTALL: starting the installer (passive, relaunch); this process exits now");
        if let Err(e) = updater.install_and_relaunch() {
            return fail(&format!("install: {e}"));
        }
    }
    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode {
    println!("FAILED: {message}");
    ExitCode::FAILURE
}
