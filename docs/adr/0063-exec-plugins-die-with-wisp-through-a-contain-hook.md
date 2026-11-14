# ADR-0063: Exec plugins die with wisp through a `Contain` hook in `ExecOptions`

- Status: accepted
- Date: 2026-11-14
- Source: round 208. Round 207 (`crates/wisp-proc/src/{guard.rs,pipes.rs}`) made every wisp-proc child end with the
  server however it ends; exec plugins (ADR-0003 tier 3, ADR-0046 user plugins, ADR-0048 trust) don't go through
  wisp-proc. Extends gate G12 (0 orphans) to them.

## Context

`wisp_kernel::exec::default_spawn` starts an exec plugin with `tokio::process::Command` and `kill_on_drop`. That
reaches only the child, and only when the server runs its drop (a clean stop). Killed hard (SIGKILL, an OOM kill,
`TerminateProcess`) the server runs nothing: on Unix the plugin's stdin closes, so a plugin that exits on EOF goes, but
one that ignores it, and every process a plugin started, keeps running; on Windows there is no job at all. wisp-proc
has the fix (a guard process on Unix, a `KILL_ON_JOB_CLOSE` Job Object joined before the child runs on Windows), but
the kernel can't depend on wisp-proc (wisp-proc → nothing, plugins → kernel + wisp-proc: a kernel → wisp-proc edge
is a cycle through every plugin that uses both, and the kernel stays OS-feature-free, ADR-0003).

## Decision

1. **The kernel declares the hook, the install fills it.** `wisp_kernel::Contain { prepare(&mut Command); adopt(pid)
   -> io::Result<Box<dyn Contained>> }` and `Contained { end() }`; `ExecOptions::contain: Option<Arc<dyn Contain>>`,
   `None` by default (the kernel alone keeps today's behaviour). Additive: every `ExecOptions` literal uses
   `..Default::default()`.
2. **`default_spawn` with a `Contain`**: `prepare` the command, spawn, `adopt` the pid at once. An `adopt` error is a
   failed start (the implementation ended the child before it ran). The `Contained` lives in the supervisor: switching
   the plugin off calls `end()` (the whole group / job) before the child's own kill, and so does the child exiting on
   its own (a crash: what it started goes with it; the group still has members, so its pgid isn't a stranger's); it is
   dropped after the reap.
3. **wisp-proc's `contain` module** is the implementation (`prepare`, `adopt`, `Contained`): Unix, `process_group(0)`
   and `guard::watch(pgid)`, `guard::reaped` on drop, `end` = `killpg(SIGKILL)`; Windows, `CREATE_NO_WINDOW |
   CREATE_SUSPENDED`, a new `KILL_ON_JOB_CLOSE` job, assign, resume (any failure: `TerminateProcess`, `Err`), `end` =
   `TerminateJobObject`, the job closes on drop.
4. **wisp passes it for every user exec plugin**, trusted and untrusted (`plugins/plugins/host/src/install.rs`,
   `ProcContain`).
5. **An `ExecSpawner` service (the grid's sandbox) is not contained by this**: it builds its own command and owns its
   children's lifetime (the box). `contain` is ignored there, as documented on the field.

### Edge cases (each with its test)

- Server SIGKILLed / `TerminateProcess`ed while a trusted exec plugin that ignores its stdin closing runs with a
  grandchild in its group: neither survives. `crates/wisp/tests/serve_orphans.rs`
  `a_server_killed_hard_leaves_no_exec_plugin_and_no_grandchild` (the fake's `exec-plugin` role).
- An exec plugin crashes right after starting a grandchild; the server runs on: the grandchild is ended (Unix:
  before, only unlisted, it then outlived even a hard kill). `an_exec_plugin_that_crashes_leaves_no_grandchild`.
- The guard's own list handling (a group dropped with `-` is let go, malformed lines skipped): round 207's
  `guard::tests::at_eof_the_guard_ends_the_listed_groups_and_only_those`, unchanged and shared.

## The Windows PTY window (left from round 207)

portable-pty builds its `CreateProcessW` call itself, so a ConPTY child can't start `CREATE_SUSPENDED`; wisp-proc
assigns its job right after `spawn_command` returns (`pty.rs`, `Tree::new`). A process the program starts in that
window (from `CreateProcessW` returning to `AssignProcessToJobObject`, one `process_id` call apart) is outside the job
and can outlive a hard kill. ConPTY holds a program's console I/O until wisp answers its startup cursor query, and
that answer is sent only by the reader thread started after the assignment, so a program that starts anything only
after it first touches its console is already in the job; one that starts a process before any console I/O can still
slip through. A
job-wide alternative (the server putting itself in a `KILL_ON_JOB_CLOSE` job so every child inherits it from birth)
would also kill processes wisp means to outlive it (the desktop app's relaunch, `wisp update`'s restart) and change
what an embedding job (CI, a service manager) sees, so it is not taken. The window stays, documented here and in
`pty.rs`; a clean stop's tree kill still reaches such a process by walking parent links.

## Consequences

- A user's exec plugin and everything in its group/job ends with wisp, however wisp ends; on Unix, as for every
  wisp-proc child, not a grandchild that left its group (setsid, a daemon) after a hard kill.
- On Unix an exec plugin no longer shares the server's terminal process group: Ctrl-C in the server's terminal reaches
  it through the clean stop (plugins off), not the terminal's SIGINT.
- What only Windows CI can confirm: the suspended start + job + resume path for exec plugins, and the test above on
  Windows.
