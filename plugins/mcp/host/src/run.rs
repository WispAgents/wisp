//! A message's tool calls, run: a built-in as its owner's command (by name, through the kernel, as the
//! caller), a contributed tool through its own function. v1's words for each answer come from
//! `wisp-mcp-core`.

use serde_json::{Map, Value, json};
use wisp_kernel::{Call, Command, Ctx};
use wisp_mcp_core::{AgentAction, Outcome, Step, Who, agent_action, owner_text, user, words};
use wisp_runtime_api::commands as rt;

use crate::{Sessions, extras_for, who as who_of};

/// How often a waiting tool looks again (an ask, a task to finish).
const POLL_MS: u64 = 250;
const ASK_WAIT_MS: i64 = 30 * 60_000;

/// One body in, its answer out (`None`: only notifications).
pub async fn rpc(ctx: &Ctx, call: &Call, sessions: &Sessions, text: &str, cwd: Option<String>) -> Option<String> {
    let mut w = who_of(ctx, call, sessions);
    let mut only = None;
    if let Who::Agent { session, info } = &mut w {
        merge_flags(ctx, call, session, info).await;
        only = narrowed(ctx, call, sessions, session).await;
    }
    let (ex, contributed) = extras_for(ctx, call);
    let methods = ctx.contributions(&wisp_mcp_api::METHOD_POINT);
    let plan = wisp_mcp_core::plan_served(text, &w, &ex, &served(&methods));
    let auto_off = crate::auto_off(ctx);
    let mut replies = Vec::with_capacity(plan.steps.len());
    for step in plan.steps {
        replies.push(match step {
            Step::Reply(r) => {
                // Auto off: no harness list offers `auto` (audit H HS7)
                let r = if auto_off { wisp_mcp_core::table::without_auto(&r).into_owned() } else { r };
                Some(only.as_ref().map_or_else(|| r.clone(), |o| crate::kinds::filter_list(&r, o)))
            }
            Step::Silent => None,
            Step::Method { id, method, params } => Some(method_reply(ctx, call, &methods, &id, &method, params).await),
            Step::Call(c) if only.as_ref().is_some_and(|o| !o.allows(&c.name)) => Some(wisp_mcp_core::finish(
                &c.id,
                &c.name,
                &w,
                Outcome::Failed(format!("{} isn't one of this agent's tools (tools: in its formula)", c.name)),
            )),
            // the task asking the user about a guard their harness wants lowered: the harness reads it, never answers
            // or acts on it (ADR-0115)
            Step::Call(c)
                if matches!(w, Who::User { .. })
                    && crate::names_guard_task(&c.args)
                    && !(matches!(c.name.as_str(), "wisp_get" | "wisp_list" | "wisp_wait")
                        || c.extra.and_then(|i| contributed.get(i)).is_some_and(|t| t.read_only)) =>
            {
                Some(wisp_mcp_core::finish(&c.id, &c.name, &w, Outcome::Failed(wisp_mcp_core::guard::NOT_YOURS.into())))
            }
            Step::Call(c) => {
                let outcome = if let Some(t) = c.extra.and_then(|i| contributed.get(i)) {
                    let tc = wisp_mcp_api::ToolCall { args: Value::Object(c.args), session: call.caller().session.clone() };
                    match (t.run)(ctx.clone(), call.clone(), tc).await {
                        Ok(t) if t.error.is_some() => Outcome::Failed(t.error.unwrap_or_default()),
                        Ok(t) => Outcome::Text(t.text.unwrap_or_default()),
                        Err(e) => Outcome::Failed(e.message),
                    }
                } else {
                    let fact = wisp_mcp_core::telemetry::of(&c.name, &c.args, matches!(w, Who::Agent { .. }));
                    let out = builtin(ctx, call, &w, &c.name, c.args, cwd.as_deref()).await;
                    if let Some((event, props)) = fact.filter(|_| !matches!(out, Outcome::Failed(_))) {
                        track(ctx, call, &w, event, props).await;
                    }
                    out
                };
                Some(wisp_mcp_core::finish(&c.id, &c.name, &w, outcome))
            }
        });
    }
    let replies = if plan.modern { replies.into_iter().map(|r| r.map(|r| wisp_mcp_core::rpc::completed(&r))).collect() } else { replies };
    wisp_mcp_core::assemble(replies, plan.batch)
}

/// What the dispatcher hands over (`mcp.method@1`, ADR-0145): each contributed method once (the first plugin to name
/// one has it), and the extensions they declare.
fn served(methods: &[std::sync::Arc<wisp_mcp_api::MethodContribution>]) -> wisp_mcp_core::Served {
    let mut s = wisp_mcp_core::Served::default();
    for m in methods {
        if wisp_mcp_core::dispatch::core_method(&m.method) || s.methods.contains(&m.method) {
            continue;
        }
        s.methods.push(m.method.clone());
        if let Some((id, settings)) = &m.extension {
            s.extensions.entry(id.clone()).or_insert_with(|| settings.clone());
        }
    }
    s
}

/// A contributed method, run as the caller: its result, or its refusal with the code and data it chose.
async fn method_reply(
    ctx: &Ctx,
    call: &Call,
    methods: &[std::sync::Arc<wisp_mcp_api::MethodContribution>],
    id: &serde_json::value::RawValue,
    method: &str,
    params: Value,
) -> String {
    let Some(m) = methods.iter().find(|m| m.method == method) else {
        return wisp_mcp_core::rpc::error(id, wisp_mcp_core::rpc::METHOD_NOT_FOUND, &format!("Method not found: {method}"));
    };
    match (m.run)(ctx.clone(), call.clone(), params).await {
        Ok(v) => wisp_mcp_core::rpc::result(id, &v.to_string()),
        Err(e) => wisp_mcp_core::rpc::error_data(id, e.code, &e.message, e.data.as_ref()),
    }
}

/// A tool call's catalogue event (`wisp_mcp_core::telemetry`), with the caller's harness.
async fn track(ctx: &Ctx, call: &Call, w: &Who, event: &str, mut props: Value) {
    if let Who::Agent { session, .. } = w {
        let agent = typed::<rt::Get>(ctx, call, get(session)).await.ok().and_then(|s| s.get("agent").cloned());
        if let (Some(o), Some(a)) = (props.as_object_mut(), agent) {
            o.insert("agent".into(), a);
        }
    }
    if let Ok(a) = args_of::<wisp_telemetry_api::Track>(json!({ "event": event, "props": props })) {
        // write: best-effort usage telemetry; a tool call never fails or waits on it
        let _ = ctx.run::<wisp_telemetry_api::Track>(&ctx.root_call(), a).await;
    }
}

/// The `tools` spec for this session's run (its agent profile's, else its mode's), when one is declared.
async fn narrowed(ctx: &Ctx, call: &Call, sessions: &Sessions, session: &str) -> Option<crate::kinds::ToolsSpec> {
    if sessions.1.is_empty() {
        return None;
    }
    let s = typed::<rt::Get>(ctx, call, get(session)).await.ok()?;
    let profile = s.pointer("/ext/wisp.runtime/spec/agent").and_then(Value::as_str).unwrap_or_default();
    sessions.1.for_run(profile, s.get("mode").and_then(Value::as_str).unwrap_or_default())
}

/// What `mcp.session@1` contributors know of the session (a side discussion, spells, watch), and the lead's team
/// as the runtime keeps it (`ext["wisp.runtime"].subagents`, v1 `WISP_TEAM`): `delegate` names them (orch #2).
async fn merge_flags(ctx: &Ctx, call: &Call, session: &str, info: &mut wisp_mcp_api::SessionInfo) {
    let flags = ctx.contributions(&wisp_mcp_api::SESSION_POINT);
    let ext = typed::<rt::Get>(ctx, call, get(session)).await.ok().and_then(|s| s.get("ext").cloned()).unwrap_or(Value::Null);
    if info.team.is_empty() {
        info.team = team_of(&ext);
    }
    for f in flags {
        let more = f.flags(session, &ext);
        info.read_only |= more.read_only;
        info.spells |= more.spells;
        info.watch |= more.watch;
    }
}

/// The roster `delegate` describes, from the runtime's slot: each member with its copies (`x#1`, `x#2`).
fn team_of(ext: &Value) -> Vec<wisp_mcp_api::TeamEntry> {
    let list = ext.pointer("/wisp.runtime/subagents").and_then(Value::as_array).cloned().unwrap_or_default();
    let text = |m: &Value, k: &str| m.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_owned);
    list.iter()
        .filter_map(|m| {
            let name = text(m, "name")?;
            let count = m.get("count").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(1).max(1);
            let copies = if count > 1 { (1..=count).map(|i| format!("{name}#{i}")).collect() } else { Vec::new() };
            // a crew member reads as who it is (v1 WISP_TEAM): "Ada of the user's crew, CI watcher"
            let agent = match text(m, "crew") {
                Some(_) => {
                    let who = text(m, "label").unwrap_or_else(|| name.clone());
                    text(m, "role").map_or_else(|| format!("{who} of the user's crew"), |r| format!("{who} of the user's crew, {r}"))
                }
                None => text(m, "agent").unwrap_or_default(),
            };
            let model = if text(m, "crew").is_some() { Some("own folder and memory".to_owned()) } else { text(m, "model") };
            let place = text(m, "machine").map(|_| text(m, "place").unwrap_or_else(|| "another machine".into()));
            Some(wisp_mcp_api::TeamEntry { name, agent, model, effort: text(m, "effort"), use_for: text(m, "use"), count, copies, place })
        })
        .collect()
}

/// A command chosen at runtime by name (an owner tool from the table, a connector, `crew.*`): by name, with JSON.
async fn owner(ctx: &Ctx, call: &Call, cmd: &str, args: Value) -> Result<Value, String> {
    ctx.run_json(call, cmd, args).await.map_err(|e| e.message)
}

/// Typed command `C`, its answer as the JSON the tool words read (what `run_json` would have answered).
async fn typed<C: Command>(ctx: &Ctx, call: &Call, args: C::Args) -> Result<Value, String> {
    let out = ctx.run::<C>(call, args).await.map_err(|e| e.message)?;
    serde_json::to_value(out).map_err(|e| e.to_string())
}

/// `C`'s args from what a tool call carried, refused in the words its handler would use (`<id>: <why>`).
fn args_of<C: Command>(v: Value) -> Result<C::Args, String> {
    serde_json::from_value(v).map_err(|e| format!("{}: {e}", C::ID))
}

/// `C` with args from a tool call's JSON (`args_of`), then `typed`.
async fn typed_json<C: Command>(ctx: &Ctx, call: &Call, v: Value) -> Result<Value, String> {
    typed::<C>(ctx, call, args_of::<C>(v)?).await
}

fn get(id: &str) -> rt::IdArgs {
    rt::IdArgs { id: id.into() }
}

fn text_of(r: Result<Value, String>) -> Outcome {
    match r {
        Ok(v) if v.get("text").is_some() || v.get("error").is_some() => {
            owner_text(&v).map_or_else(|e| Outcome::Failed(e.to_string()), Outcome::Text)
        }
        Ok(v) => Outcome::Text(v.to_string()),
        Err(e) => Outcome::Failed(e),
    }
}

async fn builtin(ctx: &Ctx, call: &Call, w: &Who, name: &str, args: Map<String, Value>, cwd: Option<&str>) -> Outcome {
    match w {
        Who::User { .. } => user_tool(ctx, call, name, args, cwd).await,
        Who::Agent { session, info } => match agent_action(name, args, session, info) {
            Ok(Some(a)) => agent_tool(ctx, call, session, a).await,
            Ok(None) => Outcome::Failed(format!("{name} isn't something this server does")),
            Err(e) => Outcome::Failed(e.to_string()),
        },
    }
}

/// Waits for the open question's answer (`runtime.pollAnswer`), as long as v1 waited.
async fn wait_answer(ctx: &Ctx, call: &Call, ask_id: &str, wait_ms: i64) -> Option<String> {
    let clock = ctx.clock().ok()?;
    let until = clock.now_ms() + wait_ms;
    loop {
        let p = typed::<rt::PollAnswerOf>(ctx, call, rt::PollAnswerArgs { ask_id: ask_id.to_owned() }).await.ok()?;
        match p.get("state").and_then(Value::as_str) {
            Some("answered") => return p.get("answer").and_then(Value::as_str).map(str::to_owned),
            Some("pending") if clock.now_ms() < until => clock.sleep(POLL_MS).await,
            _ => return None,
        }
    }
}

async fn agent_tool(ctx: &Ctx, call: &Call, session: &str, a: AgentAction) -> Outcome {
    match a {
        AgentAction::Owner { command, args } => text_of(owner(ctx, call, command, args).await),
        AgentAction::Ask { question, context, options } => {
            let q = context.map_or(question.clone(), |c| format!("{question}\n\n{c}"));
            match typed::<rt::Ask>(ctx, call, rt::AskArgs { id: session.into(), question: q, options, details: None }).await {
                Ok(v) => {
                    let id = v.get("askId").and_then(Value::as_str).unwrap_or_default().to_owned();
                    Outcome::Text(words::ask_answer(wait_answer(ctx, call, &id, ASK_WAIT_MS).await.as_deref()))
                }
                Err(e) => Outcome::Failed(e),
            }
        }
        AgentAction::Approve { args, input } => {
            let a = json!({ "id": session, "tool": args.get("tool"), "input": input, "toolUseId": args.get("tool_use_id") });
            match typed_json::<rt::Approval>(ctx, call, a).await {
                Ok(v) => Outcome::Text(v.to_string()),
                Err(e) => Outcome::Failed(e),
            }
        }
        AgentAction::Delegate { args, wait } => {
            let tasks = args.get("tasks").cloned().unwrap_or(Value::Null);
            match typed_json::<rt::Delegate>(ctx, call, json!({ "id": session, "tasks": tasks })).await {
                // each task that didn't go is said, by its name and why: a harness that isn't here came back as
                // `{"reports":[]}` with no word (audit H HS11)
                Ok(v) => {
                    let Ok(all) = serde_json::from_value::<Vec<words::Delegation>>(v.clone()) else {
                        return Outcome::Failed(format!("delegate answered something that isn't a list of delegations: {v}"));
                    };
                    match words::delegated(&all) {
                        words::Delegated::Nothing(t) => Outcome::Text(t),
                        words::Delegated::Some { head, .. } if !wait => Outcome::Text(words::delegated_no_wait(&head, &all)),
                        words::Delegated::Some { head, ids } => text_of(
                            typed_json::<rt::Results>(ctx, call, json!({ "id": session, "ids": ids, "wait": 600 }))
                                .await
                                .map(|r| json!({ "text": format!("{head}{r}") })),
                        ),
                    }
                }
                Err(e) => Outcome::Failed(e),
            }
        }
        AgentAction::Results { args, wait } => {
            let ids = args.get("ids").cloned().unwrap_or_else(|| json!([]));
            let r = typed_json::<rt::Results>(ctx, call, json!({ "id": session, "ids": ids, "wait": if wait { 600 } else { 0 } })).await;
            text_of(r.map(|v| json!({ "text": v.to_string() })))
        }
        AgentAction::Launch { args, wait, wait_ms } => {
            let a = args.as_object().cloned().unwrap_or_default();
            match crate::launch::launch(ctx, call, Some(session), &a).await {
                Ok(c) if wait => {
                    let id = c.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                    Outcome::Text(task_wait(ctx, call, &id, wait_ms).await)
                }
                Ok(c) => Outcome::Text(format!("Launched task `{}`.", c.get("id").and_then(Value::as_str).unwrap_or_default())),
                Err(e) => Outcome::Failed(e),
            }
        }
        AgentAction::TaskWait { task, wait_ms } => Outcome::Text(task_wait(ctx, call, &task, wait_ms).await),
        AgentAction::Setup { command, args } => setup(ctx, call, session, command, args).await,
        AgentAction::ConnectorSignin { args } => text_of(owner(ctx, call, wisp_mcp_api::owners::CONNECTORS_SIGNIN, args).await),
    }
}

/// A write from the user's harness that lowers a safety guard (ADR-0115): a task that asks the user in wisp (Allow or
/// Deny, on the task's card, the phone and channels as any question), the harness's call waiting as `ask_user` does.
/// Allow runs exactly the write it showed, as the user; anything else changes nothing. The task is done after.
pub(crate) async fn guard_card(ctx: &Ctx, call: &Call, command: &str, args: Value, l: &wisp_mcp_core::guard::Lowered) -> Outcome {
    use wisp_mcp_core::guard;
    let made = typed_json::<rt::Create>(
        ctx,
        call,
        json!({ "prompt": guard::question(l), "title": format!("Your harness wants to {}", l.what), "hold": true, "worktree": false, "by": "mcp" }),
    )
    .await;
    let id = match made {
        Ok(v) => v.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
        Err(e) => return Outcome::Failed(e),
    };
    if let Ok(mut g) = crate::GUARD_TASKS.lock() {
        g.insert(id.clone());
    }
    let question = guard::question(l).trim().to_owned();
    // from here the call may go (the harness gives up, disconnects): the task settles however far it got, so a held task
    // with the guard's text as its prompt is never left behind (the review of audit I IS2)
    let mut waiting =
        Waiting { ctx: ctx.clone(), call: call.clone(), id: id.clone(), question: question.clone(), asked: false, armed: true };
    let asked = typed::<rt::Ask>(
        ctx,
        call,
        rt::AskArgs {
            id: id.as_str().into(),
            question: guard::question(l),
            options: vec![guard::ALLOW.into(), guard::DENY.into()],
            // the runtime answers it only from the user's own hands, never a follow-up (ADR-0115)
            details: Some(json!({ "guard": true })),
        },
    )
    .await;
    let answer = match asked {
        Ok(v) => {
            let ask = v.get("askId").and_then(Value::as_str).unwrap_or_default().to_owned();
            // the harness's call may go while it waits (it gives up on a slow tool, disconnects): the task then settles,
            // saying the change wasn't made, instead of asking for good with its prompt queued (audit I IS2)
            waiting.asked = true;
            let a = wait_answer(ctx, call, &ask, ASK_WAIT_MS).await;
            waiting.armed = false;
            a
        }
        Err(e) => {
            waiting.armed = false;
            settle_guard(ctx, call, &id).await;
            return Outcome::Failed(e);
        }
    };
    if answer.is_none() {
        // nobody answered while the call waited: it ends now, so the task says the change wasn't made
        let _ = typed::<rt::Unask>(ctx, call, rt::UnaskArgs { id: id.as_str().into(), question }).await;
    }
    let allowed = answer.as_deref().is_some_and(guard::allowed);
    let out = if allowed {
        match ctx.run_json(call, command, args).await {
            Ok(v) => Outcome::Text(format!("The user allowed it in wisp: {}. {}", l.change, wisp_mcp_core::generated::answer_text(&v))),
            Err(e) => Outcome::Failed(format!("The user allowed it, but it didn't go through: {}", e.message)),
        }
    } else if answer.is_some() {
        Outcome::Text(guard::DENIED.into())
    } else {
        Outcome::Text(guard::UNANSWERED.into())
    };
    settle_guard(ctx, call, &id).await;
    out
}

/// A guard task's question while the harness's call waits on it: dropped with the call (the harness gave up, or
/// disconnected), the question is taken back (`runtime.unask`), which settles the task as one whose harness stopped
/// waiting (audit I IS2).
struct Waiting {
    ctx: Ctx,
    call: Call,
    id: String,
    question: String,
    /// its question is on the task (else the call went between making the task and asking)
    asked: bool,
    armed: bool,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Ok(mut g) = crate::GUARD_TASKS.lock() {
            g.remove(&self.id);
        }
        let (ctx, call, id, question) =
            (self.ctx.clone(), self.call.clone(), std::mem::take(&mut self.id), std::mem::take(&mut self.question));
        let asked = self.asked;
        self.ctx.spawn(async move {
            if asked {
                if let Err(e) = typed::<rt::Unask>(&ctx, &call, rt::UnaskArgs { id: id.as_str().into(), question }).await {
                    tracing::warn!(task = %id, "wisp.mcp: a guard task whose call went wasn't settled: {e}");
                }
            } else {
                // nothing asked yet: the held task goes done, its prompt dropped, before anyone can send it
                settle_guard(&ctx, &call, &id).await;
            }
        });
    }
}

/// The asking task, done once it's answered: its held prompt never runs, and the harness may act on it again.
async fn settle_guard(ctx: &Ctx, call: &Call, id: &str) {
    let _ = typed::<rt::Stop>(ctx, call, rt::StopArgs { id: id.into(), clear_queue: true }).await;
    let _ = typed_json::<rt::SetState>(ctx, call, json!({ "id": id, "state": "done" })).await;
    if let Ok(mut g) = crate::GUARD_TASKS.lock() {
        g.remove(id);
    }
}

/// A change to the user's setup an agent asked for (ADR-0113): the card on its task, then the user's answer in words
/// (what the write said once allowed, or that they said no). Nothing runs here: the write runs from the user's Allow.
pub(crate) async fn setup(ctx: &Ctx, call: &Call, session: &str, command: &str, args: Value) -> Outcome {
    let asked = typed::<rt::SetupAsk>(ctx, call, rt::SetupAskArgs { id: session.into(), command: command.to_owned(), args }).await;
    let id = match asked {
        Ok(v) => v.get("askId").and_then(Value::as_str).unwrap_or_default().to_owned(),
        Err(e) => return Outcome::Failed(e),
    };
    Outcome::Text(wait_answer(ctx, call, &id, ASK_WAIT_MS).await.unwrap_or_else(|| words::SETUP_UNANSWERED.to_owned()))
}

/// How long `wisp_stop` waits for the run to let go before it says it's still stopping.
const STOP_WAIT_MS: i64 = 10_000;

/// Whether task `id` is no longer running within `wait_ms`.
async fn until_settled(ctx: &Ctx, call: &Call, id: &str, wait_ms: i64) -> bool {
    let Ok(clock) = ctx.clock() else { return false };
    let until = clock.now_ms() + wait_ms;
    loop {
        let s = typed::<rt::Get>(ctx, call, get(id)).await.unwrap_or(Value::Null);
        if s.get("state").and_then(Value::as_str).is_some_and(|st| st != "running") {
            return true;
        }
        if clock.now_ms() >= until {
            return false;
        }
        clock.sleep(POLL_MS).await;
    }
}

/// Waits for a task to settle (not running), then its report in v1's words.
async fn task_wait(ctx: &Ctx, call: &Call, id: &str, wait_ms: i64) -> String {
    let Ok(clock) = ctx.clock() else { return String::new() };
    let until = clock.now_ms() + wait_ms;
    loop {
        let s = typed::<rt::Get>(ctx, call, get(id)).await.unwrap_or(Value::Null);
        let settled = s.get("state").and_then(Value::as_str) != Some("running");
        if settled || clock.now_ms() >= until {
            let t = typed::<rt::Turns>(ctx, call, rt::TurnsArgs { id: id.into(), before: None, limit: Some(1), masked: None })
                .await
                .unwrap_or(Value::Null);
            let text =
                t.as_array().and_then(|l| l.last()).and_then(|x| x.get("text")).and_then(Value::as_str).unwrap_or_default().to_owned();
            return words::task_report(settled, None, &text);
        }
        clock.sleep(POLL_MS).await;
    }
}

/// The presentation as markdown: the hero's title and summary, then the turn's notes.
fn markdown(p: &Value) -> String {
    let mut out = Vec::new();
    for b in p.get("blocks").and_then(Value::as_array).into_iter().flatten() {
        if let Some(t) = b.get("title").and_then(Value::as_str) {
            out.push(format!("### {t}"));
        }
        if let Some(s) = b.get("summary").or_else(|| b.get("text")).and_then(Value::as_str) {
            out.push(s.to_owned());
        }
    }
    if let Some(n) = p.get("notes").and_then(Value::as_str).filter(|n| !n.is_empty()) {
        out.push(n.to_owned());
    }
    out.join("\n\n")
}

async fn describe(ctx: &Ctx, call: &Call, id: &str, extra: &str) -> Outcome {
    let s = match typed::<rt::Get>(ctx, call, get(id)).await {
        Ok(s) => s,
        Err(e) => return Outcome::Failed(e),
    };
    let p = typed::<rt::PresentationOf>(ctx, call, rt::PresentationArgs { id: id.into(), turn: None }).await.unwrap_or(Value::Null);
    Outcome::Text(user::describe(id, &s, &markdown(&p), extra))
}

/// `wisp mcp`'s tools (v1 server/mcp-wisp.js `callTool`).
async fn user_tool(ctx: &Ctx, call: &Call, name: &str, a: Map<String, Value>, cwd: Option<&str>) -> Outcome {
    let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_owned);
    let id = s("id").unwrap_or_default();
    let wait_ms = words::task_wait_ms(a.get("timeout_seconds"));
    match name {
        "wisp_launch" => {
            if s("prompt").is_none_or(|p| p.trim().is_empty()) {
                return Outcome::Failed("prompt is required".into());
            }
            // one wisp_launch everywhere (audit G6): the agent's path, from where `wisp mcp` runs, with no asking task
            let mut a = a.clone();
            if s("cwd").is_none_or(|c| c.is_empty())
                && let Some(c) = cwd
            {
                a.insert("cwd".into(), json!(c));
            }
            match crate::launch::launch(ctx, call, None, &a).await {
                Ok(c) => {
                    let id = c.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                    if a.get("wait").is_some_and(|w| w.as_bool().unwrap_or(false)) {
                        let _ = task_wait(ctx, call, &id, wait_ms).await;
                        return describe(ctx, call, &id, "").await;
                    }
                    let got = typed::<rt::Get>(ctx, call, get(&id)).await.unwrap_or(Value::Null);
                    Outcome::Text(user::launched(&got))
                }
                Err(e) => Outcome::Failed(e),
            }
        }
        "wisp_list" => match typed::<rt::List>(ctx, call, rt::ListArgs::default()).await {
            Ok(Value::Array(all)) => Outcome::Text(user::list(&all, &a)),
            Ok(_) => Outcome::Text(user::list(&[], &a)),
            Err(e) => Outcome::Failed(e),
        },
        "wisp_get" => describe(ctx, call, &id, "").await,
        "wisp_wait" => {
            let _ = task_wait(ctx, call, &id, wait_ms).await;
            describe(ctx, call, &id, "").await
        }
        "wisp_reply" => {
            let Some(text) = s("text").map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()) else {
                return Outcome::Failed("text is required".into());
            };
            match typed_json::<rt::FollowUp>(ctx, call, json!({ "id": id, "text": text, "queue": a.get("queue"), "mode": s("mode") })).await
            {
                Ok(r) => Outcome::Text(user::replied(&r).to_owned()),
                Err(e) => Outcome::Failed(e),
            }
        }
        // said once it has stopped (audit C16), or that it's still stopping after the wait
        "wisp_stop" => match typed::<rt::Stop>(ctx, call, rt::StopArgs { id: id.as_str().into(), clear_queue: false }).await {
            Ok(_) if until_settled(ctx, call, &id, STOP_WAIT_MS).await => Outcome::Text(user::stopped(&id)),
            Ok(_) => Outcome::Text(user::stopping(&id)),
            Err(e) => Outcome::Failed(e),
        },
        "wisp_agents" => {
            let here = typed::<rt::Agents>(ctx, call, rt::AgentsArgs::default())
                .await
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            let machines = typed::<wisp_machines_api::List>(ctx, call, wisp_machines_api::NoArgs {})
                .await
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            Outcome::Text(user::agents(&here, &machines))
        }
        other => crate::crew::tool(ctx, call, other, &a, cwd)
            .await
            .unwrap_or_else(|| Outcome::Failed(format!("{other} isn't something this server does"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod team_tests {
    use super::team_of;
    use serde_json::json;

    /// orch #2: `delegate`'s roster is the lead's team as the runtime keeps it (Delegate's rows, a crew member's links).
    #[test]
    fn the_delegate_roster_is_the_sessions_team() {
        let ext = json!({ "wisp.runtime": { "subagents": [
            { "name": "tester", "agent": "claude", "model": "", "use": "run the tests", "count": 2 },
            { "name": "bob", "crew": "bob123", "label": "Bob", "role": "CI watcher", "agent": "auto", "count": 1 },
        ] } });
        let team = team_of(&ext);
        assert_eq!(team.len(), 2);
        assert_eq!(
            (team[0].name.as_str(), team[0].count, team[0].copies.clone()),
            ("tester", 2, vec!["tester#1".to_owned(), "tester#2".to_owned()])
        );
        assert_eq!(team[0].use_for.as_deref(), Some("run the tests"));
        assert_eq!(team[1].agent, "Bob of the user's crew, CI watcher");
        assert_eq!(team_of(&json!({})), Vec::<wisp_mcp_api::TeamEntry>::new());
    }
}
