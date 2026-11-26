//! A write from the user's harness that lowers one of wisp's safety guards (ADR-0115): which ones, and what the card
//! says. The harness (`wisp mcp`, the HTTP MCP as the user) acts for the user (ADR-0040), but a prompt-injected session
//! must not switch off wisp's own asks unseen: a write that lowers a guard waits on the user's Allow in wisp; one that
//! raises a guard, or changes anything else, runs at once. Pure: the current value comes from `now`.

use serde_json::Value;

/// What the card says: what would change, in words, and the exact write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    /// after "Your harness wants to": "stop asking before trusting a new project's setup scripts"
    pub what: String,
    /// the write as it would run: `worktrees.projectAsk: on → off`
    pub change: String,
}

/// A setting the user's harness may raise at once but lowers only on the user's Allow, and how to tell.
struct Guard {
    key: &'static str,
    /// what turning it down does, in the user's words
    what: &'static str,
    /// the default when nothing is set (the manifest's)
    default: &'static str,
    /// `(now, new)` → true when `new` is less safe than `now`
    lower: fn(&str, &str) -> bool,
}

fn is(v: &'static str) -> impl Fn(&str, &str) -> bool {
    move |now, new| new == v && now != v
}

/// `off` is the least safe; else a larger number holds later (`budget.limitHold`: ask past 95 % is later than 90 %).
fn later(now: &str, new: &str) -> bool {
    let n = |x: &str| if x == "off" { f64::INFINITY } else { x.parse::<f64>().unwrap_or(0.0) };
    n(new) > n(now)
}

/// `off` is the least safe; else a smaller number holds less (`machine.batteryHold`: under 20 % holds less than 40 %).
fn less(now: &str, new: &str) -> bool {
    let n = |x: &str| if x == "off" { f64::NEG_INFINITY } else { x.parse::<f64>().unwrap_or(0.0) };
    n(new) < n(now)
}

const GUARDS: &[Guard] = &[
    Guard {
        key: "worktrees.projectAsk",
        what: "stop asking before trusting a new project's setup scripts",
        default: "on",
        lower: |now, new| is("off")(now, new),
    },
    Guard {
        key: "worktrees.worktree",
        what: "run new tasks in your checkout itself instead of a worktree of their own",
        default: "on",
        lower: |now, new| is("off")(now, new),
    },
    Guard {
        key: "git.maskEnvFiles",
        what: "show your .env files' values to agents (no masking)",
        default: "true",
        lower: |now, new| is("false")(now, new),
    },
    Guard {
        key: "automations.webhookTriggers",
        what: "let webhooks from outside start tasks",
        default: "false",
        lower: |now, new| is("true")(now, new),
    },
    Guard {
        key: "channels.acceptChannels",
        what: "let chat channels start tasks",
        default: "false",
        lower: |now, new| is("true")(now, new),
    },
    Guard { key: "budget.limitHold", what: "ask later (or never) before work goes past a usage limit", default: "90", lower: later },
    Guard { key: "machine.batteryHold", what: "hold less work while this computer is on battery", default: "off", lower: less },
];

/// A value as the guards compare it: a string's text, else its JSON (`false`, `90`).
fn text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// A value in the guard's own words, as the kernel stores it (`SettingDecl::coerce`): an on/off setting given `false`,
/// `"no"` or `0` is `off` (review: `{"value": false}` slipped past `"off"`), a true/false one given `"off"` is `false`.
fn as_guard(g: &Guard, v: &Value) -> String {
    let t = text(v).trim().to_lowercase();
    let yes = matches!(t.as_str(), "true" | "on" | "yes" | "1");
    let no = matches!(t.as_str(), "false" | "off" | "no" | "0");
    match g.default {
        "on" | "off" if yes => "on".into(),
        "on" | "off" if no => "off".into(),
        "true" | "false" if yes => "true".into(),
        "true" | "false" if no => "false".into(),
        _ => t,
    }
}

/// The sets a setting lists (`runtime.setupAnswerSets`): strings, as given.
fn names(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}

/// Whether `command` with `args` lowers a guard, given each setting's value `now` (None: unset). None: it raises one,
/// keeps it, or isn't a guard: it runs as it would.
pub fn lowers(command: &str, args: &Value, now: &dyn Fn(&str) -> Option<Value>) -> Option<Lowered> {
    match command {
        "worktrees.trust" => {
            let key = args.get("key").and_then(Value::as_str).unwrap_or("this project");
            Some(Lowered {
                what: format!("trust {key}'s own programs (its hooks, servers and setup then run without asking)"),
                change: format!("worktrees.trust {key}"),
            })
        }
        "settings.set" => {
            let key = args.get("key").and_then(Value::as_str)?;
            let new = args.get("value").unwrap_or(&Value::Null);
            let was = now(key);
            // who may answer a setup card: a set added is one more who may say yes for the user
            if key == "runtime.setupAnswerSets" {
                let had = names(was.as_ref());
                let added: Vec<String> = names(Some(new)).into_iter().filter(|s| !had.contains(s)).collect();
                return (!added.is_empty()).then(|| Lowered {
                    what: format!("let {} answer setup cards for you", added.join(", ")),
                    change: format!("runtime.setupAnswerSets: {} → {}", text(was.as_ref().unwrap_or(&Value::Null)), text(new)),
                });
            }
            let g = GUARDS.iter().find(|g| g.key == key)?;
            let now_v = was.as_ref().map_or_else(|| g.default.to_owned(), |v| as_guard(g, v));
            // a null value resets to the default: lower only when the default is
            let new_v = if new.is_null() { g.default.to_owned() } else { as_guard(g, new) };
            (g.lower)(&now_v, &new_v).then(|| Lowered { what: g.what.to_owned(), change: format!("{key}: {now_v} → {new_v}") })
        }
        other => runs_code(other, args),
    }
}

/// What runs code on this computer, or another, without one of wisp's asks: always the user's Allow.
fn runs_code(command: &str, args: &Value) -> Option<Lowered> {
    match command {
        "runtime.exec" => {
            // where it runs is said: a task's folder (its worktree), else the folder named (audit I, smaller notes: the
            // card didn't say which folder it ran in)
            let cmd = args.get("command").and_then(Value::as_str).unwrap_or_default();
            let place = ["cwd", "id"].iter().find_map(|k| args.get(*k).and_then(Value::as_str).filter(|v| !v.is_empty()).map(|v| (*k, v)));
            let at = match place {
                Some(("cwd", dir)) => format!(" (in {dir})"),
                Some((_, task)) => format!(" (in task {task}'s folder)"),
                None => String::new(),
            };
            Some(Lowered { what: format!("run a command on this computer{at}"), change: format!("runtime.exec: {cmd}") })
        }
        "plugins.install" => {
            Some(Lowered { what: "install a plugin (its code runs in wisp)".into(), change: format!("plugins.install {}", text(args)) })
        }
        "plugins.trust" => Some(Lowered {
            what: "trust a plugin (its code runs with wisp's own reach)".into(),
            change: format!("plugins.trust {}", text(args)),
        }),
        "plugins.setEnabled" if args.get("enabled") == Some(&Value::Bool(true)) => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
            Some(Lowered {
                what: format!("switch the plugin {id} on (its code runs in wisp)"),
                change: format!("plugins.setEnabled {id} on"),
            })
        }
        "machines.call" => {
            Some(Lowered { what: "call another computer's wisp directly".into(), change: format!("machines.call {}", text(args)) })
        }
        // a connector's command line runs on this computer for every task, and its test spawns it at once (or sends its
        // headers to the URL): the user's harness adds or changes one only on the user's Allow, as an agent does
        "connectors.add" => Some(Lowered {
            what: format!("add a connector that every task runs ({})", connector_line(args)),
            change: format!("connectors.add {}", text(args)),
        }),
        "connectors.update" if connector_edit(args) => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
            Some(Lowered {
                what: format!("change what the connector {id} runs or where it goes ({})", connector_line(args)),
                change: format!("connectors.update {}", text(args)),
            })
        }
        "connectors.test" => {
            let id = args.get("id").and_then(Value::as_str).unwrap_or_default();
            Some(Lowered { what: format!("start the connector {id} to test it"), change: format!("connectors.test {id}") })
        }
        "connectors.import" if connector_import(args) => Some(Lowered {
            what: "add connectors found in your other apps' settings (each runs for every task)".into(),
            change: format!("connectors.import {}", text(args)),
        }),
        // a project's test command runs after a task's turn, unasked: setting one is running code
        "runtime.projectTest" if project_test(args) => {
            let name = args.get("name").and_then(Value::as_str).unwrap_or_default();
            let cmd = args.get("testCommand").and_then(Value::as_str).unwrap_or_default();
            Some(Lowered {
                what: format!("run `{cmd}` as the tests of the project {name}"),
                change: format!("runtime.projectTest {name}: {cmd}"),
            })
        }
        // a command sent to another computer: guarded as it would be here
        "machines.route" => {
            let cmd = args.get("cmd").and_then(Value::as_str).unwrap_or_default();
            let inner = args.get("args").unwrap_or(&Value::Null);
            guarded(cmd, inner).then(|| Lowered {
                what: format!("run {cmd} on another computer"),
                change: format!("machines.route {cmd} {}", text(inner)),
            })
        }
        _ => None,
    }
}

/// A connector's command line or URL, as its card names it.
fn connector_line(args: &Value) -> String {
    if let Some(url) = args.get("url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
        return url.to_owned();
    }
    let cmd = args.get("command").and_then(Value::as_str).unwrap_or_default();
    let rest: Vec<&str> = args.get("args").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    if rest.is_empty() { cmd.to_owned() } else { format!("{cmd} {}", rest.join(" ")) }
}

/// Whether a `connectors.update` changes what runs or where it goes: switching it on or off or renaming it lowers
/// nothing.
fn connector_edit(args: &Value) -> bool {
    ["transport", "command", "args", "url", "env", "headers"].iter().any(|k| args.get(*k).is_some_and(|v| !v.is_null()))
}

/// Whether a `connectors.import` adds any (hiding found ones from the list adds nothing).
fn connector_import(args: &Value) -> bool {
    args.get("names").and_then(Value::as_array).is_some_and(|n| !n.is_empty())
}

/// Whether a `runtime.projectTest` sets a command (clearing one runs nothing).
fn project_test(args: &Value) -> bool {
    args.get("testCommand").and_then(Value::as_str).is_some_and(|c| !c.trim().is_empty())
}

/// Whether `command` is one the guard looks at at all (a guard's setting, whichever way, or one that runs code): what
/// may not be sent to another computer with a tool's `peer`, whose current value isn't known here.
#[must_use]
pub fn guarded(command: &str, args: &Value) -> bool {
    match command {
        "settings.set" => {
            args.get("key").and_then(Value::as_str).is_some_and(|k| k == "runtime.setupAnswerSets" || GUARDS.iter().any(|g| g.key == k))
        }
        "worktrees.trust" | "runtime.exec" | "plugins.install" | "plugins.trust" | "machines.call" | "connectors.add"
        | "connectors.test" => true,
        "connectors.update" => connector_edit(args),
        "connectors.import" => connector_import(args),
        "runtime.projectTest" => project_test(args),
        // switching a plugin off lowers nothing (audit I, smaller notes)
        "plugins.setEnabled" => args.get("enabled") == Some(&Value::Bool(true)),
        // a command sent on: guarded as that command is (audit I, smaller notes: `machines_route` of `runtime.list` with a
        // `peer` was refused as a guard change)
        "machines.route" => guarded(args.get("cmd").and_then(Value::as_str).unwrap_or_default(), args.get("args").unwrap_or(&Value::Null)),
        _ => false,
    }
}

/// What the harness reads when it sends a guarded write to another computer (a tool's `peer`).
pub const NOT_ON_A_PEER: &str = "That changes one of wisp's safety guards, which isn't sent to another computer from here: change it on that computer (its app, or its own harness). Nothing was done.";

/// The card's question, in the user's words.
#[must_use]
pub fn question(l: &Lowered) -> String {
    format!("Your harness wants to {}.\n{}\nAllow it only if you asked for this.", l.what, l.change)
}

/// The card's two buttons (a setup card's, ADR-0113).
pub const ALLOW: &str = "Allow";
pub const DENY: &str = "Deny";

/// Whether the user's answer is Allow: the button, or a plain yes typed. Anything else is a no.
#[must_use]
pub fn allowed(answer: &str) -> bool {
    let t = answer.trim();
    t == ALLOW || matches!(t.to_lowercase().as_str(), "allow" | "y" | "yes" | "ok" | "approve")
}

/// What the harness reads when the user says no, or doesn't answer.
pub const DENIED: &str = "The user said no in wisp, so nothing was changed.";
pub const UNANSWERED: &str = "The user didn't answer in wisp, so nothing was changed.";

/// What the harness reads when it tries to answer, or act on, the task that asks the user.
pub const NOT_YOURS: &str =
    "That task asks the user, not their harness: they answer it in wisp (the app, the phone, or `wisp reply`). Nothing was done.";

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[allow(clippy::needless_pass_by_value)] // the tests read best with json! inline
    fn set(key: &str, value: Value, now: Option<Value>) -> Option<Lowered> {
        lowers("settings.set", &json!({ "key": key, "value": value }), &move |_| now.clone())
    }

    /// The audit's Note (I): `settings_set {"key":"worktrees.projectAsk","value":"off"}` turned off the ask silently.
    #[test]
    fn turning_a_guard_off_from_the_harness_needs_the_users_allow() {
        let l = set("worktrees.projectAsk", json!("off"), None).unwrap();
        assert_eq!(l.change, "worktrees.projectAsk: on → off");
        assert!(question(&l).contains("stop asking before trusting"), "{}", question(&l));
        for (key, value) in [
            ("worktrees.worktree", json!("off")),
            ("git.maskEnvFiles", json!(false)),
            ("automations.webhookTriggers", json!(true)),
            ("channels.acceptChannels", json!(true)),
            ("budget.limitHold", json!("off")),
            ("budget.limitHold", json!("95")),
        ] {
            assert!(set(key, value.clone(), None).is_some(), "{key} → {value}");
        }
        assert!(set("machine.batteryHold", json!("20"), Some(json!("40"))).is_some());
        // given as a bool or a word, as the kernel stores it (review: `false` slipped past "off")
        assert!(set("worktrees.projectAsk", json!(false), None).is_some());
        assert!(set("worktrees.worktree", json!("no"), Some(json!("on"))).is_some());
        assert!(set("git.maskEnvFiles", json!("off"), None).is_some());
        // what runs code here or elsewhere
        assert!(
            lowers("runtime.exec", &json!({ "id": "t", "command": "wisp settings set worktrees.projectAsk off" }), &|_| None).is_some()
        );
        assert!(lowers("plugins.setEnabled", &json!({ "id": "x", "enabled": true }), &|_| None).is_some());
        assert!(lowers("plugins.setEnabled", &json!({ "id": "x", "enabled": false }), &|_| None).is_none());
        assert!(
            lowers(
                "machines.route",
                &json!({ "id": "p", "cmd": "settings.set", "args": { "key": "worktrees.projectAsk", "value": "off" } }),
                &|_| None
            )
            .is_some()
        );
        assert!(lowers("machines.route", &json!({ "id": "p", "cmd": "runtime.list", "args": {} }), &|_| None).is_none());
        assert!(guarded("settings.set", &json!({ "key": "worktrees.projectAsk", "value": "on" })), "a peer's guard, whichever way");
        assert!(!guarded("settings.set", &json!({ "key": "crew.tint", "value": false })));
        assert!(set("runtime.setupAnswerSets", json!(["apikey"]), Some(json!([]))).is_some());
        assert!(lowers("worktrees.trust", &json!({ "key": "/repo", "ids": ["hooks"] }), &|_| None).is_some());
    }

    #[test]
    fn raising_or_keeping_a_guard_and_other_settings_apply_at_once() {
        for (key, value, now) in [
            ("worktrees.projectAsk", json!("on"), Some(json!("off"))),
            ("worktrees.projectAsk", json!("off"), Some(json!("off"))),
            ("git.maskEnvFiles", json!(true), Some(json!(false))),
            ("budget.limitHold", json!("80"), None),
            ("machine.batteryHold", json!("40"), Some(json!("20"))),
            ("worktrees.projectAsk", json!(true), Some(json!("off"))),
            ("runtime.setupAnswerSets", json!([]), Some(json!(["apikey"]))),
            ("crew.tint", json!(false), None),
            ("settings.editor", json!("zed"), None),
        ] {
            assert!(set(key, value.clone(), now.clone()).is_none(), "{key} → {value} from {now:?}");
        }
        // a reset to a safe default raises; a read is no write
        assert!(set("worktrees.projectAsk", Value::Null, Some(json!("off"))).is_none());
        assert!(lowers("settings.get", &json!({ "key": "worktrees.projectAsk" }), &|_| None).is_none());
    }

    /// Audit I (smaller notes): `machines_route` of `runtime.list` with a `peer` was refused as a guard change, and so was
    /// switching a plugin off there; `runtime.exec`'s card didn't say where the command runs.
    #[test]
    fn only_what_lowers_a_guard_is_guarded_and_the_exec_card_names_its_folder_audit_i() {
        assert!(!guarded("machines.route", &json!({ "id": "p", "cmd": "runtime.list", "args": {} })));
        assert!(guarded("machines.route", &json!({ "id": "p", "cmd": "runtime.exec", "args": { "command": "ls" } })));
        assert!(!guarded("plugins.setEnabled", &json!({ "id": "x", "enabled": false })));
        assert!(guarded("plugins.setEnabled", &json!({ "id": "x", "enabled": true })));
        let l = lowers("runtime.exec", &json!({ "id": "t1", "command": "make" }), &|_| None).unwrap();
        assert!(question(&l).contains("in task t1's folder"), "{}", question(&l));
        let l = lowers("runtime.exec", &json!({ "cwd": "/w/app", "command": "make" }), &|_| None).unwrap();
        assert!(question(&l).contains("(in /w/app)"), "{}", question(&l));
    }

    /// A connector's command line runs for every task and its test spawns it at once: the user's harness adding or
    /// changing one (`connectors_add {command: "sh", args: ["-c", "touch PWNED"]}`) ran it unasked, as did setting a
    /// project's test command. Each asks; a switch, a rename, hiding found servers or clearing a test don't.
    #[test]
    fn a_connector_or_a_project_test_the_harness_adds_needs_the_users_allow() {
        let none = |_: &str| None;
        let l = lowers("connectors.add", &json!({ "name": "evil", "command": "sh", "args": ["-c", "touch PWNED"] }), &none).unwrap();
        assert!(question(&l).contains("(sh -c touch PWNED)"), "{}", question(&l));
        let l = lowers("connectors.add", &json!({ "name": "r", "url": "https://x.example/mcp" }), &none).unwrap();
        assert!(question(&l).contains("https://x.example/mcp"), "{}", question(&l));
        assert!(guarded("connectors.add", &json!({ "url": "https://x.example/mcp" })));
        for edit in [
            json!({ "id": "c", "command": "sh" }),
            json!({ "id": "c", "args": ["-c", "id"] }),
            json!({ "id": "c", "url": "https://evil.example" }),
            json!({ "id": "c", "env": { "PATH": "/tmp" } }),
            json!({ "id": "c", "headers": { "Authorization": "x" } }),
        ] {
            assert!(guarded("connectors.update", &edit) && lowers("connectors.update", &edit, &none).is_some(), "{edit}");
        }
        for toggle in [json!({ "id": "c", "enabled": false }), json!({ "id": "c", "enabled": true }), json!({ "id": "c", "name": "n" })] {
            assert!(!guarded("connectors.update", &toggle) && lowers("connectors.update", &toggle, &none).is_none(), "{toggle}");
        }
        assert!(guarded("connectors.test", &json!({ "id": "c" })) && lowers("connectors.test", &json!({ "id": "c" }), &none).is_some());
        assert!(guarded("connectors.import", &json!({ "names": ["x"] })));
        assert!(lowers("connectors.import", &json!({ "names": ["x"] }), &none).is_some());
        assert!(!guarded("connectors.import", &json!({ "dismiss": ["x"] })));
        assert!(lowers("connectors.import", &json!({ "dismiss": ["x"] }), &none).is_none());
        let l = lowers("runtime.projectTest", &json!({ "name": "app", "testCommand": "id > PWNED" }), &none).unwrap();
        assert!(question(&l).contains("`id > PWNED`") && question(&l).contains("app"), "{}", question(&l));
        assert!(guarded("runtime.projectTest", &json!({ "name": "app", "testCommand": "npm test" })));
        assert!(!guarded("runtime.projectTest", &json!({ "name": "app", "testCommand": "" })));
        assert!(lowers("runtime.projectTest", &json!({ "name": "app", "testCommand": " " }), &none).is_none());
        // sent to another computer: guarded as it would be here
        assert!(guarded("machines.route", &json!({ "cmd": "connectors.add", "args": { "command": "sh" } })));
        assert!(!guarded("machines.route", &json!({ "cmd": "connectors.update", "args": { "id": "c", "enabled": false } })));
    }
}
