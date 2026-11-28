//! wisp.github: CI and review requests as trigger sources (ADR-0043, `trigger.source@1`): `ci.failed` and
//! `ci.passed`, polled with `gh run list`, and `pr.review_requested`, with `gh pr list` (its events say `me`, the
//! login gh is signed in as, for `match: { reviewer: me }`), in the trigger's project, through the proc service. A 403 rate limit answers `retry_at` (from
//! `gh api rate_limit`), which the engine waits for without counting a failure.
//!
//! And the channels transport `github` (`comments.rs`): `/wisp …` comments on your repos start or follow up tasks.

pub mod comments;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use wisp_automations_api::{Polled, SOURCE, SourceOn, TriggerSource};
use wisp_github_core::{ci, pr};
use wisp_kernel::{BoxFuture, Ctx, Manifest, Plugin, Result, Setup, WireError};
use wisp_proc::{Env, ExecOpts, Proc};

#[derive(Debug, Default)]
pub struct GithubPlugin;

/// When something was read, to read it again later.
type Held<T> = Arc<Mutex<T>>;

/// The sources: the gh login and when it was read (10 minutes, so `gh auth switch` takes effect), and each author's
/// `trusted` by folder and login (an hour: permissions change rarely, and a poll asks only for new events' authors).
struct Ci(Ctx, Held<Option<(String, i64)>>, Held<HashMap<String, (bool, i64)>>);

const ME_FRESH_MS: i64 = 10 * 60_000;
const TRUST_FRESH_MS: i64 = 60 * 60_000;

/// Whether `login` may push to the repo gh finds in `cwd`; a bot, a failed call or no answer is not trusted.
async fn may_push(proc: &Proc, cwd: &str, login: &str) -> bool {
    if login.is_empty() {
        return false;
    }
    let args = [
        "api".to_owned(),
        format!("repos/{{owner}}/{{repo}}/collaborators/{login}/permission"),
        "--jq".to_owned(),
        ".permission".to_owned(),
    ];
    match proc.exec(&gh_bin(), &args, opts(cwd)).await {
        Ok(r) if r.exit.success() && !r.timed_out => pr::may_push(&r.stdout_text()),
        _ => false,
    }
}

/// Each event says whether its author may push (`trusted`), read once an hour per author.
async fn mark_trusted(
    proc: &Proc,
    cwd: &str,
    events: &mut [wisp_automations_api::TriggerEvent],
    cache: &Held<HashMap<String, (bool, i64)>>,
    now: i64,
) {
    for e in events {
        let login = e.payload.get("author").and_then(Value::as_str).unwrap_or_default().to_owned();
        let bot = e.payload.get("bot").and_then(Value::as_bool).unwrap_or(false);
        let key = format!("{cwd}\n{login}");
        let held = cache.lock().unwrap_or_else(PoisonError::into_inner).get(&key).copied().filter(|(_, at)| now - at < TRUST_FRESH_MS);
        let yes = if let Some((t, _)) = held {
            t
        } else {
            let t = !bot && may_push(proc, cwd, &login).await;
            cache.lock().unwrap_or_else(PoisonError::into_inner).insert(key, (t, now));
            t
        };
        e.payload["trusted"] = json!(yes);
    }
}

fn gh_bin() -> String {
    std::env::var("WISP_GH_BIN").unwrap_or_else(|_| "gh".into())
}

fn opts(cwd: &str) -> ExecOpts {
    ExecOpts {
        cwd: (!cwd.is_empty()).then(|| cwd.into()),
        env: Env::default().with("GH_PROMPT_DISABLED", "1").with("GH_NO_UPDATE_NOTIFIER", "1").with("GIT_TERMINAL_PROMPT", "0"),
        timeout: Some(Duration::from_secs(30)),
        ..ExecOpts::default()
    }
}

/// The login gh is signed in as, empty when it can't say (signed out, offline): `me` then matches nothing.
async fn login(proc: &Proc, cwd: &str) -> String {
    let args = ["api".to_owned(), "user".to_owned(), "--jq".to_owned(), ".login".to_owned()];
    match proc.exec(&gh_bin(), &args, opts(cwd)).await {
        Ok(r) if r.exit.success() && !r.timed_out => r.stdout_text().trim().to_owned(),
        _ => String::new(),
    }
}

async fn poll(
    proc: Arc<Proc>,
    on: String,
    spec: Value,
    cursor: Value,
    me: String,
    now: i64,
    trust: Held<HashMap<String, (bool, i64)>>,
) -> Result<Polled> {
    let s = |k: &str| spec.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let cwd = s("project");
    let review = on == "pr.review_requested";
    if review && cwd.is_empty() {
        return Err(WireError::invalid("pr.review_requested needs a project (the repo it watches)"));
    }
    let args = if review { pr::pr_list_args() } else { ci::run_list_args(&s("branch")) };
    let what = if review { "gh pr list" } else { "gh run list" };
    let r = proc.exec(&gh_bin(), &args, opts(&cwd)).await.map_err(|e| WireError::unavailable(format!("gh: {e}")))?;
    if r.timed_out {
        return Err(WireError::unavailable(format!("{what} timed out after 30 s")));
    }
    if !r.exit.success() {
        let err = r.stderr_text();
        if ci::rate_limited(&err) {
            let reset = proc
                .exec(&gh_bin(), &["api".into(), "rate_limit".into()], opts(&cwd))
                .await
                .ok()
                .and_then(|o| ci::reset_at(&o.stdout_text()));
            // no reset said (0): the engine waits an hour
            return Ok(Polled { events: Vec::new(), cursor, retry_at: Some(reset.unwrap_or(0)) });
        }
        return Err(WireError::unavailable(format!("{what}: {}", err.trim())));
    }
    let (mut events, cursor) =
        if review { pr::review_events(&on, &r.stdout_text(), &cursor, now, &me) } else { ci::ci_events(&on, &r.stdout_text(), &cursor) }
            .map_err(WireError::internal)?;
    if review && pr::asks_for(&spec, "trusted") {
        mark_trusted(&proc, &cwd, &mut events, &trust, now).await;
    }
    Ok(Polled { events, cursor, retry_at: None })
}

impl TriggerSource for Ci {
    fn plugin(&self) -> &'static str {
        "wisp.github"
    }
    fn ons(&self) -> Vec<SourceOn> {
        let fields = json!({ "type": "object", "properties": { "branch": { "type": "string" }, "workflow": { "type": "string" }, "project": { "type": "string" } } });
        vec![
            SourceOn { on: "ci.failed".into(), title: "CI failed".into(), fields: fields.clone(), mode: "poll".into(), every: Some(5) },
            SourceOn { on: "ci.passed".into(), title: "CI passed".into(), fields, mode: "poll".into(), every: Some(5) },
            SourceOn {
                on: "pr.review_requested".into(),
                title: "A review requested".into(),
                fields: review_fields(),
                mode: "poll".into(),
                every: Some(5),
            },
        ]
    }
    fn poll(&self, on: &str, spec: &Value, cursor: Value) -> BoxFuture<'static, Result<Polled>> {
        let proc = self.0.service::<Proc>();
        let now = self.0.clock().map(|c| c.now_ms());
        let held = self.1.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let (on, spec) = (on.to_owned(), spec.clone());
        let me_cache = Arc::clone(&self.1);
        let trust = Arc::clone(&self.2);
        Box::pin(async move {
            let (proc, now) = (proc?, now?);
            let mut me = String::new();
            if on == "pr.review_requested" {
                if let Some((m, _)) = held.filter(|(_, at)| now - at < ME_FRESH_MS) {
                    me = m;
                } else {
                    me = login(&proc, spec.get("project").and_then(Value::as_str).unwrap_or("")).await;
                    if !me.is_empty() {
                        *me_cache.lock().unwrap_or_else(PoisonError::into_inner) = Some((me.clone(), now));
                    }
                }
            }
            poll(proc, on, spec, cursor, me, now, trust).await
        })
    }
}

/// `pr.review_requested`'s fields: `me` in reviewer/author is whoever this computer's gh is signed in as.
fn review_fields() -> Value {
    let who = |d: &str| json!({ "type": ["string", "array"], "description": d });
    json!({ "type": "object", "properties": {
        "project": { "type": "string" },
        "reviewer": who("the reviewer just asked (a GitHub login, a team's org/slug, or me)"),
        "author": who("who opened the PR (a GitHub login, or me)"),
        "base": { "type": "string", "description": "the branch it merges into" },
        "bot": { "type": "boolean", "description": "false: not from a bot (Dependabot, Renovate, an app)" },
        "fork": { "type": "boolean", "description": "false: not from a fork" },
        "trusted": { "type": "boolean", "description": "true: only authors who may push to the repo" },
        "draft": { "type": "boolean", "description": "false: skip draft PRs" },
        "paths": { "type": ["string", "array"], "description": "changed files, as .gitignore globs (src/**, !**/*.md)" },
        "size": { "type": "string", "description": "lines added + deleted, compared: <400" },
        "milestone": { "type": "string" },
        "label": { "type": "string" },
    } })
}

impl Plugin for GithubPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let src: Arc<dyn TriggerSource> = Arc::new(Ci(s.ctx(), Arc::default(), Arc::default()));
        s.contribute(&SOURCE, src);
        let t: Arc<dyn wisp_channels_api::TransportDef> = Arc::new(comments::Def);
        s.contribute(&wisp_channels_api::TRANSPORTS, t);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(GithubPlugin.manifest().id, "wisp.github");
    }
}
