//! wisp.automations' `trigger` kind (ADR-0028, ADR-0043): the only declaration of a trigger. Kept as declared
//! (`StoredKind`); sensitive (it acts without you). Its spec is `wisp_automations_api::TriggerSpec`, checked at plan
//! (`spec::check`), and every change reloads the engine. The rest of the wiring is here too: the webhook route,
//! the durable subscription, the commands and the tick.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_automations_api::commands as c;
use wisp_automations_api::{Target, TriggerEvent, TriggerSpec};
use wisp_kernel::{BoxFuture, Call, Ctx, Result, Setup};
use wisp_resources_api::{KindInfo, Op, RESOURCE_KIND, Resource, ResourceKind, StoredKind, schema_of};

use crate::engine::{Engine, Shared, from_envelope, outcome_of};

/// A webhook's body is capped at 1 MB (413 above it, ADR-0043 edge case 11).
pub const WEBHOOK_MAX: usize = 1024 * 1024;

struct TriggerKind {
    inner: Arc<StoredKind>,
    engine: Shared,
    ctx: Ctx,
}

/// Loads the declared triggers into the engine, a `project: proj` read as that project's folder: the engine compares
/// folders and starts its task in one. The spec stays as written (audit G11: kept as its folder, `wisp diff` right
/// after `apply` said "1 to change"). One at a time (a slow load never writes over a newer one); a name it couldn't read
/// (the projects weren't there yet at start, or one went) marks the engine `unread`, and the tick loads again.
async fn refresh(ctx: &Ctx, inner: &StoredKind, engine: &Engine) {
    let _one = engine.loading.lock().await;
    let mut specs = inner.specs().await.unwrap_or_default();
    let mut read = true;
    if specs.iter().any(|(_, v)| project_name(v).is_some()) {
        let folders = project_folders(ctx, &ctx.root_call()).await.unwrap_or_default();
        for (_, v) in &mut specs {
            let Some(name) = project_name(v) else { continue };
            match folders.iter().find(|(n, _)| n == name) {
                Some((_, folder)) => v["project"] = json!(folder),
                None => read = false,
            }
        }
    }
    let list = specs.into_iter().filter_map(|(n, v)| Some((n, serde_json::from_value::<TriggerSpec>(v).ok()?))).collect();
    *engine.declared.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = list;
    engine.unread.store(!read, std::sync::atomic::Ordering::Relaxed);
}

/// An `on` no source offers (no plugin's, on or off): refused, naming the sources (audit G11: `on: "every minute"` was
/// stored "unavailable" and never fired). A glob (`pr.*`) is fine when it takes one of them.
pub(crate) fn no_source(on: &str, sources: &[wisp_automations_api::commands::SourceRow]) -> Option<String> {
    let on = on.trim();
    if on.is_empty() || sources.iter().any(|r| crate::engine::takes(on, &r.on.on) || crate::engine::takes(&r.on.on, on)) {
        return None;
    }
    let mut names: Vec<&str> = sources.iter().map(|r| r.on.on.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let more = names.len().saturating_sub(12);
    let list = names.into_iter().take(12).collect::<Vec<_>>().join(", ");
    let list = if more > 0 { format!("{list} and {more} more (wisp trigger sources)") } else { list };
    Some(format!("on: {on} is no source (a schedule is on: schedule with cron: \"every minute\"); sources: {list}"))
}

impl ResourceKind for TriggerKind {
    fn info(&self) -> KindInfo {
        self.inner.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.inner.read(call)
    }
    /// Its project's folder spelled as a task's is (ADR-0071: an 8.3 short name spelled out), so a trigger on
    /// `C:\Users\RUNNER~1\…\proj` hears the tasks in `C:\Users\runneradmin\…\proj` (CI 8bb8ccd0: `trigger:after`
    /// never fired). Its `match.project` too.
    fn normalize(&self, spec: &mut Value) {
        spell_project(spec);
    }
    fn check(&self, r: &Resource) -> Vec<String> {
        match serde_json::from_value::<TriggerSpec>(r.spec.clone()) {
            Ok(s) => {
                let mut out = wisp_automations_api::spec::check(&s);
                out.extend(no_source(&s.on, &crate::engine::sources(&self.ctx)));
                out
            }
            Err(e) => vec![format!("not a trigger: {e}")],
        }
    }
    fn describe(&self, op: &Op) -> Option<String> {
        let s: TriggerSpec = serde_json::from_value(op.spec.clone()).unwrap_or_default();
        let what = match s.target() {
            Target::Task(t) => format!("starts the task {t}"),
            Target::Prompt(_) => "starts a task".to_owned(),
            Target::Crew(m) => format!("wakes {m}"),
            Target::Workflow(w) => format!("runs the workflow {w}"),
            Target::FollowUp => "follows up the event's task".to_owned(),
        };
        Some(format!("{what} by itself on {} ({})", s.on, op.name))
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if op.op != wisp_resources_api::OpKind::Delete {
                project_named(&self.ctx, call, &op.spec).await?;
            }
            self.inner.apply(call, op).await?;
            refresh(&self.ctx, &self.inner, &self.engine).await;
            Ok(())
        })
    }
}

/// `project: api` names a declared project rather than a folder (no `/`, `\`, `:` or `~`), as a formula names one:
/// the engine reads it as that project's folder ([`refresh`]). A name no project has is refused at apply: it would
/// have matched no task and started none, saying nothing (audit A #14).
pub(crate) fn project_name(spec: &Value) -> Option<&str> {
    let p = spec.get("project")?.as_str()?.trim();
    (!p.is_empty() && p != "." && p != ".." && !p.starts_with('~') && !p.contains(['/', '\\', ':'])).then_some(p)
}

/// Each declared project's name and folder.
async fn project_folders(ctx: &Ctx, call: &Call) -> Result<Vec<(String, String)>> {
    let args = wisp_resources_api::ExportArgs { kinds: vec!["project".into()], include: Vec::new() };
    let projects = ctx.run::<wisp_resources_api::Export>(call, args).await?;
    Ok(projects
        .into_iter()
        .filter(|r| r.kind == "project")
        .filter_map(|r| Some((r.name, r.spec.get("path")?.as_str()?.to_owned())))
        .collect())
}

/// A `project` given by name names a declared project.
async fn project_named(ctx: &Ctx, call: &Call, spec: &Value) -> Result<()> {
    let Some(name) = project_name(spec) else { return Ok(()) };
    let projects = project_folders(ctx, call).await?;
    if projects.iter().any(|(n, _)| n == name) {
        return Ok(());
    }
    let known: Vec<&str> = projects.iter().map(|(n, _)| n.as_str()).collect();
    let hint = if known.is_empty() {
        "there are no projects yet: give its folder".to_owned()
    } else {
        format!("give its folder, or one of {}", known.join(", "))
    };
    Err(wisp_kernel::WireError::invalid(format!("project: no project is named {name:?}; {hint}")))
}

/// `${secret:NAME}` read from secrets; a literal stays as it is.
async fn secret_of(ctx: &Ctx, v: &str) -> Option<String> {
    let name = v.strip_prefix("${secret:").and_then(|r| r.strip_suffix('}'));
    match name {
        Some(n) => wisp_kernel::services::secrets(ctx).ok()?.get(n).await.ok().flatten(),
        None => Some(v.to_owned()).filter(|v| !v.is_empty()),
    }
}

/// Equal in constant time: how long it takes says nothing about how much of the secret was right.
pub fn same(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq as _;
    // ct_eq on unequal lengths answers 0 without comparing; compare against itself so the time doesn't depend on it
    let eq_len = a.len() == b.len();
    let other = if eq_len { b.as_bytes() } else { a.as_bytes() };
    bool::from(a.as_bytes().ct_eq(other)) && eq_len
}

/// v1 hooks.js:152: inbound webhooks are off until switched on at this computer (Settings → Automations); off,
/// every webhook answers the same 404 as one that isn't there.
pub const WEBHOOKS: &str = "automations.webhookTriggers";

/// What a webhook call gets, before it reaches the engine: 413 over the cap; the same 404 for a wrong or
/// missing secret, an unknown or off trigger, and one with no secret (an open route: the secret is the only key).
pub fn webhook_verdict(body_len: usize, want: Option<&str>, given: &str) -> Option<u16> {
    if body_len > WEBHOOK_MAX {
        return Some(413);
    }
    match want {
        Some(w) if same(given, w) => None,
        _ => Some(404),
    }
}

/// A signed call (a spec with `header`, v1 hooks.js `hmacOk`): the header holds the HMAC-SHA256 of the raw body under
/// the secret, hex, bare or after `sha256=` (GitHub, Linear, Sentry) or `v1=` (`PagerDuty`, which sends several at once,
/// comma-separated, while it rotates secrets: any one will do).
pub fn signed(secret: &str, body: &[u8], got: &str) -> bool {
    use hmac::Mac as _;
    let Ok(mut mac) = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(body);
    let want = mac.finalize().into_bytes().iter().fold(String::with_capacity(64), |mut out, b| {
        out.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
        out
    });
    got.split(',').any(|x| {
        let x = x.trim();
        let hex = x.split_once('=').filter(|(p, _)| p.eq_ignore_ascii_case("sha256") || p.eq_ignore_ascii_case("v1")).map_or(x, |(_, h)| h);
        !hex.is_empty() && same(&hex.to_ascii_lowercase(), &want)
    })
}

/// The secret a call gave: signed, the header's signature checked (so `given` is the secret itself when it holds);
/// unsigned, `x-wisp-secret`, a bearer token or `?secret=`.
fn given_of(req: &wisp_kernel::http::Request, header: &str, want: Option<&str>) -> String {
    if !header.is_empty() {
        return match (want, req.header(header)) {
            (Some(w), Some(got)) if signed(w, &req.body, got) => w.to_owned(),
            _ => String::new(),
        };
    }
    req.header("x-wisp-secret")
        .map(str::to_owned)
        .or_else(|| req.header("authorization").and_then(|a| a.strip_prefix("Bearer ")).map(str::to_owned))
        .or_else(|| req.query("secret").map(str::to_owned))
        .unwrap_or_default()
}

fn answer(status: u16, body: Value) -> wisp_kernel::http::Response {
    wisp_kernel::http::Response::Json { status, body }
}

/// `POST /api/triggers/:name`: the body is the event's payload (JSON, or `{text}`); `x-wisp-delivery` or
/// `x-github-delivery` is its dedupe key.
async fn webhook(ctx: Ctx, engine: Shared, req: wisp_kernel::http::Request) -> Result<wisp_kernel::http::Response> {
    if ctx.setting(WEBHOOKS).and_then(|v| v.as_bool()) != Some(true) {
        return Ok(answer(404, json!({ "error": "no such trigger" })));
    }
    let name = req.param("name").unwrap_or_default().to_owned();
    let t = engine.all(&ctx).into_iter().find(|t| t.name == name && t.spec.on == "webhook" && t.spec.on());
    let want = match &t {
        Some(t) => secret_of(&ctx, &t.spec.secret).await,
        None => None,
    };
    let header = t.as_ref().map(|t| t.spec.header.clone()).unwrap_or_default();
    if header.is_empty() && req.body.len() <= WEBHOOK_MAX {
        // the same hashing as a signed trigger's, so how long a call takes doesn't say which names are signed triggers
        std::hint::black_box(signed("", std::hint::black_box(&req.body), ""));
    }
    let given = if req.body.len() > WEBHOOK_MAX { String::new() } else { given_of(&req, &header, want.as_deref()) };
    if let Some(status) = webhook_verdict(req.body.len(), want.as_deref(), &given) {
        let msg = if status == 413 { "the body is over 1 MB" } else { "no such trigger" };
        return Ok(answer(status, json!({ "error": msg })));
    }
    let now = ctx.clock()?.now_ms();
    let payload = serde_json::from_slice::<Value>(&req.body)
        .unwrap_or_else(|_| json!({ "text": String::from_utf8_lossy(&req.body).chars().take(4000).collect::<String>() }));
    let key =
        req.header("x-wisp-delivery").or_else(|| req.header("x-github-delivery")).map_or_else(|| format!("webhook:{now}"), str::to_owned);
    let summary = payload
        .get("summary")
        .or_else(|| payload.get("text"))
        .and_then(Value::as_str)
        .map_or_else(|| format!("{name} was called"), |s| s.chars().take(160).collect());
    let ev = TriggerEvent { on: "webhook".into(), key, at: now, payload, summary, ..TriggerEvent::default() };
    let call = ctx.root_call();
    let out = engine.dispatch(&ctx, &call, &ev, Some(&name), now).await;
    let o = out.into_iter().next().map_or_else(
        || c::FireOut { result: "nothing".into(), why: "not taken: seen before, or not what it matches".into(), session: None },
        |(_, o)| o,
    );
    Ok(answer(202, json!({ "result": o.result, "why": o.why, "id": o.session })))
}

/// A durable event: every trigger on its type decides; a run a trigger started that ended counts.
async fn on_event(ctx: Ctx, engine: Shared, env: Arc<wisp_kernel::EventEnvelope>) -> Result<()> {
    if env.ty == "event.gap" {
        return Ok(());
    }
    let now = ctx.clock()?.now_ms();
    let data = env.data();
    match env.ty.as_str() {
        "task.finished" | "task.failed" => {
            if let Some(id) = &env.subject {
                engine.ended(&ctx, id, outcome_of(&env.ty, &data), false, now).await;
            }
        }
        "turn.held" => {
            if let Some(id) = &env.subject {
                let s = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                engine.plan_held(&ctx, id, &s("by"), &s("kind"), now).await;
            }
        }
        "crew.reported" => {
            if let Some(id) = data.get("sessionId").and_then(Value::as_str) {
                engine.ended(&ctx, id, outcome_of(&env.ty, &data), true, now).await;
            }
        }
        _ => {}
    }
    let all = engine.all(&ctx);
    if !all.iter().any(|t| core_glob(&t.spec.on, &env.ty)) {
        return Ok(());
    }
    let call = ctx.root_call();
    // a trigger never fires on its own: what started the task the event is about, and how deep that chain is
    let (by, depth) = lineage(&ctx, &call, &engine, &env, &data).await;
    let ev = from_envelope(&env, by, depth);
    engine.dispatch(&ctx, &call, &ev, None, now).await;
    Ok(())
}

fn core_glob(on: &str, ty: &str) -> bool {
    wisp_automations_core::engine::glob(on, ty)
}

async fn lineage(ctx: &Ctx, call: &Call, engine: &Engine, env: &wisp_kernel::EventEnvelope, data: &Value) -> (Option<String>, u32) {
    if env.ty == "trigger.fired" {
        return (data.get("trigger").and_then(Value::as_str).map(|t| format!("trigger:{t}")), 0);
    }
    if let Some(b) = data.get("by").and_then(Value::as_str) {
        return (Some(b.to_owned()), 0);
    }
    let Some(subject) = &env.subject else { return (None, 0) };
    let spec =
        ctx.run::<wisp_runtime_api::commands::SpecOf>(call, wisp_runtime_api::commands::IdArgs { id: subject.as_str().into() }).await;
    let by = spec.ok().and_then(|s| s.lineage.triggered_by);
    let depth = engine.states().await.iter().filter_map(|(_, st)| st.depths.get(subject).copied()).max().unwrap_or(0);
    (by, depth)
}

fn now_of(ctx: &Ctx) -> Result<i64> {
    Ok(ctx.clock()?.now_ms())
}

fn commands(s: &mut Setup, engine: &Shared) {
    let e = engine.clone();
    s.command::<c::TriggerLs, _, _>(move |ctx, _, _| {
        let e = e.clone();
        async move { Ok(e.rows(&ctx).await) }
    });
    let e = engine.clone();
    s.command::<c::TriggerFire, _, _>(move |ctx, call, a| {
        let e = e.clone();
        async move { e.fire(&ctx, &call, &a.name, a.payload, a.summary, now_of(&ctx)?).await }
    });
    let e = engine.clone();
    s.command::<c::TriggerTest, _, _>(move |ctx, call, a| {
        let e = e.clone();
        async move { e.test(&ctx, &call, &a.name, a.event, a.last.unwrap_or(5), now_of(&ctx)?).await }
    });
    let e = engine.clone();
    s.command::<c::TriggerLog, _, _>(move |ctx, _, a| {
        let e = e.clone();
        async move {
            e.find(&ctx, &a.name)?;
            Ok(e.log(&a.name).await)
        }
    });
    let e = engine.clone();
    s.command::<c::TriggerPush, _, _>(move |ctx, call, ev| {
        let e = e.clone();
        async move {
            let now = now_of(&ctx)?;
            let out = e.dispatch(&ctx, &call, &ev, None, now).await;
            mentions(&ctx, &call, &ev).await;
            Ok(c::Count { n: u32::try_from(out.iter().filter(|(_, o)| o.result == "started").count()).unwrap_or(u32::MAX) })
        }
    });
    let e = engine.clone();
    s.command::<c::TriggerApprove, _, _>(move |ctx, call, a| {
        let e = e.clone();
        async move { e.approve(&ctx, &call, &a.name, &a.job, a.yes, now_of(&ctx)?).await }
    });
    let e = engine.clone();
    s.command::<c::TriggerResume, _, _>(move |ctx, _, a| {
        let e = e.clone();
        async move {
            e.resume(&ctx, &a.name, now_of(&ctx)?).await?;
            Ok(c::Done { ok: true })
        }
    });
    s.command::<c::Sources, _, _>(|ctx, _, _| async move { Ok(crate::engine::sources(&ctx)) });
    let e = engine.clone();
    s.command::<c::Release, _, _>(move |_, _, a| {
        let e = e.clone();
        async move { Ok(c::Count { n: e.release(&a.agent, &a.account).await }) }
    });
    // an early reset freed the plan (wisp.budget, v1 app.resetEarly): rows skipped for it stop saying so
    let e = engine.clone();
    s.on::<wisp_budget_api::PlanFree, _, _>(move |_, p| {
        let e = e.clone();
        async move {
            e.release(&p.agent, &p.account).await;
        }
    });
}

/// `@member` in a pushed message wakes that member (ADR-0043 §5). A chat message only from someone the channel
/// knows (`trusted`): an open channel can't wake your crew.
async fn mentions(ctx: &Ctx, call: &Call, ev: &TriggerEvent) {
    let Some(text) = ev.payload.get("text").and_then(Value::as_str).filter(|t| t.contains('@')) else { return };
    if ev.on == "chat.message" && ev.payload.get("trusted").and_then(Value::as_bool) != Some(true) {
        return;
    }
    if let Err(e) = ctx
        .run::<wisp_crew_api::commands::WakeMentions>(
            call,
            wisp_crew_api::commands::MentionArgs { text: text.to_owned(), source: Some(ev.on.clone()) },
        )
        .await
    {
        tracing::debug!("mentions not sent: {}", e.message);
    }
}

pub fn register(s: &mut Setup, engine: &Shared) {
    let inner = Arc::new(StoredKind::new(
        KindInfo::new("trigger", "triggers", "wisp.automations", schema_of::<TriggerSpec>())
            .order(85)
            .sensitive()
            .names(wisp_resources_api::Names::new("task", "task"))
            .names(wisp_resources_api::Names::new("workflow", "workflow"))
            .names(wisp_resources_api::Names::new("crew", "crew"))
            // its `project` is checked by the trigger's own apply, in its own words (audit A #14)
            .names(wisp_resources_api::Names::new("agent", "agent"))
            .names(wisp_resources_api::Names::new("factory", "factory")),
        s.ctx(),
    ));
    let k: Arc<dyn ResourceKind> = Arc::new(TriggerKind { inner: inner.clone(), engine: engine.clone(), ctx: s.ctx() });
    s.contribute(&RESOURCE_KIND, k);
    let e = engine.clone();
    s.on_durable_any("engine", "*", move |ctx, _call, env| on_event(ctx, e.clone(), env));
    let hooks = engine.clone();
    s.route(wisp_kernel::http::Route {
        method: "POST",
        pattern: "/api/triggers/:name",
        permission: wisp_kernel::http::OPEN,
        doc: true,
        summary: "Send a webhook trigger an event (its secret as x-wisp-secret; at most 1 MB)",
        raw: true,
        max_body: Some(WEBHOOK_MAX),
        handler: Arc::new(move |ctx, _call, req| {
            let e = hooks.clone();
            Box::pin(async move { webhook(ctx, e, req).await })
        }),
    });
    commands(s, engine);
    let e = engine.clone();
    s.on_start(move |ctx| async move {
        refresh(&ctx, &inner, &e).await;
        let clock = ctx.clock()?;
        let c2 = ctx.clone();
        ctx.spawn(async move {
            loop {
                // every 15 s: a schedule's minute is never missed, a debounce closes within 15 s of its window
                clock.sleep(15_000).await;
                if e.unread.load(std::sync::atomic::Ordering::Relaxed) {
                    refresh(&c2, &inner, &e).await;
                }
                e.tick(&c2, clock.now_ms()).await;
            }
        });
        Ok(())
    });
}

/// A trigger spec's `project` and `match.project` (one folder or a list) as `wisp_proc::plain_path` spells them.
fn spell_project(spec: &mut Value) {
    let spell = |v: &mut Value| match v {
        Value::String(p) if !p.trim().is_empty() => *p = wisp_proc::plain_str_blocking(p),
        Value::Array(list) => {
            for x in list {
                if let Value::String(p) = x {
                    *p = wisp_proc::plain_str_blocking(p);
                }
            }
        }
        _ => {}
    };
    if let Some(p) = spec.get_mut("project") {
        spell(p);
    }
    if let Some(p) = spec.get_mut("match").and_then(|m| m.get_mut("project")) {
        spell(p);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Audit G11: `on: "every minute"` (no source) passed validate and apply, was stored "unavailable" and never fired.
    #[test]
    fn an_on_that_names_no_source_is_refused_naming_the_sources_audit_g11() {
        let row = |on: &str| wisp_automations_api::commands::SourceRow {
            plugin: "wisp.automations".into(),
            available: true,
            on: wisp_automations_api::SourceOn { title: on.into(), on: on.into(), fields: json!({}), mode: "event".into(), every: None },
        };
        let list = [row("schedule"), row("pr.opened"), row("task.finished")];
        let no = no_source("every minute", &list).unwrap();
        assert!(no.starts_with("on: every minute is no source") && no.contains("pr.opened, schedule, task.finished"), "{no}");
        for ok in ["schedule", "pr.*", "task.finished", ""] {
            assert_eq!(no_source(ok, &list), None, "{ok}");
        }
    }

    #[test]
    fn a_project_given_by_its_name_is_told_from_a_folder_audit_a_14() {
        assert_eq!(project_name(&json!({ "project": "api" })), Some("api"));
        assert_eq!(project_name(&json!({ "project": " my-app " })), Some("my-app"));
        for folder in ["/w/api", "~/api", "~", "C:\\w\\api", "./api", ".", "..", "", "https://x/api"] {
            assert_eq!(project_name(&json!({ "project": folder })), None, "{folder} is a folder, not a name");
        }
        assert_eq!(project_name(&json!({})), None);
        assert_eq!(project_name(&json!({ "project": ["a", "b"] })), None);
    }

    #[test]
    fn webhook_secret_compared_constant_time_and_body_capped() {
        assert_eq!(webhook_verdict(10, Some("s3cret"), "s3cret"), None);
        assert_eq!(webhook_verdict(10, Some("s3cret"), "s3creT"), Some(404));
        assert_eq!(webhook_verdict(10, Some("s3cret"), "s3cre"), Some(404), "a prefix isn't the secret");
        assert_eq!(webhook_verdict(10, Some("s3cret"), ""), Some(404));
        assert_eq!(webhook_verdict(10, None, ""), Some(404), "no secret: the same 404 as a wrong one");
        assert_eq!(webhook_verdict(WEBHOOK_MAX, Some("s"), "s"), None, "1 MB exactly is taken");
        assert_eq!(webhook_verdict(WEBHOOK_MAX + 1, Some("s"), "s"), Some(413));
    }

    #[test]
    fn a_signed_webhook_takes_github_linear_and_pagerduty_signatures_and_nothing_else_v1_hmac_ok() {
        // HMAC-SHA256("It's a Secret to Everybody", "Hello, World!") from GitHub's webhook docs
        let (secret, body) = ("It's a Secret to Everybody", b"Hello, World!");
        let hex = "757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
        assert!(signed(secret, body, &format!("sha256={hex}")), "GitHub's sha256=");
        assert!(signed(secret, body, hex), "Linear's bare hex");
        assert!(signed(secret, body, &hex.to_uppercase()), "hex in capitals");
        assert!(signed(secret, body, &format!("v1=deadbeef, v1={hex}")), "PagerDuty: any of the ones it sends");
        assert!(!signed(secret, body, &format!("sha256={}", &hex[..63])), "a prefix isn't the signature");
        assert!(!signed(secret, b"Hello, World?", &format!("sha256={hex}")), "another body");
        assert!(!signed("another secret", body, hex));
        assert!(!signed(secret, body, ""));
        assert!(!signed(secret, body, "sha256="));
        assert!(!signed(secret, body, &format!("md5={hex}")), "another scheme isn't read as the hex");
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("abc", "abcd") && !same("", "a"));
        // the route says the same cap, so the edge refuses a bigger body (413) before reading it all
        assert_eq!(WEBHOOK_MAX, 1_048_576);
    }
}
