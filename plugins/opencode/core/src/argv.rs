//! `opencode run --format json …` for one run, and the inline config it carries (v1 opencode.js
//! `run()`): wisp's MCP server, the grants the user allowed, a capped run's asks, merged over the
//! user's own `OPENCODE_CONFIG_CONTENT`.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use wisp_harness_kit::Re;
use wisp_harness_kit::protocol::ASK_TIMEOUT_MS;

static MAJOR: Re = Re::new(r"(?:^|\s)v?(\d+)\.\d+");
static ANSI: Re = Re::new(r"\x1b\[[0-9;]*m");
static CREDS: Re = Re::new(r"(\d+) credentials?");
static ENV_KEYS: Re = Re::new(r"[1-9]\d* environment variables?");

/// opencode 2 changed `run`: lowercase log levels, and the effort rides on the model (`model#high`).
pub fn major_version(version: &str) -> u64 {
    MAJOR.group(version, 1).and_then(|m| m.parse().ok()).unwrap_or(0)
}

/// `opencode auth list`: a provider signed in, or a key in the environment. None: output we don't know.
pub fn has_key(out: &str) -> Option<bool> {
    let text = ANSI.replace_all(out, "");
    let n: u64 = CREDS.group(&text, 1)?.parse().ok()?;
    Some(n > 0 || ENV_KEYS.is_match(&text))
}

#[derive(Debug, Clone, Default)]
pub struct OpencodeArgs<'a> {
    /// `plan` | `edits` | `auto`
    pub mode: &'a str,
    pub model: &'a str,
    pub effort: &'a str,
    pub resume_id: Option<&'a str>,
    pub fork: bool,
    pub attachments: &'a [String],
    pub v2: bool,
}

pub fn args(a: &OpencodeArgs<'_>) -> Vec<String> {
    let mut out: Vec<String> = vec!["run".to_owned()];
    // opencode 2's `run` attaches to the one background service on the machine (127.0.0.1:49374) when one is up (the
    // user's own opencode, or `opencode models` from wisp's harness detection), and that service keeps the config it
    // started with: the run's OPENCODE_CONFIG_CONTENT, so wisp's MCP servers (wisp, computer, a connector, the browser),
    // its grants and its permission rules, never reached the agent. `--standalone` gives the run a private server with
    // its own config (sessions stay in the same store, so `--session` resumes), and its server logs on stderr.
    if a.v2 {
        out.push("--standalone".to_owned());
    }
    out.extend(["--format", "json", "--print-logs", "--log-level", if a.v2 { "error" } else { "ERROR" }].map(str::to_owned));
    let mut push = |x: &[&str]| out.extend(x.iter().map(|s| (*s).to_owned()));
    if a.mode == "plan" {
        push(&["--agent", "plan"]);
    }
    if a.mode == "auto" {
        push(&["--auto"]);
    }
    if !a.model.is_empty() {
        let m = if a.v2 && !a.effort.is_empty() { format!("{}#{}", a.model, a.effort) } else { a.model.to_owned() };
        push(&["--model", &m]);
        if !a.effort.is_empty() && !a.v2 {
            push(&["--variant", a.effort]);
        }
    }
    if let Some(r) = a.resume_id.filter(|r| !r.is_empty()) {
        push(&["--session", r]);
        if a.fork {
            push(&["--fork"]);
        }
    }
    for f in a.attachments {
        push(&["--file", f]);
    }
    out
}

/// A grant the user allowed after `run` rejected it: v1's `{permission, patterns}`, which travels as
/// its JSON string in the runtime's grants.
fn grant(g: &str) -> Option<(String, Vec<String>)> {
    let v: Value = serde_json::from_str(g).ok()?;
    let p = v["permission"].as_str()?.to_owned();
    let pats = v["patterns"].as_array()?.iter().map(wisp_harness_kit::js::string).collect();
    Some((p, pats))
}

/// One MCP server in opencode's terms: a URL is `remote`, a command `local`.
fn server(srv: &Value) -> Value {
    if let Some(url) = srv["url"].as_str() {
        return json!({ "type": "remote", "url": url, "headers": srv.get("headers").cloned().unwrap_or_else(|| json!({})), "enabled": true, "timeout": ASK_TIMEOUT_MS });
    }
    let mut command = vec![srv["command"].clone()];
    command.extend(srv["args"].as_array().cloned().unwrap_or_default());
    json!({ "type": "local", "command": command, "environment": srv.get("env").filter(|e| !e.is_null()).cloned().unwrap_or_else(|| json!({})), "enabled": true, "timeout": ASK_TIMEOUT_MS })
}

/// The commands a Plan run's shell may run (ADR-0135): reading the project, never writing it. Each with its arguments,
/// but never with a redirect, a pipe, a chain or a substitution, which could write through another command.
const PLAN_READS: [&str; 13] =
    ["git status", "git log", "git diff", "git show", "ls", "cat", "rg", "grep", "find", "pwd", "head", "tail", "wc"];
/// What turns a read into a write: shell metacharacters after any read, and the flags that write or run a program.
/// `>` also covers `>(`; `<(` is a process substitution, a command of its own.
const PLAN_WRITES: [&str; 7] = [">", "|", ";", "&", "`", "$(", "<("];
/// `--output` writes a file and `--ext-diff` runs a configured program, on each git read that takes them. Per command,
/// not one `git *--output*`: that key would sort before `git log *` and lose to it.
const PLAN_WRITE_FLAGS: [&str; 12] = [
    "find *-delete*",
    "find *-exec*",
    "find *-ok*",
    "find *-fprint*",
    "find *-fls*",
    "git diff *--output*",
    "git log *--output*",
    "git show *--output*",
    "git diff *--ext-diff*",
    "git log *--ext-diff*",
    "git show *--ext-diff*",
    "rg *--pre*",
];

/// Plan's `bash` rules: everything denied but the reads. opencode matches the last rule that fits, and the keys go out
/// sorted, so a deny for `cat *>*` comes after the allow for `cat *` it narrows.
fn plan_bash() -> Value {
    let mut m = Map::from_iter([("*".to_owned(), json!("deny"))]);
    for c in PLAN_READS {
        m.insert(c.to_owned(), json!("allow"));
        m.insert(format!("{c} *"), json!("allow"));
        for w in PLAN_WRITES {
            m.insert(format!("{c} *{w}*"), json!("deny"));
        }
    }
    for f in PLAN_WRITE_FLAGS {
        m.insert(f.to_owned(), json!("deny"));
    }
    Value::Object(m)
}

/// The `OPENCODE_CONFIG_CONTENT` for a run, over `inline` (the user's own); None: nothing to add. `plan_commands`: a
/// Plan run may use its shell to read (a task you started), else it has none (a trigger's, a channel's, a capped one).
pub fn config(
    inline: Option<&str>,
    mcp_servers: &BTreeMap<String, Value>,
    grants: &[String],
    mode: &str,
    delegating: bool,
    capped: bool,
    plan_commands: bool,
) -> Option<String> {
    let allow: Vec<(String, Vec<String>)> = grants.iter().filter_map(|g| grant(g)).collect();
    let plan = mode == "plan";
    if mcp_servers.is_empty() && allow.is_empty() && !capped && !delegating && !plan {
        return None;
    }
    let inline: Map<String, Value> =
        inline.and_then(|s| serde_json::from_str::<Value>(s).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let mut permission: Map<String, Value> = match inline.get("permission") {
        Some(Value::Object(o)) => o.clone(),
        Some(v) if wisp_harness_kit::js::truthy(v) => Map::from_iter([("*".to_owned(), v.clone())]),
        _ => Map::new(),
    };
    // its built-in `task` subagents would bypass the user's team
    if delegating {
        permission.insert("task".into(), json!("deny"));
    }
    // capped: no sandbox, so commands and fetches wait for the user (Allow retries exactly that)
    if capped && mode != "auto" {
        permission.insert("bash".into(), json!("ask"));
        permission.insert("webfetch".into(), json!("ask"));
    }
    for (p, pats) in &allow {
        let mut m = match permission.get(p) {
            Some(Value::Object(o)) => o.clone(),
            Some(v) if wisp_harness_kit::js::truthy(v) => Map::from_iter([("*".to_owned(), v.clone())]),
            _ => Map::new(),
        };
        for x in pats {
            m.insert(x.clone(), json!("allow"));
        }
        permission.insert(p.clone(), Value::Object(m));
    }
    // Plan can't write (ADR-0135): opencode's plan agent follows the user's rules, so an `edit: allow`, a `"*": allow`
    // or a grant would let it edit, and its shell write anything; these win over all of them
    if plan {
        permission.insert("edit".into(), json!("deny"));
        permission.insert("bash".into(), if plan_commands { plan_bash() } else { json!("deny") });
    }
    let mut out = inline.clone();
    if !mcp_servers.is_empty() {
        let mut mcp = inline.get("mcp").and_then(Value::as_object).cloned().unwrap_or_default();
        for (name, srv) in mcp_servers {
            mcp.insert(name.clone(), server(srv));
        }
        out.insert("mcp".into(), Value::Object(mcp));
        let mut exp = inline.get("experimental").and_then(Value::as_object).cloned().unwrap_or_default();
        exp.insert("mcp_timeout".into(), json!(ASK_TIMEOUT_MS));
        out.insert("experimental".into(), Value::Object(exp));
    }
    if !permission.is_empty() {
        out.insert("permission".into(), Value::Object(permission));
    }
    Some(Value::Object(out).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_keys() {
        for p in [&MAJOR, &ANSI, &CREDS, &ENV_KEYS] {
            assert!(p.compiles(), "{}", p.pattern());
        }
        assert_eq!(major_version("2.0.1"), 2);
        assert_eq!(major_version("opencode v1.14.3"), 1);
        assert_eq!(has_key("┌  Credentials\n└  1 credentials"), Some(true));
        assert_eq!(has_key("└  0 credentials\n\n2 environment variables"), Some(true));
        assert_eq!(has_key("└  0 credentials"), Some(false));
        assert_eq!(has_key("what"), None);
    }

    #[test]
    fn v2_puts_the_effort_on_the_model() {
        let a = args(&OpencodeArgs { mode: "edits", model: "fake/m", effort: "high", v2: true, ..OpencodeArgs::default() });
        assert_eq!(a, ["run", "--standalone", "--format", "json", "--print-logs", "--log-level", "error", "--model", "fake/m#high"]);
    }

    /// opencode 2.4.1: with a background service already up (the user's own opencode, or `opencode models`), `run`
    /// attached to it and its MCP servers came from that service's config, never the run's: wisp's tools went missing
    /// (no request reached `/api/mcp`). A private server per run reads the run's own config. opencode 1 has no flag.
    #[test]
    fn v2_runs_on_a_private_server_so_its_own_config_and_mcp_servers_apply() {
        let v2 = args(&OpencodeArgs { mode: "auto", v2: true, ..OpencodeArgs::default() });
        assert_eq!(v2.get(1).map(String::as_str), Some("--standalone"), "{v2:?}");
        let resumed = args(&OpencodeArgs { mode: "edits", resume_id: Some("ses_1"), v2: true, ..OpencodeArgs::default() });
        assert!(resumed.contains(&"--standalone".to_owned()) && resumed.contains(&"ses_1".to_owned()), "a follow-up too: {resumed:?}");
        let v1 = args(&OpencodeArgs { mode: "auto", ..OpencodeArgs::default() });
        assert!(!v1.contains(&"--standalone".to_owned()), "opencode 1 has no such flag: {v1:?}");
    }

    #[test]
    fn the_inline_config_merges_over_the_users() {
        // v1 adapters.test.js "opencode: grants and wisp's server go in OPENCODE_CONFIG_CONTENT"
        let mut m = BTreeMap::new();
        m.insert("wisp".to_owned(), json!({ "type": "http", "url": "http://127.0.0.1:1/api/mcp", "headers": { "x-wisp-token": "t" } }));
        let g = vec![json!({ "permission": "bash", "patterns": ["npm publish"] }).to_string()];
        let c: Value = serde_json::from_str(
            &config(
                Some(r#"{"permission":{"bash":"ask"},"mcp":{"mine":{"type":"local"}},"theme":"x"}"#),
                &m,
                &g,
                "edits",
                true,
                false,
                false,
            )
            .unwrap_or_default(),
        )
        .unwrap_or_default();
        assert_eq!(c["theme"], "x");
        assert_eq!(c["mcp"]["mine"]["type"], "local");
        assert_eq!(c["mcp"]["wisp"]["type"], "remote");
        assert_eq!(c["mcp"]["wisp"]["headers"]["x-wisp-token"], "t");
        assert_eq!(c["permission"]["bash"], json!({ "*": "ask", "npm publish": "allow" }));
        assert_eq!(c["permission"]["task"], "deny");
        assert!(config(None, &BTreeMap::new(), &[], "edits", false, false, false).is_none());
        let capped: Value =
            serde_json::from_str(&config(None, &BTreeMap::new(), &[], "edits", false, true, false).unwrap_or_default()).unwrap_or_default();
        assert_eq!(capped["permission"]["webfetch"], "ask");
    }

    /// ADR-0128: the run's browser (runtime.browser on) is the runtime's `browser` stdio server, which reaches opencode in its inline config.
    #[test]
    fn the_browser_server_goes_in_the_inline_config() {
        let m = BTreeMap::from([(
            "browser".to_owned(),
            json!({ "type": "stdio", "command": "npx", "args": ["-y", "@playwright/mcp@0.0.86", "--headless", "--isolated"] }),
        )]);
        let c: Value = serde_json::from_str(&config(None, &m, &[], "edits", false, false, false).unwrap_or_default()).unwrap_or_default();
        assert_eq!(c["mcp"]["browser"]["type"], "local");
        assert_eq!(c["mcp"]["browser"]["command"], json!(["npx", "-y", "@playwright/mcp@0.0.86", "--headless", "--isolated"]));
    }

    /// ADR-0135: Plan always carries its rules, over the user's own and any grant: no edits, and a shell that only
    /// reads (none at all for a task from outside).
    #[test]
    fn plan_denies_edits_and_every_command_but_a_read_over_the_users_config() {
        let parse = |c: Option<String>| serde_json::from_str::<Value>(&c.expect("Plan always has a config")).unwrap_or_default();
        let c = parse(config(None, &BTreeMap::new(), &[], "plan", false, false, true));
        assert_eq!(c["permission"]["edit"], "deny");
        let bash = &c["permission"]["bash"];
        assert_eq!(bash["*"], "deny");
        for read in ["git status *", "git log *", "ls *", "cat *", "rg *", "pwd"] {
            assert_eq!(bash[read], "allow", "{read}");
        }
        assert_eq!(bash["cat *>*"], "deny");
        assert_eq!(bash["find *-delete*"], "deny");
        let keys: Vec<&String> = bash.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        let at = |k: &str| keys.iter().position(|x| *x == k);
        assert!(at("cat *") < at("cat *>*") && at("find *") < at("find *-exec*"), "a deny comes after the allow it narrows: {keys:?}");
        for (allow, deny) in [
            ("git log *", "git log *--output*"),
            ("git show *", "git show *--output*"),
            ("git diff *", "git diff *--output*"),
            ("git log *", "git log *--ext-diff*"),
            ("git show *", "git show *--ext-diff*"),
            ("git diff *", "git diff *--ext-diff*"),
            ("cat *", "cat *<(*"),
        ] {
            assert_eq!(bash[deny], "deny", "{deny}");
            assert!(at(allow).is_some() && at(allow) < at(deny), "{deny} sorts after {allow}: {keys:?}");
        }

        let g = vec![
            json!({ "permission": "bash", "patterns": ["npm install"] }).to_string(),
            json!({ "permission": "edit", "patterns": ["*"] }).to_string(),
        ];
        let open =
            parse(config(Some(r#"{"permission":{"edit":"allow","bash":"allow"}}"#), &BTreeMap::new(), &g, "plan", false, false, true));
        assert_eq!(open["permission"]["edit"], "deny", "the user's edit: allow doesn't reach Plan");
        assert_eq!(open["permission"]["bash"], *bash, "nor their bash: allow, nor a grant");
        let star = parse(config(Some(r#"{"permission":"allow"}"#), &BTreeMap::new(), &[], "plan", false, false, true));
        assert_eq!(star["permission"]["edit"], "deny");
        assert_eq!(star["permission"]["*"], "allow", "the rest of the user's rules stay");

        let outside = parse(config(None, &BTreeMap::new(), &[], "plan", false, true, false));
        assert_eq!(outside["permission"]["bash"], "deny", "a trigger's or a channel's Plan has no shell");
        assert_eq!(outside["permission"]["edit"], "deny");
        assert!(config(None, &BTreeMap::new(), &[], "edits", false, false, true).is_none(), "Edits as it was");
        assert!(config(None, &BTreeMap::new(), &[], "auto", false, false, true).is_none(), "Full auto as it was");
    }

    #[test]
    fn a_delegating_run_turns_task_off_with_nothing_else_to_add() {
        let c = config(None, &BTreeMap::new(), &[], "edits", true, false, false).expect("a config: its own subagents are off");
        let c: Value = serde_json::from_str(&c).unwrap_or_default();
        assert_eq!(c["permission"]["task"], "deny");
    }
}
