//! Every command the UI sends is one a real `wisp serve` offers (the owner, 2026-11-14: a task page toasted "Mark read
//! needs a newer wisp.runtime on this computer"). The UI gates a command on the peer's welcome (`hasFeature`): the
//! plugins it lists and each one's commands. This reads that welcome from the real binary with every plugin this build
//! has, and checks it against each `defineCommand` id in the shell and plugin UIs (`packages/*/src`, `plugins/*/ui/src`)
//! and each `feature: { plugin, command }` gate, so a UI command no plugin registers (or one gated on the wrong
//! plugin) can't ship again.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::*;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.ts`/`.tsx` file under `dir`, tests and `node_modules` left out.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "node_modules" && name != "test" && name != "dist" {
                sources(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "ts" || x == "tsx") && !name.contains(".test.") && !name.ends_with(".d.ts") {
            out.push(p);
        }
    }
}

/// The quoted string right after `key` (`id: 'runtime.markRead'`), if the text there starts with one.
fn quoted_after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(key)?.trim_start();
    let q = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &rest[1..];
    body.find(q).map(|end| &body[..end])
}

/// What one file's commands send: (command id, the file) and (the plugin a `feature` gate names, its command, the file).
fn scan(file: &Path, ids: &mut Vec<(String, String)>, gates: &mut Vec<(String, String, String)>) {
    let text = std::fs::read_to_string(file).unwrap();
    let rel = file.strip_prefix(root()).unwrap_or(file).display().to_string();
    // each defineCommand(...)'s own `id:` key (the first one, as a whole word): a literal is the command; anything
    // else (a parameter, a constant) is skipped, never mistaken for a later literal (a navigate's `id: 'shell.x'`)
    let parts: Vec<&str> = text.split("defineCommand").collect();
    for part in parts.iter().skip(1) {
        if !part.trim_start().starts_with(['<', '(']) {
            continue; // an import or a mention, not a call
        }
        // the definition object starts at the call's `({`, after any type arguments (which have `id:` fields of their own)
        let Some(call) = part.find("({") else { continue };
        let def = &part[call..];
        let at = def.match_indices("id:").map(|(i, _)| i).find(|&i| matches!(def.as_bytes()[i - 1], b' ' | b'\n' | b'{' | b','));
        if let Some(id) = at.and_then(|i| quoted_after(&def[i..], "id:")).filter(|id| id.contains('.')) {
            ids.push((id.to_owned(), rel.clone()));
        }
    }
    // commands made by a helper with the id as its argument (shell's copyOf('runtime.fork'))
    for (i, _) in text.match_indices("copyOf(") {
        if let Some(id) = quoted_after(&text[i..], "copyOf(") {
            ids.push((id.to_owned(), rel.clone()));
        }
    }
    for (i, _) in text.match_indices("feature: {") {
        let gate = &text[i..text[i..].find('}').map_or(text.len(), |e| i + e)];
        let field = |k: &str| gate.match_indices(k).find_map(|(j, _)| quoted_after(&gate[j..], k));
        // `plugin: RUNTIME`: a constant in the same file
        let plugin = field("plugin:").map(str::to_owned).or_else(|| {
            let name = gate.split("plugin:").nth(1)?.trim_start().split([',', ' ', '}']).next()?.trim().to_owned();
            text.match_indices(&format!("const {name} = "))
                .find_map(|(j, _)| quoted_after(&text[j..], &format!("const {name} =")).map(str::to_owned))
        });
        if let (Some(p), Some(c)) = (plugin, field("command:")) {
            gates.push((p, c.to_owned(), rel.clone()));
        }
    }
}

/// The welcome's plugins → their commands, from the real server's sync stream.
async fn welcome(port: u16) -> BTreeMap<String, BTreeSet<String>> {
    let http = reqwest::Client::new();
    let mut r =
        http.get(format!("http://127.0.0.1:{port}/api/events?proto_min=2&proto_max=2&client_id=ui-cmds&build=t")).send().await.unwrap();
    let mut seen = String::new();
    let at = Instant::now();
    while at.elapsed() < Duration::from_secs(15) {
        match tokio::time::timeout(Duration::from_millis(300), r.chunk()).await {
            Ok(Ok(Some(c))) => seen.push_str(&String::from_utf8_lossy(&c)),
            Ok(Ok(None) | Err(_)) => panic!("the sync stream ended before a welcome: {seen}"),
            Err(_) => {}
        }
        for line in seen.lines() {
            let Some(data) = line.strip_prefix("data:") else { continue };
            let Ok(v) = serde_json::from_str::<Value>(data.trim()) else { continue };
            // a frame is a list of messages (`data: [{"t":"welcome",…}]`)
            let msgs = v.as_array().or_else(|| v["messages"].as_array()).cloned().unwrap_or_else(|| vec![v.clone()]);
            if let Some(w) = msgs.iter().find(|m| m["t"] == "welcome") {
                return w["peer"]["plugins"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        let cmds = p["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()).collect();
                        (p["id"].as_str().unwrap().to_owned(), cmds)
                    })
                    .collect();
            }
        }
    }
    panic!("no welcome on the sync stream: {seen}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_command_the_ui_sends_is_offered_by_a_plugin_in_the_real_servers_welcome_mark_read_needs_a_newer_runtime() {
    let mut files = Vec::new();
    for top in ["packages", "plugins"] {
        for e in std::fs::read_dir(root().join(top)).unwrap().flatten() {
            let src = if top == "packages" { e.path().join("src") } else { e.path().join("ui/src") };
            sources(&src, &mut files);
        }
    }
    sources(&root().join("apps/web/src"), &mut files);
    let (mut ids, mut gates) = (Vec::new(), Vec::new());
    for f in &files {
        scan(f, &mut ids, &mut gates);
    }
    assert!(ids.len() > 200, "the scan finds the UI's commands: {}", ids.len());
    assert!(ids.iter().any(|(id, _)| id == "runtime.markRead"), "the scan sees runtime.markRead");
    assert!(gates.iter().any(|(p, c, _)| p == "wisp.runtime" && c == "runtime.markRead"), "the scan sees Mark read's gate");

    let dir = temp("ui-commands");
    let (serve, port) = start(&dir);
    let plugins = welcome(port).await;
    let offered: BTreeSet<&str> = plugins.values().flatten().map(String::as_str).collect();
    let missing: BTreeSet<String> =
        ids.iter().filter(|(id, _)| !offered.contains(id.as_str())).map(|(id, f)| format!("{id} ({f})")).collect();
    let wrong: BTreeSet<String> = gates
        .iter()
        .filter(|(p, c, _)| !plugins.get(p).is_some_and(|cmds| cmds.contains(c)))
        .map(|(p, c, f)| format!("{p} / {c} ({f})"))
        .collect();
    drop(serve);
    assert!(missing.is_empty(), "UI commands no plugin offers in the welcome:\n{}", missing.into_iter().collect::<Vec<_>>().join("\n"));
    assert!(
        wrong.is_empty(),
        "feature gates on a plugin that doesn't offer the command:\n{}",
        wrong.into_iter().collect::<Vec<_>>().join("\n")
    );
}
