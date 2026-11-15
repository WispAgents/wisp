# ADR-0070: On Windows, `wisp serve`'s children are born in its nursery job

- Status: accepted
- Date: 2026-11-15
- Source: round 257, CI 708ba936 (windows-latest, node 22). Supersedes the "not taken" alternative in ADR-0063's
  *The Windows PTY window*; everything else in ADR-0063 stands.

## Context

wisp-proc puts each child in a `KILL_ON_JOB_CLOSE` Job Object of its own, so it ends with wisp. `std`/tokio give no
way to create a process inside a job (`PROC_THREAD_ATTRIBUTE_JOB_LIST` needs `raw_attribute`, unstable in 1.100), so
the child is started `CREATE_SUSPENDED` and joins its job after `CreateProcess` returns (`pipes.rs`, `contain.rs`;
`pty.rs` can't even start it suspended). A server killed in that window (`TerminateProcess`, a crash, a test's
`Child::kill`) leaves the child outside every job: suspended for ever (pipes) or running free (PTY), holding every
handle it inherited. In CI 708ba936 the `cargo test --workspace … | tee` step printed its result at 04:19:38 and never
ended: an `opencode` harness probe (the fake CLI) left that way held the step's stdout pipe, `tee` never saw EOF, and
the job was cancelled at 60 minutes ("Terminate orphan process: pid (1868) (opencode)"). The serve tests start ~50
servers, each probing ~10 CLIs as it starts, and many stop their server within a second of its start.

ADR-0063 rejected the server putting itself in a job: (1) it would also kill what wisp means to outlive it (the
desktop app's relaunch, `wisp update`'s restart, a browser, a terminal window), (2) it would change what an embedding
job sees.

## Decision

1. **`wisp_proc::nursery()`**: the process joins a new job with `KILL_ON_JOB_CLOSE | BREAKAWAY_OK`, its handle kept in
   a static (it closes only when the process ends). Every process it starts afterwards is born in it, with no window:
   however wisp ends, they end. Per-child jobs still apply on top (Windows 8+ nests a fresh job inside the nursery), so
   `TerminateJobObject` on one child's tree is unchanged. Unix: a no-op (the guard, guard.rs, does this job there).
2. **`serve::boot` calls it first**, before any plugin starts anything. A failure is logged and wisp runs on (per-child
   jobs still cover everything but the window).
3. **What must outlive wisp breaks away** (answers objection 1): `spawn_detached` (the update relaunch, terminal
   windows) and the new `wisp_proc::spawn_outliving` (the browser from `open_url`, `desktop::launch`) add
   `CREATE_BREAKAWAY_FROM_JOB`. A job around wisp that allows no breakaway (a CI runner's, a service manager's) refuses
   that with `ERROR_ACCESS_DENIED`; the start then goes ahead without the flag, inside the nursery, and ends with wisp,
   as everything in such a job does when it is closed. That is a choice of the embedding job, not wisp's.
4. **Objection 2**: nested jobs keep the embedding job's accounting and limits; the nursery only adds "ends with wisp".

### Edge cases (each with its test)

- A child started just before its server is killed hard, never put in a job of its own (the window), ends with the
  server; a per-child job inside the nursery still ends its tree with no survivor:
  `crates/wisp-proc/tests/nursery_windows.rs` `a_child_started_as_its_server_is_killed_ends_with_it` (Windows only:
  there is no job, and no such window, elsewhere).
- Probes, runs and exec plugins of a server killed hard: unchanged and still covered by
  `crates/wisp/tests/serve_orphans.rs`.

## Consequences

- G12 (0 orphans) holds on Windows with no window, for pipes and PTYs (ADR-0063's documented PTY window is closed),
  and for any process `wisp serve` starts without wisp-proc.
- Under an embedding job without `BREAKAWAY_OK`, a browser, a terminal window or the update relaunch started by
  `wisp serve` ends when wisp does (before: it outlived wisp). For `wisp update`'s relaunch that means the waiter
  ends with the server it stops, and no new server starts: the user restarts it by hand (as when the relaunch can't
  start at all). To confirm on a real machine under such a job; a loud refusal there is the alternative.
- The CLI (`wisp "task"`, `wisp attach`) is not in a nursery: it starts nothing that must end with it.
- Unix keeps a smaller window (from `fork` to `guard::watch`); not seen in CI, left as it is.
- New `windows-sys` feature `Win32_Storage_FileSystem` (for `GetLongPathNameW`, `wisp_proc::long_path`, same round):
  a feature of a dependency already pinned (ADR-0015), no new crate.
