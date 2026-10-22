//! `cargo xtask ci` — everything that must pass before a commit.

mod layering;
mod release;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        Some("layering") => report(check_layering()),
        Some("icon") => report(
            release::write_icon(&root())
                .map(|p| println!("xtask: wrote {}", p.display()))
                .map_err(|e| vec![e]),
        ),
        Some("release") => {
            let publish = std::env::args().any(|a| a == "--publish");
            report(release::release(&root(), publish).map_err(|e| vec![e]))
        }
        _ => {
            eprintln!("usage: cargo xtask <ci|layering|icon|release [--publish]>");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> ExitCode {
    let steps: [(&str, &[&str]); 3] = [
        ("format", &["fmt", "--all", "--check"]),
        (
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("tests", &["test", "--workspace"]),
    ];
    for (name, args) in steps {
        println!("xtask: {name}");
        let ok = Command::new(env!("CARGO"))
            .args(args)
            .current_dir(root())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("xtask: {name} failed");
            return ExitCode::FAILURE;
        }
    }
    println!("xtask: layering");
    report(check_layering())
}

fn check_layering() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for (dir, rule) in layering::RULES {
        let path = root().join(dir).join("Cargo.toml");
        match std::fs::read_to_string(&path) {
            Ok(manifest) => errors.extend(
                layering::violations(&manifest, rule)
                    .into_iter()
                    .map(|dep| format!("{dir} must not depend on `{dep}`")),
            ),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn report(result: Result<(), Vec<String>>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(errors) => {
            for e in errors {
                eprintln!("xtask: {e}");
            }
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}
