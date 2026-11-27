//! wisp.risk: merge risk (ADR-0140, `risk.harness`, off by default). For each open PR of a task, a harness other than
//! the one that wrote it (or the one picked) reads the PR's diff (`ship.pr_diff`) and answers low, medium or high, a
//! score and why (`runtime.quick`). One verdict per PR, of its head; the verdicts sit in the task's `wisp.risk` slot
//! and reach the row as `risk` (with `riskOn`). Started by a PR opening, ship seeing a new head on any open PR, a
//! turn's end, and by hand (`risk.judge` with `force`: Judge again).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use wisp_kernel::{Call, Ctx, Manifest, Plugin, Result, Setup};
use wisp_risk_api::{EXT, Judge, JudgeArgs, Judged, Pick, RiskJudged, SETTING, SHIP_EXT, Slot, Status, View};
use wisp_risk_core::{self as core, Agent, OpenPr, Outcome};
use wisp_runtime_api::commands as rt;
use wisp_runtime_api::events::TurnEnded;
use wisp_runtime_api::ext::update_ext;
use wisp_runtime_api::model::{Session, SessionState, Turn};
use wisp_runtime_api::points::{SUMMARY, SummaryField};

#[derive(Debug, Default)]
pub struct RiskPlugin;

impl Plugin for RiskPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let ctx = s.ctx();
        s.contribute(&SUMMARY, Arc::new(Row(ctx.clone())) as Arc<dyn SummaryField>);
        // the run outlives the command: it goes on the plugin's own call (a command's call is cancelled once it
        // answers, as wisp.review found, orch #20)
        s.command::<Judge, _, _>(move |ctx, _call, a| async move { judge(&ctx, &ctx.root_call(), &a).await });
        s.command::<View, _, _>(move |ctx, call, a| async move {
            let task = ctx.run::<rt::Get>(&call, rt::IdArgs { id: a.id.as_str().into() }).await?;
            Ok(core::view(&core::slot_of(task.ext_of(EXT)), &core::open_prs(task.ext_of(SHIP_EXT)), now(&ctx)))
        });
        s.on::<wisp_ship_api::PrOpened, _, _>(move |ctx, e| async move {
            quietly(&ctx, &e.session).await;
        });
        s.on::<TurnEnded, _, _>(move |ctx, e| async move {
            quietly(&ctx, e.session_id.as_str()).await;
        });
        Ok(())
    }
}

fn now(ctx: &Ctx) -> i64 {
    ctx.clock().map_or(0, |c| c.now_ms())
}

fn setting(ctx: &Ctx) -> String {
    ctx.setting(SETTING).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| "off".into())
}

/// A judge nobody waits on (an event's): what it says is for the log only.
async fn quietly(ctx: &Ctx, id: &str) {
    let a = JudgeArgs { id: id.to_owned(), force: false, url: None };
    if let Err(e) = judge(ctx, &ctx.root_call(), &a).await {
        tracing::debug!(task = %id, "risk: not judged: {}", e.message);
    }
}

/// Starts judging the task's open PRs that are due ([`core::due`]); the runs go on the plugin's scope, one PR after
/// another (never one judge per PR at once on the user's account).
async fn judge(ctx: &Ctx, call: &Call, a: &JudgeArgs) -> Result<Judged> {
    let set = setting(ctx);
    if !core::is_on(&set) {
        return Ok(Judged { started: 0, why: "Merge risk is off.".into() });
    }
    let task = ctx.run::<rt::Get>(call, rt::IdArgs { id: a.id.as_str().into() }).await?;
    if task.parent_id.is_some() {
        return Ok(Judged { started: 0, why: "A subagent's PRs are judged with its lead's.".into() });
    }
    let open = core::open_prs(task.ext_of(SHIP_EXT));
    if open.is_empty() {
        return Ok(Judged { started: 0, why: "This task has no open pull request.".into() });
    }
    // its turn's end judges it: a push mid-turn is likely not the last
    if task.state == SessionState::Running && !a.force {
        return Ok(Judged::default());
    }
    if core::due(&open, &core::slot_of(task.ext_of(EXT)), now(ctx), a.force, a.url.as_deref()).is_empty() {
        return Ok(Judged::default());
    }
    let turns = ctx
        .run::<rt::Turns>(call, rt::TurnsArgs { id: a.id.as_str().into(), before: None, limit: None, masked: None })
        .await
        .unwrap_or_default();
    let first = turns.iter().min_by_key(|t| t.idx).map(|t| t.prompt.clone()).unwrap_or_default();
    let authors = core::authors_of(&task.agent, turns.iter().map(|t| t.agent.as_str()));
    let agents: Vec<Agent> = ctx
        .run::<rt::Agents>(call, rt::AgentsArgs { fresh: None })
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|x| Agent { id: x.id, name: x.name, available: x.available })
        .collect();
    let pick = match core::pick_judge(&set, &authors, &agents) {
        Ok(p) => p,
        Err(why) => return Ok(Judged { started: 0, why }),
    };
    // claim the due heads in one compare-and-set: a second judge racing this one finds them running and starts none
    let mut claimed: Vec<OpenPr> = Vec::new();
    let at = now(ctx);
    update_ext(ctx, call, &a.id, EXT, |slot, t| {
        let open = core::open_prs(t.ext_of(SHIP_EXT));
        let was = core::slot_of(slot);
        claimed = core::due(&open, &was, at, a.force, a.url.as_deref());
        if claimed.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::to_value(core::start(&was, &open, &claimed, &pick, at)).unwrap_or(Value::Null)))
    })
    .await?;
    let started = u32::try_from(claimed.len()).unwrap_or(u32::MAX);
    if claimed.is_empty() {
        return Ok(Judged::default());
    }
    let job = Job {
        ctx: ctx.clone(),
        id: a.id.clone(),
        title: task.title.clone(),
        asked: first,
        author: authors.first().cloned().unwrap_or_default(),
        cwd: task.cwd.clone(),
        pick,
    };
    ctx.spawn(async move {
        for pr in claimed {
            job.one(&pr).await;
        }
    });
    Ok(Judged { started, why: String::new() })
}

/// One task's judging run: what every PR's judgment needs.
struct Job {
    ctx: Ctx,
    id: String,
    title: String,
    asked: String,
    author: String,
    cwd: String,
    pick: Pick,
}

impl Job {
    async fn one(&self, pr: &OpenPr) {
        let call = self.ctx.root_call();
        let (outcome, partial) = self.ask(&call, pr).await;
        let at = now(&self.ctx);
        let level = match &outcome {
            Outcome::Answered(a) => Some(a.level),
            Outcome::Failed(_) => None,
        };
        let status = if level.is_some() { Status::Done } else { Status::Failed };
        let wrote = update_ext(&self.ctx, &call, &self.id, EXT, |slot, _| {
            Ok(core::finish(&core::slot_of(slot), &pr.url, &pr.head, outcome.clone(), partial, at)
                .and_then(|s: Slot| serde_json::to_value(s).ok()))
        })
        .await;
        match wrote {
            Ok(Some(_)) => self.ctx.emit(
                &call,
                &RiskJudged {
                    session_id: self.id.clone(),
                    url: pr.url.clone(),
                    head: pr.head.clone(),
                    status,
                    level,
                    judge: self.pick.agent.clone(),
                    author: self.author.clone(),
                },
            ),
            // a newer head (or a closed PR) took its place: this answer is of a head nobody shows
            Ok(None) => {}
            Err(e) => tracing::warn!(task = %self.id, pr = %pr.url, "risk: couldn't keep the verdict: {}", e.message),
        }
    }

    /// The judge's verdict on the PR's diff, and whether the diff was cut to fit.
    async fn ask(&self, call: &Call, pr: &OpenPr) -> (Outcome, bool) {
        let diff = match self
            .ctx
            .run::<wisp_ship_api::PrDiff>(call, wisp_ship_api::PrDiffArgs { session: self.id.clone(), url: pr.url.clone() })
            .await
        {
            Ok(d) if !d.trim().is_empty() => d,
            Ok(_) => return (Outcome::Failed("GitHub gave an empty diff for this PR.".into()), false),
            Err(e) => return (Outcome::Failed(format!("Couldn't read the PR's diff: {}", e.message)), false),
        };
        let (diff, partial) = core::fit_diff(&diff);
        let model = (!self.pick.model.is_empty()).then(|| self.pick.model.clone());
        let args = rt::QuickArgs {
            prompt: core::prompt(&self.title, &self.asked, pr, &diff, partial),
            cwd: self.cwd.clone(),
            pick: Some(rt::RunPick { agent: Some(self.pick.agent.clone()), model, effort: None }),
            timeout_ms: Some(core::JUDGE_MS),
            json: true,
            run_id: None,
        };
        let outcome = match self.ctx.run::<rt::Quick>(call, args).await {
            Ok(r) => {
                let answer = r.json.unwrap_or(Value::String(r.text));
                core::read_answer(&answer)
                    .map_or_else(|| Outcome::Failed("The judge's answer wasn't a verdict wisp can read.".into()), Outcome::Answered)
            }
            Err(e) => Outcome::Failed(format!("The judge didn't answer: {}", e.message)),
        };
        (outcome, partial)
    }
}

/// `runtime.summary@1`: whether Settings → Merge risk is on (`riskOn`) and every PR's verdict (`risk`, as `risk.view`
/// says it), only while it's on.
struct Row(Ctx);

/// The row's merge risk fields from the session's slots.
pub fn row_fields(session: &Session, on: bool, now: i64, out: &mut BTreeMap<String, Value>) {
    if !on {
        return;
    }
    out.insert("riskOn".into(), Value::Bool(true));
    let views = core::view(&core::slot_of(session.ext_of(EXT)), &core::open_prs(session.ext_of(SHIP_EXT)), now);
    if !views.is_empty()
        && let Ok(v) = serde_json::to_value(views)
    {
        out.insert("risk".into(), v);
    }
}

impl SummaryField for Row {
    fn plugin(&self) -> &str {
        EXT
    }
    fn fields(&self, session: &Session, _last: Option<&Turn>, out: &mut BTreeMap<String, Value>) {
        row_fields(session, core::is_on(&setting(&self.0)), now(&self.0), out);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod row_tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use wisp_runtime_api::model::Session;

    use super::row_fields;

    fn session(ship: serde_json::Value, risk: serde_json::Value) -> Session {
        let mut s = Session { id: "t1".into(), ..Session::default() };
        s.ext.insert("wisp.ship".into(), ship);
        s.ext.insert("wisp.risk".into(), risk);
        s
    }

    #[test]
    fn the_row_says_each_prs_risk_only_while_the_setting_is_on() {
        let ship = json!({ "repos": [{ "name": "app", "pr": { "url": "u1", "number": 1, "state": "OPEN", "head": "h2" } }],
            "prs": [{ "repo": "lib", "pr": { "url": "u2", "number": 7, "state": "OPEN", "head": "k1" } }] });
        let risk = json!({ "prs": [
            { "url": "u2", "repo": "lib", "number": 7, "head": "k1", "status": "done", "level": "high", "at": 5 },
            { "url": "u1", "repo": "app", "number": 1, "head": "h1", "status": "done", "level": "low", "at": 5 } ] });
        let s = session(ship, risk);
        let mut out = BTreeMap::new();
        row_fields(&s, true, 10, &mut out);
        assert_eq!(out["riskOn"], true);
        let r = out["risk"].as_array().unwrap();
        assert_eq!(
            (r[0]["url"].as_str(), r[0]["current"].as_bool()),
            (Some("u1"), Some(false)),
            "the task's own PR first; a push since: not current"
        );
        assert_eq!((r[1]["level"].as_str(), r[1]["current"].as_bool()), (Some("high"), Some(true)));
        let mut off = BTreeMap::new();
        row_fields(&s, false, 10, &mut off);
        assert!(off.is_empty(), "off: the row says nothing");
    }
}
