//! The crew's commands against a stand-in runtime whose every answer awaits once (as a real one may): what another
//! command writes while one is awaiting is kept, and a member removed meanwhile stays removed.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::json;
use wisp_crew_api::QUEUE_MAX;
use wisp_kernel::{Declarative, Manifest, Plugin, Setup};
use wisp_runtime_api::model::{Session, SessionState};
use wisp_testkit::{FakeClock, TestApp};

use super::*;
use crate::CrewPlugin;

/// wisp.runtime with tasks in memory: `runtime.get`, `.create` and `.profiles`, each yielding once first. `.create`
/// fails while `.1` says so.
#[derive(Clone, Default)]
struct Rt(Arc<Mutex<BTreeMap<String, Session>>>, Arc<std::sync::atomic::AtomicBool>);

impl Rt {
    fn task(&self, id: &str, state: SessionState, job: JobKind) {
        let mut s = Session { id: id.into(), state, ..Session::default() };
        s.ext.insert(PLUGIN_ID.into(), json!({ "id": "x", "job": job }));
        lock(&self.0).insert(id.to_owned(), s);
    }
    fn made(&self) -> Vec<Session> {
        lock(&self.0).values().filter(|s| s.id.as_str().starts_with("made-")).cloned().collect()
    }
}

impl Plugin for Rt {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.runtime", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> wisp_kernel::Result<()> {
        let me = self.clone();
        s.command::<rt::Get, _, _>(move |_c, _call, a: rt::IdArgs| {
            let me = me.clone();
            async move {
                tokio::task::yield_now().await;
                lock(&me.0).get(a.id.as_str()).cloned().ok_or_else(|| WireError::not_found("That task is gone."))
            }
        });
        let me = self.clone();
        s.command::<rt::Create, _, _>(move |_c, _call, a: rt::StartArgs| {
            let me = me.clone();
            async move {
                tokio::task::yield_now().await;
                if me.1.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(WireError::unavailable("the harness couldn't start"));
                }
                let mut t = lock(&me.0);
                let id = format!("made-{}", t.len());
                let mut s = Session { id: id.as_str().into(), state: SessionState::Running, ..Session::default() };
                s.ext = a.ext;
                (s.cwd, s.dirs, s.fallbacks) = (a.cwd.unwrap_or_default(), a.dirs, a.fallbacks);
                t.insert(id.clone(), s);
                Ok(rt::Created { id: id.as_str().into(), turn_id: None })
            }
        });
        s.command::<rt::Profiles, _, _>(move |_c, _call, _a: Value| async move {
            tokio::task::yield_now().await;
            Ok(Vec::new())
        });
        Ok(())
    }
}

/// wisp.automations offering a few sources.
struct Sources;

impl Plugin for Sources {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.automations", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> wisp_kernel::Result<()> {
        use wisp_automations_api::commands::{Empty, SourceRow, Sources as S};
        s.command::<S, _, _>(|_c, _call, _a: Empty| async move {
            let row = |on: &str| SourceRow {
                plugin: "wisp.automations".into(),
                available: true,
                on: wisp_automations_api::SourceOn {
                    title: on.into(),
                    on: on.into(),
                    fields: json!({}),
                    mode: "event".into(),
                    every: None,
                },
            };
            Ok(vec![row("schedule"), row("pr.opened"), row("task.finished")])
        });
        Ok(())
    }
}

const T0: i64 = 1_700_000_000_000;

/// A kernel with the stand-ins, the crew's manifest (its permissions) and its context; the crew's state, empty.
async fn kernel(rt: &Rt) -> (wisp_kernel::Kernel, Ctx, Arc<State>, FakeClock) {
    let clock = FakeClock::at(T0);
    let k = TestApp::new().clock(&clock).with(rt.clone()).with(Sources).with(Declarative(CrewPlugin.manifest())).start().await;
    let ctx = k.ctx(PLUGIN_ID).unwrap();
    (k, ctx, Arc::new(State::new()), clock)
}

/// Ada, one job at a time, already running one: a job for her waits.
fn busy_ada(st: &State, rt: &Rt) -> Member {
    let m =
        Member { id: "ada1".into(), name: "Ada".into(), agent: "claude".into(), mode: "edits".into(), parallel: 1, ..Member::default() };
    st.insert(m.clone());
    rt.task("live1", SessionState::Running, JobKind::Task);
    lock(&st.jobs).insert(m.id.clone(), vec!["live1".into()]);
    m
}

fn job(prompt: &str) -> WaitingJob {
    work(prompt.into(), JobKind::Task, Vec::new(), None)
}

/// Two jobs given to Ada at once, at her limit: each read her, awaited her jobs, and wrote back its own copy's queue,
/// so one of the two was lost. Both wait.
#[tokio::test]
async fn two_jobs_given_at_once_both_wait() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = busy_ada(&st, &rt);
    let call = ctx.root_call();
    let (a, b) = tokio::join!(assign(&ctx, &call, &st, &m, job("one")), assign(&ctx, &call, &st, &m, job("two")));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(a.queued.is_some() && b.queued.is_some());
    let mut queue: Vec<String> = st.member("ada1").unwrap().queue.into_iter().map(|w| w.prompt).collect();
    queue.sort();
    assert_eq!(queue, ["one", "two"]);
    let mut at = [a.position, b.position];
    at.sort();
    assert_eq!(at, [Some(1), Some(2)], "each its own place");
}

/// Ada, idle, one job at a time.
fn idle_ada(st: &State) -> Member {
    let m =
        Member { id: "ada1".into(), name: "Ada".into(), agent: "claude".into(), mode: "edits".into(), parallel: 1, ..Member::default() };
    st.insert(m.clone());
    m
}

/// Two jobs given at once to idle Ada (one at a time): each read her idle before the other's job was recorded, and
/// both started. One starts; the other waits first in line.
#[tokio::test]
async fn two_jobs_given_at_once_to_an_idle_member_start_one() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = idle_ada(&st);
    let call = ctx.root_call();
    let (a, b) = tokio::join!(assign(&ctx, &call, &st, &m, job("one")), assign(&ctx, &call, &st, &m, job("two")));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(rt.made().len(), 1, "one started: {:?}", rt.made());
    assert_eq!([a.session.is_some(), b.session.is_some()].iter().filter(|x| **x).count(), 1);
    let waits = if a.queued.is_some() { a } else { b };
    assert_eq!(waits.position, Some(1), "the other waits first in line");
    assert_eq!(st.member("ada1").unwrap().queue.len(), 1);
    assert!(lock(&st.starting).is_empty(), "every place reserved was given back");
}

/// Two jobs handed out at once by leads to idle Ada (one at a time): both started. One starts; the other is refused,
/// busy, as a lead's job never waits in the queue.
#[tokio::test]
async fn two_jobs_handed_out_at_once_to_an_idle_member_start_one() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = idle_ada(&st);
    let call = ctx.root_call();
    let handed = |p: &str| WaitingJob { job: JobKind::Delegated, ..job(p) };
    let (a, b) = tokio::join!(delegated(&ctx, &call, &st, &m, handed("one"), None), delegated(&ctx, &call, &st, &m, handed("two"), None));
    assert_eq!(rt.made().len(), 1, "one started");
    let refused = if a.is_ok() { b.unwrap_err() } else { a.unwrap_err() };
    assert!(refused.message.contains("already running 1 job, their limit"), "{}", refused.message);
    assert!(st.member("ada1").unwrap().queue.is_empty(), "never queued");
}

/// A job whose start fails gives its place back and waits first in line (it was lost): the next one given starts.
#[tokio::test]
async fn a_job_that_fails_to_start_gives_its_place_back() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = idle_ada(&st);
    let call = ctx.root_call();
    rt.1.store(true, std::sync::atomic::Ordering::SeqCst);
    let failed = assign(&ctx, &call, &st, &m, job("one")).await.unwrap();
    assert_eq!(failed.position, Some(1), "it waits first in line: {failed:?}");
    assert!(failed.session.is_none());
    assert!(lock(&st.starting).is_empty());
    let queue = st.member("ada1").unwrap().queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].prompt, "one");
    assert_eq!(Some(&queue[0].id), failed.queued.as_ref());
    rt.1.store(false, std::sync::atomic::Ordering::SeqCst);
    let next = assign(&ctx, &call, &st, &m, job("two")).await.unwrap();
    assert!(next.session.is_some(), "it started: {next:?}");
}

/// An edit read Ada before a drain started her waiting job, and wrote back the queue it had read: the job ran and
/// waited again. The edit lands on the queue as it is.
#[tokio::test]
async fn an_edit_read_before_a_drain_doesnt_put_the_started_job_back() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = Member {
        id: "ada1".into(),
        name: "Ada".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 1,
        queue: vec![WaitingJob { id: "w1".into(), ..job("waiting") }],
        ..Member::default()
    };
    st.insert(m);
    let call = ctx.root_call();
    let a = c::UpdateArgs {
        id: "ada1".into(),
        member: c::MemberBody { role: Some("Reviewer".into()), ..c::MemberBody::default() },
        ..c::UpdateArgs::default()
    };
    let (edit, ()) = tokio::join!(update(&ctx, &call, &st, &a), drain(&ctx, &call, &st, "ada1"));
    edit.unwrap();
    let now = st.member("ada1").unwrap();
    assert_eq!(now.role, "Reviewer");
    assert!(now.queue.is_empty(), "the job started: {:?}", now.queue);
    assert_eq!(rt.made().len(), 1, "and ran once");
}

/// Ada removed while a job for her was being placed: the write after it brought her back.
#[tokio::test]
async fn a_member_removed_while_a_job_is_placed_stays_removed() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = busy_ada(&st, &rt);
    let call = ctx.root_call();
    let (r, ()) = tokio::join!(assign(&ctx, &call, &st, &m, job("one")), async { st.remove("ada1") });
    assert_eq!(r.unwrap_err().message, "No such crew member.");
    assert!(st.member("ada1").is_err(), "still gone");
    st.put(m);
    assert!(st.member("ada1").is_err(), "a copy written back doesn't bring her back");
}

#[test]
fn an_update_of_a_member_gone_writes_nothing() {
    let st = State::new();
    assert_eq!(st.update("nobody", |m| m.paused = true), None);
    st.insert(Member { id: "ada1".into(), name: "Ada".into(), ..Member::default() });
    assert_eq!(st.update("ada1", |m| std::mem::replace(&mut m.paused, true)), Some(false));
    assert!(st.member("ada1").unwrap().paused);
}

/// A wake-up came while Ada was still checking in: the check-in was refused ("already checking in") and the wake-up was
/// gone with it. It's kept until a check-in starts for it.
#[tokio::test]
async fn a_wake_up_refused_while_checking_in_fires_once_that_check_in_ends() {
    let rt = Rt::default();
    let (_k, ctx, st, clock) = kernel(&rt).await;
    let m = Member {
        id: "ada1".into(),
        name: "Ada".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 2,
        wake_at: Some(T0 - 1),
        ..Member::default()
    };
    st.insert(m);
    rt.task("check1", SessionState::Running, JobKind::Checkin);
    lock(&st.jobs).insert("ada1".into(), vec!["check1".into()]);
    run_tick(&ctx, &st).await;
    assert_eq!(st.member("ada1").unwrap().wake_at, Some(T0 - 1), "kept: its check-in was refused");
    assert!(rt.made().is_empty());
    rt.task("check1", SessionState::Done, JobKind::Checkin);
    clock.advance(20_000);
    run_tick(&ctx, &st).await;
    assert!(rt.made().is_empty(), "the same minute: asked again the next one");
    clock.advance(40_000);
    run_tick(&ctx, &st).await;
    let made = rt.made();
    assert_eq!(made.len(), 1, "the wake-up's check-in");
    assert_eq!(made[0].ext[PLUGIN_ID]["job"], json!("checkin"));
    assert_eq!(st.member("ada1").unwrap().wake_at, None, "used up");
}

/// `@ada @bob` with Ada's queue full: her refusal stopped the call, and Bob never woke. Bob gets his job.
#[tokio::test]
async fn a_mention_wakes_the_rest_when_one_member_cant_take_it() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let ada = busy_ada(&st, &rt);
    st.update(&ada.id, |m| m.queue = (0..QUEUE_MAX).map(|i| WaitingJob { id: format!("w{i}"), ..job("waiting") }).collect());
    st.insert(Member {
        id: "bob1".into(),
        name: "Bob".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 1,
        ..Member::default()
    });
    let call = ctx.root_call();
    let a = c::MentionArgs { text: "@ada @bob please look".into(), source: None };
    let out = wake_mentions(&ctx, &call, &st, &a).await;
    assert_eq!(out.len(), 1, "Bob's");
    assert!(out[0].session.is_some(), "Bob had room: it started");
    assert_eq!(st.member("ada1").unwrap().queue.len(), QUEUE_MAX);
    assert_eq!(rt.made().len(), 1);
}

/// A wake on an `on` no source offers never fires: `crew.create` refuses it, as the triggers kind does.
#[tokio::test]
async fn a_wake_on_no_source_is_refused() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let call = ctx.root_call();
    let body = |on: &str| c::CreateArgs {
        member: c::MemberBody {
            name: Some("Ada".into()),
            agent: Some("claude".into()),
            wakes: Some(vec![serde_json::from_value(json!({ "on": on })).unwrap()]),
            ..c::MemberBody::default()
        },
        ..c::CreateArgs::default()
    };
    let e = create(&ctx, &call, &st, body("not.an.event")).await.unwrap_err();
    assert!(e.message.starts_with("wakes[0].on: not.an.event is no source"), "{}", e.message);
    assert!(lock(&st.saved).crew.is_empty(), "no one joined");
    assert!(create(&ctx, &call, &st, body("pr.opened")).await.is_ok());
    let bad = vec![
        serde_json::from_value(json!({ "on": "pr.opened" })).unwrap(),
        serde_json::from_value(json!({ "on": "not.an.event" })).unwrap(),
    ];
    let e = unknown_sources(&ctx, &call, &bad, &[]).await.unwrap_err();
    assert!(e.message.starts_with("wakes[1].on: not.an.event is no source"), "{}", e.message);
    assert!(e.message.contains("pr.opened, schedule, task.finished"), "{}", e.message);
    assert!(unknown_sources(&ctx, &call, &bad, &bad).await.is_ok(), "one the member had already isn't asked again");
    assert!(unknown_sources(&ctx, &call, &bad[..1], &[]).await.is_ok());
}

fn waiting_ada(st: &State) {
    st.insert(Member {
        id: "ada1".into(),
        name: "Ada".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 1,
        queue: vec![WaitingJob { id: "w1".into(), ..job("waiting") }],
        ..Member::default()
    });
}

fn agent_edit_args(fields: c::MemberBody) -> c::AgentEditArgs {
    c::AgentEditArgs { session: "s1".into(), member: "Ada".into(), remember: None, fields }
}

/// The agent's edit (`wisp_crew_edit`) read Ada before a drain started her waiting job, and wrote back the queue it had
/// read: the job ran and waited again. The edit lands on the queue as it is.
#[tokio::test]
async fn an_agent_edit_read_before_a_drain_doesnt_put_the_started_job_back() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    waiting_ada(&st);
    let call = ctx.root_call();
    let a = agent_edit_args(c::MemberBody { role: Some("Reviewer".into()), ..c::MemberBody::default() });
    let (edit, ()) = tokio::join!(crate::agent::agent_edit(&ctx, &call, &st, &a), drain(&ctx, &call, &st, "ada1"));
    assert!(edit.error.is_none(), "{edit:?}");
    let now = st.member("ada1").unwrap();
    assert_eq!(now.role, "Reviewer");
    assert!(now.queue.is_empty(), "the job started: {:?}", now.queue);
    assert_eq!(rt.made().len(), 1, "and ran once");
}

/// A job given to Ada while the agent's edit awaited: the edit's copy had no such job, and writing it back lost it.
#[tokio::test]
async fn a_job_queued_during_an_agent_edit_still_waits() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = busy_ada(&st, &rt);
    let call = ctx.root_call();
    let a = agent_edit_args(c::MemberBody { role: Some("Reviewer".into()), ..c::MemberBody::default() });
    let (edit, queued) = tokio::join!(crate::agent::agent_edit(&ctx, &call, &st, &a), assign(&ctx, &call, &st, &m, job("one")));
    assert!(edit.error.is_none(), "{edit:?}");
    assert!(queued.unwrap().queued.is_some());
    let now = st.member("ada1").unwrap();
    assert_eq!(now.role, "Reviewer");
    assert_eq!(now.queue.iter().map(|w| w.prompt.as_str()).collect::<Vec<_>>(), ["one"]);
}

/// Ada removed while an edit (the app's or the agent's) awaited the harnesses: the write after it brought her back.
#[tokio::test]
async fn a_member_removed_during_an_edit_stays_removed() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    waiting_ada(&st);
    let call = ctx.root_call();
    let body = c::MemberBody { role: Some("Reviewer".into()), ..c::MemberBody::default() };
    let a = c::UpdateArgs { id: "ada1".into(), member: body.clone(), ..c::UpdateArgs::default() };
    let (r, ()) = tokio::join!(update(&ctx, &call, &st, &a), async { st.remove("ada1") });
    assert_eq!(r.unwrap_err().message, "No such crew member.");
    assert!(st.member("ada1").is_err(), "still gone after the app's edit");
    waiting_ada(&st);
    let a = agent_edit_args(body);
    let (r, ()) = tokio::join!(crate::agent::agent_edit(&ctx, &call, &st, &a), async { st.remove("ada1") });
    assert_eq!(r.error.as_deref(), Some("No such crew member."));
    assert!(st.member("ada1").is_err(), "still gone after the agent's edit");
}

/// The agent's tools refuse a wake on no source as `crew.create` / `crew.update` do: the tool, the app and apply agree.
#[tokio::test]
async fn an_agents_wake_on_no_source_is_refused() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let call = ctx.root_call();
    let wakes = |on: &str| Some(vec![serde_json::from_value(json!({ "on": on })).unwrap()]);
    let add = |on: &str| c::AgentAddArgs {
        session: "s1".into(),
        member: c::MemberBody {
            name: Some("Ada".into()),
            agent: Some("claude".into()),
            cwd: Some(std::env::temp_dir().to_string_lossy().into_owned()),
            wakes: wakes(on),
            ..c::MemberBody::default()
        },
        ..c::AgentAddArgs::default()
    };
    let out = crate::agent::agent_add(&ctx, &call, &st, &add("not.an.event")).await;
    assert!(out.error.as_deref().unwrap_or_default().starts_with("wakes[0].on: not.an.event is no source"), "{out:?}");
    assert!(lock(&st.saved).crew.is_empty(), "no one joined");
    let out = crate::agent::agent_add(&ctx, &call, &st, &add("pr.opened")).await;
    assert!(out.error.is_none(), "{out:?}");
    let edit = agent_edit_args(c::MemberBody { wakes: wakes("not.an.event"), ..c::MemberBody::default() });
    let out = crate::agent::agent_edit(&ctx, &call, &st, &edit).await;
    assert!(out.error.as_deref().unwrap_or_default().starts_with("wakes[0].on: not.an.event is no source"), "{out:?}");
    assert_eq!(st.member("Ada").unwrap().wakes[0].on, "pr.opened", "unchanged");
}

/// wisp.sandbox on a grid at its per-user cap: `sandbox.place` refused as the grid refuses it (its words, its code).
struct CappedGrid;

impl Plugin for CappedGrid {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(
            json!({ "wispPlugin": 1, "id": "wisp.sandbox", "version": "1.0.0-alpha.1", "api": "^1", "activation": ["onStartup"] }),
        )
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> wisp_kernel::Result<()> {
        s.command::<wisp_sandbox_api::Place, _, _>(|_c, _call, _a: wisp_sandbox_api::PlaceArgs| async move {
            Err::<wisp_sandbox_api::Placed, _>(
                WireError::invalid("You have 10 sandboxes, the most this grid keeps. Free one in Settings → Computers → Sandboxes first.")
                    .with_details(json!({ "code": wisp_sandbox_api::CAP_CODE, "have": 10, "max": 10 })),
            )
        });
        Ok(())
    }
}

/// A member whose jobs each get a sandbox, on a grid at its cap: the job waits its turn again. (Before, the grid's
/// "the most this grid keeps" wasn't read as full: the job was logged and lost.)
#[tokio::test]
async fn a_job_that_meets_the_grid_s_sandbox_cap_waits_again() {
    let rt = Rt::default();
    let clock = FakeClock::at(T0);
    let k = TestApp::new().clock(&clock).with(rt.clone()).with(CappedGrid).with(Declarative(CrewPlugin.manifest())).start().await;
    let ctx = k.ctx(PLUGIN_ID).unwrap();
    let st = State::new();
    st.insert(Member {
        id: "ada1".into(),
        name: "Ada".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 1,
        sandbox: "job".into(),
        queue: vec![WaitingJob { id: "w1".into(), ..job("waiting") }],
        ..Member::default()
    });
    drain(&ctx, &ctx.root_call(), &st, "ada1").await;
    let queue: Vec<String> = st.member("ada1").unwrap().queue.into_iter().map(|w| w.id).collect();
    assert_eq!(queue, ["w1"], "it waits again");
    assert!(rt.made().is_empty(), "nothing ran");
}

/// A member's other folders and fallbacks go with every job of theirs: the job starts in their folder, may work in
/// the others, and carries on with the fallbacks in order at a usage limit (owner, 2026-11-27).
#[tokio::test]
async fn a_job_gets_the_members_folders_and_fallbacks() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let fb = |agent: &str, model: &str| wisp_crew_api::Fallback { agent: agent.into(), model: model.into(), effort: String::new() };
    let m = Member {
        id: "ada1".into(),
        name: "Ada".into(),
        agent: "claude".into(),
        mode: "edits".into(),
        parallel: 1,
        cwd: "/repo/app".into(),
        dirs: vec!["/repo/api".into(), "/repo/docs".into()],
        fallbacks: vec![fb("codex", "gpt-5.5"), fb("", "sonnet")],
        ..Member::default()
    };
    st.insert(m.clone());
    let call = ctx.root_call();
    assign(&ctx, &call, &st, &m, job("ship it")).await.unwrap();
    let made = rt.made();
    let s = made.first().expect("the job started");
    assert_eq!((s.cwd.as_str(), s.dirs.as_slice()), ("/repo/app", ["/repo/api".to_owned(), "/repo/docs".to_owned()].as_slice()));
    let picks: Vec<(Option<&str>, Option<&str>)> = s.fallbacks.iter().map(|f| (f.agent.as_deref(), f.model.as_deref())).collect();
    assert_eq!(picks, [(Some("codex"), Some("gpt-5.5")), (None, Some("sonnet"))]);
}

/// ADR-0148: a trigger's project is where a woken member's job works, not the member's own folder (a team hub's
/// triager wakes on an issue in the repo the trigger names: `crew.wake {cwd}`); with none, the member's own.
#[tokio::test]
async fn a_job_with_a_folder_works_there_and_without_one_in_the_members() {
    let rt = Rt::default();
    let (_k, ctx, st, _clock) = kernel(&rt).await;
    let m = Member {
        id: "tri1".into(),
        name: "Triager".into(),
        agent: "claude".into(),
        mode: "plan".into(),
        parallel: 2,
        cwd: "/repo/own".into(),
        ..Member::default()
    };
    st.insert(m.clone());
    let call = ctx.root_call();
    assign(&ctx, &call, &st, &m, WaitingJob { cwd: Some("/hub/repos/app/api".into()), ..job("an issue") }).await.unwrap();
    assign(&ctx, &call, &st, &m, job("another")).await.unwrap();
    let mut cwds: Vec<String> = rt.made().into_iter().map(|s| s.cwd).collect();
    cwds.sort();
    assert_eq!(cwds, ["/hub/repos/app/api", "/repo/own"]);
    assert_eq!(st.member("tri1").unwrap().cwd, "/repo/own", "the member's own folder is left as it was");
}
