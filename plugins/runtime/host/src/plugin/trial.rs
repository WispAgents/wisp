//! `runtime.trial` (Try it two ways, F-comp-31; ADR-0056): the pair, each run through `runtime.create` (its hooks,
//! its worktree, its permission), linked; and `runtime.trial.keep`.

use wisp_kernel::{Call, Ctx, Result as WireResult, Setup, WireError};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::{self as c, StartArgs, TrialArgs, TrialLink, TrialStarted};

use super::{Slot, rt};

pub(super) fn trial(s: &mut Setup, slot: &Slot) {
    let ctx = s.ctx();
    let start = slot.clone();
    s.command::<c::Trial, _, _>(move |_ctx, call, a| {
        let (slot, ctx) = (start.clone(), ctx.clone());
        // boxed: each run's create starts its first turn, whose future is large (clippy large_futures)
        async move { Box::pin(pair(&ctx, &call, &slot, a)).await }
    });
    cmd!(s, slot, c::TrialKeep, |r, call, a| r.trial_keep(a.id.as_str()).await.map(|()| c::Ack {}));
}

async fn pair(ctx: &Ctx, call: &Call, slot: &Slot, a: TrialArgs) -> WireResult<TrialStarted> {
    let rt = rt(slot)?;
    // every refusal before anything runs: nothing is half started
    let p = rt.trial_plan(&a).map_err(WireError::from)?;
    // a folder outside git gets no worktree, and both runs would edit it at once
    if !in_git(p.a.cwd.as_deref().unwrap_or_default()).await {
        return Err(WireError::invalid("Try it two ways needs a project in git: each run keeps its own worktree."));
    }
    let group = format!("g-{}", &wisp_kernel::ids::uuid7()[20..]);
    let link = |run: u8| TrialLink { group: group.clone(), run, differs: p.differs.clone(), ..TrialLink::default() };
    let (mut a, mut b) = (p.a, p.b);
    crate::trial::link(&mut a, &link(0));
    crate::trial::link(&mut b, &link(1));
    let made = start_pair(&Live { ctx, call, rt: &rt }, a, b).await?;
    // a count with nothing in it (v1 `try_two_ways`): only a pair that started, never a refusal; telemetry's own
    // switch and environment (DO_NOT_TRACK, CI) decide whether it leaves
    super::track(ctx, "try_two_ways", serde_json::json!({})).await;
    Ok(TrialStarted { group, runs: made, differs: p.differs })
}

/// What starting a pair needs, so its failure paths are tested with a fake (`Live` is the runtime's).
trait Pair {
    /// `runtime.create`: the run, its first turn started (or held, or refused)
    fn create(&self, start: StartArgs) -> impl Future<Output = WireResult<c::Created>> + Send;
    /// its first turn read back: why it didn't start (None: it runs), and the account it went on
    fn first_turn(&self, id: &str) -> impl Future<Output = WireResult<(Option<String>, Option<String>)>> + Send;
    /// each run lists both
    fn link(&self, ids: &[SessionId]) -> impl Future<Output = WireResult<()>> + Send;
    /// the runs of a pair that won't go: stopped and archived (a failure is logged; the refusal stands either way)
    fn drop_runs(&self, made: &[c::Created]) -> impl Future<Output = ()> + Send;
}

/// Runs A then B; the pair is refused as one (v1's 409, never one run alone): once a run exists, every error (B
/// refused, a run that didn't start, a read-back or the link failing) stops and archives the runs made, so no lone
/// run of a pair goes on billing as if it were one. Run B goes on the account run A went on when the task named
/// none: Ask near the limit counted both on it at A's start (`together`), so that's where both run (if that account
/// is skipped for B by then, at its limit or signed out, B is asked about alone, on its own account).
async fn start_pair<P: Pair + Sync>(p: &P, a: StartArgs, mut b: StartArgs) -> WireResult<Vec<c::Created>> {
    let mut made: Vec<c::Created> = Vec::new();
    match pair_steps(p, a, &mut b, &mut made).await {
        Ok(()) => Ok(made),
        Err(err) => {
            p.drop_runs(&made).await;
            Err(err)
        }
    }
}

async fn pair_steps<P: Pair + Sync>(p: &P, a: StartArgs, b: &mut StartArgs, made: &mut Vec<c::Created>) -> WireResult<()> {
    let auto = b.account.as_deref().is_none_or(|x| x.is_empty() || x == "auto");
    let mut next = Some(a);
    while let Some(start) = next.take() {
        let m = p.create(start).await?;
        let id = m.id.clone();
        made.push(m);
        // a run that didn't start (Ask near the limit asked for both on A, a hook refused it, the budget held it):
        // the words are the refusal's own (Ask near the limit's say how to go ahead: confirmLimit, the CLI's --yes)
        let (why, account) = p.first_turn(id.as_str()).await?;
        if let Some(why) = why {
            return Err(WireError::conflict(why));
        }
        if made.len() == 1 {
            if auto && let Some(acc) = account.filter(|x| !x.is_empty()) {
                b.account = Some(acc);
            }
            next = Some(std::mem::take(b));
        }
    }
    let ids: Vec<_> = made.iter().map(|m| m.id.clone()).collect();
    p.link(&ids).await
}

struct Live<'a> {
    ctx: &'a Ctx,
    call: &'a Call,
    rt: &'a std::sync::Arc<crate::Runtime>,
}

impl Pair for Live<'_> {
    async fn create(&self, start: StartArgs) -> WireResult<c::Created> {
        self.ctx.run::<c::Create>(self.call, start).await
    }
    async fn first_turn(&self, id: &str) -> WireResult<(Option<String>, Option<String>)> {
        self.rt.first_turn(id).await.map_err(WireError::from)
    }
    async fn link(&self, ids: &[SessionId]) -> WireResult<()> {
        self.rt.trial_runs(ids).await.map_err(WireError::from)
    }
    async fn drop_runs(&self, made: &[c::Created]) {
        for m in made {
            if let Err(err) = self.rt.stop(m.id.as_str(), true).await {
                tracing::warn!(error = %err, id = m.id.as_str(), "runtime.trial: stop");
            }
            if let Err(err) = self.rt.archive(m.id.as_str(), true).await {
                tracing::warn!(error = %err, id = m.id.as_str(), "runtime.trial: archive");
            }
        }
    }
}

/// The folder is in a git checkout (a `.git` in it or above it).
async fn in_git(cwd: &str) -> bool {
    for d in std::path::Path::new(cwd).ancestors() {
        if tokio::fs::try_exists(d.join(".git")).await.unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A runtime whose steps fail where asked: `fail` names the step (`create:1` is run B's create).
    #[derive(Default)]
    struct Fake {
        fail: &'static str,
        account: Option<&'static str>,
        starts: Mutex<Vec<StartArgs>>,
        dropped: Mutex<Vec<String>>,
    }
    impl Fake {
        fn fails(&self, step: &str) -> bool {
            self.fail == step
        }
    }
    #[allow(clippy::unused_async_trait_impl)] // a fake: the real steps await
    impl Pair for Fake {
        async fn create(&self, start: StartArgs) -> WireResult<c::Created> {
            let n = { self.starts.lock().unwrap().len() };
            if self.fails(&format!("create:{n}")) {
                return Err(WireError::invalid("a hook refused it"));
            }
            self.starts.lock().unwrap().push(start);
            Ok(c::Created { id: SessionId(format!("t{n}")), turn_id: None })
        }
        async fn first_turn(&self, id: &str) -> WireResult<(Option<String>, Option<String>)> {
            if self.fails(&format!("read:{id}")) {
                return Err(WireError::internal("store: the read failed"));
            }
            Ok((None, self.account.map(str::to_owned)))
        }
        async fn link(&self, _ids: &[SessionId]) -> WireResult<()> {
            if self.fails("link") { Err(WireError::internal("store: the write failed")) } else { Ok(()) }
        }
        async fn drop_runs(&self, made: &[c::Created]) {
            self.dropped.lock().unwrap().extend(made.iter().map(|m| m.id.0.clone()));
        }
    }

    fn run() -> StartArgs {
        StartArgs { prompt: "fix it".into(), agent: Some("claude".into()), ..StartArgs::default() }
    }

    /// Round 177's review: an error reading a run back (or linking the two) once a run exists left it going alone.
    /// Every error after the first run exists stops and archives what was made, and the error stands.
    #[tokio::test]
    async fn an_error_after_a_run_exists_stops_and_archives_the_runs_made_never_one_alone() {
        for (fail, dropped) in
            [("read:t0", vec!["t0"]), ("read:t1", vec!["t0", "t1"]), ("link", vec!["t0", "t1"]), ("create:1", vec!["t0"])]
        {
            let f = Fake { fail, ..Fake::default() };
            let err = start_pair(&f, run(), run()).await.unwrap_err();
            assert!(!err.message.is_empty(), "{fail}: the error stands");
            assert_eq!(*f.dropped.lock().unwrap(), dropped, "{fail}");
        }
        let f = Fake { fail: "create:0", ..Fake::default() };
        start_pair(&f, run(), run()).await.unwrap_err();
        assert!(f.dropped.lock().unwrap().is_empty(), "nothing made, nothing to drop");
        let ok = Fake::default();
        assert_eq!(start_pair(&ok, run(), run()).await.unwrap().len(), 2);
        assert!(ok.dropped.lock().unwrap().is_empty());
    }

    /// Round 177's review: Ask near the limit counts both runs on run A's account at A's start, so run B goes on the
    /// account A went on when the task named none (Auto), rather than wherever its own pick lands; a task that named
    /// one keeps it on both.
    #[tokio::test]
    async fn run_b_goes_on_the_account_run_a_went_on_where_both_were_counted() {
        let f = Fake { account: Some("work"), ..Fake::default() };
        start_pair(&f, run(), run()).await.unwrap();
        let accounts: Vec<_> = f.starts.lock().unwrap().iter().map(|s| s.account.clone()).collect();
        assert_eq!(accounts, vec![None, Some("work".to_owned())]);
        let named = StartArgs { account: Some("home".into()), ..run() };
        let f = Fake { account: Some("work"), ..Fake::default() };
        start_pair(&f, named.clone(), named).await.unwrap();
        assert_eq!(f.starts.lock().unwrap()[1].account.as_deref(), Some("home"), "a named account is the task's");
        let f = Fake::default();
        start_pair(&f, run(), run()).await.unwrap();
        assert_eq!(f.starts.lock().unwrap()[1].account, None, "one account: nothing to pin");
    }
}
