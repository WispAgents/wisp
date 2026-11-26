//! wisp.connectors' resource kind (ADR-0028): the MCP servers agents get. Kept as declared (`StoredKind`)
//! and given to runs through `runtime.run@1`: a connector goes to a run when it's on, its `agents` list
//! names the task's agent profile (or is empty, or the task names it) and its `modes` the task's mode (empty: Edits and
//! Full auto; Plan mode only when they name plan). `${secret:NAME}` in its env and headers is read from secrets when it's applied
//! (and at start), so a run never waits on it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Ctx, Result, Setup};
use wisp_resources_api::{KindInfo, Op, RESOURCE_KIND, Resource, ResourceKind, StoredKind, schema_of};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{McpQuery, RUN, RunPart};

/// An MCP server: a command wisp starts, or a URL it reaches.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectorSpec {
    /// the program (stdio servers)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// the server's URL (HTTP servers)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// how a URL is spoken: `http` (streamable HTTP, the default) or `sse` (the 2024 SSE transport)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<RemoteTransport>,
    /// its environment; secrets as `${secret:NAME}`
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// HTTP headers; secrets as `${secret:NAME}`
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// the agent profiles that get it (empty: every one)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// the modes that get it (plan, edits, auto; empty: edits and auto, Plan mode only when named here)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
    /// off: declared but no agent gets it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// it signs in with OAuth (the token stays in secrets)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<bool>,
}

/// A remote server's transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RemoteTransport {
    #[default]
    Http,
    Sse,
}

impl ConnectorSpec {
    /// A remote server over the 2024 SSE transport.
    pub fn sse(&self) -> bool {
        !self.url.is_empty() && self.transport == Some(RemoteTransport::Sse)
    }
}

/// The connectors as runs get them: (name, spec with secrets read).
type Cache = Arc<Mutex<Vec<(String, ConnectorSpec)>>>;

struct ConnectorKind {
    inner: Arc<StoredKind>,
    cache: Cache,
}

const SECRET_REF: &str = "${secret:";

/// Whether connector `id` may read the secret `name`: its own (`connector:<id>:…`, what Settings → Connectors keeps) or
/// a formula's (`LINEAR_KEY`: `wisp secret set`'s names), never another connector's token nor a plugin's own secret
/// (`channels/sign`): a header naming one would send it to the connector's server.
fn may_read(id: &str, name: &str) -> bool {
    name.strip_prefix(&wisp_connectors_api::secrets_ns(id)).is_some_and(|r| r.starts_with(':')) || wisp_resources_api::secret_name_ok(name)
}

/// The `${secret:NAME}` references in `text` connector `id` may read, in order.
fn refs(id: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find(SECRET_REF) {
        let Some(b) = rest[a..].find('}') else { break };
        let name = &rest[a + SECRET_REF.len()..a + b];
        if may_read(id, name) && !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
        rest = &rest[a + b + 1..];
    }
    out
}

/// `text` with each `${secret:NAME}` connector `id` may read filled from `lookup`, once, left to right: a value put in
/// is never read again (a value holding a reference, its own included, stays as it is; it looped for good), and a
/// reference it may not read, or that isn't set, stays as written.
pub(crate) fn expand(id: &str, text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(a) = rest.find(SECRET_REF) {
        let Some(b) = rest[a..].find('}') else { break };
        let name = &rest[a + SECRET_REF.len()..a + b];
        out.push_str(&rest[..a]);
        match Some(name).filter(|n| may_read(id, n)).and_then(lookup) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[a..=a + b]),
        }
        rest = &rest[a + b + 1..];
    }
    out.push_str(rest);
    out
}

/// Connector `id`'s `${secret:NAME}` references in `text` read from secrets (`expand`).
pub(crate) async fn reveal(ctx: &Ctx, id: &str, text: &str) -> String {
    let Ok(secrets) = wisp_kernel::services::secrets(ctx) else { return text.to_owned() };
    let mut got = std::collections::HashMap::new();
    for name in refs(id, text) {
        if let Ok(Some(v)) = secrets.get(&name).await {
            got.insert(name, v);
        }
    }
    expand(id, text, &|n| got.get(n).cloned())
}

/// A URL's origin (scheme, host and port, in lower case, the scheme's own port left out): what a sign-in is for.
pub(crate) fn origin(url: &str) -> String {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_lowercase() };
    let scheme = scheme.to_lowercase();
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h).to_lowercase();
    let host = match (scheme.as_str(), host.strip_suffix(":443"), host.strip_suffix(":80")) {
        ("https", Some(h), _) | ("http", _, Some(h)) => h.to_owned(),
        _ => host,
    };
    format!("{scheme}://{host}")
}

/// Whether a sign-in's token may go to `url` as a bearer header: over https, or plain http to this computer only (a
/// token sent in the clear to another host can be read on the way).
pub(crate) fn bearer_ok(url: &str) -> bool {
    let o = origin(url);
    if o.starts_with("https://") {
        return true;
    }
    let Some(host) = o.strip_prefix("http://") else { return false };
    let host =
        if host.starts_with('[') { host.split(']').next().map_or(host, |h| &h[1..]) } else { host.split(':').next().unwrap_or(host) };
    host == "localhost" || host == "::1" || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
}

/// A signed-in connector's stored token, as it is (never expanded: a server's `access_token` of `${secret:…}` is
/// a value, not a reference), when its URL may carry one (`bearer_ok`).
pub(crate) async fn bearer(ctx: &Ctx, id: &str, url: &str) -> Option<String> {
    if !bearer_ok(url) {
        return None;
    }
    let s = wisp_kernel::services::secrets(ctx).ok()?;
    s.get(&format!("{}:token", wisp_connectors_api::secrets_ns(id))).await.ok().flatten().filter(|t| !t.is_empty())
}

/// Reads every stored connector into the cache, secrets read.
async fn refresh(inner: &StoredKind, cache: &Cache) {
    let mut list = Vec::new();
    for (name, v) in inner.specs().await.unwrap_or_default() {
        let Ok(mut c) = serde_json::from_value::<ConnectorSpec>(v) else { continue };
        for x in c.env.values_mut().chain(c.headers.values_mut()) {
            *x = reveal(&inner.ctx, &name, x).await;
        }
        // signed in (Settings → Connectors): the token goes as a bearer header
        if c.oauth == Some(true)
            && !c.headers.contains_key("Authorization")
            && let Some(t) = bearer(&inner.ctx, &name, &c.url).await
        {
            c.headers.insert("Authorization".into(), format!("Bearer {t}"));
        }
        list.push((name, c));
    }
    *cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = list;
}

impl ResourceKind for ConnectorKind {
    fn info(&self) -> KindInfo {
        self.inner.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.inner.read(call)
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            self.inner.apply(call, op).await?;
            refresh(&self.inner, &self.cache).await;
            Ok(())
        })
    }
}

/// A connector as a harness's MCP config takes it.
pub fn server_of(c: &ConnectorSpec) -> Value {
    if c.url.is_empty() {
        json!({ "type": "stdio", "command": c.command, "args": c.args, "env": c.env })
    } else {
        json!({ "type": if c.sse() { "sse" } else { "http" }, "url": c.url, "headers": c.headers })
    }
}

/// Whether a run of `profile` in `mode`, naming `named`, gets connector `c` called `name`. Plan mode gets only one whose
/// `modes` names plan: its tools would run in a read-only task, unasked (an empty list means Edits and Full auto). A task
/// that names it gets it whatever its `agents` say, never in a mode it's not for.
pub fn gets(name: &str, c: &ConnectorSpec, profile: &str, mode: &str, named: &[String]) -> bool {
    if c.enabled == Some(false) {
        return false;
    }
    let for_mode =
        if mode == "plan" { c.modes.iter().any(|m| m == "plan") } else { c.modes.is_empty() || c.modes.iter().any(|m| m == mode) };
    for_mode && (named.iter().any(|n| n == name) || c.agents.is_empty() || c.agents.iter().any(|a| a == profile))
}

/// A `wisp run --formula` overlay's connectors with their `${secret:}` read, by task: in memory only (a
/// secret never goes in the task's row), filled by `before:run` and dropped when the plugin stops.
type Revealed = Arc<Mutex<std::collections::HashMap<String, Vec<(String, ConnectorSpec)>>>>;

/// harness → the server names its own config already has (`harness::own_names`), as the pane last read them: kept here
/// so a run's servers are made without reading a file.
pub(crate) type Own = Arc<Mutex<BTreeMap<String, BTreeSet<String>>>>;

/// A name wisp gives a server of its own (`wisp`, `browser`, `computer`, …: `OWN_MCP_SERVERS`): Claude Code and Codex
/// run those servers' tools without asking, so a connector can't take one.
pub(crate) fn reserved(name: &str) -> bool {
    wisp_harness_kit::protocol::OWN_MCP_SERVERS.contains(&name)
}

struct Runs {
    cache: Cache,
    revealed: Revealed,
    own: Own,
    /// the harnesses an SSE connector is skipped for
    no_sse: Vec<String>,
}

impl Runs {
    /// Whether harness `h` gets connector `n`: not when its own config has a server by that name (Codex would merge the
    /// two into one broken server, Claude list every tool twice), nor an SSE one it can't speak. Never one by the name
    /// of a server wisp gives a run itself (`reserved`): the harnesses trust those by name, so it would run unasked.
    fn reaches(&self, own: &BTreeMap<String, BTreeSet<String>>, h: &str, n: &str, c: &ConnectorSpec) -> bool {
        if reserved(n) {
            return false;
        }
        let own_one = own.get(h).is_some_and(|names| names.contains(n));
        let no_sse = c.sse() && self.no_sse.iter().any(|x| x == h);
        !(own_one || no_sse)
    }
}

impl RunPart for Runs {
    fn id(&self) -> &'static str {
        "wisp.connectors"
    }
    fn mcp_servers(&self, session: &Session, q: &McpQuery) -> BTreeMap<String, Value> {
        let spec = session.ext.get("wisp.runtime").and_then(|m| m.get("spec"));
        let profile = spec.and_then(|s| s.get("agent")).and_then(Value::as_str).unwrap_or_default();
        let named: Vec<String> = spec
            .and_then(|s| s.get("connectors"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let mode = serde_json::to_value(q.mode).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        // a `wisp run --formula` overlay's connectors first: they're this task's alone
        let only = self
            .revealed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(session.id.as_str())
            .cloned()
            .unwrap_or_else(|| overlay_of(session.ext.get("wisp.runtime")));
        let own = self.own.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let wanted = |n: &String, c: &ConnectorSpec| gets(n, c, profile, &mode, &named) && self.reaches(&own, &q.harness, n, c);
        let mut out: BTreeMap<String, Value> = only.iter().filter(|(n, c)| wanted(n, c)).map(|(n, c)| (n.clone(), server_of(c))).collect();
        for (n, c) in self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().filter(|(n, c)| wanted(n, c)) {
            out.entry(n.clone()).or_insert_with(|| server_of(c));
        }
        out
    }
}

/// The overlay's connectors as the task carries them (secrets still `${secret:NAME}`).
fn overlay_of(own: Option<&Value>) -> Vec<(String, ConnectorSpec)> {
    own.and_then(|m| m.get("connectors"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(n, v)| Some((n.clone(), serde_json::from_value(v.clone()).ok()?)))
        .collect()
}

/// Before a run of a task with overlay connectors: their secrets read into memory.
async fn reveal_overlay(ctx: &Ctx, call: &Call, revealed: &Revealed, session: &str) {
    if revealed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains_key(session) {
        return;
    }
    let task = match ctx.run::<wisp_runtime_api::commands::Get>(call, wisp_runtime_api::commands::IdArgs { id: session.into() }).await {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(error = %err.message, task = session, "connectors: runtime.get before a run");
            return;
        }
    };
    let mut list = overlay_of(task.ext.get("wisp.runtime"));
    if list.is_empty() {
        return;
    }
    for (name, c) in &mut list {
        for x in c.env.values_mut().chain(c.headers.values_mut()) {
            *x = reveal(ctx, name, x).await;
        }
    }
    revealed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(session.to_owned(), list);
}

/// The connectors as runs get them, read again: after a token was refreshed, so a new run gets the new bearer.
#[derive(Clone)]
pub(crate) struct RunCache {
    inner: Arc<StoredKind>,
    cache: Cache,
    own: Own,
}

impl RunCache {
    pub(crate) async fn rebuild(&self) {
        refresh(&self.inner, &self.cache).await;
    }

    /// The harnesses' own server names as just read (the pane's view, an import): what the next run skips.
    pub(crate) fn set_own(&self, own: BTreeMap<String, BTreeSet<String>>) {
        *self.own.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = own;
    }
}

pub(crate) fn register(s: &mut Setup) -> (Arc<StoredKind>, Arc<dyn ResourceKind>, RunCache) {
    let inner = Arc::new(StoredKind::new(
        KindInfo::new("connector", "connectors", "wisp.connectors", schema_of::<ConnectorSpec>())
            .order(18)
            .names(wisp_resources_api::Names::new("agents[]", "agent")),
        s.ctx(),
    ));
    let cache: Cache = Arc::default();
    let k: Arc<dyn ResourceKind> = Arc::new(ConnectorKind { inner: inner.clone(), cache: cache.clone() });
    s.contribute(&RESOURCE_KIND, k.clone());
    let own: Own = Arc::default();
    let out = (inner.clone(), k, RunCache { inner: inner.clone(), cache: cache.clone(), own: own.clone() });
    let revealed: Revealed = Arc::default();
    let runs: Arc<dyn RunPart> =
        Arc::new(Runs { cache: cache.clone(), revealed: revealed.clone(), own, no_sse: wisp_connectors_core::harness::without_sse() });
    s.contribute(&RUN, runs);
    s.intercept::<wisp_runtime_api::hooks::BeforeRun, _, _>(
        wisp_kernel::InterceptOpts { priority: 60, failure: wisp_kernel::Failure::Open, timeout_ms: 2000 },
        move |ctx, call, c| {
            let revealed = revealed.clone();
            async move {
                reveal_overlay(&ctx, &call, &revealed, c.session_id.as_str()).await;
                wisp_kernel::Verdict::Continue
            }
        },
    );
    s.on_start(move |_ctx| async move {
        refresh(&inner, &cache).await;
        Ok(())
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_gets_a_connector() {
        let mut c = ConnectorSpec { url: "https://mcp.linear.app/sse".into(), agents: vec!["reviewer".into()], ..ConnectorSpec::default() };
        assert!(gets("linear", &c, "reviewer", "edits", &[]));
        assert!(gets("linear", &c, "reviewer", "auto", &[]));
        assert!(!gets("linear", &c, "fixer", "edits", &[]));
        assert!(gets("linear", &c, "fixer", "edits", &["linear".into()]), "the task names it");
        // Plan mode: none unless its modes name plan, a task naming it included
        assert!(!gets("linear", &c, "reviewer", "plan", &[]));
        assert!(!gets("linear", &c, "fixer", "plan", &["linear".into()]), "naming it doesn't open Plan mode");
        let plan = ConnectorSpec { modes: vec!["plan".into()], ..c.clone() };
        assert!(gets("linear", &plan, "reviewer", "plan", &[]));
        assert!(!gets("linear", &plan, "reviewer", "edits", &[]), "modes: [plan] is for plan only");
        c.enabled = Some(false);
        assert!(!gets("linear", &c, "reviewer", "edits", &[]));
        assert_eq!(
            server_of(&ConnectorSpec { command: "npx".into(), args: vec!["x".into()], ..ConnectorSpec::default() })["type"],
            "stdio"
        );
    }

    /// SSE was lost on the way: every remote connector went to a harness as `"type": "http"`.
    #[test]
    fn an_sse_connector_goes_as_sse_and_an_old_spec_stays_http() {
        let old: ConnectorSpec = serde_json::from_value(json!({ "url": "https://a.example/mcp" })).unwrap();
        assert_eq!(server_of(&old)["type"], "http", "no transport: streamable HTTP");
        let sse: ConnectorSpec = serde_json::from_value(json!({ "url": "https://a.example/sse", "transport": "sse" })).unwrap();
        assert_eq!(server_of(&sse)["type"], "sse");
        assert_eq!(serde_json::to_value(&sse).unwrap()["transport"], "sse");
    }

    fn runs(own: &[(&str, &[&str])], list: Vec<(String, ConnectorSpec)>) -> Runs {
        let own = own.iter().map(|(h, ns)| ((*h).to_owned(), ns.iter().map(|n| (*n).to_owned()).collect())).collect();
        Runs {
            cache: Arc::new(Mutex::new(list)),
            revealed: Arc::default(),
            own: Arc::new(Mutex::new(own)),
            no_sse: wisp_connectors_core::harness::without_sse(),
        }
    }

    fn names(r: &Runs, harness: &str) -> Vec<String> {
        let q = McpQuery {
            mode: wisp_runtime_api::model::Mode::Edits,
            harness: harness.into(),
            team: Vec::new(),
            turn_id: String::new(),
            sub: false,
        };
        r.mcp_servers(&Session::default(), &q).into_keys().collect()
    }

    /// A harness whose own config already has a server by a connector's name got both (Codex merged them into one
    /// broken server, Claude listed every tool twice): it keeps its own, the others get the connector.
    #[test]
    fn a_harness_with_its_own_server_by_that_name_keeps_its_own() {
        let linear = ("linear".to_owned(), ConnectorSpec { url: "https://mcp.linear.app/mcp".into(), ..ConnectorSpec::default() });
        let r = runs(&[("codex", &["linear"])], vec![linear]);
        assert_eq!(names(&r, "codex"), Vec::<String>::new());
        assert_eq!(names(&r, "claude"), ["linear"]);
    }

    /// Codex speaks stdio and streamable HTTP only: an SSE connector is left out for it, kept for the others.
    #[test]
    fn codex_gets_no_sse_connector() {
        let sse = ConnectorSpec { url: "https://a.example/sse".into(), transport: Some(RemoteTransport::Sse), ..ConnectorSpec::default() };
        let http = ConnectorSpec { url: "https://b.example/mcp".into(), ..ConnectorSpec::default() };
        let r = runs(&[], vec![("old".into(), sse), ("new".into(), http)]);
        assert_eq!(names(&r, "codex"), ["new"]);
        assert_eq!(names(&r, "claude"), ["new", "old"]);
    }

    /// Claude and Codex trust wisp's own servers by name (`--allowedTools mcp__browser`, `default_tools_approval_mode`):
    /// a connector stored (or carried by a formula) as `browser` or `computer` would run unasked. It never reaches a run.
    #[test]
    fn a_connector_by_the_name_of_wisps_own_server_never_reaches_a_run() {
        let one = |n: &str| (n.to_owned(), ConnectorSpec { url: format!("https://{n}.example/mcp"), ..ConnectorSpec::default() });
        let r = runs(&[], vec![one("browser"), one("computer"), one("wisp"), one("linear")]);
        assert_eq!(names(&r, "claude"), ["linear"]);
        assert_eq!(names(&r, "codex"), ["linear"]);
        assert!(reserved("browser") && reserved("computer") && reserved("wisp") && !reserved("linear"));
    }

    fn from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let m: std::collections::HashMap<String, String> = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        move |n| m.get(n).cloned()
    }

    /// `reveal` re-read the text from the start after each value it put in: a secret holding its own reference looped
    /// for good, one holding another's was read again, and any name was read (another connector's token, a plugin's own
    /// secret), so a header could carry it to the connector's server. Once, left to right, its own and a formula's only.
    #[test]
    fn reveal_expands_once_left_to_right_and_only_what_the_connector_may_read() {
        let own = "connector:linear:headers:Authorization";
        let own_ref = format!("${{secret:{own}}}");
        // a value holding its own reference: it ends, the value as it is
        let got = expand("linear", &own_ref, &from(&[(own, own_ref.as_str())]));
        assert_eq!(got, own_ref);
        // a value holding another reference isn't read again
        let got = expand("linear", &format!("Bearer {own_ref}"), &from(&[(own, "${secret:LINEAR_KEY}"), ("LINEAR_KEY", "lin-1")]));
        assert_eq!(got, "Bearer ${secret:LINEAR_KEY}");
        // another connector's token, a plugin's own secret: left as written
        let all = from(&[("connector:github:token", "gho-1"), ("channels/sign", "k"), (own, "tok-1"), ("LINEAR_KEY", "lin-1")]);
        assert_eq!(expand("linear", "${secret:connector:github:token}", &all), "${secret:connector:github:token}");
        assert_eq!(expand("linear", "${secret:channels/sign}", &all), "${secret:channels/sign}");
        assert_eq!(expand("linearx", &own_ref, &all), own_ref, "a namespace that only starts the same isn't its own");
        // its own and a formula's (`wisp secret set`), each in its place; an unset one stays
        assert_eq!(
            expand("linear", &format!("a {own_ref} b ${{secret:LINEAR_KEY}} c ${{secret:NOPE}}"), &all),
            "a tok-1 b lin-1 c ${secret:NOPE}"
        );
        assert_eq!(expand("linear", "no ref ${secret:open", &all), "no ref ${secret:open");
        let twice = format!("{own_ref}${{secret:connector:github:token}}{own_ref}");
        assert_eq!(refs("linear", &twice), vec![own.to_owned()]);
    }

    /// A sign-in is for its URL's origin; its token goes over https, or plain http to this computer only.
    #[test]
    fn a_sign_in_is_for_its_origin_and_its_token_never_goes_in_the_clear_to_another_host() {
        assert_eq!(origin("https://MCP.Linear.app:443/mcp?x=1"), "https://mcp.linear.app");
        assert_eq!(origin("https://mcp.linear.app/sse"), origin("https://mcp.linear.app/mcp"));
        assert_ne!(origin("https://mcp.linear.app/mcp"), origin("https://evil.example/mcp"));
        assert_ne!(origin("https://a.example/mcp"), origin("http://a.example/mcp"));
        assert_ne!(origin("http://127.0.0.1:1/mcp"), origin("http://127.0.0.1:2/mcp"));
        assert_ne!(origin("https://a.example/mcp"), origin(""));
        for ok in ["https://a.example/mcp", "http://127.0.0.1:9/mcp", "http://localhost:9/mcp", "http://[::1]:9/mcp", "http://127.1.2.3/"] {
            assert!(bearer_ok(ok), "{ok}");
        }
        for no in ["http://example.com/mcp", "http://10.0.0.2:9/mcp", "http://localhost.evil.example/mcp", "ftp://x/", ""] {
            assert!(!bearer_ok(no), "{no}");
        }
    }
}
