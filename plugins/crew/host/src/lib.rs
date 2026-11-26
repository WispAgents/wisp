//! wisp.crew host (v1 server/crew.js, routes/crew.js): the members, their jobs as ordinary runtime
//! tasks (`runtime.create` with `ext["wisp.crew"]`), check-ins on their schedule (the 20 s tick), and
//! the crew note every job's system prompt carries (`runtime.prompt@1`). The rules are wisp-crew-core's.
//!
//! The crew lives in crew-owned store tables (`store.rs`): every change is one transaction of the rows it
//! touched. v1's config.json crew is imported once at start.

mod agent;
mod answers;
mod calls;
mod jobs;
mod kind;
mod offers;
mod remote;
mod report;
mod shared;
mod stats;
mod store;
mod transfer;
mod wakes;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use wisp_crew_api::commands::{self as c, Layout};
use wisp_crew_api::{CrewExt, Member, MemberView};
use wisp_crew_core::cron::{CronOnly, Zone};
use wisp_crew_core::kit::Ids;
use wisp_crew_core::member::{Checks, normalize_member};
use wisp_crew_core::mentions::handle_of;
use wisp_crew_core::schedule::next_checkin;
use wisp_kernel::services::Paths;
use wisp_kernel::{Manifest, Plugin, Result, Setup, WireError};
use wisp_runtime_api::model::Session;
use wisp_runtime_api::points::{OUTCOME, Outcome, PROMPT, PromptPart, TurnOutcome};

#[derive(Debug, Default)]
pub struct CrewPlugin;

/// The crew as the commands read it (the tables' rows, cached).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Saved {
    #[serde(default)]
    pub crew: Vec<Member>,
    #[serde(default)]
    pub crew_layout: Layout,
}

pub(crate) struct State {
    pub saved: Mutex<Saved>,
    /// member id → the tasks its jobs run as (newest last)
    pub jobs: Mutex<std::collections::HashMap<String, Vec<String>>>,
    store: std::sync::OnceLock<wisp_store::Store>,
    /// members changed (or gone) since the last save
    dirty: Mutex<std::collections::BTreeSet<String>>,
    layout_dirty: std::sync::atomic::AtomicBool,
    /// one writer at a time for the answers kept on a task's turns (read, change, write back)
    pub answering: tokio::sync::Mutex<()>,
    /// a sandbox's job (`<machine>|<id>`) → the turns of it already reported from here (`crew_job.reported`)
    pub reported: Mutex<std::collections::HashMap<String, u32>>,
    /// one sandbox report at a time (its row can change twice before the first is read back)
    pub reporting: tokio::sync::Mutex<()>,
    /// the user's IANA zone (`settings.timeZone`, as the client sent it); None: the machine's
    pub zone: Mutex<Option<String>>,
}

/// The zone check-ins run in and notes write times in: the user's, else the machine's, as automations' triggers
/// read theirs (audit doc23c: a member's `0 8 * * 1-5` checked in at 08:00 UTC).
pub(crate) struct Local(jiff::tz::TimeZone);

impl Local {
    pub fn named(name: Option<&str>) -> Self {
        Self(name.filter(|n| !n.is_empty()).and_then(|n| jiff::tz::TimeZone::get(n).ok()).unwrap_or_else(jiff::tz::TimeZone::system))
    }
}

impl Zone for Local {
    fn offset_min(&self, utc_ms: i64) -> i32 {
        jiff::Timestamp::from_millisecond(utc_ms).map_or(0, |ts| self.0.to_offset(ts).seconds() / 60)
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn wire(e: wisp_crew_core::Error) -> WireError {
    use wisp_crew_core::Error as E;
    match e {
        E::Invalid(m) => WireError::invalid(m),
        E::NotFound(m) => WireError::not_found(m),
        E::Conflict(m) => WireError::conflict(m),
        E::TooMany(m) => WireError::conflict(m).with_details(serde_json::json!({ "status": wisp_crew_api::STATUS_TOO_MANY })),
    }
}

pub(crate) struct NewIds;
impl Ids for NewIds {
    fn next_id(&mut self) -> String {
        wisp_kernel::ids::uuid7().replace('-', "").chars().rev().take(16).collect()
    }
}

impl State {
    fn new() -> Self {
        Self {
            saved: Mutex::default(),
            jobs: Mutex::default(),
            store: std::sync::OnceLock::new(),
            dirty: Mutex::default(),
            layout_dirty: false.into(),
            answering: tokio::sync::Mutex::new(()),
            reported: Mutex::default(),
            reporting: tokio::sync::Mutex::new(()),
            zone: Mutex::default(),
        }
    }

    pub fn local(&self) -> Local {
        Local::named(lock(&self.zone).as_deref())
    }

    /// The tables, then v1's crew once, then the cache. At start, before any command.
    async fn open(&self, store: wisp_store::Store, home: Option<PathBuf>) -> std::result::Result<(), WireError> {
        store::install(&store).map_err(WireError::from)?;
        if let Some(h) = home {
            store::import_v1(&store, &h).await.map_err(WireError::from)?;
        }
        let (crew, layout, jobs) = store::load(&store).await.map_err(WireError::from)?;
        *lock(&self.saved) = Saved { crew, crew_layout: layout };
        let mut j = lock(&self.jobs);
        let mut r = lock(&self.reported);
        for (m, s, n) in jobs {
            if n > 0 {
                r.insert(s.clone(), n);
            }
            j.entry(m).or_default().push(s);
        }
        drop((j, r));
        let _ = self.store.set(store);
        Ok(())
    }

    /// Writes what changed since the last save in one transaction: the members touched (or deleted),
    /// and the layout when it moved.
    pub async fn save(&self) {
        let Some(store) = self.store.get().cloned() else { return };
        let ids: Vec<String> = std::mem::take(&mut *lock(&self.dirty)).into_iter().collect();
        let layout = self.layout_dirty.swap(false, std::sync::atomic::Ordering::Relaxed).then(|| lock(&self.saved).crew_layout.clone());
        let rows: Vec<(String, Option<Member>)> = {
            let s = lock(&self.saved);
            ids.into_iter()
                .map(|id| {
                    let m = s.crew.iter().find(|m| m.id == id).cloned();
                    (id, m)
                })
                .collect()
        };
        if rows.is_empty() && layout.is_none() {
            return;
        }
        let r = store
            .write(move |w| {
                for (id, m) in &rows {
                    if let Some(m) = m {
                        store::put_member(w, m)?;
                    } else {
                        w.del(store::MEMBER.kind, id.as_str())?;
                        w.del(store::LAYOUT.kind, id.as_str())?;
                    }
                }
                if let Some(l) = &layout {
                    let had = w.query(store::LAYOUT.kind, &wisp_store::Query::new())?;
                    for row in had {
                        if let Some(id) = row.get("id").and_then(serde_json::Value::as_str).filter(|id| !l.contains_key(*id)) {
                            w.del(store::LAYOUT.kind, id)?;
                        }
                    }
                    for (id, p) in l {
                        store::put_point(w, id, *p)?;
                    }
                }
                Ok(())
            })
            .await;
        if let Err(e) = r {
            tracing::warn!(error = %e, "crew: saving");
        }
    }

    /// A member left: its row and its place go at the next save.
    pub fn remove(&self, id: &str) {
        {
            let mut s = lock(&self.saved);
            s.crew.retain(|x| x.id != id);
            s.crew_layout.remove(id);
        }
        lock(&self.dirty).insert(id.to_owned());
    }

    pub fn set_layout(&self, l: Layout) {
        lock(&self.saved).crew_layout = l;
        self.layout_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// A job started as task `session`.
    pub async fn add_job(&self, member: &str, session: &str, at: i64) {
        lock(&self.jobs).entry(member.to_owned()).or_default().push(session.to_owned());
        let Some(store) = self.store.get().cloned() else { return };
        let mut r = wisp_store::Row::new();
        r.insert("session_id".into(), serde_json::json!(session));
        r.insert("crew_id".into(), serde_json::json!(member));
        r.insert("at".into(), serde_json::json!(at));
        if let Err(e) = store.write(move |w| w.upsert(store::JOB.kind, r).map(|_| ())).await {
            tracing::warn!(error = %e, "crew: a job");
        }
    }

    /// The turns of a sandbox's job reported from here, kept so none is reported twice.
    pub async fn set_reported(&self, job: &str, turns: u32) {
        lock(&self.reported).insert(job.to_owned(), turns);
        let Some(store) = self.store.get().cloned() else { return };
        let job = job.to_owned();
        if let Err(e) = store.write(move |w| store::put_reported(w, &job, turns)).await {
            tracing::warn!(error = %e, "crew: a sandbox job's report");
        }
    }

    pub fn member(&self, key: &str) -> std::result::Result<Member, WireError> {
        let s = lock(&self.saved);
        let k = key.trim_start_matches('@');
        s.crew
            .iter()
            .find(|m| m.id == key || m.name.eq_ignore_ascii_case(k) || handle_of(&m.name) == handle_of(k))
            .cloned()
            .ok_or_else(|| WireError::not_found("No such crew member."))
    }

    /// A member written back whole. Only one still in the crew: a member removed while its copy was read and awaited on
    /// stays removed (a write after `remove` brought them back). A new member goes in with [`State::insert`].
    pub fn put(&self, m: Member) {
        let mut s = lock(&self.saved);
        let Some(x) = s.crew.iter_mut().find(|x| x.id == m.id) else { return };
        let id = m.id.clone();
        *x = m;
        drop(s);
        lock(&self.dirty).insert(id);
    }

    /// A member who joins (create, adopt, import), or an edit of one still here.
    pub fn insert(&self, m: Member) {
        lock(&self.dirty).insert(m.id.clone());
        let mut s = lock(&self.saved);
        match s.crew.iter_mut().find(|x| x.id == m.id) {
            Some(x) => *x = m,
            None => s.crew.push(m),
        }
    }

    /// Changes member `id`'s row as it is now, under the lock: what another command wrote while this one awaited (a job
    /// queued, a queue drained, an edit) stays, where writing back a copy read before the await undid it. `None`: they
    /// are gone (removed meanwhile), and nothing is written.
    pub fn update<T>(&self, id: &str, f: impl FnOnce(&mut Member) -> T) -> Option<T> {
        let mut s = lock(&self.saved);
        let out = f(s.crew.iter_mut().find(|x| x.id == id)?);
        drop(s);
        lock(&self.dirty).insert(id.to_owned());
        Some(out)
    }

    pub fn view(&self, m: &Member, running: u32, now: i64) -> MemberView {
        let jobs = u32::try_from(lock(&self.jobs).get(&m.id).map_or(0, Vec::len)).unwrap_or(u32::MAX);
        MemberView {
            member: m.clone(),
            running,
            jobs,
            next_at: next_checkin(m, now, &self.local()),
            handle: handle_of(&m.name),
            on: m.wakes.iter().map(|w| w.on.clone()).collect(),
        }
    }

    /// A new or edited member, checked against the rest of the crew.
    pub fn normalize(
        &self,
        body: &c::MemberBody,
        prev: Option<&Member>,
        now: i64,
        profiles: &[String],
        machines: &[String],
        default_agent: &str,
    ) -> std::result::Result<Member, WireError> {
        let taken = lock(&self.saved).crew.clone();
        if prev.is_none() && taken.len() >= wisp_crew_api::MAX_CREW {
            return Err(WireError::conflict(format!("A crew has at most {} members.", wisp_crew_api::MAX_CREW)));
        }
        let exists = |p: &str| std::path::Path::new(p).is_dir();
        // a machine this wisp knows (v1 `remote.ok(id)`), or the one the member is already on: a machine gone or a
        // list that failed just now mustn't lock every edit (pausing them above all)
        let machine_ok = |m: &str| machines.iter().any(|x| x == m) || prev.is_some_and(|p| p.machine == m);
        let agent_ok = |a: &str| profiles.iter().any(|p| p == a);
        let checks =
            Checks { taken: &taken, exists: &exists, machine_ok: &machine_ok, words: &CronOnly, agent_ok: &agent_ok, default_agent };
        normalize_member(body, prev, &checks, &mut NewIds, now).map_err(wire)
    }
}

/// Every job's system prompt carries the member's identity, brief, notebook and log (v1 `crewNote`).
struct Note(Arc<State>);

impl PromptPart for Note {
    fn id(&self) -> &'static str {
        "wisp.crew"
    }
    fn part(&self, s: &Session) -> String {
        let Some(job) = s.ext.get(wisp_crew_api::PLUGIN_ID).and_then(|v| serde_json::from_value::<CrewExt>(v.clone()).ok()) else {
            return String::new();
        };
        // a job from another wisp (this one runs it for a member over there) carries its note
        let Ok(m) = self.0.member(&job.id) else { return job.note.unwrap_or_default() };
        wisp_crew_core::note::crew_note(&m, &job, None, &self.0.local())
    }
}

/// How a member's job lands (v1 runner.js `#finishTurn` with `crewOut`): strips the hidden `crew` block before
/// persisting (`prepare`), and tells the runtime whether it lands quiet (archived), For later, or filed under Done.
struct Filed;

impl TurnOutcome for Filed {
    fn id(&self) -> &'static str {
        "wisp.crew"
    }
    fn prepare(&self, s: &Session, turn: &mut wisp_runtime_api::model::Turn) {
        report::prepare(s, turn);
    }
    fn outcome(&self, s: &Session, turn: &wisp_runtime_api::model::Turn) -> Option<Outcome> {
        report::outcome(s, turn)
    }
}

/// A member's job ended: applies memory/log/offers/follow-ups, fills in any answer under the question that asked,
/// and emits `crew.reported` (ADR-0043), from the runtime's durable task facts so a hand-off survives a restart.
fn reported(s: &mut Setup, st: &Arc<State>) {
    for (name, failed) in [("reported", false), ("reported_failed", true)] {
        let st = st.clone();
        let on = move |ctx: wisp_kernel::Ctx, call: wisp_kernel::Call, f: wisp_runtime_api::events::TaskFact| {
            let st = st.clone();
            async move {
                let Some(id) = f.crew.clone() else { return Ok(()) };
                let Ok(m) = st.member(&id) else { return Ok(()) };
                let turns = calls::turns(&ctx, &call, f.session_id.as_str()).await.unwrap_or_default();
                let n = u32::try_from(turns.len()).unwrap_or(u32::MAX);
                // Durable events can redeliver on restart: apply memory/log/follow-ups at most once per turn count.
                let _g = st.reporting.lock().await;
                if n > 0 && lock(&st.reported).get(f.session_id.as_str()).is_some_and(|prev| *prev >= n) {
                    return Ok(());
                }
                if n > 0 {
                    st.set_reported(f.session_id.as_str(), n).await;
                }
                report::report(&ctx, &call, &st, &m, &f.session_id.0, turns.last(), &f.text, failed).await;
                Ok(())
            }
        };
        if failed {
            s.on_durable::<wisp_runtime_api::events::TaskFailed, _, _>(name, move |ctx, call, d| {
                let on = on.clone();
                async move { if let wisp_kernel::durable::Durable::Event { event, .. } = d { on(ctx, call, event.0).await } else { Ok(()) } }
            });
        } else {
            s.on_durable::<wisp_runtime_api::events::TaskFinished, _, _>(name, move |ctx, call, d| {
                let on = on.clone();
                async move { if let wisp_kernel::durable::Durable::Event { event, .. } = d { on(ctx, call, event.0).await } else { Ok(()) } }
            });
        }
    }
}

impl Plugin for CrewPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::embedded(include_str!("../wisp-plugin.json"))
    }

    fn setup(&self, s: &mut Setup) -> Result<()> {
        let k: Arc<dyn wisp_resources_api::ResourceKind> = Arc::new(kind::CrewKind::new(s.ctx()));
        s.contribute(&wisp_resources_api::RESOURCE_KIND, k);
        let st = Arc::new(State::new());
        let note: Arc<dyn PromptPart> = Arc::new(Note(st.clone()));
        s.contribute(&PROMPT, note);
        let filed: Arc<dyn TurnOutcome> = Arc::new(Filed);
        s.contribute(&OUTCOME, filed);
        jobs::register(s, &st);
        answers::register(s, &st);
        reported(s, &st);
        remote::register(s, &st);
        wakes::register(s, &st);
        transfer::register(s, &st);
        offers::register(s, &st);
        agent::register(s, &st);
        agent::register_reads(s, &st);
        agent::register_changes(s, &st);
        let tick = st.clone();
        s.on_start(move |ctx| async move {
            let store = ctx.service::<wisp_store::Store>()?.as_ref().clone();
            let home = ctx.service::<Paths>().ok().map(|p| p.home().to_owned());
            tick.open(store, home).await?;
            jobs::start_ticks(&ctx, tick);
            Ok(())
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        assert_eq!(CrewPlugin.manifest().id, "wisp.crew");
    }

    #[test]
    fn a_quiet_check_in_keeps_its_scheduled_follow_up() {
        let job = CrewExt { id: "m1".into(), job: wisp_crew_api::JobKind::Checkin, ..CrewExt::default() };
        let mut s = Session::default();
        s.ext.insert(wisp_crew_api::PLUGIN_ID.into(), serde_json::to_value(&job).unwrap());
        let crew = serde_json::json!({ "type": "crew", "quiet": true });
        let turn = wisp_runtime_api::model::Turn {
            presentation: Some(serde_json::from_value(serde_json::json!({ "v": 1, "blocks": [crew] })).unwrap()),
            ..wisp_runtime_api::model::Turn::default()
        };
        assert_eq!(Filed.outcome(&s, &turn), Some(Outcome { quiet: true, filed: false, later: false }), "nothing scheduled: archived");
        s.ext.insert(wisp_later_api::PLUGIN_ID.into(), serde_json::json!({ "later": [{ "id": "l1", "at": 5, "text": "check again" }] }));
        assert_eq!(
            Filed.outcome(&s, &turn),
            Some(Outcome { quiet: false, filed: true, later: false }),
            "a follow-up is due: filed under Done so it fires"
        );
        assert_eq!(Filed.outcome(&Session::default(), &turn), None, "not a crew job");
    }

    #[test]
    fn a_member_is_made_from_v1s_body_and_found_by_handle() {
        let st = State::new();
        let body = c::MemberBody {
            name: Some("Ada Lovelace".into()),
            role: Some("CI watcher".into()),
            cron: Some("*/30 * * * *".into()),
            ..c::MemberBody::default()
        };
        let m = st.normalize(&body, None, 1_000, &[], &[], "auto").unwrap();
        assert_eq!((m.name.as_str(), m.cron.as_str(), m.agent.as_str()), ("Ada Lovelace", "*/30 * * * *", "auto"));
        st.insert(m.clone());
        assert_eq!(st.member("@ada-lovelace").unwrap().id, m.id);
        let dup =
            st.normalize(&c::MemberBody { name: Some("ada lovelace".into()), ..c::MemberBody::default() }, None, 2_000, &[], &[], "auto");
        assert!(dup.is_err(), "the same name twice");
    }

    /// Audit H HS7: with Auto off, an agent's `wisp_crew_add` with no harness stored `Qa Reviewer · auto`. A member
    /// gets the default harness; `auto` is refused for a new one or a move, and one already on it keeps it.
    #[test]
    fn with_auto_off_a_member_gets_the_default_harness_and_auto_is_refused_audit_h_hs7() {
        let st = State::new();
        let body = |agent: Option<&str>| c::MemberBody {
            name: Some("Qa Reviewer".into()),
            agent: agent.map(str::to_owned),
            ..c::MemberBody::default()
        };
        let m = st.normalize(&body(None), None, 1_000, &[], &[], "codex").unwrap();
        assert_eq!(m.agent, "codex", "the default harness, never auto");
        assert_eq!(st.normalize(&body(Some("")), None, 1_000, &[], &[], "codex").unwrap().agent, "codex");
        let e = st.normalize(&body(Some("auto")), None, 1_000, &[], &[], "codex").unwrap_err();
        assert_eq!(e.message, "Auto is off (runtime.auto): name a harness, or leave it out for the default one.");
        assert!(st.normalize(&body(Some("auto")), Some(&m), 2_000, &[], &[], "codex").is_err(), "nor moved onto it");
        let mut old = m.clone();
        old.agent = "auto".into();
        let paused = c::MemberBody { agent: Some("auto".into()), paused: Some(true), ..c::MemberBody::default() };
        assert_eq!(st.normalize(&paused, Some(&old), 3_000, &[], &[], "codex").unwrap().agent, "auto", "one already on it keeps it");
        // Auto on: as before
        assert_eq!(st.normalize(&body(None), None, 1_000, &[], &[], "auto").unwrap().agent, "auto");
    }

    #[test]
    fn a_member_on_a_machine_gone_from_the_list_can_still_be_edited_but_not_moved_to_an_unknown_one() {
        let st = State::new();
        let on = |m: &str| c::MemberBody { name: Some("Scout".into()), machine: Some(m.into()), ..c::MemberBody::default() };
        assert!(st.normalize(&on("studio"), None, 1_000, &[], &[], "auto").is_err(), "a machine this wisp doesn't know");
        let m = st.normalize(&on("studio"), None, 1_000, &[], &["studio".into()], "auto").unwrap();
        assert_eq!(m.machine, "studio");
        // the machine is gone from machines.list (removed, or the list failed just now): pausing them still works
        let pause = c::MemberBody { paused: Some(true), ..c::MemberBody::default() };
        assert!(st.normalize(&pause, Some(&m), 2_000, &[], &[], "auto").unwrap().paused);
        assert!(st.normalize(&on("elsewhere"), Some(&m), 3_000, &[], &[], "auto").is_err(), "nor moved to one it doesn't know");
    }

    /// Audit doc23c: check-ins ran on UTC, so `0 8 * * 1-5` in New York checked in at 03:00 or 04:00 local.
    #[test]
    fn a_weekday_8am_check_in_is_at_8am_in_the_users_zone_not_utc() {
        let st = State::new();
        *lock(&st.zone) = Some("America/New_York".into());
        let body = c::MemberBody { name: Some("Ada".into()), cron: Some("0 8 * * 1-5".into()), ..c::MemberBody::default() };
        let m = st.normalize(&body, None, 1_000, &[], &[], "auto").unwrap();
        // Monday 2026-11-23, 12:00 UTC (07:00 in New York): 08:00 New York that day, 13:00 UTC
        let now = jiff::Timestamp::from_second(1_795_435_200).unwrap().as_millisecond();
        let at = st.view(&m, 0, now).next_at.unwrap();
        let local = jiff::Timestamp::from_millisecond(at).unwrap().to_zoned(jiff::tz::TimeZone::get("America/New_York").unwrap());
        assert_eq!((local.hour(), local.minute(), local.day()), (8, 0, 23), "{local}");
        assert_eq!(at, now + 3_600_000);
        // and the note's times are the zone's too
        assert_eq!(st.local().offset_min(now), -300);
    }

    #[test]
    fn a_job_from_another_wisp_runs_with_the_note_it_carries() {
        let st = Arc::new(State::new());
        let note = Note(st.clone());
        let job = CrewExt { id: "m-elsewhere".into(), note: Some("You are Scout.".into()), ..CrewExt::default() };
        let mut s = Session::default();
        s.ext.insert(wisp_crew_api::PLUGIN_ID.into(), serde_json::to_value(&job).unwrap());
        // no such member on this wisp: the job's own note
        assert_eq!(note.part(&s), "You are Scout.");
        // a member here: the note is written here, as always
        let m =
            st.normalize(&c::MemberBody { name: Some("Ada".into()), ..c::MemberBody::default() }, None, 1_000, &[], &[], "auto").unwrap();
        st.insert(m.clone());
        let mine = CrewExt { id: m.id.clone(), note: Some("QQ-carried-QQ".into()), ..CrewExt::default() };
        s.ext.insert(wisp_crew_api::PLUGIN_ID.into(), serde_json::to_value(&mine).unwrap());
        let part = note.part(&s);
        assert!(part.contains("Ada") && !part.contains("QQ-carried-QQ"), "{part}");
    }
}
