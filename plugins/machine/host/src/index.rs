//! The tasks This Mac's sample names (perf audit 2026-11-18 #4): read once with `runtime.list`, then kept current from
//! runtime's events. A sample re-reads only the tasks that changed since the last one (`runtime.get` each), where it
//! read every task's list row every 10 s while anything ran.
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};

use wisp_kernel::{Call, Ctx};
use wisp_machine_core::this_mac::view::TaskInfo;
use wisp_runtime_api::model::{SessionState, Summary};

use crate::calls;

#[derive(Debug, Default)]
struct Inner {
    filled: bool,
    tasks: HashMap<String, (TaskInfo, SessionState)>,
    /// changed since the last sample (`session.changed`): read again before the next one answers
    dirty: HashSet<String>,
}

#[derive(Debug, Default)]
pub(crate) struct TaskIndex(Mutex<Inner>);

/// What a sample reads of the tasks: every live (not archived) one, and the ones running now.
pub(crate) struct Known {
    pub tasks: HashMap<String, TaskInfo>,
    pub running: HashSet<String>,
}

impl TaskIndex {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn changed(&self, id: &str) {
        let mut g = self.lock();
        if g.filled {
            g.dirty.insert(id.to_owned());
        }
    }

    pub fn created(&self, s: &Summary) {
        let mut g = self.lock();
        if g.filled && !s.archived {
            g.tasks.insert(s.id.0.clone(), (calls::task_of(s), s.state));
        }
    }

    pub fn deleted(&self, id: &str) {
        let mut g = self.lock();
        g.tasks.remove(id);
        g.dirty.remove(id);
    }

    fn put(&self, id: &str, row: Option<(TaskInfo, SessionState)>) {
        let mut g = self.lock();
        match row {
            Some(r) => g.tasks.insert(id.to_owned(), r),
            None => g.tasks.remove(id),
        };
    }

    /// The tasks now: the first call lists them all, later ones re-read what changed meanwhile.
    pub async fn current(&self, ctx: &Ctx, call: &Call) -> Known {
        // taken before the reads: a change that lands while they run stays dirty for the next sample
        let (filled, dirty) = {
            let mut g = self.lock();
            (g.filled, std::mem::take(&mut g.dirty))
        };
        if filled {
            for id in dirty {
                let row = match calls::session(ctx, call, &id).await {
                    Ok(s) if !s.archived => Some((calls::task_of_session(&s), s.state)),
                    Ok(_) => None,
                    Err(e) if e.code == wisp_kernel::Code::NotFound => None,
                    // can't tell now: read again next time
                    Err(_) => {
                        self.lock().dirty.insert(id);
                        continue;
                    }
                };
                self.put(&id, row);
            }
        } else {
            let list = calls::tasks(ctx, call, Some(false)).await;
            let mut g = self.lock();
            g.tasks = list.iter().map(|s| (s.id.0.clone(), (calls::task_of(s), s.state))).collect();
            g.filled = true;
        }
        let g = self.lock();
        Known {
            tasks: g.tasks.iter().map(|(id, (t, _))| (id.clone(), t.clone())).collect(),
            running: g.tasks.iter().filter(|(_, (_, st))| *st == SessionState::Running).map(|(id, _)| id.clone()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, state: SessionState, archived: bool) -> Summary {
        Summary { id: id.into(), title: id.into(), state, archived, ..Summary::default() }
    }

    #[test]
    fn events_keep_it_current_once_filled() {
        let ix = TaskIndex::default();
        // before the first list read, events are nothing to keep: the list will have them
        ix.changed("x");
        ix.created(&row("y", SessionState::Running, false));
        assert!(ix.lock().dirty.is_empty() && ix.lock().tasks.is_empty());
        {
            let mut g = ix.lock();
            g.filled = true;
            g.tasks.insert("a".into(), (calls::task_of(&row("a", SessionState::Review, false)), SessionState::Review));
        }
        ix.created(&row("b", SessionState::Running, false));
        ix.created(&row("old", SessionState::Done, true));
        ix.changed("a");
        assert_eq!(ix.lock().dirty, HashSet::from(["a".to_owned()]));
        ix.deleted("a");
        let g = ix.lock();
        assert!(g.dirty.is_empty());
        assert_eq!(g.tasks.keys().collect::<Vec<_>>(), ["b"]);
    }
}
