//! wisp's built-in MCP tools, called the way a harness calls them (`POST /api/mcp`): as a task's agent, with
//! the token its run was given, and as the user's editor (`wisp mcp`). Each tool here once ran an owner command no
//! plugin registered, and failed for every caller ("unknown command"); `tools/check` rust-mcp-owners now catches
//! that statically, and these tests catch a tool whose owner answers in the wrong shape.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use serde_json::{Value, json};

/// A running wisp with one task whose fake agent is mid-turn, and that task's MCP token.
struct Agent {
    serve: Serve,
    api: Api,
    port: u16,
    id: String,
    token: String,
    dir: std::path::PathBuf,
}

impl Agent {
    async fn start(name: &str) -> Self {
        Self::start_env(name, |_| Vec::new()).await
    }

    /// `start`, with more variables for the server (made from the test's folder, before it starts).
    async fn start_env(name: &str, env: impl FnOnce(&std::path::Path) -> Vec<(String, String)>) -> Self {
        Self::start_spec(name, env, json!({})).await
    }

    /// A task started from Settings → Plugins' Cast box: its spec says so (`ext["wisp.plugins"].kind`).
    async fn start_cast(name: &str) -> Self {
        Self::start_spec(name, |_| Vec::new(), json!({ "ext": { "wisp.plugins": { "kind": "cast" } } })).await
    }

    /// `start_env`, with more fields for the task's `runtime.create`.
    async fn start_spec(name: &str, env: impl FnOnce(&std::path::Path) -> Vec<(String, String)>, more_spec: Value) -> Self {
        let dir = temp(name);
        let rec = dir.join("rec");
        std::fs::create_dir_all(&rec).unwrap();
        let more = env(&dir);
        let mut vars = vec![("FAKE_DELAY", "20000"), ("FAKE_RECORD", rec.to_str().unwrap())];
        vars.extend(more.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let (serve, port) = start_with(&dir, &vars);
        let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
        let mut spec = json!({ "prompt": "hi", "cwd": dir.join("proj"), "agent": "claude" });
        if let (Some(o), Some(more)) = (spec.as_object_mut(), more_spec.as_object()) {
            o.extend(more.clone());
        }
        let made = api.cmd("runtime.create", spec).await;
        let id = made["id"].as_str().unwrap().to_owned();
        api.until(&id, "the turn running", |t| t.len() == 1 && t[0]["status"] == "running").await;
        let runs = until_records(&rec.join("claude.jsonl"), "the run's record", |r| !r.is_empty()).await;
        let token = common::wisp_token(&runs[0]);
        Self { serve, api, port, id, token, dir }
    }

    async fn post(&self, body: Value, agent: bool) -> Value {
        let mut r = self.api.http.post(format!("http://127.0.0.1:{}/api/mcp", self.port)).header("content-type", "application/json");
        if agent {
            r = r.header("x-wisp-token", &self.token).header("x-wisp-session", &self.id);
        } else {
            r = r.header("x-wisp-cwd", self.dir.join("proj").to_string_lossy().into_owned());
        }
        let r = r.body(body.to_string()).send().await.unwrap();
        serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null)
    }

    /// `tools/call` as the task's agent: (the tool's text, whether it's an error result).
    async fn call(&self, tool: &str, args: Value) -> (String, bool) {
        Self::text(
            &self
                .post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } }), true)
                .await,
        )
    }

    /// `tools/call` as the user's editor (`wisp mcp`).
    async fn user_call(&self, tool: &str, args: Value) -> (String, bool) {
        Self::text(
            &self
                .post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } }), false)
                .await,
        )
    }

    fn text(r: &Value) -> (String, bool) {
        let res = &r["result"];
        let text = res["content"].as_array().into_iter().flatten().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n");
        (if text.is_empty() { r.to_string() } else { text }, res["isError"] == true || r.get("error").is_some())
    }

    async fn names(&self, agent: bool) -> Vec<String> {
        let list = self.post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }), agent).await;
        list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect()
    }
}

/// Never "unknown command": the tool's owner is registered (whatever it then says about these arguments).
fn reached(tool: &str, (text, _): &(String, bool)) {
    assert!(!text.contains("unknown command"), "{tool} runs an owner command no plugin registers: {text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_builtin_tool_reaches_a_registered_owner() {
    // a Cast task: every built-in is listed, the plugin-writing tools too
    let a = Agent::start_cast("mcp-sweep").await;
    // these wait on the user or on other tasks; their owners are typed commands rust-mcp-owners checks
    let waits = ["ask_user", "delegate", "subagent_results", "wisp_launch"];
    let agent = a.names(true).await;
    assert!(agent.len() >= 20, "{agent:?}");
    for t in agent.iter().filter(|t| (!waits.contains(&t.as_str()) && t.starts_with("wisp_")) || *t == "resume_later") {
        reached(t, &a.call(t, json!({})).await);
    }
    for t in ["wisp_crew", "wisp_crew_add", "wisp_crew_edit", "wisp_crew_remove", "wisp_list", "wisp_agents"] {
        reached(t, &a.user_call(t, json!({})).await);
    }
    a.serve.stop();
}

/// The user's own harness (`wisp mcp`) adds a project through its MCP tool (`runtime_projectAdd`, generated from the
/// catalogue, ADR-0040) and gets the server's naming (ADR-0077): a second `wisp` folder is `wisp-2`, the same folder again
/// is the one it is. An agent inside a task asks for it on a card instead (ADR-0113, the setup cards' test).
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_adds_a_project_through_its_mcp_tool_and_the_server_names_it() {
    let a = Agent::start("mcp-project-add").await;
    assert!(a.names(false).await.iter().any(|n| n == "runtime_projectAdd"), "the user's harness has it");
    let (one, two) = (a.dir.join("a/wisp"), a.dir.join("b/wisp"));
    // a project is a folder that's there (audit A F1)
    for d in [&one, &two] {
        std::fs::create_dir_all(d).unwrap();
    }
    for (p, want) in [(&one, "\"name\":\"wisp\""), (&two, "\"name\":\"wisp-2\""), (&one, "\"created\":false")] {
        let (t, err) = a.user_call("runtime_projectAdd", json!({ "path": p })).await;
        assert!(!err && t.replace(' ', "").contains(want), "{}: {t}", p.display());
    }
    let names: Vec<String> = a
        .api
        .cmd("resources.export", json!({ "kinds": ["project"] }))
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"wisp".to_owned()) && names.contains(&"wisp-2".to_owned()), "{names:?}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_mcp_crew_tools_add_read_edit_and_remove_a_member() {
    let a = Agent::start("mcp-user-crew").await;
    let (t, err) = a.user_call("wisp_crew_add", json!({ "name": "Quill", "role": "Docs writer", "agent": "claude" })).await;
    assert!(!err && t.starts_with("Added Quill to the crew.") && t.contains("Docs writer"), "add: {t}");
    let (t, err) = a.user_call("wisp_crew", json!({})).await;
    assert!(!err && t.contains("**Quill** (@quill"), "list: {t}");
    let (t, err) = a.user_call("wisp_crew_edit", json!({ "member": "@Quill", "role": "Editor", "remember": ["likes short PRs"] })).await;
    assert!(!err && t.starts_with("Updated.") && t.contains("Editor"), "edit: {t}");
    let (t, err) = a.user_call("wisp_crew", json!({ "member": "Quill" })).await;
    assert!(!err && t.contains("## Notebook") && t.contains("- likes short PRs"), "detail: {t}");
    let (t, err) = a.user_call("wisp_crew_remove", json!({ "member": "Quill" })).await;
    assert!(!err && t.starts_with("Removed Quill from the crew."), "remove: {t}");
    let (t, _) = a.user_call("wisp_crew", json!({})).await;
    assert_eq!(t, "No crew yet. Add a member with wisp_crew_add.");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_crew_lists_the_crew_and_reads_one_member_in_full() {
    let a = Agent::start("mcp-crew-list").await;
    assert_eq!(a.call("wisp_crew", json!({})).await.0, "The user has no crew yet. Add a member with wisp_crew_add.");
    a.api.cmd("crew.create", json!({ "name": "Scout", "role": "CI watcher", "agent": "claude", "notebook": "main is green" })).await;
    let (t, err) = a.call("wisp_crew", json!({})).await;
    assert!(!err && t.starts_with("1 on the user's crew:") && t.contains("**Scout** (@scout"), "{t}");
    let (t, err) = a.call("wisp_crew", json!({ "member": "@scout" })).await;
    assert!(!err && t.contains("## Notebook\n\nmain is green") && t.contains("## Kit"), "{t}");
    let (t, err) = a.call("wisp_crew", json!({ "member": "Nobody" })).await;
    assert!(err && t.contains("No one called “Nobody” on the user's crew."), "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_crew_edit_changes_only_the_fields_given_and_remembers_lines() {
    let a = Agent::start("mcp-crew-edit").await;
    a.api.cmd("crew.create", json!({ "name": "Scout", "role": "CI watcher", "agent": "claude", "notebook": "main is green" })).await;
    // an agent in a task asks on a card (ADR-0113, the setup cards' test); the user's harness changes it at once
    let (t, err) =
        a.user_call("wisp_crew_edit", json!({ "member": "Scout", "role": "Release captain", "remember": ["ships on fridays"] })).await;
    assert!(!err && t.starts_with("Updated.") && t.contains("Release captain"), "{t}");
    let m = a.api.cmd("crew.get", json!({ "id": "Scout" })).await;
    assert_eq!(m["notebook"], "main is green\n- ships on fridays");
    assert_eq!(m["agent"], "claude", "a field not given stays");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_crew_remove_removes_a_member() {
    let a = Agent::start("mcp-crew-remove").await;
    a.api.cmd("crew.create", json!({ "name": "Scout", "agent": "claude" })).await;
    // an agent's own token can't (ADR-0112); its tool asks the user on a card (ADR-0113)
    let direct = raw_cmd(&a, "crew.agent.remove", json!({ "session": a.id, "member": "scout" })).await;
    assert!(direct["ok"] == false && direct.to_string().contains("can't change the crew itself"), "{direct}");
    let (t, err) = a.user_call("wisp_crew_remove", json!({ "member": "scout" })).await;
    assert!(!err && t.starts_with("Removed Scout from the crew."), "{t}");
    assert_eq!(a.api.cmd("crew.list", json!({})).await, json!([]));
    let (t, err) = a.user_call("wisp_crew_remove", json!({ "member": "scout" })).await;
    assert!(err && t.contains("No such crew member"), "{t}");
    a.serve.stop();
}

/// A remote connector the user's harness adds by its header's name (`linear`, `Authorization` unset): the same in each
/// test. An agent in a task can't add one (ADR-0112).
async fn add_linear(a: &Agent) -> Value {
    a.api
        .cmd(
            "connectors.agent.add",
            json!({ "sid": a.id, "name": "Linear", "url": "https://mcp.linear.app/mcp", "headers": ["Authorization"] }),
        )
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_connectors_lists_the_users_connectors() {
    let a = Agent::start("mcp-connectors-list").await;
    let (t, err) = a.call("wisp_connectors", json!({})).await;
    assert!(!err, "{t}");
    assert_eq!(t, "The user has no connectors yet. Add one with wisp_connector_add.");
    add_linear(&a).await;
    let (t, err) = a.call("wisp_connectors", json!({})).await;
    assert!(!err, "{t}");
    assert_eq!(
        t,
        "The user's connectors (MCP servers every task gets; not in Plan mode, not Cursor or Antigravity):\n\n\
         - **linear** · remote: https://mcp.linear.app/mcp · needs Authorization from the user (wisp_connector_signin)"
    );
    a.serve.stop();
}

/// An agent in a task never adds a connector itself (ADR-0112: a command it wrote would run for every task); it asks on
/// a card (ADR-0113). The user adds one by its header's name, the value typed on the card.
#[tokio::test(flavor = "multi_thread")]
async fn wisp_connector_add_is_the_users_and_takes_key_names() {
    let a = Agent::start("mcp-connectors-add").await;
    let direct = raw_cmd(&a, "connectors.agent.add", json!({ "sid": a.id, "name": "Linear", "url": "https://mcp.linear.app/mcp" })).await;
    assert!(direct["ok"] == false && direct.to_string().contains("the user allows it on a card"), "{direct}");
    let refused = a
        .api
        .try_cmd(
            "connectors.agent.add",
            json!({ "sid": a.id, "name": "linear", "url": "https://mcp.linear.app/mcp", "headers": { "Authorization": "Bearer x" } }),
        )
        .await;
    assert!(refused.to_string().contains("give the names only"), "a value would sit in a transcript: {refused}");
    add_linear(&a).await;
    let v = a.api.cmd("connectors.list", json!({})).await;
    let c = &v["connectors"][0];
    assert_eq!(
        (c["id"].as_str(), c["headers"][0]["key"].as_str(), c["headers"][0]["set"].as_bool()),
        (Some("linear"), Some("Authorization"), Some(false))
    );
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_connector_signin_answers_for_a_connector() {
    let a = Agent::start("mcp-connectors-signin").await;
    let (t, err) = a.call("wisp_connector_signin", json!({ "name": "github" })).await;
    assert!(err && t.contains("No connector called “github”. List them with wisp_connectors."), "{t}");
    add_linear(&a).await;
    let (t, err) = a.call("wisp_connector_signin", json!({ "name": "Linear", "why": "to read the issue" })).await;
    assert!(!err, "{t}");
    assert!(
        t.starts_with("linear needs Authorization. Ask the user to open Settings → Connectors and type Authorization on linear"),
        "{t}"
    );
    assert!(
        t.ends_with("Tasks get it from their next turn: harnesses load MCP servers when a turn starts, so this turn can’t call it yet."),
        "{t}"
    );
    a.serve.stop();
}

/// The id in `wisp_save`'s answer (`…, id <id>. …`).
fn saved_id(t: &str) -> String {
    t.split(", id ").nth(1).and_then(|r| r.split('.').next()).unwrap_or_default().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_save_keeps_text_in_my_stuff() {
    let a = Agent::start("mcp-stuff-save").await;
    let (t, err) = a.call("wisp_save", json!({})).await;
    assert!(err && t.contains("Give exactly one of path (a file), text (with a name like notes.md), url, or app (a folder)"), "{t}");
    let (t, err) = a.call("wisp_save", json!({ "text": "x", "url": "https://example.com" })).await;
    assert!(err && t.contains("Give exactly one of"), "two at once: {t}");
    let (t, err) = a.call("wisp_save", json!({ "text": "# Plan\nShip on friday\n", "name": "plan.md", "title": "Launch plan" })).await;
    assert!(!err && t.starts_with("Saved “Launch plan” (doc, 22 B) to the user's My Stuff, id "), "{t}");
    let id = saved_id(&t);
    assert!(t.ends_with(&format!("Show it in your answer as {{\"type\":\"stuff\",\"id\":\"{id}\"}}.")), "{t}");
    let list = a.api.cmd("stuff.list", json!({})).await;
    let it = &list.as_array().unwrap()[0];
    assert_eq!((it["id"].as_str(), it["by"].as_str()), (Some(id.as_str()), Some("agent")));
    assert_eq!(it["from"]["session"].as_str(), Some(a.id.as_str()), "kept for the task that saved it");
    // a file, relative to the task's folder
    std::fs::write(a.dir.join("proj/notes.txt"), "hello").unwrap();
    let (t, err) = a.call("wisp_save", json!({ "path": "notes.txt" })).await;
    assert!(!err && t.starts_with("Saved “notes”") && t.contains("5 B"), "{t}");
    let (t, err) = a.call("wisp_save", json!({ "path": "gone.png" })).await;
    assert!(err && t.contains(&format!("Nothing at {}.", a.dir.join("proj").join("gone.png").display())), "{t}");
    // id alone: only its details change, the version before kept
    let (t, err) = a.call("wisp_save", json!({ "id": id, "note": "for friday" })).await;
    assert!(!err && t.starts_with("Updated “Launch plan” (doc, 22 B) in the user's My Stuff, now v"), "{t}");
    let (t, err) = a.call("wisp_save", json!({ "id": "nope", "title": "x" })).await;
    assert!(err && t.contains("That’s no longer in My Stuff."), "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_stuff_lists_and_reads_one_item() {
    let a = Agent::start("mcp-stuff-list").await;
    let (t, err) = a.call("wisp_stuff", json!({})).await;
    assert!(!err && t == "My Stuff is empty.", "{t}");
    let (t, _) =
        a.call("wisp_save", json!({ "text": "Ship on friday", "name": "plan.md", "title": "Launch plan", "tags": ["launch"] })).await;
    let id = saved_id(&t);
    a.call("wisp_save", json!({ "url": "https://example.com/brand", "title": "Brand guide" })).await;
    let (t, err) = a.call("wisp_stuff", json!({})).await;
    assert!(!err && t.starts_with("2 items in My Stuff:\n"), "{t}");
    assert!(
        t.contains("**Launch plan** — doc") && t.contains("**Brand guide** — link") && t.contains("url: https://example.com/brand"),
        "{t}"
    );
    assert!(t.ends_with("wisp_save with an id updates it."), "{t}");
    let (t, err) = a.call("wisp_stuff", json!({ "query": "launch" })).await;
    assert!(!err && t.starts_with("1 item in My Stuff:\n") && t.contains("#launch"), "{t}");
    let (t, err) = a.call("wisp_stuff", json!({ "kind": "link", "query": "launch" })).await;
    assert!(!err && t == "Nothing in My Stuff matches that.", "{t}");
    let (t, err) = a.call("wisp_stuff", json!({ "id": id })).await;
    assert!(!err && t.contains("```md\nShip on friday\n```"), "one item in full, its text too: {t}");
    assert!(t.contains("To change it, write the new version and wisp_save with id and text (or path)."), "{t}");
    let (t, err) = a.call("wisp_stuff", json!({ "id": "nope" })).await;
    assert!(!err && t == "No item nope in My Stuff.", "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_pr_attaches_and_detaches_a_pr() {
    let a = Agent::start_env("mcp-pr", |dir| {
        // gh is wisp-testkit's fake, first on PATH, with one open PR
        let bin = dir.join("gh-bin");
        install_fakes(&bin, &["gh"]);
        let state = dir.join("gh-state.json");
        let pr = json!({ "number": 12, "url": "https://github.com/acme/repo/pull/12", "title": "Fix the login", "state": "OPEN",
            "isDraft": false, "headRefName": "fix-login", "baseRefName": "main", "reviewDecision": "", "mergeStateStatus": "CLEAN",
            "statusCheckRollup": [] });
        std::fs::write(&state, json!({ "prs": { "12": pr } }).to_string()).unwrap();
        // the OS's own separator (`;` on Windows): with `:` Windows found no fake and ran the runner's real gh
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())))
            .unwrap()
            .to_string_lossy()
            .into_owned();
        vec![("PATH".into(), path), ("FAKE_GH_STATE".into(), state.display().to_string())]
    })
    .await;
    let (t, err) = a.call("wisp_pr", json!({ "pr": " " })).await;
    assert!(
        err && t.contains("Give the pull request’s URL (https://github.com/owner/repo/pull/123), or its number in this task’s repo."),
        "{t}"
    );
    let (t, err) = a.call("wisp_pr", json!({ "pr": "99" })).await;
    assert!(err && t.contains("gh could not read that PR"), "{t}");
    let (t, err) = a.call("wisp_pr", json!({ "pr": "#12" })).await;
    assert_eq!(
        (t.as_str(), err),
        (
            "Attached PR #12 “Fix the login” (open): https://github.com/acme/repo/pull/12. It shows on the task, and wisp follows its checks and reviews.",
            false
        )
    );
    let (t, err) = a.call("wisp_pr", json!({ "pr": "https://github.com/acme/repo/pull/12", "remove": true })).await;
    assert_eq!((t.as_str(), err), ("Detached PR #12 (https://github.com/acme/repo/pull/12).", false));
    let (t, err) = a.call("wisp_pr", json!({ "pr": "12", "remove": true })).await;
    assert!(err && t.contains("No PR like that is attached to this task."), "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_tag_replaces_the_tasks_tags() {
    let a = Agent::start("mcp-tag").await;
    let (t, err) = a.call("wisp_tag", json!({ "tags": ["Auth", "#Redis", "rate limit", "auth"] })).await;
    assert_eq!(
        (t.as_str(), err),
        ("Tagged: #auth #redis #rate-limit. Call wisp_tag again only if the work moves to another topic; it replaces these.", false)
    );
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["tags"], json!(["auth", "redis", "rate-limit"]));
    let (t, err) = a.call("wisp_tag", json!({ "tags": ["billing"] })).await;
    assert!(!err && t.starts_with("Tagged: #billing."), "{t}");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["tags"], json!(["billing"]), "it replaces them");
    let (t, err) = a.call("wisp_tag", json!({ "tags": ["#", " "] })).await;
    assert!(err && t.contains("Give 3 to 8 short topic words, e.g. [\"auth\", \"rate-limit\", \"redis\"]."), "{t}");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["tags"], json!(["billing"]), "a refusal changes nothing");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_blocks_answers_the_block_formats() {
    let a = Agent::start("mcp-blocks").await;
    let (t, err) = a.call("wisp_blocks", json!({ "types": ["graph"] })).await;
    assert!(!err && t.starts_with("- {\"type\":\"graph\""), "{t}");
    let (t, err) = a.call("wisp_blocks", json!({ "types": ["App", "table"] })).await;
    assert!(!err && t.starts_with("- {\"type\":\"app\""), "{t}");
    assert!(
        t.ends_with("No long docs for \"table\": graph and app have them; the others are described in full in your instructions."),
        "{t}"
    );
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_stats_answers_tool_counts() {
    let a = Agent::start("mcp-stats").await;
    // a finished codex task from v1, with three tool calls (and an answer, which isn't one)
    let v1 = a.dir.join("v1");
    std::fs::create_dir_all(&v1).unwrap();
    let now = i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap();
    let at = now - 3_600_000;
    let step = |kind: &str, t: i64| json!({ "kind": kind, "label": format!("a {kind}"), "t": t });
    let old = json!({
        "id": "old-1", "title": "Fix the build", "agent": "codex", "model": "gpt-5", "cwd": a.dir.join("site"), "state": "done",
        "createdAt": at, "updatedAt": at + 60_000,
        "turns": [{ "id": "old-1-t1", "prompt": "fix it", "agent": "codex", "model": "gpt-5", "status": "done",
            "startedAt": at, "endedAt": at + 60_000, "stats": { "costUsd": 0.5, "inputTokens": 100, "outputTokens": 20 },
            "activity": [step("run", at + 1), step("read", at + 2), step("run", at + 3), step("answer", at + 4)] }]
    });
    std::fs::write(v1.join("old-1.json"), old.to_string()).unwrap();
    let r = a.api.cmd("runtime.import", json!({ "dir": v1 })).await;
    assert_eq!(r["imported"], 1, "{r}");

    let (t, err) = a.call("wisp_stats", json!({})).await;
    assert!(
        !err && t
            .starts_with("## wisp tool stats — last 30 days\n3 tool calls in 1 turns across 1 sessions. Success rate 100%, cost $0.50"),
        "{t}"
    );
    assert!(t.contains("| run | 2 | 67% |") && t.contains("| read | 1 | 33% |"), "{t}");
    assert!(t.contains("| codex | 3 | 1 | 100% | $0.50 | run 2, read 1 |") && t.contains("| site | 3 | 1 |"), "{t}");
    let (t, err) = a.call("wisp_stats", json!({ "days": "7", "agent": "claude" })).await;
    assert!(
        !err && t.starts_with("## wisp tool stats — last 7 days (harness claude)") && t.ends_with("No finished turns in this range."),
        "{t}"
    );
    let (t, err) = a.call("wisp_stats", json!({ "project": "current" })).await;
    assert!(
        !err && t.starts_with("## wisp tool stats — last 30 days (project proj)") && t.ends_with("No finished turns in this range."),
        "{t}"
    );
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_available_lists_harnesses_and_projects() {
    let a = Agent::start("mcp-available").await;
    let path = a.dir.join("proj").display().to_string();
    a.api
        .cmd("resources.apply", json!({ "resources": [{ "kind": "project", "name": "proj", "spec": { "path": path } }], "trust": true }))
        .await;
    let (t, err) = a.call("wisp_available", json!({})).await;
    assert!(!err && t.starts_with("## Harnesses on this computer\n"), "{t}");
    assert!(t.contains("- **claude** (") && t.contains("- **auto**: wisp picks the harness and model"), "{t}");
    assert!(t.contains("## Machines\nNone: everything runs on this computer."), "{t}");
    assert!(t.contains(&format!("## Projects\n- **proj**: {path}\n")), "{t}");
    assert!(t.contains("## Subagent presets (saved teams)\nNone."), "{t}");
    assert!(t.contains(&format!("Task `{}` on claude", a.id)) && t.contains("No subagents yet: delegate creates them on the fly."), "{t}");
    assert!(t.ends_with("wisp_task: follow up, stop, finish or archive a task."), "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_task_replies_to_another_task() {
    let a = Agent::start("mcp-task-reply").await;
    let made = a.api.cmd("runtime.create", json!({ "prompt": "other work", "cwd": a.dir.join("proj"), "agent": "claude" })).await;
    let other = made["id"].as_str().unwrap().to_owned();
    a.api.until(&other, "its turn running", |t| t.len() == 1 && t[0]["status"] == "running").await;
    a.api.cmd("runtime.stop", json!({ "id": other })).await;
    a.api.until(&other, "its turn over", |t| t.len() == 1 && ended(&t[0])).await;
    let (t, err) = a.call("wisp_task", json!({ "task": format!("wisp:{other}"), "action": "reply", "text": " " })).await;
    assert!(err && t.contains("text is required."), "{t}");
    let (t, err) = a.call("wisp_task", json!({ "task": other, "action": "reply", "text": "and the tests", "mode": "fast" })).await;
    assert!(err && t.contains("mode must be plan, edits or auto."), "{t}");
    let (t, err) = a.call("wisp_task", json!({ "task": other, "action": "reply", "text": "and the tests" })).await;
    assert!(!err && t.starts_with("Sent; it is working on it. “"), "{t}");
    assert!(t.ends_with(&format!("(`{other}`). wisp_task with action \"wait\" blocks until it is done.")), "{t}");
    let turns = a.api.until(&other, "the reply's turn", |t| t.len() == 2).await;
    assert_eq!(turns[1]["prompt"], "and the tests");
    let (t, err) = a.call("wisp_task", json!({ "task": other, "action": "rename", "title": "  Tests\n too " })).await;
    assert_eq!((t.as_str(), err), ("Renamed it “Tests too”.", false));
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": other })).await["title"], "Tests too");
    let (t, err) = a.call("wisp_task", json!({ "task": other, "action": "launch" })).await;
    assert!(err && t.contains("action is one of reply, stop, done, archive, unarchive, rename"), "{t}");
    let (t, err) = a.call("wisp_task", json!({ "task": "nope", "action": "stop" })).await;
    assert!(err && t.contains("No task `nope` on this computer. wisp_sessions lists them."), "{t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_task_refuses_this_task_itself() {
    let a = Agent::start("mcp-task-self").await;
    let (t, err) = a.call("wisp_task", json!({ "task": a.id, "action": "reply", "text": "hello me" })).await;
    assert!(err && t.contains("That is this task. resume_later schedules a follow-up to yourself."), "{t}");
    assert_eq!(a.api.turns(&a.id).await.len(), 1, "nothing was sent");
    a.serve.stop();
}

// ---- writing an untrusted plugin (ADR-0048): v1's spell tools in their v2 form ----

const PLUGIN_TOOLS: [&str; 4] = ["wisp_plugins", "wisp_plugin_schema", "wisp_plugin_data", "wisp_plugin_check"];

#[tokio::test(flavor = "multi_thread")]
async fn plugin_tools_only_in_a_cast_task_a_plain_tasks_list_lacks_them_and_a_call_is_refused() {
    let a = Agent::start("mcp-plugin-plain").await;
    let names = a.names(true).await;
    assert!(names.iter().any(|n| n == "wisp_sessions"), "{names:?}");
    for tool in PLUGIN_TOOLS {
        assert!(!names.iter().any(|n| n == tool), "{tool} isn't in a plain task's list: {names:?}");
        let (t, err) = a.call(tool, json!({})).await;
        assert!(err && t.contains("only for a task started from Settings → Plugins → Cast a plugin"), "{tool}: {t}");
    }
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_tools_a_cast_tasks_list_has_them() {
    let a = Agent::start_cast("mcp-plugin-cast").await;
    let names = a.names(true).await;
    for tool in PLUGIN_TOOLS {
        assert!(names.iter().any(|n| n == tool), "{tool} in a Cast task's list: {names:?}");
    }
    a.serve.stop();
}

/// A plugin of the user's, in a folder: a Settings pane that lists their plugins.
fn my_plugins(dir: &std::path::Path) -> (Value, String) {
    let manifest = json!({
        "wispPlugin": 1, "id": "user.my-plugins", "name": "My plugins", "version": "1.0.0", "api": "^1",
        "description": "Lists the plugins this wisp has.", "permissions": ["plugins:read"],
        "contributes": { "ui": [{ "id": "user.my-plugins.pane", "attachTo": { "id": "shell.settings", "input": "pane" }, "component": "./ui.js#default", "title": "My plugins" }] }
    });
    let ui = "wisp.call('plugins.list', {}).then((rows) => wisp.render({ c: 'Stack', k: rows.map((r) => ({ c: 'ListRow', p: { title: r.id } })) }));".to_owned();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("wisp-plugin.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("ui.js"), &ui).unwrap();
    (manifest, ui)
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_plugins_lists_the_users_own_plugins_in_plain_words_then_wisps() {
    let a = Agent::start_cast("mcp-plugins").await;
    let (t, err) = a.call("wisp_plugins", json!({})).await;
    assert!(!err && t.starts_with("The user has no plugins of their own yet"), "{t}");
    assert!(t.contains("**Plugins** (wisp.plugins): on."), "wisp's own, one line each: {t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_plugins_shows_an_installed_plugin_off_untrusted_with_its_manifest() {
    let a = Agent::start_cast("mcp-plugins-mine").await;
    let _ = my_plugins(&a.dir.join("mine"));
    a.api.cmd("plugins.install", json!({ "source": a.dir.join("mine") })).await;
    let (t, err) = a.call("wisp_plugins", json!({})).await;
    assert!(!err && t.starts_with("The user's own plugins (1)"), "{t}");
    assert!(t.contains("- **My plugins** (user.my-plugins 1.0.0): off, untrusted. Lists the plugins this wisp has."), "{t}");
    assert!(t.contains("Shows a pane in Settings → Advanced (“My plugins”)") && t.contains("May: `plugins:read`."), "{t}");
    assert!(t.contains("\"id\": \"user.my-plugins\""), "its manifest in full: {t}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_plugin_schema_is_the_reference_made_from_the_real_types() {
    let a = Agent::start_cast("mcp-plugin-schema").await;
    let (t, err) = a.call("wisp_plugin_schema", json!({})).await;
    assert!(!err && t.starts_with("# Writing a wisp plugin (the untrusted tier)"), "{}", &t[..t.len().min(300)]);
    for want in [
        "- `permissions`: ",
        "- `ui`: ",
        "- `SettingsRow` {",
        "`wisp.call(command, args)`",
        "- `shell.settings`: ",
        "- `plugins.list` ",
        "(`plugins:read`, reads)",
        "### “A Settings pane that lists my plugins and which are on”",
    ] {
        assert!(t.contains(want), "{want} in the reference");
    }
    assert!(!t.contains("`plugins.agent.list`"), "an agent's own tools aren't a plugin's commands");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_plugin_data_samples_a_read_only_command_and_refuses_the_rest() {
    let a = Agent::start_cast("mcp-plugin-data").await;
    let (t, err) = a.call("wisp_plugin_data", json!({ "command": "plugins.list" })).await;
    assert!(!err && t.starts_with("plugins.list answered a list of "), "{t}");
    assert!(t.contains("needs \"plugins:read\" in its permissions") && t.contains("\"id\": ") && t.contains(" more\""), "a sample: {t}");
    let (t, err) =
        a.call("wisp_plugin_data", json!({ "command": "plugins.setEnabled", "args": { "id": "wisp.plugins", "enabled": false } })).await;
    assert!(err && t.contains("plugins.setEnabled isn't a data source"), "{t}");
    let (t, err) = a.call("wisp_plugin_data", json!({ "command": "nope.nothing" })).await;
    assert!(err && t.contains("nope.nothing isn't a command wisp has"), "{t}");
    // another plugin's data: the nested call narrows through wisp.plugins' own grant, which declares its sources
    a.api.cmd("crew.create", json!({ "name": "Scout", "role": "CI watcher", "agent": "claude" })).await;
    let (t, err) = a.call("wisp_plugin_data", json!({ "command": "crew.list" })).await;
    assert!(!err && t.starts_with("crew.list answered a list of 1") && t.contains("\"Scout\""), "{t}");
    let (t, err) = a.call("wisp_plugin_data", json!({ "command": "runtime.unmasked" })).await;
    assert!(
        err && t.contains("runtime.unmasked (\"runtime:unmasked\") isn't one wisp_plugin_data samples"),
        "never what reveals a secret: {t}"
    );
    let (t, err) = a.call("wisp_plugin_data", json!({ "command": "plugins.agent.list" })).await;
    assert!(err && t.contains("isn't a data source"), "not its own tools: {t}");
    // nothing was switched off
    let list = a.api.cmd("plugins.list", json!({})).await;
    assert!(list.as_array().unwrap().iter().any(|p| p["id"] == "wisp.plugins" && p["enabled"] == true), "{list}");
    a.serve.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn wisp_plugin_check_validates_a_plugin_without_installing_it() {
    let a = Agent::start_cast("mcp-plugin-check").await;
    let (manifest, ui) = my_plugins(&a.dir.join("mine"));
    let (t, err) = a.call("wisp_plugin_check", json!({ "manifest": manifest, "files": { "ui.js": ui } })).await;
    assert!(!err && t.starts_with("ok: My plugins (user.my-plugins) would install."), "{t}");
    assert!(t.contains("- calls plugins.list (") && t.contains("which only reads"), "{t}");
    let list = a.api.cmd("plugins.list", json!({})).await;
    assert!(!list.to_string().contains("user.my-plugins"), "checked, never installed: {list}");
    let (t, err) = a.call("wisp_plugin_check", json!({ "manifest": { "wispPlugin": 1, "id": "my-plugins", "version": "1.0.0", "api": "^1", "permissions": [], "contributes": manifest["contributes"] } })).await;
    assert!(!err && t.contains("problems to fix") && t.contains("isn't a plugin of yours") && t.contains("ui.js isn't given"), "{t}");
    a.serve.stop();
}

/// Owner-list row 61 (ADR-0111): a task's own `wisp_launch` takes `formula` (a file in its folder): the new task runs
/// under it, nothing applied, its agent and goal named from it; a name it doesn't declare is the agent's error, saying
/// what there is.
#[tokio::test(flavor = "multi_thread")]
async fn a_tasks_wisp_launch_runs_under_a_formula_and_refuses_a_name_it_lacks() {
    let a = Agent::start("mcp-launch-formula").await;
    let proj = a.dir.join("proj");
    std::fs::write(
        proj.join("team.wisp.yml"),
        "wisp: 1\nagents:\n  critic: { harness: claude, model: opus }\ngoals:\n  green: { text: the tests pass }\n",
    )
    .unwrap();
    let (said, err) = a
        .call(
            "wisp_launch",
            json!({ "prompt": "check the tests", "formula": "team.wisp.yml", "agent": "critic", "goal": "green", "worktree": false }),
        )
        .await;
    assert!(!err, "{said}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    // the one task besides the asking one
    let id = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str())
        .find(|i| *i != a.id)
        .unwrap_or_else(|| panic!("{said} {list}"))
        .to_owned();
    let t = a.api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!((t["ext"]["wisp.runtime"]["spec"]["agent"].as_str(), t["model"].as_str()), (Some("critic"), Some("opus")), "{t}");
    assert_eq!(t["ext"]["wisp.runtime"]["spec"]["goal"]["text"], "the tests pass", "{t}");
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "x", "formula": ["team.wisp.yml"], "agent": "critc" })).await;
    assert!(err, "{said}");
    assert!(said.contains("no agent named critc: did you mean critic? agents has critic (or "), "{said}");
    a.serve.stop();
}

/// Review r194 #1: an agent's `wisp_launch` formula is read with the agent's own reach: only files inside its task's
/// folder (no absolute path, no `..`, no link out, never a secrets file), and never `file()` or `${env:…}`. Each is
/// refused with a plain reason, and no task is made.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_formula_reads_only_inside_its_own_folder() {
    let a = Agent::start("mcp-formula-reach").await;
    let proj = a.dir.join("proj");
    let outside = a.dir.join("outside.wisp.yml");
    std::fs::write(&outside, "wisp: 1\nagents:\n  critic: { harness: claude }\n").unwrap();
    std::fs::write(a.dir.join("secret.txt"), "TOPSECRET").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, proj.join("link.wisp.yml")).unwrap();
    std::fs::write(proj.join("secrets.json"), "{\"wisp\": 1}").unwrap();
    let launch = |formula: Value| json!({ "prompt": "x", "formula": formula, "worktree": false });
    let refused = |said: &str, err: bool, why: &str| {
        assert!(err, "{said}");
        assert!(said.contains(why), "{why} in: {said}");
        assert!(!said.contains("TOPSECRET"), "{said}");
    };
    let (s, e) = a.call("wisp_launch", launch(json!(outside.display().to_string()))).await;
    refused(&s, e, "is read from inside its task's folder (no absolute paths, no ..)");
    let (s, e) = a.call("wisp_launch", launch(json!("../outside.wisp.yml"))).await;
    refused(&s, e, "is read from inside its task's folder (no absolute paths, no ..)");
    // audit I14: one error, not a second about the task's names checked against the formula that wasn't read
    let (s, e) =
        a.call("wisp_launch", json!({ "prompt": "x", "formula": "../outside.wisp.yml", "agent": "critic", "worktree": false })).await;
    refused(&s, e, "is read from inside its task's folder (no absolute paths, no ..)");
    assert!(!s.contains("no agent named") && !s.contains("; "), "a second error: {s}");
    #[cfg(unix)]
    {
        let (s, e) = a.call("wisp_launch", launch(json!("link.wisp.yml"))).await;
        refused(&s, e, "leads out of the task's folder");
    }
    let (s, e) = a.call("wisp_launch", launch(json!("secrets.json"))).await;
    refused(&s, e, "a secrets file isn't read for an agent's formula");
    // `\` escapes in an expression's string: a Windows path's backslashes doubled, so it names this very file
    let file = format!("${{file(\"{}\")}}", a.dir.join("secret.txt").display().to_string().replace('\\', "\\\\"));
    let (s, e) = a.call("wisp_launch", launch(json!({ "agents": { "x": { "harness": "claude", "instructions": file } } }))).await;
    refused(&s, e, "an agent's formula can't read files");
    let (s, e) = a.call("wisp_launch", launch(json!({ "agents": { "x": { "harness": "claude", "instructions": "${env:HOME}" } } }))).await;
    refused(&s, e, "an agent's formula can't read the environment");
    let list = a.api.cmd("runtime.list", json!({})).await;
    assert_eq!(list.as_array().unwrap().len(), 1, "no task was made: {list}");
    a.serve.stop();
}

/// Review r194 #8: a child never gets more than its parent may. A plan-mode task's `wisp_launch` under a formula whose
/// agent says `mode: auto` makes a plan task; asking for Full auto outright is refused.
#[tokio::test(flavor = "multi_thread")]
async fn a_child_never_gets_a_higher_mode_than_its_parent() {
    let a = Agent::start_spec("mcp-formula-mode", |_| Vec::new(), json!({ "mode": "plan" })).await;
    std::fs::write(a.dir.join("proj/auto.wisp.yml"), "wisp: 1\nagents:\n  bold: { harness: claude, mode: auto }\n").unwrap();
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "go", "formula": "auto.wisp.yml", "worktree": false })).await;
    assert!(!err, "{said}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    let id = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str())
        .find(|i| *i != a.id)
        .unwrap_or_else(|| panic!("{list}"))
        .to_owned();
    let t = a.api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!(t["mode"], "plan", "the formula's auto, held to its parent's plan: {t}");
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "go", "mode": "auto", "worktree": false })).await;
    assert!(err && said.contains("above this task's"), "{said}");
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "go", "worktree": false })).await;
    assert!(!err, "{said}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    for t in list.as_array().unwrap().iter().filter(|t| t["id"] != a.id.as_str()) {
        let full = a.api.cmd("runtime.get", json!({ "id": t["id"] })).await;
        assert_eq!(full["mode"], "plan", "every child of a plan task is plan: {full}");
    }
    a.serve.stop();
}

/// Review r194 round 2 #1, #3, #5: an agent's formula can't declare a connector (a command would run on the host), use a
/// secret, carry a task's raw fields, or read through a link out of its folder; it names only connectors the user set
/// up. Each refused with a plain reason, and no task is made.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_formula_runs_nothing_reads_no_secret_and_carries_no_task() {
    let a = Agent::start("mcp-formula-rules").await;
    let proj = a.dir.join("proj");
    let launch = |formula: Value| json!({ "prompt": "x", "formula": formula, "worktree": false });
    let refused = |said: &str, err: bool, why: &str| {
        assert!(err, "{said}");
        assert!(said.contains(why), "{why} in: {said}");
    };
    // before the fix this command ran on the host for the child
    let marker = a.dir.join("pwned");
    let cmd = json!({ "connectors": { "evil": { "command": "touch", "args": [marker.display().to_string()] } } });
    let (s, e) = a.call("wisp_launch", launch(cmd)).await;
    refused(&s, e, "an agent's formula can't declare connectors");
    let sec = json!({ "agents": { "x": { "harness": "claude", "instructions": "${secret:GITHUB_TOKEN}" } } });
    let (s, e) = a.call("wisp_launch", launch(sec)).await;
    refused(&s, e, "an agent's formula can't use secrets");
    let (s, e) = a
        .call(
            "wisp_launch",
            json!({ "prompt": "x", "formula": { "agents": { "x": { "harness": "claude", "connectors": ["nosuch"] } } }, "agent": "x" }),
        )
        .await;
    refused(&s, e, "no connector named nosuch");
    let task = json!({ "task": { "prompt": "x", "crew": "ada", "confirmLimit": true, "account": "work" } });
    let (s, e) = a.call("wisp_launch", launch(task)).await;
    refused(&s, e, "an agent's formula can't carry a task");
    // a module folder with a link out
    std::fs::create_dir_all(proj.join("team")).unwrap();
    std::fs::write(a.dir.join("away.wisp.yml"), "agents:\n  far: { harness: claude }\n").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(a.dir.join("away.wisp.yml"), proj.join("team/far.wisp.yml")).unwrap();
        std::fs::write(proj.join("mods.wisp.yml"), "wisp: 1\nmodules:\n  team: { source: ./team }\n").unwrap();
        let (s, e) = a.call("wisp_launch", launch(json!("mods.wisp.yml"))).await;
        refused(&s, e, "leads out of the task's folder");
    }
    let (s, e) = a.call("wisp_launch", launch(json!({ "harnesses": { "claude": { "from": "path", "path": "/bin/sh" } } }))).await;
    refused(&s, e, "an agent's formula can't declare harnesses");
    let (s, e) = a.call("wisp_launch", launch(json!({ "projects": { "p": { "path": "/", "setup": ["touch /tmp/x"] } } }))).await;
    refused(&s, e, "an agent's formula can't declare projects");
    let (s, e) = a.call("wisp_launch", launch(json!({ "skills": { "s": { "path": "/etc/hosts" } } }))).await;
    refused(&s, e, "an agent's formula gives a skill its text, not a path");
    assert!(!marker.exists(), "no connector command ran");
    let list = a.api.cmd("runtime.list", json!({})).await;
    assert_eq!(list.as_array().unwrap().len(), 1, "no task was made: {list}");
    a.serve.stop();
}

/// The agent's own `runtime.create`, as its token sends it straight to `/api/cmd`.
async fn raw_create(a: &Agent, args: Value) -> Value {
    let r = a
        .api
        .http
        .post(format!("http://127.0.0.1:{}/api/cmd", a.port))
        .header("content-type", "application/json")
        .header("idempotency-key", format!("raw-{}", rand_key()))
        .header("x-wisp-token", &a.token)
        .header("x-wisp-session", &a.id)
        .body(json!({ "cmd": "runtime.create", "args": args }).to_string())
        .send()
        .await
        .unwrap();
    serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null)
}

fn rand_key() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

/// Review r194 round 2 #2: a task an agent starts never gets more than the agent's own task may, whichever way it's
/// started: its token straight to `runtime.create`, field by field.
#[tokio::test(flavor = "multi_thread")]
async fn a_child_started_by_an_agents_token_gets_nothing_its_parent_lacks() {
    let a = Agent::start_spec("raw-create-parent", |_| Vec::new(), json!({ "mode": "plan" })).await;
    let cwd = a.dir.join("proj").display().to_string();
    let base = |more: Value| {
        let mut b = json!({ "prompt": "x", "cwd": cwd, "worktree": false });
        if let (Some(o), Some(m)) = (b.as_object_mut(), more.as_object()) {
            o.extend(m.clone());
        }
        b
    };
    for (more, why) in [
        (json!({ "mode": "auto" }), "above this task's"),
        (json!({ "connectors": ["linear"] }), "a connector its own task doesn't have"),
        (json!({ "crew": "ada" }), "a crew member its own task doesn't run for"),
        (json!({ "account": "work" }), "an account its own task doesn't use"),
        (json!({ "team": "core" }), "a team its own task doesn't have"),
        (json!({ "confirmLimit": true }), "can't go past Ask near the limit"),
        (json!({ "profile": { "harness": "claude", "mode": "auto" } }), "can't give a task it starts a profile"),
        (json!({ "ext": { "wisp.budget": { "usd": 1000 } } }), "can't set a task's ext"),
        (json!({ "formula": { "connectors": [{ "name": "evil", "command": "touch" }] } }), "an agent's formula can't declare connectors"),
        (json!({ "worktreeKey": "someone-else" }), "can't pick another task's worktree"),
        // refused by the kernel's agent scope already, in its own words
        (json!({ "parentId": "01aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }), "speaks for its own task only"),
    ] {
        let r = raw_create(&a, base(more.clone())).await;
        assert!(r["ok"] == false && r.to_string().contains(why), "{more} → {r}");
    }
    // nothing asked: held to the parent's plan, also when a formula's agent says auto
    let r = raw_create(
        &a,
        base(json!({ "agent": "bold", "formula": { "agents": [{ "name": "bold", "harness": "claude", "mode": "auto" }] } })),
    )
    .await;
    let id = r["result"]["id"].as_str().or_else(|| r["id"].as_str()).unwrap_or_else(|| panic!("{r}")).to_owned();
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": id })).await["mode"], "plan");
    // a follow-up the agent sends (its token reaches its own task only) can't lift its own task's mode, or skip Ask near
    // the limit
    for (more, why) in
        [(json!({ "mode": "auto" }), "above this task's"), (json!({ "confirmLimit": true }), "can't go past Ask near the limit")]
    {
        let mut args = json!({ "id": a.id, "text": "more", "queue": true });
        if let (Some(o), Some(m)) = (args.as_object_mut(), more.as_object()) {
            o.extend(m.clone());
        }
        let r = raw_cmd(&a, "runtime.followUp", args).await;
        assert!(r["ok"] == false && r.to_string().contains(why), "{more} → {r}");
    }
    a.serve.stop();
}

async fn raw_cmd(a: &Agent, cmd: &str, args: Value) -> Value {
    let r = a
        .api
        .http
        .post(format!("http://127.0.0.1:{}/api/cmd", a.port))
        .header("content-type", "application/json")
        .header("idempotency-key", format!("raw-{}", rand_key()))
        .header("x-wisp-token", &a.token)
        .header("x-wisp-session", &a.id)
        .body(json!({ "cmd": cmd, "args": args }).to_string())
        .send()
        .await
        .unwrap();
    serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null)
}

/// Review r194 round 2 #4: a parent's channel cap travels to the task it starts.
#[tokio::test(flavor = "multi_thread")]
async fn a_capped_parents_cap_travels_to_its_child() {
    let a = Agent::start_spec(
        "cap-travels",
        |_| Vec::new(),
        json!({ "mode": "edits", "ext": { "wisp.runtime": { "capped": { "mode": "edits" } } } }),
    )
    .await;
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "go", "worktree": false })).await;
    assert!(!err, "{said}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    let id = list
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str())
        .find(|i| *i != a.id)
        .unwrap_or_else(|| panic!("{list}"))
        .to_owned();
    let t = a.api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!(t["ext"]["wisp.runtime"]["capped"], json!({ "mode": "edits" }), "{t}");
    a.serve.stop();
}

/// The write commands an agent's own capability set (`agent:session`) may run, as the kernel decides it.
async fn agent_writes(a: &Agent) -> Vec<String> {
    let list = a.api.cmd("cli.catalogue", json!({})).await;
    let agent = wisp_kernel::Caller::agent("s1", false);
    let mut w: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["readOnly"] != true && wisp_kernel::caps::allowed(&agent, c["permission"].as_str().unwrap_or("")))
        .filter_map(|c| c["id"].as_str().map(str::to_owned))
        .collect();
    w.sort();
    w
}

/// Review r194 round 3 #1: an agent never raises a task's mode (its own included); only the user does. A plan task's
/// agent setting itself to Full auto is refused, so its children stay held too.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_never_raises_a_tasks_mode() {
    let a = Agent::start_spec("raise-own-mode", |_| Vec::new(), json!({ "mode": "plan" })).await;
    let r = raw_cmd(&a, "runtime.setMode", json!({ "id": a.id, "mode": "auto" })).await;
    assert!(r["ok"] == false && r.to_string().contains("an agent can't raise a task's mode"), "{r}");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["mode"], "plan");
    let cwd = a.dir.join("proj").display().to_string();
    let r = raw_create(&a, json!({ "prompt": "x", "cwd": cwd, "mode": "auto", "worktree": false })).await;
    assert!(r["ok"] == false && r.to_string().contains("above this task's"), "{r}");
    // the user still raises it
    a.api.cmd("runtime.setMode", json!({ "id": a.id, "mode": "auto" })).await;
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["mode"], "auto");
    a.serve.stop();
}

/// Review r194 round 3 #2: an agent never adds, changes, tests or enables a connector or MCP server (a command it wrote
/// would run on the host for every task): from its token or its MCP tool, refused, nothing stored.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_never_adds_or_changes_a_connector() {
    let a = Agent::start("agent-connector-add").await;
    let evil = json!({ "name": "evil", "command": "sh", "args": ["-c", "id"] });
    for (cmd, args) in [
        ("connectors.add", evil.clone()),
        ("connectors.update", json!({ "name": "evil", "command": "sh" })),
        ("connectors.test", json!({ "name": "evil" })),
        ("connectors.import", json!({ "text": "{}" })),
        ("connectors.agent.add", evil.clone()),
    ] {
        let r = raw_cmd(&a, cmd, args).await;
        assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{cmd} → {r}");
    }
    // its MCP tool only asks the user on a card (ADR-0113): nothing is stored while it waits, nor once denied
    let call = spawn_call(&a, "wisp_connector_add", &evil);
    setup_card(&a).await;
    assert!(!a.api.cmd("resources.export", json!({ "kinds": ["connector"] })).await.to_string().contains("evil"));
    answer(&a, "Deny").await;
    assert!(call.await.unwrap().0.contains("nothing was changed"));
    let list = a.api.cmd("resources.export", json!({ "kinds": ["connector"] })).await;
    assert!(!list.to_string().contains("evil"), "{list}");
    a.serve.stop();
}

/// Review r194 round 3 #3: an agent writes its own work, never the user's setup. Every write its capability set allows,
/// exactly; the user's profiles, crew, projects, connectors, machines, sign-ins, publishing and holds aren't among them.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)] // the exact list is the point
async fn an_agents_capability_set_writes_only_its_own_work() {
    let a = Agent::start("agent-writes").await;
    let w = agent_writes(&a).await;
    for setup in [
        "runtime.profile.set",
        "runtime.profile.remove",
        "runtime.projectAdd",
        "runtime.setExt",
        "runtime.turn.setExt",
        "runtime.approveBlocked",
        "runtime.proposal.apply",
        "runtime.proposal.discard",
        "runtime.overage",
        "runtime.signIn",
        "runtime.import",
        "runtime.adoptSession",
        "runtime.trial",
        "crew.create",
        "crew.update",
        "crew.remove",
        "crew.import",
        "crew.share",
        "connectors.add",
        "connectors.update",
        "connectors.remove",
        "connectors.test",
        "machines.add",
        "machines.remove",
        "machines.letIn",
        "sandbox.env",
        "stuff.publish",
        "stuff.allow",
        "later.zone",
        // audit I1 (ADR-0115): a shell command on the user's computer, and deleting a task with its work
        "runtime.exec",
        "runtime.delete",
        // the review of that fix: a test command a factory runs, and a raw call to another computer
        "runtime.projectTest",
        "machines.call",
    ] {
        assert!(!w.contains(&setup.to_owned()), "an agent may still {setup}: {w:?}");
    }
    // every write an agent may run, exactly: a new one is a decision (ADR-0111's agent capability set)
    let want: Vec<String> = [
        "connectors.call",
        "crew.agent.kit",
        "crew.ask",
        "crew.assign",
        "crew.checkin",
        "crew.export",
        "crew.reply",
        "crew.wake",
        "crew.wakeMentions",
        "git.checkout",
        "git.clone",
        "git.commit",
        "git.restoreTurn",
        "git.revert",
        "git.stage",
        "goal.check",
        "goal.drop",
        "goal.pause",
        "goal.resume",
        "goal.set",
        "history.resumeRequest",
        "later.add",
        "later.agent.schedule",
        "later.cancel",
        "later.cancelPlan",
        "later.done",
        "later.edit",
        "later.editPlan",
        "later.park",
        "later.plan",
        "later.tick",
        "machines.bring",
        "machines.carry",
        "machines.dropCopies",
        "machines.route",
        "machines.send",
        "machines.uncarry",
        "mcp.rpc",
        "runtime.agent.tag",
        "runtime.agent.task",
        "runtime.agent.watch",
        "runtime.answer",
        "runtime.appState",
        "runtime.approval",
        "runtime.archive",
        "runtime.ask",
        "runtime.btw",
        "runtime.btw.drop",
        "runtime.card",
        "runtime.compact",
        "runtime.create",
        "runtime.delegate",
        "runtime.followUp",
        "runtime.forLater",
        "runtime.fork",
        "runtime.held",
        "runtime.markRead",
        "runtime.note",
        "runtime.pin",
        "runtime.probe",
        "runtime.projectRun",
        "runtime.queue.cancel",
        "runtime.queue.edit",
        "runtime.queue.merge",
        "runtime.queue.order",
        "runtime.queue.sendNow",
        "runtime.queue.text",
        "runtime.quick",
        "runtime.quickCancel",
        "runtime.release",
        "runtime.rename",
        "runtime.rerun",
        "runtime.restoreSession",
        "runtime.reveal",
        "runtime.setAgent",
        "runtime.setLean",
        "runtime.setMode",
        "runtime.setState",
        "runtime.settle",
        "runtime.setup.ask", // asks the user on a card; the write runs on their Allow (ADR-0113)
        "runtime.stop",
        "runtime.terminal",
        "runtime.unask",
        "sandbox.bring",
        "sandbox.dismiss",
        "sandbox.drop",
        "sandbox.launch",
        "sandbox.own",
        "sandbox.place",
        "sandbox.retry",
        "sandbox.seed",
        "sandbox.start",
        "ship.agent.pr",
        "stuff.addStarter",
        "stuff.agent.save",
        "stuff.launch",
        "stuff.receive",
        "stuff.remove",
        "stuff.removeUpload",
        "stuff.resetData",
        "stuff.restore",
        "stuff.reveal",
        "stuff.save",
        "stuff.stop",
        "stuff.update",
        "threads.fresh",
        "threads.notesUpdate",
        // its own task's workflow runs: steps under its task, in its folder, at its mode or lower (ADR-0119)
        "workflows.edit",
        "workflows.run",
        "workflows.stop",
    ]
    .iter()
    .map(|x| (*x).to_owned())
    .collect();
    assert_eq!(w, want, "the agent's writes changed: decide each one in an ADR (ADR-0115 holds the list)");
    let r = raw_cmd(&a, "runtime.profile.set", json!({ "name": "x", "harness": "codex", "mode": "auto" })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    let r = raw_cmd(&a, "crew.create", json!({ "name": "Zed", "mode": "auto" })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    a.serve.stop();
}

/// Review r194 round 3 #4: an agent's card carries no other plugin's ext.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_card_carries_no_ext() {
    let a = Agent::start("agent-card").await;
    let r = raw_cmd(&a, "runtime.card", json!({ "title": "t", "prompt": "p", "blocks": [], "ext": { "wisp.worktrees": { "path": "/" } } }))
        .await;
    assert!(r["ok"] == false && r.to_string().contains("can't set a task's ext"), "{r}");
    let r = raw_cmd(&a, "runtime.card", json!({ "title": "t", "prompt": "p", "blocks": [] })).await;
    assert_eq!(r["ok"], true, "{r}");
    a.serve.stop();
}

/// An agent's tool call that waits on the user (a setup card), run beside the test: (the tool's text, whether an error).
fn spawn_call(a: &Agent, tool: &str, args: &Value) -> tokio::task::JoinHandle<(String, bool)> {
    let (http, port, token, id) = (a.api.http.clone(), a.port, a.token.clone(), a.id.clone());
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } });
    tokio::spawn(async move {
        let r = http
            .post(format!("http://127.0.0.1:{port}/api/mcp"))
            .header("content-type", "application/json")
            .header("x-wisp-token", token)
            .header("x-wisp-session", id)
            .body(body.to_string())
            .send()
            .await
            .unwrap();
        Agent::text(&serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null))
    })
}

/// The setup card open on the agent's task (`ext["wisp.runtime"].ask.setup`), once it's there.
async fn setup_card(a: &Agent) -> Value {
    let at = std::time::Instant::now();
    loop {
        let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
        let ask = &t["ext"]["wisp.runtime"]["ask"];
        if ask["setup"].is_object() {
            return ask.clone();
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "no setup card: {t}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// The user answers the open card, naming it (as the card's buttons do).
async fn answer(a: &Agent, text: &str) -> Value {
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    let ask = t["ext"]["wisp.runtime"]["ask"]["id"].as_str().unwrap_or_default().to_owned();
    a.api.cmd("runtime.answer", json!({ "id": a.id, "text": text, "askId": ask })).await
}

async fn crew_names(a: &Agent) -> Vec<String> {
    let list = a.api.cmd("crew.list", json!({})).await;
    list.as_array().unwrap().iter().filter_map(|m| m["name"].as_str().map(str::to_owned)).collect()
}

/// ADR-0113 (owner-list row 66): an agent's change to the user's setup (crew, connectors, projects) is a card the user
/// answers, even in Full auto. The card says what in the server's words with each command verbatim; nothing is written
/// before Allow; the agent can't answer it itself; Allow writes it as the user and the agent reads what was done; Deny
/// writes nothing and tells it so. From its MCP tools and from its raw token.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)] // one task's agent through every kind of setup card
async fn an_agents_setup_change_is_a_card_the_user_allows_or_denies_never_the_agent() {
    let a = Agent::start_spec("setup-cards", |_| Vec::new(), json!({ "mode": "auto" })).await;
    assert!(a.names(true).await.iter().any(|n| n == "runtime_projectAdd"), "an agent can ask to add a project");

    // a crew member: the card first, nothing written; the agent's own Allow is refused; the user's writes it
    let call = spawn_call(&a, "wisp_crew_add", &json!({ "name": "Bob", "agent": "codex", "mode": "plan", "role": "Reviewer" }));
    let card = setup_card(&a).await;
    assert_eq!(card["setup"]["headline"], "Claude Code wants to add crew member Bob (codex, plan)", "{card}");
    assert_eq!(card["options"], json!(["Allow", "Deny"]), "no Always on a setup card (v1 had none)");
    assert_eq!(crew_names(&a).await, Vec::<String>::new(), "nothing is written before Allow");
    let own = raw_cmd(&a, "runtime.answer", json!({ "id": a.id, "text": "Allow", "askId": card["id"] })).await;
    assert!(own["ok"] == false && own.to_string().contains("the user's to answer"), "the agent can't allow it itself: {own}");
    assert_eq!(crew_names(&a).await, Vec::<String>::new());
    answer(&a, "Allow").await;
    let (said, err) = call.await.unwrap();
    assert!(!err && said.starts_with("The user allowed it. Added Bob to your crew"), "{said}");
    assert_eq!(crew_names(&a).await, vec!["Bob".to_owned()]);

    // a change to a member: Allow writes only the fields given
    let call = spawn_call(&a, "wisp_crew_edit", &json!({ "member": "Bob", "role": "Lead" }));
    let card = setup_card(&a).await;
    assert_eq!(card["setup"]["lines"], json!(["role: Lead"]), "{card}");
    answer(&a, "Allow").await;
    assert!(call.await.unwrap().0.starts_with("The user allowed it. Updated Bob."));
    assert_eq!(a.api.cmd("crew.get", json!({ "id": "Bob" })).await["role"], "Lead");

    // a local connector: its command verbatim; Deny stores nothing and says so plainly
    let evil = json!({ "name": "linear", "command": "npx", "args": ["-y", "@linear/mcp"], "env": ["LINEAR_KEY"] });
    let call = spawn_call(&a, "wisp_connector_add", &evil);
    let card = setup_card(&a).await;
    assert_eq!(card["setup"]["headline"], "Claude Code wants to add the connector “linear”: runs npx -y @linear/mcp", "{card}");
    assert_eq!(card["setup"]["commands"], json!(["npx -y @linear/mcp"]));
    assert!(card["question"].as_str().unwrap().contains("needs LINEAR_KEY from you"), "channels and the phone read it all: {card}");
    answer(&a, "Deny").await;
    let (said, _) = call.await.unwrap();
    assert!(said.starts_with("The user said no: nothing was changed."), "{said}");
    let stored = a.api.cmd("resources.export", json!({ "kinds": ["connector"] })).await;
    assert!(!stored.to_string().contains("linear"), "{stored}");

    // a project, from the generated tool: Allow adds it under the server's name
    let web = a.dir.join("code/web");
    std::fs::create_dir_all(&web).unwrap();
    let call = spawn_call(&a, "runtime_projectAdd", &json!({ "path": web }));
    let card = setup_card(&a).await;
    assert_eq!(card["setup"]["headline"], format!("Claude Code wants to add the project {}", web.display()), "{card}");
    answer(&a, "Allow").await;
    let (said, err) = call.await.unwrap();
    assert!(!err && said.starts_with("The user allowed it. The project web is "), "{said}");

    // from the raw token: the card is up at once, the answer comes through pollAnswer; removing a member
    let r = raw_cmd(
        &a,
        "runtime.setup.ask",
        json!({ "id": a.id, "command": "crew.agent.remove", "args": { "session": a.id, "member": "Bob" } }),
    )
    .await;
    let ask_id = r["result"]["askId"].as_str().unwrap_or_else(|| panic!("{r}")).to_owned();
    assert!(r["result"]["text"].as_str().unwrap().starts_with("Asked the user"), "{r}");
    assert_eq!(crew_names(&a).await, vec!["Bob".to_owned()], "nothing runs before Allow");
    let direct = raw_cmd(&a, "crew.agent.remove", json!({ "session": a.id, "member": "Bob" })).await;
    assert!(direct["ok"] == false && direct.to_string().contains("the user allows it on a card"), "{direct}");
    answer(&a, "Allow").await;
    let p = raw_cmd(&a, "runtime.pollAnswer", json!({ "askId": ask_id })).await;
    assert!(p["result"]["answer"].as_str().unwrap_or_default().starts_with("The user allowed it. Removed Bob"), "{p}");
    assert_eq!(crew_names(&a).await, Vec::<String>::new());

    // never a write off the list, whatever a raw call names
    let r = raw_cmd(&a, "runtime.setup.ask", json!({ "id": a.id, "command": "settings.set", "args": { "key": "editor", "value": "sh" } }))
        .await;
    assert!(r["ok"] == false && r.to_string().contains("isn't a change to the user's setup"), "{r}");
    a.serve.stop();
}

/// A setup card put up from the agent's raw token (`runtime.setup.ask`): (the call's answer, the card once it's up).
async fn raw_card(a: &Agent, command: &str, args: Value) -> (Value, Value) {
    let r = raw_cmd(a, "runtime.setup.ask", json!({ "id": a.id, "command": command, "args": args })).await;
    if r["ok"] != true {
        return (r, Value::Null);
    }
    (r, setup_card(a).await)
}

/// Review r194 row 66 #1: an answer names the card it answers. Without the card's id, or with another's, nothing runs;
/// while the card is open nothing replaces it (a new question is refused, the card stays); then Allow on that card runs
/// the write it showed.
#[tokio::test(flavor = "multi_thread")]
async fn a_setup_answer_names_its_card_and_nothing_replaces_an_open_card_review_1() {
    let a = Agent::start("setup-answer-id").await;
    let (_, card) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "name": "Bob", "agent": "claude" })).await;
    let id = card["id"].as_str().unwrap().to_owned();
    for (args, what) in [
        (json!({ "id": a.id, "text": "Allow" }), "no card named"),
        (json!({ "id": a.id, "text": "Allow", "askId": "an-old-card" }), "another card"),
    ] {
        let r = a.api.try_cmd("runtime.answer", args).await;
        assert_eq!(r["ok"], false, "{what}: {r}");
    }
    assert_eq!(crew_names(&a).await, Vec::<String>::new(), "nothing ran");
    // an agent's new question while the card is open: refused, the card stays as it was
    let q = raw_cmd(&a, "runtime.ask", json!({ "id": a.id, "question": "Add Eve instead?", "options": ["Allow", "Deny"] })).await;
    assert!(q["ok"] == false && q.to_string().contains("waiting on their answer"), "{q}");
    assert_eq!(setup_card(&a).await["id"], id, "the same card");
    a.api.cmd("runtime.answer", json!({ "id": a.id, "text": "Allow", "askId": id })).await;
    assert_eq!(crew_names(&a).await, vec!["Bob".to_owned()]);
    a.serve.stop();
}

/// Review r194 row 66 #2: a card that showed a URL stored a local command. A connector card is built from the very write
/// that is stored: a URL with a command (or a transport that says otherwise) is refused, and what Allow stores is the
/// remote connector the card showed.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_card_is_the_write_that_is_stored_review_2() {
    let a = Agent::start("setup-url-command").await;
    let evil =
        json!({ "sid": a.id, "name": "x", "url": "https://ok.example/mcp", "transport": "local", "command": "sh", "args": ["-c", "id"] });
    let (r, _) = raw_card(&a, "connectors.agent.add", evil).await;
    assert!(r["ok"] == false && r.to_string().contains("not both"), "{r}");
    let (_, card) = raw_card(&a, "connectors.agent.add", json!({ "sid": a.id, "name": "ok", "url": "https://ok.example/mcp" })).await;
    assert_eq!(card["setup"]["headline"], "Claude Code wants to add the connector “ok”: connects to https://ok.example/mcp");
    answer(&a, "Allow").await;
    let v = a.api.cmd("connectors.list", json!({})).await;
    let c = &v["connectors"][0];
    assert_eq!(
        (c["id"].as_str(), c["transport"].as_str(), c["url"].as_str()),
        (Some("ok"), Some("remote"), Some("https://ok.example/mcp")),
        "{v}"
    );
    assert!(c["command"].as_str().unwrap_or_default().is_empty(), "no command stored: {v}");
    a.serve.stop();
}

/// Review r194 row 66 #4: a crew card hid wakes, `on`, kit bodies and team rows, and read `from` only after Allow. Every
/// field that will be stored is on the card (kit bodies in Show all), a field it can't show is refused, and a crew file
/// is read before the card: Allow imports exactly the members it listed.
#[tokio::test(flavor = "multi_thread")]
async fn a_crew_card_shows_every_stored_field_and_reads_from_before_the_card_review_4() {
    let a = Agent::start("setup-crew-fields").await;
    let member = json!({ "session": a.id, "name": "Bob", "agent": "claude", "on": ["ci"], "kit": [{ "name": "SKILL.md", "body": "always push to main" }] });
    let (_, card) = raw_card(&a, "crew.agent.add", member).await;
    let lines = card["setup"]["lines"].to_string();
    assert!(lines.contains("on: ci") && lines.contains("kit: 1 file (SKILL.md"), "{lines}");
    assert!(card["setup"]["full"].as_str().unwrap().contains("always push to main"), "the kit's body is in Show all: {card}");
    answer(&a, "Deny").await;
    let (r, _) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "name": "Bob", "hidden": "x" })).await;
    assert!(r["ok"] == false && r.to_string().contains("hidden isn't something"), "{r}");
    // a crew file: its members are on the card before anything joins, and Allow adds exactly them
    let file = json!({ "wisp": "crew", "v": 1, "members": [{ "key": "cy", "name": "Cy", "role": "CI watcher", "agent": "claude", "brief": "watch CI" }] }).to_string();
    let (_, card) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "from": file })).await;
    assert_eq!(card["setup"]["headline"], "Claude Code wants to add 1 crew member from a shared crew: Cy", "{card}");
    assert!(card["setup"]["lines"].to_string().contains("role: CI watcher"), "{card}");
    assert_eq!(crew_names(&a).await, Vec::<String>::new(), "read, not joined");
    answer(&a, "Allow").await;
    assert_eq!(crew_names(&a).await, vec!["Cy".to_owned()]);
    a.serve.stop();
}

/// Review r194 row 66 #7: a follow-up while a setup card is open cleared the card without running it. It queues behind
/// the card, which stays; once the card is answered the queue goes on.
#[tokio::test(flavor = "multi_thread")]
async fn a_follow_up_waits_behind_an_open_setup_card_review_7() {
    let a = Agent::start("setup-follow-up").await;
    let (_, card) = raw_card(&a, "crew.agent.remove", json!({ "session": a.id, "member": "nobody" })).await;
    a.api.cmd("runtime.followUp", json!({ "id": a.id, "text": "and then fix the tests", "queue": true })).await;
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert_eq!(t["ext"]["wisp.runtime"]["ask"]["id"], card["id"], "the card stays: {t}");
    assert!(t.to_string().contains("and then fix the tests"), "queued behind it: {t}");
    answer(&a, "Deny").await;
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(t["ext"]["wisp.runtime"]["ask"].is_null(), "{t}");
    a.serve.stop();
}

/// Audit G6: the user's own `wisp_launch` over HTTP (`/api/mcp`, no token) was v1's: it dropped `goal`, `formula`,
/// `factory` and `worktree` and ran in place in the user's checkout. It is the same launch as everywhere else now: in a
/// git project the task gets its worktree, and the goal it names is its goal.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_http_wisp_launch_takes_a_goal_and_runs_in_a_worktree_audit_g6() {
    let a = Agent::start("user-launch").await;
    let repo = a.dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    for args in
        [&["init", "-q", "-b", "main"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]]
    {
        assert!(std::process::Command::new("git").args(args).current_dir(&repo).status().unwrap().success());
    }
    let (said, err) = a
        .user_call("wisp_launch", json!({ "prompt": "make the tests pass", "cwd": repo, "agent": "claude", "goal": "npm test passes" }))
        .await;
    assert!(!err, "{said}");
    let id = said.split('`').nth(1).unwrap_or_default().to_owned();
    let t = a.api.cmd("runtime.get", json!({ "id": id })).await;
    assert_eq!(t["ext"]["wisp.runtime"]["spec"]["goal"], "npm test passes", "{t}");
    let cwd = t["cwd"].as_str().unwrap_or_default();
    assert!(!cwd.is_empty() && !std::path::Path::new(cwd).ends_with("repo"), "its own worktree, not the user's checkout: {cwd}");
    assert_ne!(t["ext"]["wisp.runtime"]["spec"]["by"], "agent", "the user's own launch: {t}");
    a.serve.stop();
}

/// Audit G7: no goal or factory tool in either server (wisp.mcp's manifest granted no `goal:*` or `factory:*`, and a
/// tool runs on its grant), so a harness couldn't list the declared goals, set a goal on a task or stop a run. A run's
/// agent gets the goal tools (ADR-0112: its own work) and reads the factories; the user's editor gets them all.
#[tokio::test(flavor = "multi_thread")]
async fn goal_and_factory_tools_reach_both_mcp_servers_audit_g7() {
    let a = Agent::start("goal-tools").await;
    let (agent, user) = (a.names(true).await, a.names(false).await);
    for t in ["goal_set", "goal_declared", "goal_check", "goal_drop", "goal_pause", "goal_resume", "factory_list"] {
        assert!(agent.iter().any(|n| n == t), "{t} for the agent: {agent:?}");
    }
    for t in ["goal_set", "goal_declared", "factory_list", "factory_stop"] {
        assert!(user.iter().any(|n| n == t), "{t} for the user: {user:?}");
    }
    let (said, err) = a.call("goal_set", json!({ "id": a.id, "text": "the tests pass" })).await;
    assert!(!err, "{said}");
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert_eq!(t["ext"]["wisp.goal"]["text"], "the tests pass", "{t}");
    let (said, err) = a.user_call("goal_declared", json!({})).await;
    assert!(!err, "{said}");
    // audit G18: a tool the run isn't given is named, as the user's server names it (not "Method not found: tools/call")
    let (said, err) = a.call("nosuch_tool", json!({})).await;
    assert!(err && said.contains("Unknown tool: nosuch_tool"), "{said}");
    a.serve.stop();
}

/// Audit G21: after a restart the agent that put up a setup card is gone (its turn "Stopped · wisp restarted"), yet the
/// app still offered the card's Allow, which added the project with nobody told, and `wisp show` didn't show it. The
/// card goes with the run that asked, and the turn says it was taken back.
#[tokio::test(flavor = "multi_thread")]
async fn a_setup_card_goes_with_its_agent_on_a_restart_audit_g21() {
    let a = Agent::start("setup-restart").await;
    let (_, card) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "name": "Zed", "agent": "claude" })).await;
    assert!(card["setup"].is_object());
    let Agent { serve, dir, id, .. } = a;
    serve.stop();
    let (serve, port) = restart(&dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let t = api.cmd("runtime.get", json!({ "id": id })).await;
    assert!(t["ext"]["wisp.runtime"]["ask"].is_null(), "no card left to answer: {t}");
    let turns = api.cmd("runtime.turns", json!({ "id": id })).await;
    assert!(turns.to_string().contains("was taken back"), "{turns}");
    drop(serve);
}

/// The user's own CLI against the agent's server.
fn cli(a: &Agent, args: &[&str]) -> (i32, String) {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_wisp"))
        .args(args)
        .env("WISP_HOME", a.dir.join("home"))
        .env("WISP_PORT", a.port.to_string())
        .env("NO_COLOR", "1")
        .current_dir(&a.dir)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// Audit G4: `wisp show` said "Answer: wisp reply <id> …", but `wisp reply <id> Allow` queued "Allow" as a follow-up
/// behind the card, which stayed, and later ran as the agent's prompt. Now `wisp show` names the card's answers, `wisp
/// reply` answers the open card (the user's own CLI is theirs to answer, ADR-0113 rule 5), words are the Deny with them
/// (rule 4), and a card's answer word is never queued as a follow-up, from any client.
#[tokio::test(flavor = "multi_thread")]
async fn a_setup_card_is_answered_with_wisp_reply_and_allow_is_never_queued_audit_g4() {
    let a = Agent::start("setup-reply").await;
    let (_, card) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "name": "Bob", "agent": "claude" })).await;
    let (code, shown) = cli(&a, &["show", &a.id]);
    assert_eq!(code, 0, "{shown}");
    assert!(shown.contains(&format!("wisp reply {} Allow", a.id)) && shown.contains("Deny"), "{shown}");
    // any client: the card's answer word isn't queued behind it as a prompt
    let r = a.api.try_cmd("runtime.followUp", json!({ "id": a.id, "text": "Allow", "queue": true })).await;
    assert!(r.to_string().contains("setup card"), "{r}");
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(t["queue"].as_array().is_none_or(Vec::is_empty), "nothing queued: {t}");
    let (code, said) = cli(&a, &["reply", &a.id, "Allow"]);
    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("queued") && said.contains("You allowed"), "{said}");
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(t["ext"]["wisp.runtime"]["ask"].is_null(), "the card is answered: {t}");
    assert!(crew_names(&a).await.contains(&"Bob".to_owned()), "Allow ran the write the card showed");
    assert!(t["queue"].as_array().is_none_or(Vec::is_empty), "{t}");
    // words are the Deny, with them
    let (_, card2) = raw_card(&a, "crew.agent.add", json!({ "session": a.id, "name": "Eve", "agent": "claude" })).await;
    assert_ne!(card2["id"], card["id"]);
    let (code, said) = cli(&a, &["reply", &a.id, "keep", "Eve", "out"]);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("You denied") && said.contains("keep Eve out"), "{said}");
    assert!(!crew_names(&a).await.contains(&"Eve".to_owned()));
    a.serve.stop();
}

/// The build station's task of the factory run `run`, once the run has made it.
async fn build_station(a: &Agent, run: &str) -> Value {
    let at = std::time::Instant::now();
    loop {
        let t = a.api.cmd("runtime.get", json!({ "id": run })).await;
        let tasks =
            t["ext"]["wisp.factory"]["stations"].as_array().and_then(|s| s.iter().find(|x| x["id"] == "build")).map(|b| b["tasks"].clone());
        if let Some(id) = tasks.as_ref().and_then(|t| t[0].as_str()) {
            return a.api.cmd("runtime.get", json!({ "id": id })).await;
        }
        let why = t["ext"]["wisp.factory"]["why"].as_str().unwrap_or_default().to_owned();
        assert!(why.is_empty(), "the run ended before building: {why}");
        assert!(at.elapsed() < std::time::Duration::from_secs(30), "no build station: {t}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Review r194 factories #1: the driver made each station as the kernel, so a Plan task's agent ran a factory whose build
/// station ran in Full auto (from `wisp_launch` and from its token's raw `runtime.create`, with no mode asked). Every
/// station is held to the agent that started the run, as a task it started itself would be; no mode asked is the
/// parent's, never Full auto.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_factory_run_never_builds_above_its_parent() {
    for (parent, asks) in [("plan", vec![None, Some("plan")]), ("edits", vec![None, Some("edits"), Some("plan")])] {
        let a = Agent::start_spec(&format!("factory-held-{parent}"), |_| Vec::new(), json!({ "mode": parent })).await;
        // the user's factory builds in Full auto
        a.api
            .cmd(
                "resources.apply",
                json!({ "resources": [{ "kind": "factory", "name": "quick", "spec": { "stations": { "build": { "mode": "auto" } } } }] }),
            )
            .await;
        for mode in asks {
            let mut launch = json!({ "prompt": "build it", "factory": "quick", "worktree": false });
            if let Some(m) = mode {
                launch["mode"] = json!(m);
            }
            // from wisp_launch
            let (said, err) = a.call("wisp_launch", launch.clone()).await;
            assert!(!err, "{said}");
            let run = said.split('`').nth(1).unwrap_or_else(|| panic!("{said}")).to_owned();
            let b = build_station(&a, &run).await;
            assert_eq!(b["mode"], mode.unwrap_or(parent), "wisp_launch, {parent} parent, asked {mode:?}: {b}");
            a.api.cmd("factory.stop", json!({ "id": run })).await;
            // from its token's raw runtime.create
            let mut raw = launch.clone();
            raw["cwd"] = json!(a.dir.join("proj"));
            let made = raw_create(&a, raw).await;
            let run = made["result"]["id"].as_str().unwrap_or_else(|| panic!("{made}")).to_owned();
            let b = build_station(&a, &run).await;
            assert_eq!(b["mode"], mode.unwrap_or(parent), "raw create, {parent} parent, asked {mode:?}: {b}");
            a.api.cmd("factory.stop", json!({ "id": run })).await;
        }
        a.serve.stop();
    }
}

/// Review r194 final pass #1: the agent's `heldTo` holds only the work run for it (a factory's stations), never the
/// user's own calls on a task an agent made: a Side discussion, a follow-up, a fork and a rerun at Full auto stay the
/// user's, while a factory station under the same agent is still held.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_own_calls_on_an_agents_task_keep_the_users_mode() {
    let a = Agent::start_spec("held-user-own", |_| Vec::new(), json!({ "mode": "plan" })).await;
    let (said, err) = a.call("wisp_launch", json!({ "prompt": "look around", "worktree": false })).await;
    assert!(!err, "{said}");
    let made = said.split('`').nth(1).unwrap_or_else(|| panic!("{said}")).to_owned();
    let t = a.api.cmd("runtime.get", json!({ "id": made })).await;
    assert!(t["ext"]["wisp.runtime"]["heldTo"].is_object(), "an agent's task keeps its limits: {t}");
    // the user's Side discussion on it, at Full auto
    let side = a.api.try_cmd("runtime.create", json!({ "prompt": "talk it over", "parentId": made, "mode": "auto" })).await;
    assert_eq!(side["ok"], true, "the user's side discussion: {side}");
    let s = a.api.cmd("runtime.get", json!({ "id": side["result"]["id"] })).await;
    assert_eq!(s["mode"], "auto", "the user's own mode: {s}");
    // a rerun of it at Full auto
    let rerun = a
        .api
        .try_cmd("runtime.create", json!({ "prompt": "look around", "cwd": a.dir.join("proj"), "mode": "auto", "worktree": false, "parentId": made, "lineage": { "rerunOf": made } }))
        .await;
    assert_eq!(rerun["ok"], true, "the user's rerun: {rerun}");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": rerun["result"]["id"] })).await["mode"], "auto");
    // a follow-up at Full auto and a fork
    let f = a.api.try_cmd("runtime.followUp", json!({ "id": made, "text": "go on", "mode": "auto", "queue": true })).await;
    assert_eq!(f["ok"], true, "the user's follow-up: {f}");
    let fork = a.api.try_cmd("runtime.fork", json!({ "id": made })).await;
    assert_eq!(fork["ok"], true, "the user's fork: {fork}");
    a.serve.stop();
}

/// Audit H HS6: the trigger switch (and every automations command), the channels', worktrees' and settings' commands had
/// no MCP tool: the MCP plugin's permissions didn't name them. The user's harness has them now (the screen-only ones say
/// why, `uiOnly`); a task's agent gets what its capability set allows (ADR-0112: never a trigger, a setting, a channel
/// or a worktree write). HS7: with Auto off no list offers `auto` as a harness. HS11: `delegate` to a harness that isn't
/// here says so, by name.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_reaches_triggers_settings_channels_and_worktrees_and_an_agent_only_what_it_may_audit_h() {
    let a = Agent::start("mcp-audit-h").await;
    let user = a.names(false).await;
    for want in [
        "automations_trigger_switch",
        "automations_trigger_remove",
        "automations_trigger_ls",
        "settings_get",
        "settings_set",
        "channels_status",
        "channels_setup",
        "worktrees_status",
        "worktrees_bringIn",
    ] {
        assert!(user.iter().any(|n| n == want), "the user's harness lacks {want}: {user:?}");
    }
    for screen_only in ["channels_inbound", "channels_act", "worktrees_save", "worktrees_grep"] {
        assert!(!user.iter().any(|n| n == screen_only), "{screen_only} is the screen's");
    }
    let agent = a.names(true).await;
    for never in ["automations_trigger_switch", "automations_trigger_remove", "settings_set", "channels_setup", "worktrees_remove"] {
        assert!(!agent.iter().any(|n| n == never), "an agent got {never}: {agent:?}");
    }
    let (t, err) = a.user_call("automations_trigger_switch", json!({ "name": "nosuch", "on": false })).await;
    assert!(err && !t.contains("unknown command") && !t.contains("not allowed"), "it reaches the trigger switch: {t}");
    for agent_side in [true, false] {
        let list = a.post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }), agent_side).await.to_string();
        assert!(!list.contains(r#""auto","claude""#) && !list.contains("Harness: auto"), "auto offered (agent {agent_side})");
        // audit I IS8: nor the catalogue's own words (`runtime_create`, `runtime_profile_set`)
        assert!(!list.contains("router pick"), "auto offered by a catalogue tool (agent {agent_side})");
    }
    let (t, _) = a.call("delegate", json!({ "tasks": [{ "agent": "frob", "prompt": "x" }] })).await;
    assert!(t.starts_with("Nothing was delegated.") && t.contains("- frob: no harness frob: name one of claude"), "{t}");
    a.serve.stop();
}

/// The commands an agent may write, as the newest ADR that lists them says (its fenced `agent-writes` block: ADR-0115,
/// then each ADR that changes the list, ADR-0119 on): the decision, read from the ADR.
fn adr_agent_writes() -> Vec<String> {
    let mut adrs: Vec<std::path::PathBuf> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/adr"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains("```agent-writes\n")))
        .collect();
    adrs.sort();
    let text = std::fs::read_to_string(adrs.last().expect("an ADR with an agent-writes block")).unwrap();
    let block = text.split("```agent-writes\n").nth(1).expect("an ```agent-writes block").split("```").next().unwrap();
    let mut w: Vec<String> = block.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
    w.sort();
    w
}

/// Audit I1: `runtime.exec` slipped into an agent's writes (ADR-0112 named it), so a Plan or read-only Codex task ran
/// any shell command on the user's computer through its MCP. Every write an agent's capability set reaches, and every
/// write behind a tool its MCP lists, is one ADR-0115's list names, so a newly exposed command can't slip in unseen.
#[tokio::test(flavor = "multi_thread")]
async fn every_write_an_agent_reaches_is_one_adr_0115_lists() {
    let a = Agent::start("agent-writes-adr").await;
    let listed = adr_agent_writes();
    assert!(!listed.contains(&"runtime.exec".to_owned()) && !listed.contains(&"runtime.delete".to_owned()), "{listed:?}");
    assert_eq!(agent_writes(&a).await, listed, "the agent's capability set reaches a write ADR-0115 doesn't list (or the other way)");
    // the run's own MCP: each tool that runs a catalogue write runs one the list names (the setup ones ask on a card)
    let cat = a.api.cmd("cli.catalogue", json!({})).await;
    let writes: Vec<String> =
        cat.as_array().unwrap().iter().filter(|c| c["readOnly"] != true).filter_map(|c| c["id"].as_str().map(str::to_owned)).collect();
    let tools = a.post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }), true).await;
    for t in tools["result"]["tools"].as_array().unwrap() {
        let desc = t["description"].as_str().unwrap_or_default();
        let Some(cmd) = desc.rsplit_once("(wisp command ").map(|(_, c)| c.trim_end_matches(')').to_owned()) else { continue };
        if writes.contains(&cmd) && cmd != "runtime.projectAdd" {
            assert!(listed.contains(&cmd), "{} runs {cmd}, a write ADR-0115 doesn't give an agent", t["name"]);
        }
    }
    a.serve.stop();
}

/// Audit I1: an agent's MCP neither lists nor runs `runtime_exec` (a Plan task wrote PWNED.txt in the user's checkout
/// with it), and an agent can't delete a task, its own included (its worktree was left behind).
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_can_neither_run_a_command_through_runtime_exec_nor_delete_a_task() {
    let a = Agent::start_spec("agent-no-exec", |_| Vec::new(), json!({ "mode": "plan" })).await;
    let names = a.names(true).await;
    assert!(!names.contains(&"runtime_exec".to_owned()) && !names.contains(&"runtime_delete".to_owned()), "listed: {names:?}");
    assert!(a.names(false).await.contains(&"runtime_exec".to_owned()), "the user's harness keeps it");
    let proj = a.dir.join("proj");
    let (t, err) = a.call("runtime_exec", json!({ "id": a.id, "command": "echo pwned > PWNED.txt" })).await;
    assert!(err, "runtime_exec ran for an agent: {t}");
    let r = raw_cmd(&a, "runtime.exec", json!({ "id": a.id, "command": "echo pwned > PWNED.txt" })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    assert!(r.to_string().contains("runs when the user clicks it"), "{r}");
    assert!(!proj.join("PWNED.txt").exists(), "the command ran");
    let r = raw_cmd(&a, "runtime.delete", json!({ "id": a.id })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    let (t, err) = a.call("runtime_delete", json!({ "id": a.id })).await;
    assert!(err, "runtime_delete ran for an agent: {t}");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": a.id })).await["id"], a.id.as_str(), "the task is still there");
    // nor another way (the review of this fix): a command sent to another computer runs there as this computer's link,
    // so it's held to what the agent may run here; a raw call to another computer, and a test command a factory runs
    let r = raw_cmd(
        &a,
        "machines.route",
        json!({ "id": "studio", "cmd": "runtime.exec", "args": { "id": a.id, "command": "id" }, "mid": "m1" }),
    )
    .await;
    assert!(r["ok"] == false && r.to_string().contains("isn't something this task may run"), "{r}");
    let r = raw_cmd(&a, "machines.call", json!({ "id": "studio", "method": "POST", "path": "/api/cmd" })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    let r = raw_cmd(&a, "runtime.projectTest", json!({ "name": "proj", "testCommand": "id > PWNED.txt" })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "{r}");
    // the user's click still runs it
    let r = a.api.cmd("runtime.exec", json!({ "id": a.id, "command": "echo ok" })).await;
    assert_eq!(r["code"], 0, "{r}");
    a.serve.stop();
}

/// A user's harness tool call that waits (a guard card): `(text, is_error)` once the call returns.
fn spawn_user_call(a: &Agent, tool: &str, args: &Value) -> tokio::task::JoinHandle<(String, bool)> {
    let (http, port, cwd) = (a.api.http.clone(), a.port, a.dir.join("proj").to_string_lossy().into_owned());
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } });
    tokio::spawn(async move {
        let r = http
            .post(format!("http://127.0.0.1:{port}/api/mcp"))
            .header("content-type", "application/json")
            .header("x-wisp-cwd", cwd)
            .body(body.to_string())
            .send()
            .await
            .unwrap();
        Agent::text(&serde_json::from_slice(&r.bytes().await.unwrap()).unwrap_or(Value::Null))
    })
}

/// The task asking the user about a guard, once it's there: `(task id, ask id)`.
async fn guard_ask(a: &Agent) -> (String, String) {
    let at = std::time::Instant::now();
    loop {
        let list = a.api.cmd("runtime.list", json!({})).await;
        for t in
            list.as_array().into_iter().flatten().filter(|t| t["title"].as_str().is_some_and(|x| x.starts_with("Your harness wants to")))
        {
            let id = t["id"].as_str().unwrap().to_owned();
            let got = a.api.cmd("runtime.get", json!({ "id": id })).await;
            if let Some(ask) = got["ext"]["wisp.runtime"]["ask"]["id"].as_str() {
                return (id, ask.to_owned());
            }
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "no task asks: {list}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// ADR-0115 (the audit I Note): the user's harness switching off a safety guard (`worktrees.projectAsk` off) waits on the
/// user's Allow in wisp, on a task that asks; the harness can't answer it itself; Deny changes nothing; raising a guard,
/// or any other setting, applies at once.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_lowers_a_safety_guard_only_on_the_users_allow() {
    let a = Agent::start("guard-card").await;
    let key = "worktrees.projectAsk";
    let before = a.api.cmd("settings.get", json!({ "key": key })).await;
    let call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": "off" }));
    let (task, ask) = guard_ask(&a).await;
    let got = a.api.cmd("runtime.get", json!({ "id": task })).await;
    let q = got["ext"]["wisp.runtime"]["ask"]["question"].as_str().unwrap_or_default().to_owned();
    assert!(q.contains("stop asking before trusting") && q.contains("worktrees.projectAsk: on → off"), "{q}");
    assert_eq!(a.api.cmd("settings.get", json!({ "key": key })).await, before, "nothing changed while it asks");
    // the harness answering its own card, or replying to it, is refused; so is a follow-up from anyone (a later.add the
    // harness scheduled would send one), and the agent of another task
    let r = a.api.try_cmd("runtime.followUp", json!({ "id": task, "text": "Allow" })).await;
    assert!(r.to_string().contains("not a follow-up. Nothing was sent."), "{r}");
    let r = raw_cmd(&a, "runtime.answer", json!({ "id": task, "text": "Allow", "askId": ask })).await;
    assert!(r["ok"] == false && r["error"]["code"] == "forbidden", "an agent answered it: {r}");
    for (tool, args) in
        [("runtime_answer", json!({ "id": task, "text": "Allow", "askId": ask })), ("wisp_reply", json!({ "id": task, "text": "Allow" }))]
    {
        let (t, err) = a.user_call(tool, args).await;
        assert!(err && t.contains("asks the user, not their harness"), "{tool}: {t}");
    }
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    let (t, err) = call.await.unwrap();
    assert!(!err && t.contains("said no"), "{t}");
    assert_eq!(a.api.cmd("settings.get", json!({ "key": key })).await, before, "Deny changed it");
    assert_eq!(a.api.cmd("runtime.get", json!({ "id": task })).await["state"], "done", "the asking task is done");
    // Allow runs exactly the write shown
    let call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": "off" }));
    let (task, ask) = guard_ask(&a).await;
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Allow", "askId": ask })).await;
    let (t, err) = call.await.unwrap();
    assert!(!err && t.contains("allowed it"), "{t}");
    assert!(
        a.api.cmd("settings.get", json!({ "key": key })).await.to_string().contains("off"),
        "{}",
        a.api.cmd("settings.get", json!({ "key": key })).await
    );
    // a guarded write isn't sent to another computer
    let (t, err) = a.user_call("settings_set", json!({ "key": key, "value": "on", "peer": "studio" })).await;
    assert!(err && t.contains("isn't sent to another computer"), "{t}");
    // raising it again applies at once, as does any other setting
    let (t, err) = a.user_call("settings_set", json!({ "key": key, "value": "on" })).await;
    assert!(!err, "{t}");
    assert!(a.api.cmd("settings.get", json!({ "key": key })).await.to_string().contains("on"));
    let (t, err) = a.user_call("settings_set", json!({ "key": "crew.tint", "value": false })).await;
    assert!(!err, "{t}");
    // given as a bool, as the kernel would store it: asks too (the review: `false` slipped past "off")
    let call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": false }));
    let (task, ask) = guard_ask(&a).await;
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    let (t, _) = call.await.unwrap();
    assert!(t.contains("said no"), "{t}");
    a.serve.stop();
}

/// The guard-task check read only a top-level `id`, and a generated tool lifts `{"args": {"id": …}}` to the top level:
/// the user's harness answered its own guard card with `runtime_answer {"args": {"id": <card>, "text": "Allow"}}` and
/// the guarded write ran. Nested or not it's refused, the card stays open and nothing changes; a task that isn't a
/// guard's is still answered through nested args.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_cannot_answer_a_guard_card_through_nested_args() {
    let a = Agent::start("guard-nested").await;
    let key = "worktrees.projectAsk";
    let before = a.api.cmd("settings.get", json!({ "key": key })).await;
    let call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": "off" }));
    let (task, ask) = guard_ask(&a).await;
    for args in [
        json!({ "args": { "id": task, "text": "Allow", "askId": ask } }),
        json!({ "args": { "id": task, "text": "Allow" } }),
        json!({ "id": task, "text": "Allow", "askId": ask }),
    ] {
        let (t, err) = a.user_call("runtime_answer", args.clone()).await;
        assert!(err && t.contains("asks the user, not their harness"), "{args}: {t}");
    }
    let got = a.api.cmd("runtime.get", json!({ "id": task })).await;
    assert_eq!(got["ext"]["wisp.runtime"]["ask"]["id"].as_str(), Some(ask.as_str()), "the card is still open: {got}");
    assert_eq!(a.api.cmd("settings.get", json!({ "key": key })).await, before, "the guarded write ran");
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    assert!(call.await.unwrap().0.contains("said no"));
    assert_eq!(a.api.cmd("settings.get", json!({ "key": key })).await, before);
    // a task that isn't a guard's: answered through nested args as before
    let asked = a.api.cmd("runtime.ask", json!({ "id": a.id, "question": "go?", "options": ["yes", "no"] })).await;
    let ask = asked["askId"].as_str().unwrap().to_owned();
    let (t, err) = a.user_call("runtime_answer", json!({ "args": { "id": a.id, "text": "yes", "askId": ask } })).await;
    assert!(!err, "{t}");
    let got = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(got["ext"]["wisp.runtime"]["ask"].is_null(), "not answered: {got}");
    a.serve.stop();
}

/// Waits until no guard card asks any more (the one denied settled).
async fn no_guard_ask(a: &Agent) {
    let at = std::time::Instant::now();
    loop {
        let list = a.api.cmd("runtime.list", json!({})).await;
        let open = list
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["title"].as_str().is_some_and(|x| x.starts_with("Your harness wants to")) && t["state"] != "done");
        if !open {
            return;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "a guard card still asks: {list}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// A connector's command line runs on this computer for every task, and adding one tests it at once: the user's
/// harness `connectors_add {command: "sh", args: ["-c", "touch PWNED"]}` ran it unasked, and so did a project's test
/// command it set. Each waits on the user's Allow on a guard card; nothing is stored or run before, nor once denied.
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_adds_a_connector_or_a_project_test_only_on_the_users_guard_allow() {
    let a = Agent::start("guard-connector").await;
    let pwned = a.dir.join("PWNED");
    let line = format!("touch {}", pwned.display());
    let call = spawn_user_call(&a, "connectors_add", &json!({ "name": "evil", "command": "sh", "args": ["-c", line] }));
    let (task, ask) = guard_ask(&a).await;
    let got = a.api.cmd("runtime.get", json!({ "id": task })).await;
    let q = got["ext"]["wisp.runtime"]["ask"]["question"].as_str().unwrap_or_default().to_owned();
    assert!(q.contains("add a connector") && q.contains(&format!("sh -c {line}")), "{q}");
    assert!(!a.api.cmd("resources.export", json!({ "kinds": ["connector"] })).await.to_string().contains("evil"));
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    assert!(call.await.unwrap().0.contains("said no"));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!pwned.exists(), "the connector's command ran");
    let list = a.api.cmd("resources.export", json!({ "kinds": ["connector"] })).await;
    assert!(!list.to_string().contains("evil"), "{list}");
    no_guard_ask(&a).await;

    // a project's test command
    a.api.cmd("runtime.projectAdd", json!({ "path": a.dir.join("proj") })).await;
    let call = spawn_user_call(&a, "runtime_projectTest", &json!({ "name": "proj", "testCommand": line }));
    let (task, ask) = guard_ask(&a).await;
    let got = a.api.cmd("runtime.get", json!({ "id": task })).await;
    let q = got["ext"]["wisp.runtime"]["ask"]["question"].as_str().unwrap_or_default().to_owned();
    assert!(q.contains(&format!("`{line}`")), "{q}");
    assert!(!a.api.cmd("resources.export", json!({ "kinds": ["project"] })).await.to_string().contains("PWNED"), "stored before the Allow");
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    assert!(call.await.unwrap().0.contains("said no"));
    assert!(!a.api.cmd("resources.export", json!({ "kinds": ["project"] })).await.to_string().contains("PWNED"), "stored once denied");
    // switching a connector off asks nothing
    let (t, _) = a.user_call("connectors_update", json!({ "id": "nope", "enabled": false })).await;
    assert!(!t.contains("said no"), "{t}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    assert!(
        !list
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["title"].as_str().is_some_and(|x| x.starts_with("Your harness")) && t["state"] != "done"),
        "it asked: {list}"
    );
    assert!(!pwned.exists());
    a.serve.stop();
}

/// Audit I6: an agent's `runtime_projectAdd` for a folder that isn't there put "add the project …" on a card, and after
/// the user's Allow read "it didn't go through: No folder at …". It's refused to the agent first; nobody is asked.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_project_add_for_a_missing_folder_is_refused_before_any_card_audit_i6() {
    let a = Agent::start("project-add-missing").await;
    let gone = a.dir.join("proj9start");
    let (t, err) = a.call("runtime_projectAdd", json!({ "path": gone })).await;
    assert!(err && t.contains("No folder at") && t.contains("nothing was asked"), "{t}");
    let task = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(task["ext"]["wisp.runtime"]["ask"].is_null(), "a card was made: {}", task["ext"]["wisp.runtime"]["ask"]);
    a.serve.stop();
}

/// Waits until the guard task `id` settled as one whose harness stopped waiting; its row then.
async fn guard_gone(api: &Api, id: &str) -> Value {
    let at = std::time::Instant::now();
    loop {
        let got = api.cmd("runtime.get", json!({ "id": id })).await;
        if got["state"] == "done" && got["title"].as_str().is_some_and(|t| t.ends_with("not changed: the harness stopped waiting")) {
            return got;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(15), "the guard task didn't settle: {got}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Audit I IS2: a guard task whose harness call went (the harness gave up on a slow tool call; `wisp serve` restarted)
/// stayed in Needs you with its held prompt queued, answers changed nothing silently, and a reply ran the guard's own
/// text as a Full-auto prompt. It settles: done, its prompt dropped, saying the change wasn't made because the harness
/// stopped waiting; an answer or a follow-up after says so and runs nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_guard_task_whose_harness_stopped_waiting_settles_and_runs_nothing_audit_i_is2() {
    let a = Agent::start("guard-gone").await;
    let key = "worktrees.projectAsk";
    let before = a.api.cmd("settings.get", json!({ "key": key })).await;
    // a harness that gives up on a slow tool call after 1.5 s
    let impatient = reqwest::Client::builder().timeout(std::time::Duration::from_millis(1500)).build().unwrap();
    let (port, cwd) = (a.port, a.dir.join("proj").to_string_lossy().into_owned());
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "settings_set", "arguments": { "key": key, "value": "off" } } });
    let gave_up = tokio::spawn(async move {
        impatient
            .post(format!("http://127.0.0.1:{port}/api/mcp"))
            .header("content-type", "application/json")
            .header("x-wisp-cwd", cwd)
            .body(body.to_string())
            .send()
            .await
            .is_err()
    });
    let (task, ask) = guard_ask(&a).await;
    assert!(gave_up.await.unwrap(), "the harness's call timed out");
    let row = guard_gone(&a.api, &task).await;
    assert!(row["queue"].as_array().is_none_or(Vec::is_empty), "its held prompt is dropped: {row}");
    assert!(row["ext"]["wisp.runtime"]["ask"].is_null(), "nothing is asked any more: {row}");
    // an answer (the page's Allow, `wisp reply`) after: refused, saying why; nothing changes
    let r = a.api.try_cmd("runtime.answer", json!({ "id": task, "text": "Allow", "askId": ask })).await;
    assert!(r.to_string().contains("stopped waiting") && r.to_string().contains("wasn't made"), "{r}");
    let r = a.api.try_cmd("runtime.answer", json!({ "id": task, "text": "Allow" })).await;
    assert!(r.to_string().contains("stopped waiting"), "{r}");
    // a reply never runs the guard's text (nor the reply) as a prompt: guard tasks take no follow-ups
    let r = a.api.try_cmd("runtime.followUp", json!({ "id": task, "text": "hello there" })).await;
    assert!(r.to_string().contains("stopped waiting"), "{r}");
    let turns = a.api.cmd("runtime.turns", json!({ "id": task })).await;
    assert!(turns.as_array().is_none_or(Vec::is_empty), "nothing ran: {turns}");
    assert_eq!(a.api.cmd("settings.get", json!({ "key": key })).await, before, "nothing changed");

    // answered in time, then a follow-up: still none (it only asked)
    let call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": "off" }));
    let (task2, ask2) = guard_ask(&a).await;
    a.api.cmd("runtime.answer", json!({ "id": task2, "text": "Deny", "askId": ask2 })).await;
    assert!(call.await.unwrap().0.contains("said no"));
    let r = a.api.try_cmd("runtime.followUp", json!({ "id": task2, "text": "hello there" })).await;
    assert!(r.to_string().contains("takes no follow-ups"), "{r}");

    // `wisp serve` restarted while it asked: the call that would run it is gone, so it settles at start
    let _call = spawn_user_call(&a, "settings_set", &json!({ "key": key, "value": "off" }));
    let (task3, _) = guard_ask(&a).await;
    a.serve.stop();
    let (serve, port) = common::restart(&a.dir);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let row = guard_gone(&api, &task3).await;
    assert!(row["queue"].as_array().is_none_or(Vec::is_empty), "{row}");
    let r = api.try_cmd("runtime.answer", json!({ "id": task3, "text": "Allow" })).await;
    assert!(r.to_string().contains("stopped waiting"), "{r}");
    assert_eq!(api.cmd("settings.get", json!({ "key": key })).await, before, "nothing changed");
    serve.stop();
}

/// Audit I IS8 and IS9: with Auto off, an agent's `wisp_crew_add {agent: auto}` made a card that only after Allow said
/// "it didn't go through: Auto is off", `runtime_projectAdd` for a folder that's a project already made a card whose
/// Allow said "✓ You allowed" and changed nothing, and `runtime.profile.set {harness: auto}` stored an `auto` profile.
/// Each is refused up front, nobody asked, nothing stored.
#[tokio::test(flavor = "multi_thread")]
async fn a_change_that_cant_or_neednt_happen_is_refused_before_any_card_audit_i_is8_is9() {
    let a = Agent::start("refused-before-card").await;
    let proj = a.dir.join("proj");
    a.api.cmd("runtime.projectAdd", json!({ "path": proj })).await;
    let (t, err) = a.call("runtime_projectAdd", json!({ "path": proj })).await;
    assert!(err && t.contains("already") && t.contains("nothing was asked"), "{t}");
    let (t, err) = a.call("wisp_crew_add", json!({ "name": "Qa Auto", "role": "x", "agent": "auto" })).await;
    assert!(err && t.contains("Auto is off") && t.contains("Nothing was asked"), "{t}");
    let task = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(task["ext"]["wisp.runtime"]["ask"].is_null(), "a card was made: {}", task["ext"]["wisp.runtime"]["ask"]);
    let r = a.api.try_cmd("runtime.profile.set", json!({ "name": "qa-auto-prof", "harness": "auto" })).await;
    assert!(r.to_string().contains("Auto is off"), "{r}");
    let (t, err) = a.user_call("runtime_profile_set", json!({ "name": "qa-auto-prof", "harness": "auto" })).await;
    assert!(err && t.contains("Auto is off"), "{t}");
    let exported = a.api.cmd("resources.export", json!({ "kinds": ["agent"] })).await;
    assert!(!exported.to_string().contains("qa-auto-prof"), "stored: {exported}");
    a.serve.stop();
}

/// Audit I (smaller notes): an agent's `later.add` was stored as the user's (`"by": "user"`). An agent's scheduled
/// follow-up is the agent's, whatever it says; the user's stays the user's.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_scheduled_follow_up_is_the_agents_audit_i() {
    let a = Agent::start("later-by-agent").await;
    for by in [None, Some("user")] {
        let mut args = json!({ "sessionId": a.id, "text": "check CI", "in": 60 });
        if let Some(b) = by {
            args["by"] = json!(b);
        }
        let r = raw_cmd(&a, "later.add", args).await;
        assert!(r["ok"] != false, "{r}");
    }
    a.api.cmd("later.add", json!({ "sessionId": a.id, "text": "mine", "in": 60 })).await;
    let list = a.api.cmd("later.list", json!({ "sessionId": a.id })).await;
    let by: Vec<(String, String)> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["text"].as_str().unwrap_or_default().to_owned(), l["by"].as_str().unwrap_or_default().to_owned()))
        .collect();
    assert_eq!(by.iter().filter(|(t, b)| t == "check CI" && b == "agent").count(), 2, "{by:?}");
    assert!(by.iter().any(|(t, b)| t == "mine" && b == "user"), "{by:?}");
    a.serve.stop();
}

/// Audit I (smaller notes on the guard card): switching on a plugin that's already on asked the user; `machines_route`
/// of `runtime.list` with a `peer` was refused as a guard change; `runtime.exec`'s card didn't say which folder it runs
/// in. The first applies at once, the second isn't a guard, and the card names the task's folder.
#[tokio::test(flavor = "multi_thread")]
async fn the_guard_asks_only_about_what_lowers_one_and_says_where_a_command_runs_audit_i() {
    let a = Agent::start("guard-notes").await;
    let (t, err) = a.user_call("plugins_setEnabled", json!({ "id": "wisp.learn", "enabled": true })).await;
    assert!(!err && !t.contains("said no") && !t.contains("Your harness"), "{t}");
    let list = a.api.cmd("runtime.list", json!({})).await;
    assert!(!list.to_string().contains("Your harness wants to"), "it asked: {list}");
    let (t, _) = a.user_call("machines_route", json!({ "cmd": "runtime.list", "args": {}, "peer": "studio" })).await;
    assert!(!t.contains("safety guards"), "{t}");
    let call = spawn_user_call(&a, "runtime_exec", &json!({ "id": a.id, "command": "echo hi" }));
    let (task, ask) = guard_ask(&a).await;
    let got = a.api.cmd("runtime.get", json!({ "id": task })).await;
    let q = got["ext"]["wisp.runtime"]["ask"]["question"].as_str().unwrap_or_default().to_owned();
    let cwd = a.api.cmd("runtime.get", json!({ "id": a.id })).await["cwd"].as_str().unwrap_or_default().to_owned();
    assert!(!cwd.is_empty() && q.contains(&format!("(in {cwd})")), "{q}");
    a.api.cmd("runtime.answer", json!({ "id": task, "text": "Deny", "askId": ask })).await;
    assert!(call.await.unwrap().0.contains("said no"));
    a.serve.stop();
}

/// The review of audit I IS2: `runtime.ask` took `details.guard` from any caller, so an agent's token could mark its own
/// task as a guard task, which then took no follow-ups for good. Only the user's side makes a guard question.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_cannot_make_a_guard_question_audit_i_is2() {
    let a = Agent::start("agent-no-guard").await;
    let r = raw_cmd(&a, "runtime.ask", json!({ "id": a.id, "question": "x?", "options": ["Allow", "Deny"], "details": { "guard": true } }))
        .await;
    assert!(r["ok"] != false, "{r}");
    let task = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert!(task["ext"]["wisp.runtime"]["ask"]["guard"].is_null(), "an agent marked a guard question: {task}");
    assert!(task["ext"]["wisp.runtime"]["guardTask"].is_null(), "{task}");
    a.serve.stop();
}

/// ADR-0119: an agent runs a workflow from its task with `workflows_run` (Claude Code's workflows, on any harness): its
/// steps are its task's children, each reads what the steps it needs gave, the task's view says where the run stands,
/// and the agent is sent every step's result when it ends. It can't send steps to another folder.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_runs_a_workflow_under_its_own_task_and_is_told_how_it_ended() {
    let a = Agent::start_spec(
        "agent-workflow",
        |dir| vec![("FAKE_DELAY".into(), "0".into()), ("FAKE_GATE".into(), common::gates(dir).to_string_lossy().into_owned())],
        json!({ "prompt": "gate lead hi" }),
    )
    .await;
    let names = a.names(true).await;
    for t in ["workflows_run", "workflows_status", "workflows_stop"] {
        assert!(names.iter().any(|n| n == t), "{t} for the agent: {names:?}");
    }
    let spec = json!({ "tasks": {
        "look": { "prompt": "look at math.js", "phase": "Explore" },
        "fix": { "prompt": "fix it, after: ${{ tasks.look.output.text }}", "needs": ["look"], "phase": "Build", "mode": "auto" },
    } });
    let (said, err) = a.call("workflows_run", json!({ "name": "audit", "spec": spec, "project": "/tmp" })).await;
    assert!(err && said.contains("leave out project"), "{said}");
    let (said, err) = a.call("workflows_run", json!({ "name": "audit", "spec": spec })).await;
    assert!(!err && said.contains("Don't do its steps yourself"), "{said}");
    // the run is over once its view says done and its result waits in the task's queue (wisp's own follow-up);
    // finish() queues the message before the view says done, so the first snapshot with both is the end
    let ended = |t: &Value| {
        let q = t["queue"].as_array().cloned().unwrap_or_default();
        q.iter().any(|i| i["text"].as_str().is_some_and(|x| x.starts_with("wisp: the workflow audit ended done")))
    };
    let at = std::time::Instant::now();
    let t = loop {
        let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
        if t.pointer("/ext/wisp.workflows/runs/0/state").and_then(Value::as_str) == Some("done") && ended(&t) {
            break t;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(60), "the run ends with its result queued: {t}");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    };
    let run = &t["ext"]["wisp.workflows"]["runs"][0];
    assert_eq!(run["workflow"], "audit");
    assert_eq!(run["phases"], json!(["Explore", "Build"]));
    assert_eq!((run["done"].as_u64(), run["total"].as_u64()), (Some(2), Some(2)), "{run}");
    for s in run["steps"].as_array().unwrap() {
        let step = a.api.try_cmd("runtime.get", json!({ "id": s["task"] })).await;
        let step = step.get("result").cloned().unwrap_or(step);
        assert_eq!(step["parentId"], a.id.as_str(), "a step is its owner's child: {step} {run}");
        assert_eq!(step["ext"]["wisp.workflows"]["node"], s["id"], "{step}");
    }
    // the task's agent is still in its turn: the result waits in its queue, as wisp's own follow-up
    assert!(ended(&t), "{t}");
    let (said, err) = a.call("workflows_status", json!({ "id": run["id"] })).await;
    assert!(!err && said.contains("\"state\":\"done\""), "{said}");
    a.serve.stop();
}

/// ADR-0119 rule 7: `/workflow <words>` reaches the harness with the lead's note in front (naming the tool as Claude
/// names it), while the task shows what was typed; `/workflow` alone is refused, saying what to type.
#[tokio::test(flavor = "multi_thread")]
async fn slash_workflow_makes_the_agent_the_runs_lead_and_the_task_shows_what_was_typed() {
    let dir = temp("slash-workflow");
    let rec = dir.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let (serve, port) = start_with(&dir, &[("FAKE_RECORD", rec.to_str().unwrap())]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let r = api.try_cmd("runtime.create", json!({ "prompt": "/workflow", "cwd": dir.join("proj"), "agent": "claude" })).await;
    assert!(r.to_string().contains("Say what the workflow should do"), "{r}");
    let made =
        api.cmd("runtime.create", json!({ "prompt": "/workflow audit every route", "cwd": dir.join("proj"), "agent": "claude" })).await;
    let id = made["id"].as_str().unwrap().to_owned();
    let turns = api.until(&id, "the turn ends", |t| t.last().is_some_and(ended)).await;
    assert_eq!(turns[0]["prompt"], "/workflow audit every route", "the task shows what was typed");
    let runs = until_records(&rec.join("claude.jsonl"), "the run's record", |r| !r.is_empty()).await;
    let sent = runs[0].to_string();
    assert!(sent.contains("wisp-workflow") && sent.contains("mcp__wisp__workflows_run"), "{sent}");
    serve.stop();
}

/// ADR-0120: a task whose agent ended its turn while its workflow runs stays in Working on it (never "waiting on you"
/// while its steps work), and the run's end starts the agent's next turn with every step's result, after which the task
/// lands. `workflow.started` and `workflow.ended` are durable events about the task.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_stays_working_while_its_workflow_runs_and_lands_after_the_results_turn() {
    let gates = std::cell::RefCell::new(std::path::PathBuf::new());
    let a = Agent::start_spec(
        "workflow-hold",
        |dir| {
            let g = common::gates(dir);
            gates.replace(g.clone());
            vec![("FAKE_DELAY".into(), "0".into()), ("FAKE_GATE".into(), g.to_string_lossy().into_owned())]
        },
        json!({ "prompt": "gate lead hi" }),
    )
    .await;
    let gates = gates.into_inner();
    let spec = json!({ "tasks": { "look": { "prompt": "gate steps look at math.js" }, "fix": { "prompt": "gate steps fix it", "needs": ["look"] } } });
    let (said, err) = a.call("workflows_run", json!({ "name": "held", "spec": spec })).await;
    assert!(!err, "{said}");
    // the agent ends its turn; its step still works
    std::fs::write(gates.join("lead"), "").unwrap();
    a.api.until(&a.id, "the lead's turn ends", |t| t.len() == 1 && ended(&t[0])).await;
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
    assert_eq!(t["state"], "running", "held in Working while its workflow runs: {t}");
    // the steps end: wisp's message starts the agent's next turn, and after it the task lands
    std::fs::write(gates.join("steps"), "").unwrap();
    let turns = a.api.until(&a.id, "the results' turn ends", |t| t.len() == 2 && ended(&t[1])).await;
    assert!(turns[1]["prompt"].as_str().is_some_and(|p| p.starts_with("wisp: the workflow held ended done")), "{turns:?}");
    let at = std::time::Instant::now();
    loop {
        let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
        if t["state"] != "running" {
            break;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(20), "the task lands: {t}");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    let ev = a.api.cmd("events.tail", json!({ "type": "workflow.*", "subject": a.id })).await.to_string();
    assert!(ev.contains("workflow.started") && ev.contains("workflow.ended"), "{ev}");
    a.serve.stop();
}

/// ADR-0121: the agent changes its run while it goes: it skips a stuck step (its task stops; the steps after it run
/// without it), then adds a step to the run that ended, which starts it again under its task. A refusal changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_skips_and_adds_steps_and_is_told_when_one_fails() {
    let a = Agent::start_spec(
        "workflow-edit",
        |dir| vec![("FAKE_DELAY".into(), "0".into()), ("FAKE_GATE".into(), common::gates(dir).to_string_lossy().into_owned())],
        json!({ "prompt": "gate lead hi" }),
    )
    .await;
    let spec = json!({ "tasks": {
        "stuck": { "prompt": "gate never look around" },
        "next": { "prompt": "write it up", "needs": ["stuck"] },
    } });
    let (said, err) = a.call("workflows_run", json!({ "name": "edited", "spec": spec })).await;
    assert!(!err, "{said}");
    let run = said.split("run ").nth(1).and_then(|r| r.split('.').next()).unwrap().to_owned();
    let (said, err) = a.call("workflows_edit", json!({ "id": run, "add": { "bad": { "prompt": "x", "needs": ["ghost"] } } })).await;
    assert!(err && said.contains("no task named ghost"), "a refusal says why: {said}");
    let view = |t: &Value| t["ext"]["wisp.workflows"]["runs"][0].clone();
    // the stuck step is working: skip it, and the step after it runs
    let at = std::time::Instant::now();
    while view(&a.api.cmd("runtime.get", json!({ "id": a.id })).await)["steps"][0]["state"] != "running" {
        assert!(at.elapsed() < std::time::Duration::from_secs(20), "stuck starts");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    let (said, err) = a.call("workflows_edit", json!({ "id": run, "skip": ["stuck"] })).await;
    assert!(!err && said.contains("skipped stuck"), "{said}");
    let v = run_ends_with(&a, 2).await;
    assert_eq!(v["steps"][0]["why"], "skipped by the agent", "{v}");
    assert_eq!(v["steps"][1]["state"], "done", "{v}");
    // the run ended: a step added starts it again
    let (said, err) =
        a.call("workflows_edit", json!({ "id": run, "add": { "more": { "prompt": "one more look", "needs": ["next"] } } })).await;
    assert!(!err, "{said}");
    let v = run_ends_with(&a, 3).await;
    assert_eq!(v["steps"][2]["id"], "more", "{v}");
    a.serve.stop();
}

/// The task's first run's view once it ended done with `n` steps.
async fn run_ends_with(a: &Agent, n: u64) -> Value {
    let at = std::time::Instant::now();
    loop {
        let t = a.api.cmd("runtime.get", json!({ "id": a.id })).await;
        let v = t["ext"]["wisp.workflows"]["runs"][0].clone();
        if v["state"] == "done" && v["total"].as_u64() == Some(n) {
            return v;
        }
        assert!(at.elapsed() < std::time::Duration::from_secs(30), "the run ends with {n} steps: {v}");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
