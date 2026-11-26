//! The Steward's IO (ADR-0132). On each PR refresh it reads the failing checks (their job logs, and whether they
//! fail on the base too), the open review threads and, with `review`, wisp.review's word on the head. It sends one
//! follow-up, pushes what the turn fixed once it ends, and records green or why it stopped. The decisions are
//! `wisp_ship_core::steward`'s.

use std::sync::Arc;

use wisp_kernel::{BoxFuture, Call};
use wisp_review_api::{ReviewView, Severity, Status};
use wisp_ship_api::{PrSummary, ShipTask, Shipping, Steward};
use wisp_ship_core::decide::Refusal;
use wisp_ship_core::follow::fix_ended;
use wisp_ship_core::pull_coords;
use wisp_ship_core::steward::{self as core, Failing, Gate, HighFinding, OpenThread, Plan, ReviewRead, Seen};
use wisp_ship_core::text::check_log_job;

use crate::error::Result;
use crate::follow::TurnEnd;
use crate::ops::ShipReq;
use crate::ship::{Out, Ship};

/// Failing checks read per look (their logs are the expensive part).
const MAX_FAILING: usize = 5;

/// What wisp.review said for the Steward: its view of the task, and why it didn't start one ("" when it did, or one
/// is under way). The plugin asks `review.view`, then `review.maybe` with `other-vendor` when the head isn't read.
#[derive(Debug, Clone, Default)]
pub struct ReviewSaid {
    pub view: Option<ReviewView>,
    pub why: String,
}

/// (task id, PR head) → what wisp.review says. Absent: wisp.review isn't there, the review never comes.
pub type ReviewPort = Arc<dyn Fn(Call, String, String) -> BoxFuture<'static, ReviewSaid> + Send + Sync>;

/// wisp.review's answer as the core reads it, for `head`.
pub fn review_read(said: &ReviewSaid, head: &str) -> ReviewRead {
    match said.view.as_ref().filter(|v| v.head == head) {
        Some(v) if v.status == Status::Done => ReviewRead::Done {
            by: v.by.as_ref().map(|p| core::reviewer_name(&p.agent, &p.model)).unwrap_or_default(),
            high: v
                .findings
                .iter()
                .filter(|f| f.severity == Severity::High)
                .map(|f| HighFinding { file: f.file.clone(), line: f.line, text: f.text.clone() })
                .collect(),
        },
        Some(v) if v.status == Status::Failed => ReviewRead::Skipped(v.summary.clone().unwrap_or_else(|| "The review failed.".into())),
        _ if !said.why.is_empty() => ReviewRead::Skipped(said.why.clone()),
        _ => ReviewRead::Pending,
    }
}

impl Ship {
    pub(crate) fn steward_on(task: &ShipTask) -> bool {
        task.steward.as_ref().is_some_and(|s| core::mode_of(&s.mode).is_some() && s.step != "stopped")
    }

    fn edit_steward(&self, session: &str, f: impl FnOnce(&mut Steward)) {
        self.tasks.update(session, |t| {
            if let Some(s) = t.steward.as_mut() {
                f(s);
            }
        });
    }

    /// `ship.steward` (the dialog's toggle, the row's Stop, the MCP tool): `fix`, `review` or `off`.
    pub fn set_steward(&self, session: &str, mode: &str) -> Result<Option<Steward>> {
        let mut task = self.task(session)?;
        let m = mode.trim();
        if m == core::OFF || m.is_empty() {
            task.steward = None;
            self.save(task);
            return Ok(None);
        }
        let Some(m) = core::mode_of(m) else {
            return Err(Refusal::Invalid(format!("The Steward is fix, review or off, not {mode:?}.")).into());
        };
        let st = core::started(m, self.shipper.now(), task.steward.as_ref());
        task.steward = Some(st.clone());
        self.save(task);
        Ok(Some(st))
    }

    /// After a refresh: read what's wrong with the PR and send one pass, or say green, or stop.
    pub async fn steward_look(&self, call: &Call, session: &str) -> Result<()> {
        let task = self.task(session)?;
        match core::gate(&task, self.tasks.busy(&task, self.is_starting(session))) {
            Gate::Skip => return Ok(()),
            Gate::Stop(why) => {
                self.edit_steward(session, |s| core::stop(s, why));
                return Ok(());
            }
            Gate::Look => {}
        }
        let (Some(pr), Some(st)) = (task.pr().cloned(), task.steward.clone()) else { return Ok(()) };
        let dir = Self::gh_dir(&task).await;
        let failing = if pr.checks.fail > 0 { self.failing(&dir, &pr).await } else { Vec::new() };
        let threads = self.open_threads(&dir, &pr).await;
        let review = if core::mode_of(&st.mode) == Some(core::REVIEW) { Some(self.review_of(call, session, &pr.head).await) } else { None };
        // gh took a while: the task may have moved on (a turn started, a push, the Steward switched off)
        let task = self.task(session)?;
        if core::gate(&task, self.tasks.busy(&task, self.is_starting(session))) != Gate::Look
            || task.pr().map(|p| p.head.as_str()) != Some(&pr.head)
        {
            return Ok(());
        }
        match core::plan(&task, &Seen { failing, threads, review }) {
            Plan::Wait(why) => self.edit_steward(session, |s| s.why = why),
            Plan::Green { reviewer } => self.edit_steward(session, |s| {
                s.step = "green".into();
                pr.head.clone_into(&mut s.head);
                s.reviewer = reviewer;
                s.why = None;
            }),
            Plan::Stop(why) => self.edit_steward(session, |s| core::stop(s, why)),
            Plan::Pass(p) => {
                self.edit_steward(session, |s| core::sent(s, &pr.head, &p));
                self.emit(Out::FollowUp { session: session.into(), prompt: p.prompt });
            }
        }
        Ok(())
    }

    async fn failing(&self, dir: &str, pr: &PrSummary) -> Vec<Failing> {
        let list = match self.shipper.gh.checks(dir, &pr.number.to_string()).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "ship: the Steward couldn't read the PR's checks");
                return Vec::new();
            }
        };
        let coords = pull_coords(&pr.url);
        let mut out = Vec::new();
        for c in list.into_iter().filter(|c| c.state == "fail").take(MAX_FAILING) {
            let log = match check_log_job(&c.url) {
                Some((o, r, job)) => core::log_tail(&self.shipper.gh.job_log(dir, &o, &r, &job).await),
                None => String::new(),
            };
            let on_base = match &coords {
                Some((o, r, _)) => self.shipper.gh.fails_on_base(dir, o, r, &pr.base, &c.name).await,
                None => false,
            };
            out.push(Failing { name: c.name, url: c.url, log, on_base });
        }
        out
    }

    async fn open_threads(&self, dir: &str, pr: &PrSummary) -> Vec<OpenThread> {
        let Some((o, r, n)) = pull_coords(&pr.url) else { return Vec::new() };
        match self.shipper.gh.review_threads(dir, &o, &r, n).await {
            Ok(j) => core::threads_from(&j),
            Err(e) => {
                tracing::warn!(error = %e, "ship: the Steward couldn't read the PR's review threads");
                Vec::new()
            }
        }
    }

    async fn review_of(&self, call: &Call, session: &str, head: &str) -> ReviewRead {
        let port = self.review_port();
        let Some(port) = port else { return ReviewRead::Skipped("Review by another harness isn't installed.".into()) };
        let said = port(call.clone(), session.to_owned(), head.to_owned()).await;
        review_read(&said, head)
    }

    /// A pass's turn ended: push what it fixed, or say why not. The next refresh looks at the new head.
    pub async fn finish_steward(&self, call: &Call, session: &str, end: &TurnEnd) -> Result<()> {
        let task = self.task(session)?;
        if task.steward.as_ref().is_none_or(|s| s.step != "fixing") {
            return Ok(());
        }
        if let Some(why) = fix_ended(end.interrupted, end.queued, end.error.as_deref()) {
            self.edit_steward(session, |s| core::stop(s, why));
            return Ok(());
        }
        let Some(wt) = task.repos.first().map(|r| r.worktree.clone()) else { return Ok(()) };
        let st = self.shipper.status(call, &wt, task.pr()).await.unwrap_or_default();
        if st.dirty == 0 && st.unpushed == 0 {
            self.edit_steward(session, |s| core::stop(s, "The agent finished without a change to push: see its answer."));
            return Ok(());
        }
        let causes = task.steward.as_ref().map(|s| s.causes.clone()).unwrap_or_default();
        self.tasks.update(session, |t| {
            if let Some(s) = t.steward.as_mut() {
                s.step = "pushing".into();
            }
            t.shipping = Some(Shipping { step: "start".into(), update: true, started_at: self.shipper.now(), ..Shipping::default() });
        });
        let on_step = |step: &str| {
            let shipping = self
                .tasks
                .update(session, |t| {
                    t.shipping.as_mut().map(|s| {
                        step.clone_into(&mut s.step);
                        s.clone()
                    })
                })
                .flatten();
            self.emit(Out::Step { session: session.into(), shipping });
        };
        let title = pass_title(&causes);
        let shipped =
            self.shipper.ship(call, &wt, &ShipReq { task: session, title: &title, body: "", draft: false, pr: task.pr() }, &on_step).await;
        let now = self.shipper.now();
        self.tasks.update(session, |t| {
            t.shipping = None;
            match &shipped {
                Ok(pr) => {
                    if let Some(r) = t.repos.first_mut() {
                        r.pr = Some(pr.clone());
                    }
                    t.pushed_at = now;
                    if let Some(s) = t.steward.as_mut() {
                        s.step = "watching".into();
                    }
                }
                Err(e) => {
                    if let Some(s) = t.steward.as_mut() {
                        core::stop(s, format!("Push failed: {e}"));
                    }
                }
            }
        });
        Ok(())
    }
}

/// The commit message of a pass (when the agent left changes uncommitted).
fn pass_title(causes: &[String]) -> String {
    let what: Vec<&str> = causes
        .iter()
        .map(|c| match c.as_str() {
            "checks" => "failing checks",
            "threads" => "review comments",
            _ => "review findings",
        })
        .collect();
    if what.is_empty() { "Steward: fix the PR".into() } else { format!("Steward: fix {}", what.join(", ")) }
}
