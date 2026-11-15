//! The dependency rules between crates, checked against every manifest.
//!
//! A crate is a business module unless it is listed here as foundation or
//! assembly, so a new crate is held to the strictest rule by default.

use std::collections::BTreeMap;
use std::path::Path;

const PREFIX: &str = "tendhost-";

/// Foundation crates and the workspace crates each may use.
const FOUNDATION: &[(&str, &[&str])] = &[
    ("kernel", &[]),
    ("protocol", &[]),
    ("store", &["kernel"]),
    ("http", &["kernel"]),
];

/// Crates that assemble others; they may use anything.
const ASSEMBLY: &[&str] = &["server", "gateway", "hostd", "cli"];

/// What a business module may use. Never another module.
const MODULE_MAY_USE: &[&str] = &["kernel", "store", "http"];

fn workspace_deps(manifest: &Path) -> Vec<String> {
    let manifest: toml::Table = std::fs::read_to_string(manifest).unwrap().parse().unwrap();
    ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .filter_map(|section| manifest.get(*section)?.as_table())
        .flat_map(|deps| deps.keys())
        .filter_map(|name| name.strip_prefix(PREFIX).map(str::to_string))
        .collect()
}

fn crates() -> BTreeMap<String, Vec<String>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| dir.join("Cargo.toml").exists())
        .map(|dir| {
            let name = dir.file_name().unwrap().to_str().unwrap().to_string();
            (name, workspace_deps(&dir.join("Cargo.toml")))
        })
        .collect()
}

fn allowed(name: &str) -> Option<&'static [&'static str]> {
    if ASSEMBLY.contains(&name) {
        return None;
    }
    Some(
        FOUNDATION
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(MODULE_MAY_USE, |(_, uses)| uses),
    )
}

#[test]
fn no_crate_depends_on_something_its_layer_forbids() {
    let crates = crates();
    assert!(
        crates.contains_key("kernel") && crates.contains_key("identity"),
        "crates were not found"
    );
    let mut violations = Vec::new();
    for (name, deps) in &crates {
        let Some(allowed) = allowed(name) else {
            continue;
        };
        for dep in deps.iter().filter(|dep| !allowed.contains(&dep.as_str())) {
            violations.push(format!("{name} must not depend on {dep}"));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn the_rule_catches_a_module_that_uses_another_module() {
    let module = allowed("jobs").unwrap();
    assert!(!module.contains(&"hosts"));
    assert!(module.contains(&"kernel"));
    assert!(allowed("server").is_none());
    assert!(!allowed("store").unwrap().contains(&"http"));
}
