//! The same death-with-wisp guarantee for a child wisp-proc didn't start itself (ADR-0063): an exec plugin, which the
//! kernel starts with tokio (it can't depend on this crate). [`prepare`] the command, spawn it, [`adopt`] its pid:
//! Unix: it leads a process group of its own, listed with the guard (guard.rs) until the [`Contained`] drops (after
//! the reap); Windows: it starts suspended, joins a `KILL_ON_JOB_CLOSE` job of its own, then runs, so nothing it
//! starts is ever outside the job, and the job (and whatever is left in it) ends when the `Contained` drops or wisp dies.

use crate::sys;

/// Before the spawn: Unix, a process group of its own; Windows, no console window and suspended until [`adopt`].
pub fn prepare(cmd: &mut tokio::process::Command) {
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW | sys::CREATE_SUSPENDED);
    }
}

/// A child that was [`prepare`]d and spawned, held so it and what it starts end with wisp. Drop it once the child is
/// reaped (Unix: its pgid may be a stranger's after that).
#[derive(Debug)]
pub struct Contained {
    pid: u32,
    #[cfg(windows)]
    job: sys::Job,
}

/// Right after the spawn of a [`prepare`]d command. Windows: an `Err` means it couldn't join a job; it was ended
/// before it ran (the caller reaps it).
pub fn adopt(pid: u32) -> std::io::Result<Contained> {
    #[cfg(unix)]
    {
        crate::guard::watch(pid);
        Ok(Contained { pid })
    }
    #[cfg(windows)]
    {
        let job = sys::Job::new().and_then(|job| job.assign(pid).map(|()| job)).and_then(|job| sys::resume(pid).map(|()| job));
        match job {
            Ok(job) => Ok(Contained { pid, job }),
            Err(e) => {
                let _ = sys::terminate_pid(pid, 1);
                Err(e)
            }
        }
    }
}

impl Contained {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Ends the child and everything still in its group (Windows: its job) now.
    pub fn end(&self) {
        #[cfg(unix)]
        let r = sys::signal_group(self.pid, sys::SIGKILL);
        #[cfg(windows)]
        let r = self.job.terminate(1);
        if let Err(e) = r {
            tracing::debug!(pid = self.pid, error = %e, "contained end");
        }
    }
}

impl Drop for Contained {
    fn drop(&mut self) {
        #[cfg(unix)]
        crate::guard::reaped(self.pid);
    }
}
