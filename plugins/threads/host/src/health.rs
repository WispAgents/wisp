//! A thread's health at each turn's end (`turn.ended`), from runtime's own reads: its turns (compactions, failures,
//! context, edits) and the failed steps of the last three. Saved on the task's `session:<id>` topic; no model call.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use serde_json::json;
use wisp_kernel::{Call, Ctx, Result};
use wisp_runtime_api::commands::{ActivityArgs, ActivityOf, Turns, TurnsArgs};
use wisp_runtime_api::model::{Activity, FileChange, Turn, TurnStatus};
use wisp_threads_api::{Health, Level, Repeat};
use wisp_threads_core::health::{EditIn, TurnIn, health};

use crate::store;
use crate::{State, db, notes};

/// Turns read back for health: enough for the thresholds (60 turns is the most any counts).
const LOOK_BACK: u32 = 80;

fn edits(t: &Turn) -> Vec<EditIn> {
    let changes: Vec<FileChange> =
        t.ext.get("wisp.runtime").and_then(|r| r.get("changes")).cloned().and_then(|c| serde_json::from_value(c).ok()).unwrap_or_default();
    changes
        .iter()
        .flat_map(|c| c.edits.iter().map(|e| EditIn { file: c.file.clone(), before: e.before.clone(), after: e.after.clone() }))
        .collect()
}

/// Its answer says it didn't get there (a blocked or partial hero), or it ended in an error.
fn failed(t: &Turn) -> bool {
    t.status == TurnStatus::Error
        || t.presentation
            .as_ref()
            .and_then(|p| p.blocks.iter().find(|b| b["type"] == "hero"))
            .is_some_and(|h| matches!(h["status"].as_str(), Some("blocked" | "partial")))
}

pub fn turn_in(t: &Turn, errors: Vec<String>) -> TurnIn {
    let stats = t.stats.as_ref();
    TurnIn {
        compact: t.by.as_deref() == Some("compact"),
        held: t.status == TurnStatus::Held,
        failed: failed(t),
        fresh: crate::audit::fresh_mark(t),
        // a `/compact` turn is counted as one already
        compacted: if t.by.as_deref() == Some("compact") { 0 } else { crate::audit::compactions(t) },
        context: stats.and_then(|s| Some((s.context_tokens?, s.context_window?))),
        errors,
        edits: edits(t),
    }
}

/// The failed steps of a turn of the work, in the words health keeps.
async fn errors_of(ctx: &Ctx, call: &Call, task: &str, t: &Turn) -> Vec<String> {
    let Ok(page) =
        ctx.run::<ActivityOf>(call, ActivityArgs { id: task.into(), turn_id: t.id.clone(), since_seq: None, masked: None }).await
    else {
        return Vec::new();
    };
    page.items
        .into_iter()
        .filter_map(|s| match s.activity {
            Activity::ToolError { name, text, .. } => Some(format!("{name} failed: {}", text.lines().next().unwrap_or_default())),
            _ => None,
        })
        .collect()
}

fn work(t: &Turn) -> bool {
    t.by.as_deref() != Some("compact")
}

/// What health reads of a task's last turns, kept between its turn ends (perf audit 2026-11-18 #9): a turn's end
/// reads that turn alone (its steps once, for its errors), where it read the last 80 turns and the last three turns'
/// steps again at every turn end. Only the last three turns of the work keep their errors, as a full read has them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Window {
    /// (idx, a turn of the work, what health reads of it), oldest first, at most `LOOK_BACK`
    turns: VecDeque<(u32, bool, TurnIn)>,
}

impl Window {
    pub fn total(&self) -> u32 {
        self.turns.back().map_or(0, |(i, _, _)| i + 1)
    }
    pub fn ins(&self) -> Vec<TurnIn> {
        self.turns.iter().map(|(_, _, t)| t.clone()).collect()
    }
    /// Takes the turn that just ended (`idx`). False: it doesn't follow the last one kept (a turn missed, a restart):
    /// read the window again.
    pub fn follow(&mut self, idx: u32, is_work: bool, t: TurnIn) -> bool {
        match self.turns.back().map(|(i, _, _)| *i) {
            Some(last) if last == idx => {
                self.turns.pop_back();
            }
            Some(last) if last + 1 == idx => {}
            _ => return false,
        }
        self.turns.push_back((idx, is_work, t));
        while self.turns.len() > LOOK_BACK as usize {
            self.turns.pop_front();
        }
        self.trim();
        true
    }
    /// Errors only on the last three turns of the work.
    fn trim(&mut self) {
        let mut seen = 0;
        for (_, w, t) in self.turns.iter_mut().rev() {
            if *w {
                seen += 1;
                if seen > 3 {
                    t.errors.clear();
                }
            }
        }
    }
}

/// The windows of the tasks whose turns ended lately, at most `WINDOWS` (the oldest goes).
#[derive(Debug, Default)]
pub struct Windows(Mutex<(HashMap<String, Window>, VecDeque<String>)>);

const WINDOWS: usize = 64;

impl Windows {
    fn get(&self, task: &str) -> Option<Window> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).0.get(task).cloned()
    }
    fn put(&self, task: &str, w: Window) {
        let mut g = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let (map, order) = &mut *g;
        if map.insert(task.to_owned(), w).is_none() {
            order.push_back(task.to_owned());
            while order.len() > WINDOWS {
                if let Some(old) = order.pop_front() {
                    map.remove(&old);
                }
            }
        }
    }
    pub fn forget(&self, task: &str) {
        let mut g = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        g.0.remove(task);
        g.1.retain(|t| t != task);
    }
}

/// The window read whole: the last `LOOK_BACK` turns, the last three turns of the work's steps.
async fn read_window(ctx: &Ctx, call: &Call, task: &str) -> Result<Window> {
    let mut turns = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(LOOK_BACK), masked: None }).await?;
    turns.sort_by_key(|t| t.idx);
    let last_work: Vec<usize> = turns.iter().enumerate().filter(|(_, t)| work(t)).map(|(i, _)| i).rev().take(3).collect();
    let mut w = Window::default();
    for (i, t) in turns.iter().enumerate() {
        let errors = if last_work.contains(&i) { errors_of(ctx, call, task, t).await } else { Vec::new() };
        w.turns.push_back((t.idx, work(t), turn_in(t, errors)));
    }
    Ok(w)
}

/// The window after the turn `ended` (None: as it is now): that turn alone read, else the whole window again.
async fn window(ctx: &Ctx, call: &Call, st: &State, task: &str, ended: Option<&str>) -> Result<Window> {
    let kept = st.windows.get(task);
    let w = match (kept, ended) {
        (Some(w), None) => w,
        (Some(mut w), Some(turn)) => {
            let latest = ctx.run::<Turns>(call, TurnsArgs { id: task.into(), before: None, limit: Some(1), masked: None }).await?;
            match latest.into_iter().next().filter(|t| t.id.as_str() == turn) {
                Some(t) => {
                    let errors = if work(&t) { errors_of(ctx, call, task, &t).await } else { Vec::new() };
                    if w.follow(t.idx, work(&t), turn_in(&t, errors)) { w } else { read_window(ctx, call, task).await? }
                }
                None => read_window(ctx, call, task).await?,
            }
        }
        (None, _) => read_window(ctx, call, task).await?,
    };
    st.windows.put(task, w.clone());
    Ok(w)
}

/// Works the task's health out after the turn `ended` (None: from its turns as they are) and saves it.
pub async fn work_out(ctx: &Ctx, call: &Call, st: &State, task: &str, ended: Option<&str>) -> Result<Health> {
    let w = window(ctx, call, st, task, ended).await?;
    let (_, m) = notes::load(&st.store, task).await.map_err(|e| db(&e))?;
    // the Notes' age: the task's turns now, less its turns when they were last written
    let h = health(task, &w.ins(), (m.rev > 0).then(|| w.total().saturating_sub(m.turn)));
    save(st, &h).await?;
    Ok(h)
}

/// What the Thread tab and the TUI show of a thread's health: its level, what made it so, the error it repeats. The
/// counts behind them (turns, the context's share) move every turn; a row is written only when this changes, and a
/// thread with no row is Fine (ADR-0100: a write at every turn's end cost the stream's write amplification).
fn shown(h: &Health) -> (Level, &[String], Option<&Repeat>) {
    (h.level, &h.reasons, h.repeat.as_ref())
}

/// Saves `h` when what it shows differs from the kept row's (none kept: Fine, nothing to say), in one read-and-write.
async fn save(st: &State, h: &Health) -> Result<()> {
    let h = h.clone();
    st.store
        .write(move |w| {
            let kept: Option<Health> = w
                .get(store::HEALTH.kind, h.task.clone())?
                .and_then(|r| r.get("body").cloned())
                .and_then(|b| serde_json::from_value(b).ok());
            let was = kept.as_ref().map_or((Level::Ok, &[][..], None), shown);
            if was == shown(&h) {
                return Ok(());
            }
            w.upsert(store::HEALTH.kind, store::row(json!({ "id": h.task, "body": h }))).map(drop)
        })
        .await
        .map_err(|e| db(&e))
}

/// The health now, worked out from the task's turns (the kept row may be behind on the counts).
pub async fn read(ctx: &Ctx, call: &Call, st: &State, task: &str) -> Result<Health> {
    work_out(ctx, call, st, task, None).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod window_tests {
    use super::*;

    /// What a full read gives: the last `LOOK_BACK` turns, errors only on the last three turns of the work.
    fn full(all: &[(u32, bool, TurnIn)]) -> Window {
        let from = all.len().saturating_sub(LOOK_BACK as usize);
        let mut turns: VecDeque<(u32, bool, TurnIn)> = all[from..].iter().cloned().collect();
        let mut seen = 0;
        for (_, w, t) in turns.iter_mut().rev() {
            if *w {
                seen += 1;
                if seen > 3 {
                    t.errors.clear();
                }
            }
        }
        Window { turns }
    }

    /// Perf audit 2026-11-18 #9: following a thread turn by turn gives what reading it whole gives, at every turn.
    #[test]
    fn a_window_followed_turn_by_turn_is_the_full_read() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut all: Vec<(u32, bool, TurnIn)> = Vec::new();
        let mut w = Window::default();
        for idx in 0..200u32 {
            let is_work = next() % 5 != 0;
            let t = TurnIn {
                compact: !is_work,
                failed: next() % 3 == 0,
                errors: if next() % 2 == 0 { vec![format!("Bash failed: {idx}")] } else { Vec::new() },
                ..TurnIn::default()
            };
            all.push((idx, is_work, t.clone()));
            if idx == 0 {
                w = full(&all);
            } else {
                assert!(w.follow(idx, is_work, t));
            }
            assert_eq!(w, full(&all), "after turn {idx}");
        }
        // the same turn again (it ended once more) replaces it; a turn missed can't be followed
        assert!(w.follow(199, true, TurnIn::default()));
        assert_eq!(w.total(), 200);
        assert!(!w.follow(205, true, TurnIn::default()));
    }
}
