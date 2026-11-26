//! `codex exec --json …` for one run (v1 codex.js `run()`'s argv). The prompt goes on stdin.

use std::collections::BTreeMap;

use serde_json::Value;

/// A lean run (the retro) only reads its prompt and answers: none of codex's extras (v1 `LEAN_FEATURES`).
pub const LEAN_FEATURES: [&str; 8] =
    ["plugins", "apps", "multi_agent", "skill_search", "browser_use", "computer_use", "goals", "code_mode_host"];
/// The one-line base prompt a lean run gets instead of the coding agent's (v1 `LEAN_BASE`).
pub const LEAN_BASE: &str = "You answer in one message. Do not run tools, read files or edit anything.";
/// `ask_user` may wait a week (v1 `ASK_TIMEOUT_MS / 1000`).
pub const ASK_TIMEOUT_S: u64 = 7 * 24 * 3600;

/// What the argv depends on. `mode`: `plan` | `edits` | `auto`.
#[derive(Debug, Clone, Default)]
pub struct CodexArgs<'a> {
    pub mode: &'a str,
    pub model: &'a str,
    pub effort: &'a str,
    /// the task's other folders, writable like cwd
    pub dirs: &'a [String],
    pub resume_id: Option<&'a str>,
    pub fork: bool,
    pub capped: bool,
    /// a side discussion's run: it has its main task's access, so its sandbox reaches the network as Full auto would
    /// (codex's workspace-write keeps it off by default). Never past a channel's cap.
    pub network: bool,
    /// a lean run: the base prompt file's path
    pub lean_base: Option<&'a str>,
    /// wisp's protocol as developer instructions (codex ≥ 0.53, not Windows, not set in config.toml)
    pub developer: Option<&'a str>,
    pub mcp_servers: Option<&'a BTreeMap<String, Value>>,
    /// image attachments (`-i`)
    pub images: &'a [String],
    /// the folders of the other attachments (`--add-dir`, new threads only)
    pub attachment_dirs: &'a [String],
    /// the wisp binary for the long-thread hooks (ADR-0097): `PreToolUse` on Bash sends long output to a file
    /// (`wisp hook offload --codex`), `PostCompact` counts a compaction (`wisp hook compacted`). Only in Full auto: a
    /// hook given on the command line runs only with `--dangerously-bypass-hook-trust`, which would also run a repo's
    /// own untrusted hooks outside the sandbox that plan and edits keep.
    pub hooks: Option<&'a str>,
}

/// A path as a POSIX shell word.
fn sh_word(p: &str) -> String {
    format!("'{}'", p.replace('\'', r"'\''"))
}

/// v1's `toml`: arrays and inline tables, scalars as JSON.
pub fn toml(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{}]", a.iter().map(toml).collect::<Vec<_>>().join(",")),
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, x)| format!("{k}={}", toml(x))).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

fn s(v: &str) -> String {
    toml(&Value::String(v.to_owned()))
}

/// Environment names a process is shaped by: a server's own value for one of them is no secret and stays in its `env`
/// table (forwarding it would change codex's own).
const PROCESS_ENV: [&str; 11] = ["PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "TMP", "TEMP", "LANG", "LC_ALL", "PWD"];

/// One value an MCP server needs that goes through codex's environment instead of its argv (ADR-0116, audit I IS3): an
/// HTTP server's header (wisp's token, a connector's key), under a name of wisp's own, or a command server's env value,
/// under its own name (codex's `env_vars` forwards a variable by its name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secret {
    pub server: String,
    /// the header it fills, for an HTTP server; `None`: a command server's env
    pub header: Option<String>,
    /// the variable in codex's environment
    pub var: String,
    pub value: String,
}

/// The values [`args`] names by variable, in order. Two command servers asking for one name with different values: the
/// first keeps it, the other's isn't passed (one environment holds one value per name).
pub fn mcp_secrets(servers: Option<&BTreeMap<String, Value>>) -> Vec<Secret> {
    let mut out: Vec<Secret> = Vec::new();
    for (i, (name, srv)) in servers.into_iter().flatten().enumerate() {
        let str_of = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_owned);
        if srv.get("url").and_then(Value::as_str).is_some() {
            for (j, (h, v)) in srv.get("headers").and_then(Value::as_object).into_iter().flatten().enumerate() {
                // named `…TOKEN…`: codex's default shell policy keeps such names from its agent's commands, and a user's own
                // `shell_environment_policy` is left as they set it (the review of audit I IS3)
                out.push(Secret {
                    server: name.clone(),
                    header: Some(h.clone()),
                    var: format!("WISP_MCP_TOKEN_{i}_{j}"),
                    value: str_of(v),
                });
            }
        } else {
            for (k, v) in srv.get("env").and_then(Value::as_object).into_iter().flatten() {
                if PROCESS_ENV.contains(&k.as_str()) || codex_reads(k) || out.iter().any(|x| x.header.is_none() && x.var == *k) {
                    continue;
                }
                out.push(Secret { server: name.clone(), header: None, var: k.clone(), value: str_of(v) });
            }
        }
    }
    out
}

/// Names codex itself (or what it runs on) reads: a server's value for one isn't put in codex's environment, where it
/// would change codex's own account, endpoint, proxy or loader (the review of audit I IS3). Such a server gets none.
fn codex_reads(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    ["OPENAI_", "CODEX_", "SSL_", "NODE_", "LD_", "DYLD_", "RUST_"].iter().any(|p| n.starts_with(p))
        || n.ends_with("_PROXY")
        || n == "NO_PROXY"
}

/// What codex's environment gets for its MCP servers: each [`Secret`]'s variable.
pub fn mcp_env(servers: Option<&BTreeMap<String, Value>>) -> BTreeMap<String, String> {
    mcp_secrets(servers).into_iter().map(|x| (x.var, x.value)).collect()
}

/// A TOML key: bare when it can be, quoted otherwise (`x-wisp-token` is bare; `Authorization` too).
fn toml_key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { k.to_owned() } else { s(k) }
}

pub fn args(a: &CodexArgs<'_>) -> Vec<String> {
    let mut out: Vec<String> = ["exec", "--json", "--skip-git-repo-check"].map(str::to_owned).to_vec();
    let c = |out: &mut Vec<String>, kv: String| {
        out.push("-c".into());
        out.push(kv);
    };
    if let Some(base) = a.lean_base {
        for f in LEAN_FEATURES {
            c(&mut out, format!("features.{f}=false"));
        }
        c(&mut out, format!("model_instructions_file={}", s(base)));
    }
    if let Some(dev) = a.developer.map(str::trim).filter(|d| !d.is_empty()) {
        c(&mut out, format!("developer_instructions={}", s(dev)));
    }
    if a.mode == "auto" {
        out.push("--dangerously-bypass-approvals-and-sandbox".into());
    } else {
        c(&mut out, format!("sandbox_mode={}", s(if a.mode == "plan" { "read-only" } else { "workspace-write" })));
    }
    if let Some(wisp) = a.hooks.filter(|w| !w.is_empty() && a.mode == "auto" && a.lean_base.is_none()) {
        let hook = |cmd: &str| format!("{{type={},command={},timeout=10}}", s("command"), s(&format!("{} {cmd}", sh_word(wisp))));
        c(&mut out, format!("hooks.PreToolUse=[{{matcher={},hooks=[{}]}}]", s("Bash"), hook("hook offload --codex")));
        c(&mut out, format!("hooks.PostCompact=[{{hooks=[{}]}}]", hook("hook compacted")));
        out.push("--dangerously-bypass-hook-trust".into());
    }
    if a.capped && a.mode != "auto" {
        c(&mut out, "sandbox_workspace_write.network_access=false".into());
    } else if a.network && a.mode != "auto" {
        c(&mut out, "sandbox_workspace_write.network_access=true".into());
    }
    if !a.dirs.is_empty() && a.mode != "auto" {
        c(&mut out, format!("sandbox_workspace_write.writable_roots={}", toml(&Value::from(a.dirs.to_vec()))));
    }
    if !a.model.is_empty() {
        out.push("-m".into());
        out.push(a.model.into());
    }
    if !a.effort.is_empty() {
        c(&mut out, format!("model_reasoning_effort={}", s(a.effort)));
    }
    let secrets = mcp_secrets(a.mcp_servers);
    for (name, srv) in a.mcp_servers.into_iter().flatten() {
        let key = format!("mcp_servers.{name}");
        let mine: Vec<&Secret> = secrets.iter().filter(|x| x.server == *name).collect();
        if let Some(url) = srv.get("url").and_then(Value::as_str) {
            // a streamable-HTTP server (wisp's own, in v2): its headers (wisp's token, a connector's key) read from
            // codex's environment, never written on argv (ADR-0116, audit I IS3)
            c(&mut out, format!("{key}.url={}", s(url)));
            let headers: Vec<String> =
                mine.iter().filter_map(|x| x.header.as_deref().map(|h| format!("{}={}", toml_key(h), s(&x.var)))).collect();
            if !headers.is_empty() {
                c(&mut out, format!("{key}.env_http_headers={{{}}}", headers.join(",")));
            }
        } else {
            c(&mut out, format!("{key}.command={}", toml(srv.get("command").unwrap_or(&Value::Null))));
            c(&mut out, format!("{key}.args={}", toml(srv.get("args").filter(|x| !x.is_null()).unwrap_or(&Value::Array(Vec::new())))));
            // the names a process is shaped by (PATH, HOME …) stay as they are: no secret, and passing them through would
            // change codex's own; every other value is forwarded from codex's environment by name (`env_vars`)
            let plain: serde_json::Map<String, Value> = srv
                .get("env")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter(|(k, _)| PROCESS_ENV.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !plain.is_empty() {
                c(&mut out, format!("{key}.env={}", toml(&Value::Object(plain))));
            }
            let vars: Vec<Value> = mine.iter().filter(|x| x.header.is_none()).map(|x| Value::String(x.var.clone())).collect();
            if !vars.is_empty() {
                c(&mut out, format!("{key}.env_vars={}", toml(&Value::Array(vars))));
            }
        }
        c(&mut out, format!("{key}.tool_timeout_sec={ASK_TIMEOUT_S}"));
        // wisp's own tools need no approval: outside Full auto, exec's approval policy refuses every MCP call
        c(&mut out, format!("{key}.default_tools_approval_mode={}", s("approve")));
    }
    for img in a.images {
        out.push("-i".into());
        out.push(img.clone());
    }
    let resume = a.resume_id.filter(|r| !r.is_empty());
    if resume.is_none() {
        let mut seen = std::collections::BTreeSet::new();
        for d in a.dirs.iter().chain(a.attachment_dirs) {
            if seen.insert(d.clone()) {
                out.push("--add-dir".into());
                out.push(d.clone());
            }
        }
    }
    if let Some(r) = resume {
        out.push(if a.fork { "fork" } else { "resume" }.into());
        out.push(r.into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mcp_servers_go_through_config() {
        let mut m = BTreeMap::new();
        m.insert("wisp".to_owned(), json!({ "type": "http", "url": "http://127.0.0.1:1/api/mcp", "headers": { "x-wisp-token": "t" } }));
        m.insert("x".to_owned(), json!({ "command": "node", "args": ["s.js"], "env": { "A": "1" } }));
        let a = args(&CodexArgs { mode: "edits", mcp_servers: Some(&m), ..CodexArgs::default() }).join(" ");
        assert!(a.contains(r#"-c mcp_servers.wisp.url="http://127.0.0.1:1/api/mcp" -c mcp_servers.wisp.env_http_headers={x-wisp-token="WISP_MCP_TOKEN_0_0"} -c mcp_servers.wisp.tool_timeout_sec=604800 -c mcp_servers.wisp.default_tools_approval_mode="approve""#), "{a}");
        assert!(a.contains(r#"-c mcp_servers.x.command="node" -c mcp_servers.x.args=["s.js"] -c mcp_servers.x.env_vars=["A"]"#), "{a}");
        assert_eq!(
            mcp_env(Some(&m)),
            BTreeMap::from([("WISP_MCP_TOKEN_0_0".to_owned(), "t".to_owned()), ("A".to_owned(), "1".to_owned())])
        );
    }

    #[test]
    fn developer_instructions_and_images() {
        let imgs = vec!["/up/a.png".to_owned()];
        let a =
            args(&CodexArgs { mode: "plan", developer: Some(" # You are running inside wisp\n"), images: &imgs, ..CodexArgs::default() });
        assert!(a.iter().any(|x| x == r##"developer_instructions="# You are running inside wisp""##));
        assert!(a.windows(2).any(|w| w == ["-i", "/up/a.png"]));
    }

    /// ADR-0097: in Full auto a run with the wisp binary gets the offload hook on Bash and the compaction counter, with
    /// the trust bypass they need; plan, edits and a lean run get none (the bypass would run a repo's untrusted hooks).
    #[test]
    fn full_auto_gets_the_long_thread_hooks_and_nothing_else_does() {
        let a = args(&CodexArgs { mode: "auto", hooks: Some("/opt/wisp it/wisp"), ..CodexArgs::default() });
        assert!(
            a.iter().any(|x| x == r#"hooks.PreToolUse=[{matcher="Bash",hooks=[{type="command",command="'/opt/wisp it/wisp' hook offload --codex",timeout=10}]}]"#),
            "{a:?}"
        );
        assert!(
            a.iter()
                .any(|x| x == r#"hooks.PostCompact=[{hooks=[{type="command",command="'/opt/wisp it/wisp' hook compacted",timeout=10}]}]"#)
        );
        assert!(a.iter().any(|x| x == "--dangerously-bypass-hook-trust"));
        for mode in ["plan", "edits"] {
            let a = args(&CodexArgs { mode, hooks: Some("/opt/wisp"), ..CodexArgs::default() });
            assert!(!a.iter().any(|x| x.starts_with("hooks.") || x.contains("hook-trust")), "{mode}: {a:?}");
        }
        let lean = args(&CodexArgs { mode: "auto", hooks: Some("/opt/wisp"), lean_base: Some("BASE"), ..CodexArgs::default() });
        assert!(!lean.iter().any(|x| x.contains("hook-trust")), "a lean run only answers");
        assert!(!args(&CodexArgs { mode: "auto", ..CodexArgs::default() }).iter().any(|x| x.contains("hook-trust")));
    }

    /// Audit I IS3: wisp's token (`http_headers={x-wisp-token=…}`) and a connector's secret env (`env={QA_VAR=…}`) were
    /// on codex's argv, readable by any local user with `ps`. They go through codex's environment by name; argv names
    /// the variables only. A name codex reads itself (`OPENAI_API_KEY`) isn't put there.
    #[test]
    fn no_secret_goes_on_argv_audit_i_is3() {
        let mut m = BTreeMap::new();
        m.insert("wisp".to_owned(), json!({ "type": "http", "url": "http://127.0.0.1:1/api/mcp", "headers": { "x-wisp-token": "tok-SECRET-1", "x-wisp-session": "s1" } }));
        m.insert("old-echo".to_owned(), json!({ "command": "echo-mcp", "env": { "QA_VAR": "v1SECRET", "PATH": "/opt/bin" } }));
        m.insert("other".to_owned(), json!({ "command": "o", "env": { "QA_VAR": "otherSECRET", "OPENAI_API_KEY": "sk-otherSECRET" } }));
        let a = args(&CodexArgs { mode: "edits", mcp_servers: Some(&m), ..CodexArgs::default() }).join(" ");
        assert!(!a.contains("SECRET") && !a.contains("s1\""), "{a}");
        assert!(a.contains(r#"mcp_servers.old-echo.env={PATH="/opt/bin"} -c mcp_servers.old-echo.env_vars=["QA_VAR"]"#), "{a}");
        assert!(!a.contains("mcp_servers.other.env_vars"), "a name another server holds isn't passed twice: {a}");
        assert!(
            a.contains(r#"mcp_servers.wisp.env_http_headers={x-wisp-session="WISP_MCP_TOKEN_2_0",x-wisp-token="WISP_MCP_TOKEN_2_1"}"#),
            "{a}"
        );
        assert!(!a.contains("shell_environment_policy"), "the user's own policy is left alone: {a}");
        let env = mcp_env(Some(&m));
        assert_eq!(env["QA_VAR"], "v1SECRET");
        assert_eq!(env.values().filter(|v| v.contains("tok-SECRET-1")).count(), 1);
        assert!(!env.contains_key("PATH"));
        assert!(!env.contains_key("OPENAI_API_KEY"), "codex's own key isn't replaced by a server's");
    }
}
