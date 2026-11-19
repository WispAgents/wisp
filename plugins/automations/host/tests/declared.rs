//! Audit D12: `automations.triggers` (and fifteen more: trigger create/update/remove, the old recipe commands, runNow,
//! nextWake, the crew's sync calls) were declared in the api with no handler: a call answered "unknown command
//! automations.triggers (is its plugin off?)" and logged a warning. Every command the api declares is one the manifest
//! lists (and the kernel checks a listed command is handled when the plugin installs).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;

/// The ids a source declares with `command!(Name = "id" …)`.
fn declared(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in src.split("command!(").skip(1) {
        let head = chunk.lines().map(str::trim).find(|l| !l.starts_with("///") && !l.is_empty()).unwrap_or_default();
        if let Some(id) = head.split('"').nth(1).filter(|_| head.contains('=')) {
            out.push(id.to_owned());
        }
    }
    out
}

#[test]
fn declared_commands_are_listed() {
    let api = [include_str!("../../api/src/commands.rs"), include_str!("../../api/src/tasks.rs")];
    let ids: Vec<String> = api.iter().flat_map(|s| declared(s)).collect();
    assert!(ids.len() >= 10, "the declarations were read: {ids:?}");
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../wisp-plugin.json")).unwrap();
    let listed: BTreeSet<&str> = manifest["contributes"]["commands"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
    let missing: Vec<&String> = ids.iter().filter(|id| !listed.contains(id.as_str())).collect();
    assert!(missing.is_empty(), "declared but not listed (nothing handles them): {missing:?}");
}
