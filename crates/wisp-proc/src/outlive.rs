//! Which starts leave which jobs on Windows (ADR-0070, DESK2-05): the rule `spawn_outliving` and `spawn_detached` follow,
//! pure so it's tested on every OS.

/// `CreateProcess`'s `CREATE_BREAKAWAY_FROM_JOB`: the child is in none of this process's jobs.
pub const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

/// Set to `1` by a parent that puts this process in a `KILL_ON_JOB_CLOSE` job of its own after starting it (the
/// desktop shell, desktop/src-tauri/src/server.rs): that job holds everything wisp starts, as a job it was started in
/// does, even when wisp's nursery was made before the parent joined it to its job.
pub const PARENT_JOB_ENV: &str = "WISP_IN_PARENT_JOB";

/// The jobs around this process, as a start meant to outlive it sees them (Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Jobs {
    /// wisp's own nursery ([`crate::nursery`])
    pub nursery: bool,
    /// a job it was started in (the desktop shell's, a CI runner's): closing the app ends everything in it
    pub outer: bool,
}

/// The creation flags a start meant to outlive this process tries on Windows, in order. Inside a job wisp didn't make
/// (the desktop shell's: closing the window must stop everything wisp started) it stays there, unless it's the one
/// start that must outlive the app (`leave_every_job`: the update's relaunch helper), which asks to break away. In
/// wisp's nursery alone it breaks away from that (a terminal, a browser outlive `wisp serve` run in a terminal).
/// A job that allows no breakaway refuses with access denied: the second try is `flags` alone, inside. When the two
/// are the same it's tried once.
#[must_use]
pub fn outliving_flags(flags: u32, jobs: Jobs, leave_every_job: bool) -> [u32; 2] {
    let breakaway = if jobs.outer { leave_every_job } else { jobs.nursery };
    [if breakaway { flags | CREATE_BREAKAWAY_FROM_JOB } else { flags }, flags]
}

/// The jobs around this process now (Windows; off Windows none).
#[must_use]
pub fn jobs() -> Jobs {
    #[cfg(windows)]
    {
        Jobs { nursery: crate::sys::in_nursery(), outer: crate::sys::in_outer_job() }
    }
    #[cfg(not(windows))]
    {
        Jobs::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{CREATE_BREAKAWAY_FROM_JOB, Jobs, outliving_flags};

    const DETACHED: u32 = 0x8 | 0x200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    const OUT: u32 = DETACHED | CREATE_BREAKAWAY_FROM_JOB;

    /// Which starts leave which jobs (DESK2-05): inside the desktop shell's job only the update's relaunch helper
    /// breaks away; a terminal or browser wisp opens there stays, so closing the window stops everything wisp started.
    /// `wisp serve` in a terminal (its nursery alone): what outlives it leaves the nursery, as before.
    #[test]
    fn only_the_relaunch_helper_leaves_a_job_wisp_was_started_in() {
        let none = Jobs::default();
        let nursery = Jobs { nursery: true, outer: false };
        let desktop = Jobs { nursery: true, outer: true };
        let outer_only = Jobs { nursery: false, outer: true };
        // the desktop shell's job (or CI's): ordinary detached starts stay inside, tried once
        assert_eq!(outliving_flags(DETACHED, desktop, false), [DETACHED, DETACHED]);
        assert_eq!(outliving_flags(DETACHED, outer_only, false), [DETACHED, DETACHED]);
        // the relaunch helper asks to leave every job, then (a job that refuses) starts inside
        assert_eq!(outliving_flags(DETACHED, desktop, true), [OUT, DETACHED]);
        assert_eq!(outliving_flags(DETACHED, outer_only, true), [OUT, DETACHED]);
        // only wisp's nursery: every outliving start leaves it
        assert_eq!(outliving_flags(DETACHED, nursery, false), [OUT, DETACHED]);
        assert_eq!(outliving_flags(DETACHED, nursery, true), [OUT, DETACHED]);
        // no job at all: nothing to leave
        assert_eq!(outliving_flags(DETACHED, none, false), [DETACHED, DETACHED]);
        assert_eq!(outliving_flags(DETACHED, none, true), [DETACHED, DETACHED]);
        assert_eq!(CREATE_BREAKAWAY_FROM_JOB, 0x0100_0000, "CreateProcess's value");
    }
}
