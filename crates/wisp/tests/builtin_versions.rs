//! Every built-in plugin carries the product's version: Settings → Plugins and `wisp plugin ls` show the version of
//! wisp you installed, never a number of its own. A release bumps them with the workspace
//! (`node npm/scripts/release.mjs set-plugins <version>`, which `release.mjs check` asks for), and the ranges the
//! built-ins require each other with must hold for this version and every later 1.x (the kernel's semver lets a
//! pre-release satisfy only a range that names a pre-release of the same x.y.z, as npm does).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use wisp_kernel::semver::satisfies;

const PRODUCT: &str = env!("CARGO_PKG_VERSION");

fn manifests() -> BTreeMap<String, Value> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(&root).unwrap().flatten() {
        let Ok(text) = std::fs::read_to_string(e.path().join("host/wisp-plugin.json")) else { continue };
        let m: Value = serde_json::from_str(&text).unwrap();
        out.insert(m["id"].as_str().unwrap().to_owned(), m);
    }
    assert!(out.len() > 40, "the built-in plugins: {}", out.len());
    out
}

#[test]
fn no_built_in_plugin_says_a_version_other_than_the_workspaces() {
    let off: Vec<String> = manifests()
        .iter()
        .filter(|(_, m)| m["version"].as_str() != Some(PRODUCT))
        .map(|(id, m)| format!("{id}: {}", m["version"]))
        .collect();
    assert!(off.is_empty(), "these built-ins don't say {PRODUCT} (node npm/scripts/release.mjs set-plugins {PRODUCT}): {off:?}");
}

#[test]
fn what_a_built_in_requires_of_another_holds_now_and_after_the_alpha() {
    let all = manifests();
    let mut checked = 0;
    for (id, m) in &all {
        for (dep, want) in m["requires"].as_object().into_iter().flatten() {
            let Some(target) = all.get(dep) else { continue }; // a service contract (`git.service`), not a plugin
            let range = want.as_str().or_else(|| want["version"].as_str()).unwrap();
            let have = target["version"].as_str().unwrap();
            assert!(satisfies(have, range), "{id} requires {dep} {range}, but {dep} is {have}");
            // the same range takes the release and the rest of 1.x, and never a 2
            for later in ["1.0.0", "1.4.2"] {
                assert!(satisfies(later, range), "{id} requires {dep} {range}: {later} wouldn't satisfy it");
            }
            assert!(!satisfies("2.0.0", range), "{id} requires {dep} {range}: it takes 2.0.0");
            checked += 1;
        }
    }
    assert!(checked > 10, "built-ins requiring built-ins: {checked}");
}
