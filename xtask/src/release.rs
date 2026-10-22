//! `cargo xtask icon` and `cargo xtask release`.

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../../crates/platform/src/icon_art.rs"]
mod icon_art;

/// GitHub repository the app updates from (see `UPDATE_MANIFEST` in the app).
const REPO: &str = "mrahmanashiq/wfh-tracker";

/// Writes `packaging/icon.ico` (16–256 px) from the code-drawn app icon.
pub fn write_icon(root: &Path) -> Result<PathBuf, String> {
    let path = root.join("packaging").join("icon.ico");
    std::fs::write(&path, icon_art::app_icon_ico()).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Builds and signs the installer, then writes the update manifest next to it.
/// With `publish`, uploads both to a new GitHub release `v<version>`.
pub fn release(root: &Path, publish: bool) -> Result<(), String> {
    let version = workspace_version(root)?;
    let key = signing_key()?;
    write_icon(root)?;

    let out = root.join("dist");
    let _ = std::fs::remove_dir_all(&out);
    run(Command::new("cargo")
        .args([
            "packager",
            "--release",
            "-p",
            "wfh-tracker",
            "-f",
            "nsis",
            "-o",
        ])
        .arg(&out)
        .arg("-k")
        .arg(&key)
        // Without a password variable the packager waits for one on stdin.
        .env(
            "CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD",
            std::env::var("CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD").unwrap_or_default(),
        )
        .stdin(std::process::Stdio::null())
        .current_dir(root))?;

    let setup = find(&out, "-setup.exe")?;
    let signature =
        std::fs::read_to_string(find(&out, "-setup.exe.sig")?).map_err(|e| e.to_string())?;
    let file_name = setup
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad installer name")?
        .replace(' ', ".");
    let manifest = format!(
        r#"{{
  "version": "{version}",
  "notes": "See https://github.com/{REPO}/releases/tag/v{version}",
  "pub_date": "{date}",
  "platforms": {{
    "windows-x86_64": {{
      "url": "https://github.com/{REPO}/releases/download/v{version}/{file_name}",
      "signature": "{signature}",
      "format": "nsis"
    }}
  }}
}}
"#,
        date = jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%SZ"),
        signature = signature.trim(),
    );
    let manifest_path = out.join("latest.json");
    std::fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;
    println!("xtask: built {}", setup.display());
    println!("xtask: wrote {}", manifest_path.display());

    if publish {
        run(Command::new("gh")
            .args(["release", "create", &format!("v{version}"), "--repo", REPO])
            .args([
                "--title",
                &format!("WFH Tracker {version}"),
                "--generate-notes",
            ])
            .arg(&setup)
            .arg(&manifest_path)
            .current_dir(root))?;
    } else {
        println!("xtask: not published (run `cargo xtask release --publish` to upload to GitHub)");
    }
    Ok(())
}

/// The private signing key: `WFH_SIGNING_KEY`, else the default location
/// outside the repository.
fn signing_key() -> Result<PathBuf, String> {
    let path = std::env::var_os("WFH_SIGNING_KEY").map_or_else(
        || {
            let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
            home.map(|h| {
                PathBuf::from(h)
                    .join(".wfh-tracker-signing")
                    .join("updater.key")
            })
            .unwrap_or_default()
        },
        PathBuf::from,
    );
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "signing key not found at {} (set WFH_SIGNING_KEY)",
            path.display()
        ))
    }
}

fn workspace_version(root: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    manifest
        .split("[workspace.package]")
        .nth(1)
        .and_then(|section| {
            section
                .lines()
                .find_map(|l| l.trim().strip_prefix("version = \""))
        })
        .and_then(|v| v.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "no version in [workspace.package]".to_owned())
}

fn find(dir: &Path, suffix: &str) -> Result<PathBuf, String> {
    std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(suffix))
        .ok_or_else(|| format!("no *{suffix} in {}", dir.display()))
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} failed"))
    }
}
