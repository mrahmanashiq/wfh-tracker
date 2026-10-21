//! Keeps the crate layers honest: `wt-core` may only use a few pure crates,
//! and adapter crates may not reach into each other.
//!
//! Manifests are scanned as text (section headers and keys) rather than parsed
//! as TOML, which is enough for the plain layouts this workspace uses.

pub enum Rule {
    /// The crate may depend on these and nothing else.
    Only(&'static [&'static str]),
    /// The crate may depend on anything except these.
    Not(&'static [&'static str]),
}

pub const RULES: &[(&str, Rule)] = &[
    ("crates/core", Rule::Only(&["jiff", "thiserror", "uuid"])),
    (
        "crates/storage",
        Rule::Not(&["wt-platform", "wt-ui-egui", "egui", "eframe"]),
    ),
    (
        "crates/platform",
        Rule::Not(&["wt-storage", "wt-ui-egui", "rusqlite"]),
    ),
    (
        "crates/ui-egui",
        Rule::Not(&["wt-storage", "wt-platform", "rusqlite"]),
    ),
];

/// Names of normal, target-specific and build dependencies that break `rule`.
pub fn violations(manifest: &str, rule: &Rule) -> Vec<String> {
    dependencies(manifest)
        .into_iter()
        .filter(|dep| match rule {
            Rule::Only(allowed) => !allowed.contains(&dep.as_str()),
            Rule::Not(denied) => denied.contains(&dep.as_str()),
        })
        .collect()
}

fn dependencies(manifest: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_deps = false;
            let header = header.trim();
            if let Some((table, name)) = header.rsplit_once('.')
                && is_dependency_table(table)
            {
                deps.push(name.trim_matches('"').to_owned());
            } else {
                in_deps = is_dependency_table(header);
            }
        } else if in_deps
            && let Some(key) = line.split(['=', '.']).next().map(str::trim)
            && !key.is_empty()
            && !key.starts_with('#')
        {
            deps.push(key.trim_matches('"').to_owned());
        }
    }
    deps
}

/// `[dependencies]`, `[build-dependencies]`, `[target.'…'.dependencies]` — not dev-dependencies.
fn is_dependency_table(header: &str) -> bool {
    let last = header.rsplit('.').next().unwrap_or(header);
    last == "dependencies" || last == "build-dependencies"
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_OK: &str = r#"
[package]
name = "wt-core"

[dependencies]
jiff.workspace = true
thiserror = "2"
uuid = { workspace = true }

[dev-dependencies]
proptest = "1"
"#;

    #[test]
    fn allowed_dependencies_pass_and_dev_dependencies_are_ignored() {
        assert_eq!(
            violations(CORE_OK, &Rule::Only(&["jiff", "thiserror", "uuid"])),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_dependency_outside_the_allowlist_is_reported() {
        let manifest = format!("{CORE_OK}\n[dependencies.egui]\nversion = \"0.37\"\n");

        assert_eq!(
            violations(&manifest, &Rule::Only(&["jiff", "thiserror", "uuid"])),
            ["egui"]
        );
    }

    #[test]
    fn platform_specific_and_build_dependencies_are_checked_too() {
        let manifest = r#"
[dependencies]
jiff = "0.2"

[target.'cfg(windows)'.dependencies]
windows-sys = "0.60"

[build-dependencies]
cc = "1"
"#;

        assert_eq!(
            violations(manifest, &Rule::Only(&["jiff"])),
            ["windows-sys", "cc"]
        );
    }

    #[test]
    fn a_forbidden_sibling_crate_is_reported() {
        let manifest = "[dependencies]\nwt-core.workspace = true\nwt-storage.workspace = true\negui = \"0.37\"\n";

        assert_eq!(
            violations(manifest, &Rule::Not(&["wt-storage", "wt-platform"])),
            ["wt-storage"]
        );
    }
}
