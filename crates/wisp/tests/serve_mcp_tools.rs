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
        let argv = argv(&runs[0]);
        let cfg: Value = serde_json::from_str(&argv[argv.iter().position(|a| a == "--mcp-config").expect("--mcp-config") + 1]).unwrap();
        let token = cfg["mcpServers"]["wisp"]["headers"]["x-wisp-token"].as_str().unwrap().to_owned();
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
/// is the one it is. An agent inside a task has no such tool: the user's setup is the user's (ADR-0112).
#[tokio::test(flavor = "multi_thread")]
async fn the_users_harness_adds_a_project_through_its_mcp_tool_and_the_server_names_it() {
    let a = Agent::start("mcp-project-add").await;
    assert!(!a.names(true).await.iter().any(|n| n == "runtime_projectAdd"), "an agent in a task has no tool to add a project");
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
    // an agent in a task doesn't change the user's crew (ADR-0112); the user's harness does
    let (t, err) = a.call("wisp_crew_edit", json!({ "member": "Scout", "role": "Boss" })).await;
    assert!(err && t.contains("can't change the crew: the user does"), "{t}");
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
    let (t, err) = a.call("wisp_crew_remove", json!({ "member": "scout" })).await;
    assert!(err && t.contains("can't change the crew: the user does"), "an agent in a task doesn't (ADR-0112): {t}");
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

/// An agent in a task never adds a connector (ADR-0112: a command it wrote would run for every task); the user does, by
/// its header's name, the value typed on the card.
#[tokio::test(flavor = "multi_thread")]
async fn wisp_connector_add_is_the_users_and_takes_key_names() {
    let a = Agent::start("mcp-connectors-add").await;
    let (t, err) = a.call("wisp_connector_add", json!({ "name": "Linear", "url": "https://mcp.linear.app/mcp" })).await;
    assert!(err && t.contains("the user adds them"), "{t}");
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
    assert!(said.contains("no agent named critc: agents has critic (or "), "{said}");
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
    #[cfg(unix)]
    {
        let (s, e) = a.call("wisp_launch", launch(json!("link.wisp.yml"))).await;
        refused(&s, e, "leads out of the task's folder");
    }
    let (s, e) = a.call("wisp_launch", launch(json!("secrets.json"))).await;
    refused(&s, e, "a secrets file isn't read for an agent's formula");
    let file = format!("${{file(\"{}\")}}", a.dir.join("secret.txt").display());
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
    // its MCP tool only asks the user, who adds it or not: unanswered, nothing is added
    let (said, err) = a.call("wisp_connector_add", evil).await;
    assert!(err || said.contains("nothing was added"), "{said}");
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
        "machines.call",
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
        "runtime.delete",
        "runtime.exec",
        "runtime.followUp",
        "runtime.forLater",
        "runtime.fork",
        "runtime.held",
        "runtime.markRead",
        "runtime.note",
        "runtime.pin",
        "runtime.probe",
        "runtime.projectRun",
        "runtime.projectTest",
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
        "runtime.stop",
        "runtime.terminal",
        "runtime.unask",
        "sandbox.bring",
        "sandbox.drop",
        "sandbox.launch",
        "sandbox.own",
        "sandbox.place",
        "sandbox.seed",
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
    ]
    .iter()
    .map(|x| (*x).to_owned())
    .collect();
    assert_eq!(w, want, "the agent's writes changed: decide each one in ADR-0111");
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
