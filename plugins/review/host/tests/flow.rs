//! The review flow (v1 createReview, and the review half of test/ship.test.js) on a fake port: when it
//! starts, holds or refuses, what a run keeps, and that an overtaken run keeps nothing.
#![allow(
    clippy::await_holding_lock,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::range_plus_one,
    clippy::format_push_string,
    clippy::needless_pass_by_value,
    clippy::needless_lifetimes,
    clippy::elidable_lifetime_names,
    clippy::option_option,
    clippy::doc_markdown,
    clippy::map_unwrap_or,
    clippy::format_collect
)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_git_api::FileDiff;
use wisp_kernel::{Call, Caller, Result, WireError};
use wisp_review::{Env, Fut, Reviewer, Task, boot, cut_short, task_of};
use wisp_review_api::{Pick, Review, ReviewFinished, Severity, Status, TestsRun};
use wisp_review_core::Agent;

#[derive(Default)]
struct State {
    task: Task,
    setting: String,
    head: Option<String>,
    files: Option<Vec<FileDiff>>,
    answer: Option<Value>,
    hold: Option<String>,
    /// the PR's head after the reviewer answers (a push meanwhile)
    head_after_ask: Option<String>,
    /// another run's record, written over the slot once the reviewer answers (a write between the run's last
    /// `live` check and its save: the claim is let go before the save)
    review_after_ask: Option<Review>,
    saved: Vec<(Option<Review>, bool)>,
    asked: Vec<(String, Pick)>,
    finished: Vec<ReviewFinished>,
    /// `task` reads, by id
    reads: Vec<String>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<State>>);

impl Fake {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
}

fn ready<T: Send + 'static>(v: T) -> Fut<T> {
    Box::pin(async move { v })
}

impl Env for Fake {
    fn task(&self, _call: &Call, id: &str) -> Fut<Result<Task>> {
        let mut st = self.st();
        st.reads.push(id.to_owned());
        let t = st.task.clone();
        ready(if t.id == id { Ok(t) } else { Err(WireError::not_found("no such session")) })
    }
    fn turns(&self, _call: &Call, _id: &str) -> Fut<(Vec<String>, String)> {
        ready((vec!["claude".into()], "Fix add".into()))
    }
    fn setting(&self, _call: &Call) -> Fut<String> {
        ready(self.st().setting.clone())
    }
    fn agents(&self, _call: &Call) -> Fut<Vec<Agent>> {
        ready(vec![
            Agent { id: "claude".into(), name: "Claude".into(), available: true },
            Agent { id: "codex".into(), name: "Codex".into(), available: true },
        ])
    }
    fn hold(&self, _call: &Call, _agent: &str) -> Fut<Option<String>> {
        ready(self.st().hold.clone())
    }
    fn constraints(&self, _call: &Call, _id: &str) -> Fut<Vec<String>> {
        ready(vec!["Don't touch migrations/".into()])
    }
    fn head(&self, _call: &Call, _dir: &str) -> Fut<Option<String>> {
        ready(self.st().head.clone())
    }
    fn changes(&self, _call: &Call, _dir: &str, _base: &str) -> Fut<Option<(Vec<FileDiff>, Vec<String>)>> {
        let f = self.st().files.clone();
        ready(f.map(|f| (f.clone(), f.iter().map(|x| x.path.clone()).collect())))
    }
    fn tests(&self, _call: &Call, _dir: &str, _cwd: &str) -> Fut<Option<TestsRun>> {
        ready(Some(TestsRun { verdict: "pass".into(), command: "npm test".into(), output: String::new(), passed: 4, failed: 0 }))
    }
    fn ask(&self, _call: &Call, prompt: String, _cwd: &str, pick: &Pick, _timeout_ms: u64) -> Fut<Option<Value>> {
        let mut st = self.st();
        st.asked.push((prompt, pick.clone()));
        if let Some(h) = st.head_after_ask.take() {
            st.task.pr_head = h;
        }
        if let Some(r) = st.review_after_ask.take() {
            st.task.review = Some(r);
        }
        ready(st.answer.clone())
    }
    fn save(&self, _call: &Call, _id: &str, review: Option<&Review>, unread: bool, over: Option<&Review>) -> Fut<Result<()>> {
        let mut st = self.st();
        // as runtime.setExt's `expect`: refused unless the slot still holds `over`
        if over.is_some_and(|o| st.task.review.as_ref() != Some(o)) {
            return ready(Err(WireError::conflict("changed meanwhile")));
        }
        st.saved.push((review.cloned(), unread));
        st.task.review = review.cloned();
        ready(Ok(()))
    }
    fn finished(&self, _call: &Call, e: &ReviewFinished) {
        self.st().finished.push(e.clone());
    }
    fn now(&self) -> i64 {
        42
    }
}

fn file() -> FileDiff {
    serde_json::from_value(json!({ "path": "src/add.js", "status": "M", "add": 1, "del": 1, "binary": false, "more": 0, "rows": [{ "op": "-", "text": "a - b", "a": 3 }, { "op": "+", "text": "a + b", "b": 3 }] })).unwrap()
}

fn fake() -> Fake {
    let task = Task {
        id: "s1".into(),
        title: "fix add".into(),
        agent: "claude".into(),
        pr_open: true,
        pr_head: "h1".into(),
        dir: "/wt".into(),
        cwd: "/wt".into(),
        base_ref: "origin/main".into(),
        ..Task::default()
    };
    let f = Fake::default();
    {
        let mut st = f.st();
        st.task = task;
        st.setting = "other".into();
        st.head = Some("h1".into());
        st.files = Some(vec![file()]);
        st.answer = Some(
            json!({ "summary": "One bug.", "findings": [{ "severity": "low", "text": "nit" }, { "severity": "medium", "file": "src/add.js", "line": 3, "text": "wrong sign", "constraint": 1 }] }),
        );
    }
    f
}

fn call() -> Call {
    Call::new_root(Caller::user())
}

async fn maybe(f: &Fake, r: &Arc<Reviewer>, force: bool) -> String {
    let s = r.maybe(&call(), "s1", force).await.unwrap();
    if let Some(job) = s.job {
        job.await;
    }
    let _ = f;
    s.why
}

#[tokio::test]
async fn a_due_review_runs_on_the_pr_head_and_keeps_findings_tests_and_coverage() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    assert_eq!(maybe(&f, &r, false).await, "");
    let st = f.st();
    // the reviewer is another harness than the author, and reads the diff, the tests and the constraint
    let (prompt, pick) = &st.asked[0];
    assert_eq!(pick.agent, "codex");
    assert!(
        prompt.contains("+a + b")
            && prompt.contains("<tests command=\"npm test\" verdict=\"pass\">")
            && prompt.contains("1. Don't touch migrations/")
    );
    // running first, then done, marked unread
    assert_eq!(st.saved[0].0.as_ref().unwrap().status, Status::Running);
    let (done, unread) = st.saved.last().unwrap();
    let done = done.as_ref().unwrap();
    assert!(*unread);
    assert_eq!((done.status, done.head.as_str(), done.summary.as_deref()), (Status::Done, "h1", Some("One bug.")));
    assert_eq!(done.findings[0].severity, Severity::High); // breaks the constraint
    assert_eq!(done.findings[0].rule.as_deref(), Some("Don't touch migrations/"));
    assert_eq!(done.tests.as_ref().unwrap().passed, 4);
    assert_eq!(done.coverage.as_ref().map(|c| (c.files, c.seen)), Some((1, 1)));
    assert_eq!((st.finished[0].status, st.finished[0].broken, st.finished[0].reviewer.as_str()), (Status::Done, 1, "codex"));
    drop(st);
    assert!(!r.is_running("s1"));
    // the same head again: nothing to do
    assert_eq!(maybe(&f, &r, false).await, "");
    assert_eq!(f.st().asked.len(), 1);
    // Review again (forced) runs it again
    assert_eq!(maybe(&f, &r, true).await, "");
    assert_eq!(f.st().asked.len(), 2);
}

#[tokio::test]
async fn no_open_pr_off_or_still_working_say_why() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().task.pr_open = false;
    assert_eq!(maybe(&f, &r, true).await, "No open pull request of the task’s own branch.");
    f.st().task.pr_open = true;
    f.st().setting = "off".into();
    assert_eq!(maybe(&f, &r, false).await, "Review by another harness is off.");
    f.st().task.live = true;
    assert_eq!(maybe(&f, &r, true).await, "Still working: it’s reviewed once the run ends.");
    assert!(f.st().saved.is_empty());
}

#[tokio::test]
async fn the_reviewers_account_past_stop_at_holds_it_a_forced_one_runs() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().hold = Some("Codex is at Automations' stop at.".into());
    assert_eq!(maybe(&f, &r, false).await, "Codex is at Automations' stop at.");
    let held = f.st().saved[0].0.clone().unwrap();
    assert_eq!((held.status, held.by.unwrap().agent.as_str()), (Status::Held, "codex"));
    assert!(!r.is_running("s1"));
    assert_eq!(maybe(&f, &r, true).await, "");
    assert_eq!(f.st().task.review.as_ref().unwrap().status, Status::Done);
}

#[tokio::test]
async fn no_reviewer_is_a_failed_review_with_why() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().setting = "grok".into();
    assert_eq!(maybe(&f, &r, false).await, "grok isn't installed here.");
    assert_eq!(f.st().task.review.as_ref().unwrap().status, Status::Failed);
}

#[tokio::test]
async fn commits_the_pr_doesnt_have_yet_fail_it_with_push_them() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().head = Some("h2".into());
    assert_eq!(maybe(&f, &r, false).await, "");
    let st = f.st();
    let r = st.task.review.as_ref().unwrap();
    assert_eq!(
        (r.status, r.why.as_deref()),
        (Status::Failed, Some("The branch has commits the PR doesn’t have yet: push them (Ship it) to review."))
    );
    assert!(st.asked.is_empty());
}

#[tokio::test]
async fn no_diff_or_no_answer_fail_it() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().files = None;
    maybe(&f, &r, false).await;
    assert_eq!(f.st().task.review.as_ref().unwrap().why.as_deref(), Some("Couldn’t read the PR’s diff"));
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().answer = Some(json!("junk"));
    maybe(&f, &r, false).await;
    let st = f.st();
    let review = st.task.review.as_ref().unwrap();
    assert_eq!((review.status, review.why.as_deref()), (Status::Failed, Some("Codex didn’t answer.")));
    assert!(review.tests.is_some() && review.coverage.is_some());
}

#[tokio::test]
async fn a_push_while_it_reviews_keeps_nothing_of_the_old_head() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    f.st().head_after_ask = Some("h2".into());
    maybe(&f, &r, false).await;
    let st = f.st();
    assert_eq!(st.saved.len(), 1); // only the running record
    assert!(st.finished.is_empty());
    assert!(!r.is_running("s1"));
}

#[tokio::test]
async fn two_callers_for_one_head_start_one_run() {
    let f = fake();
    let r = Reviewer::new(Arc::new(f.clone()));
    let a = r.maybe(&call(), "s1", false).await.unwrap();
    let b = r.maybe(&call(), "s1", false).await.unwrap();
    assert!(a.job.is_some() && b.job.is_none());
    assert_eq!(b.why, "");
    a.job.unwrap().await;
    assert_eq!(f.st().asked.len(), 1);
}

#[tokio::test]
async fn a_review_cut_short_by_a_restart_starts_over() {
    let f = fake();
    let running = Review { status: Status::Running, head: "h1".into(), ..Review::default() };
    f.st().task.review = Some(running);
    let r = Reviewer::new(Arc::new(f.clone()));
    boot(&r, &call(), vec!["s1".into()]).await;
    assert_eq!(f.st().saved, vec![(None, false)]);
    assert_eq!(maybe(&f, &r, false).await, "");
    assert_eq!(f.st().task.review.as_ref().unwrap().status, Status::Done);
}

#[test]
fn a_session_reads_as_a_task_from_its_ext_slots() {
    let s: wisp_runtime_api::model::Session = serde_json::from_value(json!({
        "id": "s1", "title": "t", "agent": "claude", "model": "", "effort": "", "mode": "auto", "cwd": "/p", "state": "review",
        "createdAt": 1, "updatedAt": 1, "parentId": "lead",
        "ext": { "wisp.ship": { "pr": { "state": "OPEN", "head": "abc" } }, "wisp.worktrees": { "path": "/wt", "baseRef": "origin/main" }, "wisp.goal": { "text": "done" }, "wisp.review": { "status": "held", "head": "abc", "findings": [], "at": 3 } }
    })).unwrap();
    let t = task_of(&s);
    assert!(t.is_child && t.pr_open && !t.live);
    assert_eq!((t.pr_head.as_str(), t.dir.as_str(), t.base_ref.as_str(), t.goal.as_str()), ("abc", "/wt", "origin/main", "done"));
    assert_eq!(t.review.unwrap().status, Status::Held);
}

/// A newer record lands on the slot while a run finishes (after its last `live` check, before its save): the run's
/// result is refused over it and dropped, so the newer record stays and no finished event goes out for the old one.
#[tokio::test]
async fn a_record_written_while_a_run_keeps_its_result_wins() {
    let f = fake();
    let newer = Review { head: "h2".into(), status: Status::Running, ..Review::default() };
    f.st().review_after_ask = Some(newer.clone());
    let r = Reviewer::new(Arc::new(f.clone()));
    assert_eq!(maybe(&f, &r, false).await, "");
    let st = f.st();
    assert_eq!(st.task.review.as_ref(), Some(&newer));
    assert_eq!(st.saved.len(), 1); // only the running record
    assert!(st.finished.is_empty());
}

/// Startup reads the list once and reads again only the tasks whose row says a review is running (was: `runtime.get`
/// on every task, ~360 ms of a 437 ms cold start on 5k tasks); the one a restart cut short is still found and cleared.
#[tokio::test]
async fn startup_reads_again_only_the_tasks_whose_row_says_a_review_is_running() {
    let row = |i: usize, review: Value| -> wisp_runtime_api::model::Summary {
        serde_json::from_value(json!({
            "id": format!("s{i}"), "title": "t", "agent": "claude", "model": "", "effort": "", "mode": "auto", "cwd": "/p",
            "state": "review", "status": "done", "unread": false, "archived": false, "createdAt": 1, "updatedAt": 1,
            "turnCount": 1, "queued": 0, "preview": "", "live": "", "review": review
        }))
        .unwrap()
    };
    let mut rows: Vec<_> =
        (2..5_000).map(|i| row(i, if i % 2 == 0 { json!({ "status": "done", "head": "h" }) } else { Value::Null })).collect();
    rows.push(row(1, json!({ "status": "running", "head": "h1", "current": true, "findings": [] })));
    let ids = cut_short(&rows);
    assert_eq!(ids, vec!["s1".to_owned()], "only the running one, out of 5 000 rows");

    let f = fake();
    let running = Review { status: Status::Running, head: "h1".into(), ..Review::default() };
    f.st().task.review = Some(running);
    boot(&Reviewer::new(Arc::new(f.clone())), &call(), ids).await;
    let st = f.st();
    assert_eq!(st.reads, vec!["s1".to_owned()], "one read, not one per task");
    assert_eq!(st.saved, vec![(None, false)], "the review a restart cut short is still cleared");
}

/// The fake, with a new run's record landing on the slot right after boot reads it.
struct Racing(Fake, Review);

impl Env for Racing {
    fn task(&self, c: &Call, id: &str) -> Fut<Result<Task>> {
        let t = self.0.task(c, id);
        self.0.st().task.review = Some(self.1.clone());
        t
    }
    fn turns(&self, c: &Call, id: &str) -> Fut<(Vec<String>, String)> {
        self.0.turns(c, id)
    }
    fn setting(&self, c: &Call) -> Fut<String> {
        self.0.setting(c)
    }
    fn agents(&self, c: &Call) -> Fut<Vec<Agent>> {
        self.0.agents(c)
    }
    fn hold(&self, c: &Call, a: &str) -> Fut<Option<String>> {
        self.0.hold(c, a)
    }
    fn constraints(&self, c: &Call, id: &str) -> Fut<Vec<String>> {
        self.0.constraints(c, id)
    }
    fn head(&self, c: &Call, d: &str) -> Fut<Option<String>> {
        self.0.head(c, d)
    }
    fn changes(&self, c: &Call, d: &str, b: &str) -> Fut<Option<(Vec<FileDiff>, Vec<String>)>> {
        self.0.changes(c, d, b)
    }
    fn tests(&self, c: &Call, d: &str, w: &str) -> Fut<Option<TestsRun>> {
        self.0.tests(c, d, w)
    }
    fn ask(&self, c: &Call, p: String, w: &str, k: &Pick, t: u64) -> Fut<Option<Value>> {
        self.0.ask(c, p, w, k, t)
    }
    fn save(&self, c: &Call, id: &str, r: Option<&Review>, u: bool, o: Option<&Review>) -> Fut<Result<()>> {
        self.0.save(c, id, r, u, o)
    }
    fn finished(&self, c: &Call, e: &ReviewFinished) {
        self.0.finished(c, e);
    }
    fn now(&self) -> i64 {
        self.0.now()
    }
}

/// Boot runs after startup, off serve's path, so a run of this server can get to a task first: a run that claimed it
/// before boot reads it, or one whose record lands between boot's read and its write, keeps its record; a review that
/// ended meanwhile is left too.
#[tokio::test]
async fn a_run_this_server_started_before_boot_reached_its_task_keeps_its_record() {
    let stale = Review { status: Status::Running, head: "h0".into(), at: 1, ..Review::default() };

    // ended meanwhile: nothing to clear
    let f = fake();
    f.st().task.review = Some(Review { status: Status::Done, head: "h1".into(), ..Review::default() });
    boot(&Reviewer::new(Arc::new(f.clone())), &call(), vec!["s1".into()]).await;
    assert!(f.st().saved.is_empty());

    // a run claimed the task (Review again, a turn's end) and wrote its running record before boot read it
    let f = fake();
    f.st().task.review = Some(stale.clone());
    f.st().answer = None;
    let r = Reviewer::new(Arc::new(f.clone()));
    let started = r.maybe(&call(), "s1", false).await.unwrap();
    assert!(started.job.is_some() && r.is_running("s1"));
    let mine = f.st().task.review.clone();
    assert_eq!(mine.as_ref().map(|x| x.status), Some(Status::Running));
    boot(&r, &call(), vec!["s1".into()]).await;
    assert_eq!(f.st().task.review, mine, "boot leaves a claimed task alone");
    assert_eq!(f.st().saved.len(), 1, "only the run's own record");

    // a new run's record between boot's read and its write: the clear is refused over it
    let f = fake();
    let newer = Review { status: Status::Running, head: "h1".into(), at: 42, ..Review::default() };
    f.st().task.review = Some(stale);
    boot(&Reviewer::new(Arc::new(Racing(f.clone(), newer.clone()))), &call(), vec!["s1".into()]).await;
    let st = f.st();
    assert!(st.saved.is_empty());
    assert_eq!(st.task.review.as_ref(), Some(&newer), "the new run's record stays");
}
