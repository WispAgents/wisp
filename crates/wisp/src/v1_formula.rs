//! v1's `~/.wisp/config.json` as a formula's resources (ADR-0026: `wisp import` is "export v1 as a
//! formula, then apply", so it shows its plan first). v1 kept one config file:
//! `{ projects, defaults, recipes, triggers, teams, crew, remote, machines, connectors }`. Tasks are
//! history, not configuration: their sessions are imported apart (`runtime.import`).
//!
//! Secrets are never copied into a formula: a connector's environment and a channel's token become
//! `${secret:…}` references only when the secret is already here; otherwise they're left out and named
//! in the notes.

use serde_json::{Map, Value, json};
use wisp_resources_api::Resource;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// `fields` of `v` that are set (not empty), renamed `(from, to)`.
fn pick(v: &Value, fields: &[(&str, &str)]) -> Map<String, Value> {
    let mut o = Map::new();
    for (from, to) in fields {
        if let Some(x) = v.get(*from).filter(|x| !(x.is_null() || **x == json!("") || **x == json!([]) || **x == json!({}))) {
            o.insert((*to).to_owned(), x.clone());
        }
    }
    o
}

/// A name a formula can use (lowercase, dashes): v1 names are free text.
fn name_of(v: &Value, fallback: &str) -> String {
    let n = s(v, "name");
    let n = if n.is_empty() { fallback } else { n };
    let mut out = String::new();
    for c in n.chars() {
        if c.is_alphanumeric() || "-_.".contains(c) {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let t = out.trim_end_matches('-');
    if t.is_empty() { fallback.to_owned() } else { t.to_owned() }
}

/// v1's recipes as declared tasks: the name a key (`run-tests`), v1's own words ("Run tests") its title, which New
/// task's chip shows (audit D11).
fn recipes_of(recipes: Vec<Value>, out: &mut Vec<Resource>) {
    for r in recipes {
        let mut spec = pick(
            &r,
            &[("prompt", "prompt"), ("cwd", "project"), ("agent", "agent"), ("model", "model"), ("effort", "effort"), ("mode", "mode")],
        );
        let name = name_of(&r, "recipe");
        let words = s(&r, "name").trim();
        if !words.is_empty() && words != name {
            spec.insert("title".into(), json!(words));
        }
        out.push(Resource::new("task", name, Value::Object(spec)));
    }
}

/// The resources v1's config declares, and notes on what didn't carry over.
pub fn formula_of(cfg: &Value) -> (Vec<Resource>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let list = |k: &str| cfg.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    for p in list("projects") {
        let spec = pick(&p, &[("path", "path"), ("color", "color"), ("agent", "agent")]);
        if spec.contains_key("path") {
            out.push(Resource::new("project", name_of(&p, "project"), Value::Object(spec)));
        }
    }
    let d = cfg.get("defaults").cloned().unwrap_or_default();
    // with a default harness, its model and effort are that harness's in New task's pick (`default_harness_of`); on the
    // profile they'd follow a task onto any harness (Claude Code run with `--model gpt-5-codex`)
    let fields: &[(&str, &str)] =
        if s(&d, "agent").is_empty() { &[("model", "model"), ("effort", "effort"), ("mode", "mode")] } else { &[("mode", "mode")] };
    let def = pick(&d, fields);
    if !def.is_empty() {
        out.push(Resource::new("agent", "default", Value::Object(def)));
    }
    default_harness_of(&d, &mut out, &mut notes);
    defaults_of(&d, &mut out, &mut notes);
    recipes_of(list("recipes"), &mut out);
    triggers_of(list("triggers"), &mut out, &mut notes);
    for t in list("teams") {
        let mut members = Map::new();
        for (i, m) in t.get("members").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let mut slot = pick(m, &[("count", "count"), ("why", "why")]);
            let agent = if s(m, "agent").is_empty() { "auto" } else { s(m, "agent") };
            slot.insert("agent".into(), json!(agent));
            members.insert(name_of(m, &format!("member-{}", i + 1)), Value::Object(slot));
        }
        out.push(Resource::new("team", name_of(&t, "team"), json!({ "members": members })));
    }
    for m in list("crew") {
        let mut spec = pick(
            &m,
            &[
                ("role", "role"),
                ("avatar", "avatar"),
                ("color", "color"),
                ("brief", "brief"),
                ("cwd", "cwd"),
                ("machine", "machine"),
                ("sandbox", "sandbox"),
                ("agent", "agent"),
                ("model", "model"),
                ("effort", "effort"),
                ("mode", "mode"),
                ("cron", "cron"),
                ("parallel", "parallel"),
                ("kit", "kit"),
                ("team", "team"),
                ("paused", "paused"),
                ("notebook", "notebook"),
            ],
        );
        // v1's `on` (finished, done, ci) is the member's wakes now (ADR-0043)
        let on: Vec<String> =
            m.get("on").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
        let wakes = wisp_automations_api::v1::member_wakes(s(&m, "name"), &on);
        if !wakes.is_empty() {
            spec.insert("wakes".into(), serde_json::to_value(wakes).unwrap_or_default());
        }
        out.push(Resource::new("crew", s(&m, "name"), Value::Object(spec)));
    }
    for m in list("machines") {
        let addr = ["host", "ssh", "address"].iter().map(|k| s(&m, k)).find(|x| !x.is_empty()).unwrap_or_default();
        out.push(Resource::new("machine", name_of(&m, "machine"), json!({ "transport": "ssh", "address": addr })));
    }
    for c in list("connectors") {
        let spec = pick(&c, &[("command", "command"), ("args", "args"), ("url", "url"), ("enabled", "enabled")]);
        if c.get("env").is_some_and(|e| e.as_array().is_some_and(|a| !a.is_empty()) || e.as_object().is_some_and(|o| !o.is_empty())) {
            notes.push(format!("connector {}: its environment stays in secrets; set it with wisp secret set", s(&c, "name")));
        }
        out.push(Resource::new("connector", name_of(&c, "connector"), Value::Object(spec)));
    }
    if let Some(r) = cfg.get("remote").and_then(Value::as_object) {
        for (k, v) in r.iter().filter(|(k, v)| *k != "secret" && v.is_object()) {
            let text = |x: &Value| json!(x.as_str().map_or_else(|| x.to_string(), str::to_owned));
            // who may speak to it; ntfy's `topic` is where it publishes, kept with its server (setup audit F24: it
            // became an `allow` entry and the phone listening on it heard nothing)
            let speakers: &[&str] = if k == "ntfy" { &["user"] } else { &["chatId", "user", "topic"] };
            let allow: Vec<Value> = speakers.iter().filter_map(|f| v.get(*f).filter(|x| !x.is_null()).map(text)).collect();
            let mut spec = json!({ "transport": k, "enabled": v.get("enabled").cloned().unwrap_or(json!(true)), "allow": allow });
            if k == "ntfy" {
                let routing: Map<String, Value> = ["topic", "server"]
                    .iter()
                    .filter_map(|f| v.get(*f).filter(|x| !x.is_null()).map(|x| ((*f).to_owned(), text(x))))
                    .collect();
                if !routing.is_empty() {
                    spec["routing"] = Value::Object(routing);
                }
            }
            out.push(Resource::new("channel", k.clone(), spec));
            // only a channel v1 kept a secret for (server/remote.js's masked fields): v1's ntfy has none, unless it
            // was given an access token (audit E7)
            let secret = |f: &str| v.pointer(f).and_then(Value::as_str).is_some_and(|x| !x.is_empty());
            if ["/token", "/appToken", "/botToken", "/signingSecret", "/publicKey", "/hook/secret"].iter().any(|f| secret(f)) {
                notes.push(format!("channel {k}: its token stays in secrets (${{secret:…}} when you declare it)"));
            }
        }
    }
    (out, notes)
}

/// v1's triggers: a schedule is a declared task, every other kind the trigger source it is now.
fn triggers_of(triggers: Vec<Value>, out: &mut Vec<Resource>, notes: &mut Vec<String>) {
    for t in triggers {
        let kind = s(&t, "kind");
        if kind == "schedule" {
            // a schedule is a declared task with its schedule (ADR-0027)
            let mut spec = pick(
                &t,
                &[
                    ("prompt", "prompt"),
                    ("cwd", "project"),
                    ("agent", "agent"),
                    ("model", "model"),
                    ("effort", "effort"),
                    ("mode", "mode"),
                    ("cron", "schedule"),
                    ("crew", "crew"),
                ],
            );
            if !spec.contains_key("prompt") {
                notes.push(format!("the schedule {} runs a recipe by id: declare its prompt", s(&t, "name")));
                continue;
            }
            if t.get("enabled") == Some(&json!(false)) {
                spec.insert("paused".into(), json!(true));
            }
            out.push(Resource::new("task", name_of(&t, "schedule"), Value::Object(spec)));
        } else {
            // each of v1's kinds as the source it is now (ADR-0043: ci → ci.failed, slack → chat.message, …)
            // v1 rows leave out what they didn't set: read over a blank row
            let mut base = serde_json::to_value(wisp_automations_api::TriggerBase::default()).unwrap_or_default();
            if let (Some(b), Some(o)) = (base.as_object_mut(), t.as_object()) {
                b.extend(o.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), v.clone())));
            }
            let row = serde_json::from_value::<wisp_automations_api::TriggerBase>(base).ok();
            match row.as_ref().and_then(wisp_automations_api::v1::from_v1) {
                Some((_, spec)) => out.push(Resource::new("trigger", name_of(&t, kind), serde_json::to_value(spec).unwrap_or_default())),
                None => notes.push(format!(
                    "the trigger {} is a kind v2 doesn't know ({kind}): declare it again with wisp trigger add",
                    s(&t, "name")
                )),
            }
        }
    }
}

/// The type a v1 default takes as its v2 setting.
#[derive(Debug, Clone, Copy)]
enum As {
    /// true/false (v1 kept "on"/"off" too)
    Bool,
    /// "on"/"off" (v1 kept true/false too)
    OnOff,
    /// text as it is (a number's digits)
    Text,
    /// a whole number (v1 kept "30" too)
    Int,
}

impl As {
    /// None: not a value this type takes (an object, a word that's no number): named, never sent to fail the import.
    fn of(self, v: &Value) -> Option<Value> {
        if v.is_object() || v.is_array() {
            return None;
        }
        let text = v.as_str().map_or_else(|| v.to_string(), str::to_owned);
        Some(match self {
            Self::Bool => json!(match v {
                Value::Bool(b) => *b,
                _ => !matches!(text.as_str(), "off" | "false" | "0" | ""),
            }),
            Self::OnOff => json!(match v {
                Value::Bool(b) =>
                    if *b {
                        "on"
                    } else {
                        "off"
                    },
                _ => text.as_str(),
            }),
            Self::Text => json!(text),
            Self::Int => json!(v.as_i64().or_else(|| text.trim().parse().ok())?),
        })
    }
}

/// v1 defaults with a v2 setting beyond the aliases (setup audit F25: they were dropped), and the type each takes.
/// v1's defaults that are settings now (wisp-settings-api `V1_ALIASES`, and the rest with a v2 setting: setup audit
/// F25), each in its setting's type; the ones with none are named in `notes`, never dropped without a word.
fn defaults_of(d: &Value, out: &mut Vec<Resource>, notes: &mut Vec<String>) {
    for (v1, key) in wisp_settings_api_aliases() {
        if let Some(v) = d.get(v1).filter(|v| !v.is_null()) {
            out.push(Resource::new("setting", key, v.clone()));
        }
    }
    let mut odd = Vec::new();
    for (v1, key, to) in V1_DEFAULTS {
        if let Some(v) = d.get(*v1).filter(|v| !v.is_null()) {
            match to.of(v) {
                Some(v) => out.push(Resource::new("setting", *key, v)),
                None => odd.push(format!("{v1} ({key})")),
            }
        }
    }
    if !odd.is_empty() {
        notes.push(format!("v1 defaults in a shape v2 doesn't read, not carried: {}", odd.join(", ")));
    }
    let carried = |k: &str| {
        ["model", "effort", "mode", "subagentModel", "agent"].contains(&k)
            || wisp_settings_api_aliases().iter().any(|(v1, _)| *v1 == k)
            || V1_DEFAULTS.iter().any(|(v1, _, _)| *v1 == k)
    };
    let left: Vec<&str> = d.as_object().into_iter().flatten().map(|(k, _)| k.as_str()).filter(|k| !carried(k)).collect();
    if !left.is_empty() {
        notes.push(format!("v1 defaults with no v2 setting, not carried: {}", left.join(", ")));
    }
}

/// Setup audit B11: one default harness and one subagent model, where the user sets them. v1's `defaults.agent` is New
/// task's pick (`runtime.launch`), which a task that names no harness runs on too (CLI, MCP, crew); it went on the
/// `default` profile, so the CLI ran Codex while New task offered Claude Code. `subagentModel` is Usage's setting.
fn default_harness_of(d: &Value, out: &mut Vec<Resource>, notes: &mut Vec<String>) {
    let agent = s(d, "agent");
    if !agent.is_empty() {
        let mut pick = json!({ "agent": agent });
        for (v1, key) in [("model", "models"), ("effort", "efforts")] {
            if !s(d, v1).is_empty() {
                pick[key] = json!({ agent: s(d, v1) });
            }
        }
        out.push(Resource::new("setting", "runtime.launch", pick));
    }
    match s(d, "subagentModel") {
        "" => {}
        m if SUBAGENT_MODELS.contains(&m) => out.push(Resource::new("setting", "runtime.subagentModel", json!(m))),
        m => notes.push(format!("v1 subagentModel {m}: v2's are {}, not carried", SUBAGENT_MODELS.join(", "))),
    }
}

/// `runtime.subagentModel`'s values (wisp.runtime's manifest).
const SUBAGENT_MODELS: [&str; 3] = ["task", "haiku", "sonnet"];

const V1_DEFAULTS: &[(&str, &str, As)] = &[
    ("maskEnvFiles", "git.maskEnvFiles", As::Bool),
    ("leanRuns", "runtime.leanRuns", As::OnOff),
    ("editor", "settings.editor", As::Text),
    ("notify", "settings.notify", As::OnOff),
    ("limitHold", "budget.limitHold", As::Text),
    ("retentionDays", "runtime.retentionDays", As::Int),
];

/// v1's `defaults` names that are settings now, and their keys (as wisp-settings-api's `V1_ALIASES`).
fn wisp_settings_api_aliases() -> [(&'static str, &'static str); 6] {
    [
        ("timeZone", "settings.timeZone"),
        ("review", "review.harness"),
        ("panes", "stuff.panes"),
        ("worktreeCleanup", "worktrees.cleanup"),
        ("worktreeMax", "worktrees.max"),
        ("worktreeOnDone", "worktrees.onDone"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit D11: v1's recipe "Run tests" became the task `run-tests` and New task's Recipes chip read `run-tests`. Its
    /// name stays the key (`wisp recipes run run-tests`); v1's own words come along as its title, which the chip shows.
    #[test]
    fn a_v1_recipe_keeps_its_name_as_its_title_audit_d11() {
        let cfg = json!({ "recipes": [{ "name": "Run tests", "prompt": "run the tests" }, { "name": "lint", "prompt": "lint" }] });
        let (rs, _) = formula_of(&cfg);
        let tasks: Vec<&Resource> = rs.iter().filter(|r| r.kind == "task").collect();
        assert_eq!((tasks[0].name.as_str(), &tasks[0].spec["title"]), ("run-tests", &json!("Run tests")));
        assert_eq!(tasks[1].spec.get("title"), None, "a name that is its key needs no title");
    }

    /// Audit E7: the import noted "channel ntfy: its token stays in secrets" for a v1 ntfy of `{enabled, topic,
    /// server}`, which has no token. The note is for a channel v1 kept a secret for.
    #[test]
    fn only_a_channel_with_a_secret_gets_the_token_note_audit_e7() {
        let cfg = json!({ "remote": {
            "ntfy": { "enabled": true, "topic": "wisp-old-topic", "server": "http://x" },
            "telegram": { "enabled": true, "token": "123:abc", "chatId": 5 },
            "slack": { "enabled": false, "botToken": "", "appToken": "" },
        } });
        let (rs, notes) = formula_of(&cfg);
        assert_eq!(rs.iter().filter(|r| r.kind == "channel").count(), 3);
        let token: Vec<&String> = notes.iter().filter(|n| n.contains("token stays in secrets")).collect();
        assert_eq!(token.len(), 1, "{notes:?}");
        assert!(token[0].starts_with("channel telegram"), "{notes:?}");
    }

    #[test]
    fn the_fixture_home_becomes_resources() {
        let cfg: Value = serde_json::from_str(include_str!("../tests/fixtures/v1-home/config.json")).unwrap();
        let (rs, notes) = formula_of(&cfg);
        let names: Vec<String> = rs.iter().map(|r| format!("{}/{}", r.kind, r.name)).collect();
        for want in [
            "project/api",
            "agent/default",
            "setting/settings.timeZone",
            "task/fix-issue",
            "task/nightly",
            "trigger/ci",
            "team/review-pair",
            "crew/Ada",
            "machine/studio",
            "connector/linear",
            "channel/telegram",
        ] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
        }
        let ci = rs.iter().find(|r| r.kind == "trigger" && r.name == "ci").unwrap();
        assert_eq!(
            (ci.spec["on"].as_str(), ci.spec["branch"].as_str(), ci.spec["every"].as_u64()),
            (Some("ci.failed"), Some("main"), Some(5)),
            "v1's ci is ci.failed now"
        );
        let nightly = rs.iter().find(|r| r.name == "nightly").unwrap();
        assert_eq!(nightly.spec["schedule"], "0 2 * * *");
        assert!(notes.iter().any(|n| n.contains("linear")), "{notes:?}");
    }

    /// Setup audit B11: v1's default harness went on the `default` profile, so the CLI ran Codex while New task offered
    /// Claude Code, and its subagent model sat where Usage never shows it. Both are the settings the app sets.
    #[test]
    fn v1s_default_harness_is_new_tasks_pick_and_its_subagent_model_is_usages() {
        let v1 = json!({ "defaults": { "agent": "codex", "model": "gpt-5-codex", "effort": "high", "mode": "edits", "subagentModel": "haiku" } });
        let (rs, notes) = formula_of(&v1);
        let get = |rs: &[Resource], k: &str, n: &str| rs.iter().find(|r| r.kind == k && r.name == n).map(|r| r.spec.clone());
        assert_eq!(
            get(&rs, "setting", "runtime.launch"),
            Some(json!({ "agent": "codex", "models": { "codex": "gpt-5-codex" }, "efforts": { "codex": "high" } })),
            "the model and effort are the harness's, never another's"
        );
        assert_eq!(get(&rs, "setting", "runtime.subagentModel"), Some(json!("haiku")));
        assert_eq!(get(&rs, "agent", "default"), Some(json!({ "mode": "edits" })), "no harness, model or subagent model on the profile");
        assert!(notes.is_empty(), "{notes:?}");
        // no default harness: the model stays the profile's, as before
        let (rs, _) = formula_of(&json!({ "defaults": { "model": "opus", "effort": "high" } }));
        assert_eq!(get(&rs, "agent", "default"), Some(json!({ "model": "opus", "effort": "high" })));
        assert!(get(&rs, "setting", "runtime.launch").is_none());
        let (rs, notes) = formula_of(&json!({ "defaults": { "subagentModel": "opus" } }));
        assert!(
            rs.iter().all(|r| r.name != "runtime.subagentModel") && notes.iter().any(|n| n.contains("subagentModel opus")),
            "{notes:?}"
        );
    }

    /// Setup audit F24, F25: the v1 ntfy topic became an `allow` entry (its server dropped), so the phone that listened
    /// on it heard nothing; and v1 defaults beyond six were dropped without a word. ntfy's topic and server are where it
    /// publishes (`routing`), each default with a v2 setting is that setting, in its type, and the rest are named.
    #[test]
    fn ntfy_keeps_its_topic_and_server_and_every_v1_default_is_carried_or_named() {
        let cfg = json!({
            "remote": { "ntfy": { "enabled": true, "topic": "wisp-old-topic-1", "server": "https://ntfy.example" },
                        "telegram": { "enabled": false, "chatId": 42 } },
            "defaults": { "maskEnvFiles": "off", "leanRuns": "all", "editor": "zed", "notify": true, "limitHold": 90,
                          "retentionDays": "30", "timeZone": "Europe/Zurich", "model": "opus", "someOldThing": 1 }
        });
        let (rs, notes) = formula_of(&cfg);
        let get = |kind: &str, name: &str| {
            rs.iter().find(|r| r.kind == kind && r.name == name).unwrap_or_else(|| panic!("{kind}/{name}: {rs:?}")).spec.clone()
        };
        let ntfy = get("channel", "ntfy");
        assert_eq!(ntfy["routing"], json!({ "topic": "wisp-old-topic-1", "server": "https://ntfy.example" }), "{ntfy}");
        assert_eq!(ntfy["allow"], json!([]), "a topic isn't someone allowed to speak: {ntfy}");
        assert_eq!(get("channel", "telegram")["allow"], json!(["42"]), "a chat id still is");
        for (key, want) in [
            ("git.maskEnvFiles", json!(false)),
            ("runtime.leanRuns", json!("all")),
            ("settings.editor", json!("zed")),
            ("settings.notify", json!("on")),
            ("budget.limitHold", json!("90")),
            ("runtime.retentionDays", json!(30)),
            ("settings.timeZone", json!("Europe/Zurich")),
        ] {
            assert_eq!(get("setting", key), want, "{key}");
        }
        assert!(notes.iter().any(|n| n.contains("someOldThing") && !n.contains("maskEnvFiles")), "{notes:?}");
        // a shape v2 doesn't read: named, not sent (it would fail the whole import)
        let (rs, notes) = formula_of(&json!({ "defaults": { "notify": { "sound": true } } }));
        assert!(rs.iter().all(|r| r.name != "settings.notify") && notes.iter().any(|n| n.contains("notify")), "{notes:?}");
    }
}
