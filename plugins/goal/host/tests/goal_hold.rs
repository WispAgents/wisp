//! A goal round is an after-turn step (`runtime.after@1`): the turn's end holds its task in Working while the judges
//! vote, and the verdict moves it on (met lands it once, unread; another round's follow-up starts the next turn; a
//! stall asks). Its judges vote all at once, and with `any` or `all` the rest stop once the outcome is fixed. Against a
//! stand-in wisp.runtime whose `runtime.settle` lands a task as runtime does (asking the after-turn steps again) and
//! whose judges take the time they're given.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::watch;
use wisp_goal::GoalPlugin;
use wisp_kernel::{Caller, Kernel, Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::commands as r;
use wisp_runtime_api::model::{Session, SessionState, Summary, Turn, TurnStatus};
use wisp_runtime_api::points::{AFTER, AfterTurn, TurnEnd};
use wisp_testkit::TestApp;

/// How a judge answers: after `ms` (unless it's stopped first), with `text`.
type Judges = BTreeMap<&'static str, (u64, &'static str)>;

#[derive(Clone)]
struct Rt {
    task: Arc<Mutex<Session>>,
    judges: Judges,
    /// the judges' runs stopped so far
    cancelled: watch::Sender<Vec<String>>,
    /// run id → judge
    runs: Arc<Mutex<BTreeMap<String, String>>>,
    /// what happened, in order: `judge:a`, `stop:a`, `followUp`, `ask`, `landed`, `held`
    log: Arc<Mutex<Vec<String>>>,
    /// `runtime.list` names the task (a restart's recovery finds it); else it lists nothing
    listed: bool,
}

impl Rt {
    fn log(&self, s: impl Into<String>) {
        self.log.lock().unwrap().push(s.into());
    }
    fn seen(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
    fn count(&self, s: &str) -> usize {
        self.seen().iter().filter(|x| *x == s).count()
    }
    fn task(&self) -> Session {
        self.task.lock().unwrap().clone()
    }
    fn goal(&self) -> Value {
        self.task().ext.get("wisp.goal").cloned().unwrap_or(Value::Null)
    }
}

fn last_turn() -> Turn {
    Turn { id: "u1".into(), session_id: "t1".into(), status: TurnStatus::Done, text: Some("done".into()), ..Turn::default() }
}

const DONE: TurnEnd = TurnEnd { stopped: false, is_error: false };

struct Runtime(Rt);
impl Plugin for Runtime {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1" })).unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let rt = self.0.clone();
        s.command::<r::Get, _, _>(move |_, _, _a| {
            let t = rt.task();
            async move { Ok(t) }
        });
        s.command::<r::Turns, _, _>(|_, _, _a| async move { Ok(vec![last_turn()]) });
        let rt = self.0.clone();
        s.command::<r::List, _, _>(move |_, _, _a| {
            let t = rt.task();
            let listed = rt.listed;
            async move { Ok(if listed { vec![Summary { id: t.id, state: t.state, ..Summary::default() }] } else { Vec::new() }) }
        });
        s.command::<r::Profiles, _, _>(|_, _, _a: Value| async move { Ok(Vec::new()) });
        let rt = self.0.clone();
        s.command::<r::Quick, _, _>(move |_, _, a: r::QuickArgs| {
            let rt = rt.clone();
            async move {
                let judge = a.pick.and_then(|p| p.agent).unwrap_or_default();
                let id = a.run_id.unwrap_or_default();
                rt.runs.lock().unwrap().insert(id.clone(), judge.clone());
                rt.log(format!("judge:{judge}"));
                let (ms, text) = rt.judges.get(judge.as_str()).copied().unwrap_or((0, "MET"));
                let mut rx = rt.cancelled.subscribe();
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_millis(ms)) => Ok(r::QuickResult {
                        text: text.into(), json: None, agent: judge, model: String::new(), cost_usd: None, activity: Vec::new(),
                    }),
                    _ = rx.wait_for(|c| c.contains(&id)) => {
                        Err(WireError::conflict("It was cancelled.").with_details(json!({ "cancelled": true })))
                    }
                }
            }
        });
        let rt = self.0.clone();
        s.command::<r::QuickCancel, _, _>(move |_, _, a: r::QuickCancelArgs| {
            let rt = rt.clone();
            async move {
                let judge = rt.runs.lock().unwrap().get(&a.run_id).cloned().unwrap_or_default();
                rt.log(format!("stop:{judge}"));
                rt.cancelled.send_modify(|c| c.push(a.run_id));
                Ok(r::QuickCancelled { stopped: true })
            }
        });
        let rt = self.0.clone();
        s.command::<r::SetExt, _, _>(move |_, _, a: r::SetExtArgs| {
            let rt = rt.clone();
            async move {
                let mut t = rt.task.lock().unwrap();
                let now = t.ext.get(&a.plugin).cloned().unwrap_or(Value::Null);
                if a.expect.as_ref().is_some_and(|e| e.value != now) {
                    return Err(WireError::conflict("changed meanwhile"));
                }
                t.ext.insert(a.plugin.clone(), a.value.clone());
                Ok(r::Ack {})
            }
        });
        let rt = self.0.clone();
        s.command::<r::FollowUp, _, _>(move |_, _, _a| {
            rt.log("followUp");
            async move { Ok(r::FollowedUp { turn_id: None, queued: None, interrupts: false }) }
        });
        let rt = self.0.clone();
        s.command::<r::Ask, _, _>(move |_, _, _a| {
            rt.log("ask");
            rt.task.lock().unwrap().state = SessionState::Waiting;
            async move { Ok(r::Asked { ask_id: "q1".into() }) }
        });
        // as runtime.settle: a task held in Working lands (Needs review, unread, one `turn.finished`) unless an
        // after-turn step wants its last turn again
        let rt = self.0.clone();
        s.command::<r::Settle, _, _>(move |ctx, _, _a: r::SettleArgs| {
            let rt = rt.clone();
            async move {
                let task = rt.task();
                if task.state != SessionState::Running {
                    return Ok(r::Ack {});
                }
                let again = ctx.contributions_by(&AFTER).into_iter().find(|(_, a)| a.wants(&task, &last_turn(), DONE));
                if let Some((_, step)) = again {
                    rt.log("held");
                    tokio::spawn(step.run(task, last_turn()));
                } else {
                    let mut t = rt.task.lock().unwrap();
                    t.state = SessionState::Review;
                    t.unread = true;
                    drop(t);
                    rt.log("landed");
                }
                Ok(r::Ack {})
            }
        });
        Ok(())
    }
}

/// A task held in Working at its turn's end, with a goal judged by `judges` (each answering as said) under `vote`.
async fn start(judges: Judges, vote: &str) -> (Kernel, Rt) {
    let mut task = Session { id: "t1".into(), cwd: "/tmp".into(), title: "t".into(), state: SessionState::Running, ..Session::default() };
    let names: Vec<&str> = judges.keys().copied().collect();
    task.ext.insert("wisp.runtime".into(), json!({ "spec": { "goal": { "text": "a.txt says hello", "judges": names, "vote": vote } } }));
    boot(task, judges, false).await
}

async fn boot(task: Session, judges: Judges, listed: bool) -> (Kernel, Rt) {
    let rt = Rt {
        task: Arc::new(Mutex::new(task)),
        judges,
        cancelled: watch::channel(Vec::new()).0,
        runs: Arc::default(),
        log: Arc::default(),
        listed,
    };
    let k = TestApp::new().with(GoalPlugin).with(Runtime(rt.clone())).start().await;
    (k, rt)
}

/// wisp starts again with the task in Working, its last turn done, its runtime slot `runtime` (the goal's spec added)
/// and its goal at `goal` (no `judging` mark, no check's record): a round a restart cut.
async fn restarted(judges: Judges, mut runtime: Value, goal: Value) -> (Kernel, Rt) {
    let mut task = Session { id: "t1".into(), cwd: "/tmp".into(), title: "t".into(), state: SessionState::Running, ..Session::default() };
    let names: Vec<&str> = judges.keys().copied().collect();
    runtime["spec"] = json!({ "goal": { "text": "a.txt says hello", "judges": names, "vote": "majority" } });
    task.ext.insert("wisp.runtime".into(), runtime);
    if !goal.is_null() {
        task.ext.insert("wisp.goal".into(), goal);
    }
    boot(task, judges, true).await
}

/// The goal's after-turn step.
fn round(k: &Kernel) -> Arc<dyn AfterTurn> {
    let ctx = k.ctx("wisp.runtime").unwrap();
    ctx.contributions_by(&AFTER).into_iter().map(|(_, a)| a).find(|a| a.id() == "wisp.goal.round").expect("the goal's round step")
}

/// The turn ends: runtime finds the round wants it, and runs it.
fn turn_ends(k: &Kernel, rt: &Rt) {
    let step = round(k);
    assert!(step.wants(&rt.task(), &last_turn(), DONE), "the round holds the task");
    tokio::spawn(step.run(rt.task(), last_turn()));
}

async fn until(what: &str, f: impl Fn() -> bool) {
    let at = Instant::now();
    while !f() {
        assert!(at.elapsed() < Duration::from_secs(10), "{what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn judges(list: &[(&'static str, u64, &'static str)]) -> Judges {
    list.iter().map(|(n, ms, t)| (*n, (*ms, *t))).collect()
}

/// The goal was judged from `turn.ended` after the turn had landed: the task sat in Needs review, unread, announced,
/// while its judges still voted, and every round said done. It stays in Working until the verdict, then lands once.
#[tokio::test(flavor = "multi_thread")]
async fn a_judged_round_holds_its_task_in_working_and_met_lands_it_once() {
    let (k, rt) = start(judges(&[("a", 300, "MET"), ("b", 300, "MET"), ("c", 300, "MET")]), "majority").await;
    turn_ends(&k, &rt);
    until("the judges vote", || rt.count("judge:a") + rt.count("judge:b") + rt.count("judge:c") == 3).await;
    let t = rt.task();
    assert_eq!((t.state, t.unread), (SessionState::Running, false), "in Working, not unread, while they vote");
    assert_eq!(rt.count("landed"), 0, "nothing announced before the verdict");
    until("it lands", || rt.count("landed") == 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let t = rt.task();
    assert_eq!((t.state, t.unread), (SessionState::Review, true));
    assert_eq!(rt.count("landed"), 1, "once: {:?}", rt.seen());
    assert_eq!(rt.count("held"), 0, "the round landing it never takes it back");
    assert_eq!(rt.goal()["state"], "met");
}

/// Another round: its follow-up starts the next turn while the task is still in Working; it never lands in between.
#[tokio::test(flavor = "multi_thread")]
async fn another_round_follows_up_without_passing_through_review() {
    let (k, rt) = start(judges(&[("a", 10, "NOT MET: no test yet")]), "majority").await;
    turn_ends(&k, &rt);
    until("the follow-up", || rt.count("followUp") == 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(rt.count("landed"), 0, "{:?}", rt.seen());
    assert_eq!(rt.task().state, SessionState::Running);
    assert_eq!(rt.goal()["state"], "again");
}

/// A stopped or failed turn, a paused goal, a met one or a round already under way isn't held.
#[tokio::test(flavor = "multi_thread")]
async fn only_a_done_turn_of_a_goal_still_worked_on_is_held() {
    let (k, rt) = start(judges(&[("a", 10, "MET")]), "majority").await;
    let step = round(&k);
    let t = rt.task();
    assert!(step.wants(&t, &last_turn(), DONE));
    assert!(!step.wants(&t, &last_turn(), TurnEnd { stopped: true, is_error: false }), "stopped");
    assert!(!step.wants(&t, &Turn { status: TurnStatus::Error, ..last_turn() }, TurnEnd { stopped: false, is_error: true }), "failed");
    for state in ["paused", "met", "cleared"] {
        let mut s = t.clone();
        s.ext.insert("wisp.goal".into(), json!({ "state": state, "rounds": 1 }));
        assert!(!step.wants(&s, &last_turn(), DONE), "{state}");
    }
    let mut judging = t.clone();
    judging.ext.insert("wisp.goal".into(), json!({ "state": "judging", "rounds": 0, "running": [{ "run": "r0", "name": "a", "at": 1 }] }));
    assert!(!step.wants(&judging, &last_turn(), DONE), "a round already under way");
    let mut none = t;
    none.ext.insert("wisp.runtime".into(), json!({ "spec": {} }));
    assert!(!step.wants(&none, &last_turn(), DONE), "no goal");
}

/// You stop the task mid-round: its judges stop, and their cut-short votes decide nothing (no follow-up, no question).
#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_task_mid_round_stops_its_judges_and_decides_nothing() {
    let (k, rt) = start(judges(&[("a", 5_000, "MET"), ("b", 5_000, "MET")]), "majority").await;
    turn_ends(&k, &rt);
    until("the judges vote", || rt.count("judge:a") + rt.count("judge:b") == 2).await;
    // as runtime.stop does: the after-turn steps are told, and a task with no turn running lands
    let step = round(&k);
    rt.task.lock().unwrap().state = SessionState::Review;
    assert!(step.cancel(&rt.task(), true, None), "the round held it");
    until("both judges stop", || rt.count("stop:a") + rt.count("stop:b") == 2).await;
    until("the round lets go", || rt.goal()["state"] != "judging").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!((rt.count("followUp"), rt.count("ask")), (0, 0), "{:?}", rt.seen());
    assert!(!step.cancel(&rt.task(), true, None), "nothing held now");
}

/// A round took the sum of its judges' times (each up to five minutes): they vote at once.
#[tokio::test(flavor = "multi_thread")]
async fn the_judges_vote_at_once() {
    let n = 400;
    let (k, rt) = start(judges(&[("a", n, "MET"), ("b", n, "MET"), ("c", n, "MET")]), "majority").await;
    let at = Instant::now();
    turn_ends(&k, &rt);
    until("it lands", || rt.count("landed") == 1).await;
    let took = at.elapsed();
    assert!(took < Duration::from_millis(2 * n), "a round of three {n} ms judges took {took:?}");
}

/// `all` with one no: the outcome is fixed, the rest stop, and their records say they weren't needed (not a failure).
#[tokio::test(flavor = "multi_thread")]
async fn all_with_one_no_stops_the_rest() {
    let (k, rt) = start(judges(&[("a", 10, "NOT MET: no test yet"), ("b", 5_000, "MET"), ("c", 5_000, "MET")]), "all").await;
    turn_ends(&k, &rt);
    until("another round", || rt.count("followUp") == 1).await;
    assert_eq!((rt.count("stop:b"), rt.count("stop:c")), (1, 1), "{:?}", rt.seen());
    let g = rt.goal();
    assert_eq!(g["state"], "again", "{g}");
    let v = g["verdicts"].as_array().cloned().unwrap_or_default();
    assert_eq!(v.iter().map(|x| x["judge"].as_str().unwrap_or_default()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(v[0]["why"], "no test yet");
    for x in &v[1..] {
        assert_eq!(x["skipped"], true, "{x}");
        assert!(x["why"].as_str().is_some_and(|w| w.ends_with("not needed: the vote was already decided")), "{x}");
    }
}

/// `any` with one yes: met at once, the rest stop.
#[tokio::test(flavor = "multi_thread")]
async fn any_with_one_yes_stops_the_rest() {
    let (k, rt) = start(judges(&[("a", 5_000, "NOT MET: x"), ("b", 10, "MET"), ("c", 5_000, "NOT MET: y")]), "any").await;
    let at = Instant::now();
    turn_ends(&k, &rt);
    until("it lands", || rt.count("landed") == 1).await;
    assert!(at.elapsed() < Duration::from_secs(3), "it didn't wait for the slow ones");
    assert_eq!((rt.count("stop:a"), rt.count("stop:c")), (1, 1), "{:?}", rt.seen());
    assert_eq!(rt.goal()["state"], "met");
}

/// The goal dropped mid-round: every judge still running stops.
#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_goal_mid_round_stops_every_judge() {
    let (k, rt) = start(judges(&[("a", 5_000, "MET"), ("b", 5_000, "MET"), ("c", 5_000, "MET")]), "majority").await;
    turn_ends(&k, &rt);
    until("the judges vote", || rt.count("judge:a") + rt.count("judge:b") + rt.count("judge:c") == 3).await;
    k.call(Caller::user(), "goal.drop", json!({ "id": "t1" })).await.unwrap();
    until("every judge stops", || ["a", "b", "c"].iter().all(|j| rt.count(&format!("stop:{j}")) == 1)).await;
    assert_eq!(rt.goal()["state"], "cleared");
}

/// The verdicts are in the goal's order of judges, whatever order they ended in.
#[tokio::test(flavor = "multi_thread")]
async fn the_verdicts_keep_the_judges_order() {
    let (k, rt) = start(judges(&[("a", 300, "NOT MET: a says"), ("b", 10, "NOT MET: b says"), ("c", 150, "MET")]), "majority").await;
    turn_ends(&k, &rt);
    until("another round", || rt.count("followUp") == 1).await;
    let v = rt.goal()["verdicts"].as_array().cloned().unwrap_or_default();
    let got: Vec<(&str, bool)> = v.iter().map(|x| (x["judge"].as_str().unwrap_or_default(), x["met"] == true)).collect();
    assert_eq!(got, [("a", false), ("b", false), ("c", true)]);
}

/// A restart after the round took the task but before it wrote `judging`: the task sat in Working for ever. It's
/// judged again, and lands once on met.
#[tokio::test(flavor = "multi_thread")]
async fn a_round_cut_before_judging_is_judged_again_after_a_restart() {
    let (_k, rt) = restarted(judges(&[("a", 10, "MET")]), json!({}), Value::Null).await;
    until("it lands", || rt.count("landed") == 1).await;
    assert_eq!(rt.count("judge:a"), 1, "{:?}", rt.seen());
    let t = rt.task();
    assert_eq!((t.state, t.unread), (SessionState::Review, true));
    assert_eq!(rt.goal()["state"], "met");
}

/// A restart after the verdict (met) was written but before the round's landing: the task lands, not judged again.
#[tokio::test(flavor = "multi_thread")]
async fn a_round_cut_after_its_verdict_lands_after_a_restart() {
    let goal = json!({ "state": "met", "rounds": 1, "text": "a.txt says hello" });
    let (_k, rt) = restarted(judges(&[("a", 10, "MET")]), json!({}), goal).await;
    until("it lands", || rt.count("landed") == 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(rt.count("judge:a"), 0, "{:?}", rt.seen());
    assert_eq!(rt.count("landed"), 1);
    assert_eq!(rt.task().state, SessionState::Review);
}

/// A task in Working that waits on its delegates' reports isn't a cut round: a restart leaves it as it is.
#[tokio::test(flavor = "multi_thread")]
async fn a_lead_waiting_on_its_delegates_is_left_alone_after_a_restart() {
    let (_k, rt) = restarted(judges(&[("a", 10, "MET")]), json!({ "waitsOn": ["Bob"] }), Value::Null).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rt.seen().is_empty(), "{:?}", rt.seen());
    assert_eq!(rt.task().state, SessionState::Running);
}
