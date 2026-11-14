# ADR-0062: Server errors reach telemetry through a kernel point, `kernel.failures@1`; panics through a hook

- Status: accepted
- Date: 2026-11-14
- Source: F-upd-4 (docs/v1-features.md): "Given telemetry on, when I use wisp, then anonymous stats and errors go to
  PostHog (off switch)." v1: server/index.js (`captureError` on every 5xx, its route with ids as `:id`),
  server/services/telemetry.js (`crashed` on `uncaughtExceptionMonitor`, the queue written to disk; an agent's
  `errorText` as `AgentError`). Extends ADR-0021 (the telemetry pipeline) and ADR-0003 (the kernel knows no feature).

## Context

v2's telemetry core could build `$exception` (`Telemetry::capture_error`, `crashed`, a golden) but nothing called it.
The kernel already sees every server error: a command that fails is recorded in Diagnostics with its plugin, a route
that answers 5xx too. But it told no plugin, and wisp.telemetry can't name the edge (the kernel has no feature to call
and a plugin has no hook into dispatch). A release build is `panic = "abort"`: a panic ends the process right after
the panic hook, so `catch_unwind` never reports it, and v1's "write the queue, send it on the next start" is the only
way the crash is seen.

## Decision

1. **`kernel.failures@1`** (`wisp_kernel::FAILURES`, trait `FailureWatch { fn failed(&FailureRecord) }`), a list
   point declared by the kernel like `kernel.tasks` and `kernel.http`. The kernel calls every contribution, with the
   record it keeps in Diagnostics, for exactly two cases: a **root** command (depth 0, the call nobody above answers
   for, not run by a route) that failed with `internal` (a handler's panic included, in a build that unwinds), and a
   **route** that failed with `internal`. A refusal (invalid, not found, forbidden, conflict) is an answer, not an
   error (v1's `status < 500`); a plugin that's off, a service gone or a timeout (503/504) is a state Diagnostics keeps,
   not a server error. A failure is told once: by its root, or by the route whose handler ran it (`CallInner::under_route`). `failed` runs on the failing call's task: it must only hand the
   record on (a lock and a push), never block or run a call.
2. **wisp.telemetry contributes one** and queues `$exception` (`type: InternalError`, `route`: the command id or
   `METHOD /pattern`, never the request's values), through the same opt-out, scrubbing and queue as every event.
3. **Panics:** wisp.telemetry installs a process panic hook once (chained to the one before, so the default report
   still prints), unless the environment turns stats off (`DO_NOT_TRACK`, `WISP_TELEMETRY=0`, `CI`). It captures the
   panic unhandled (`type: Panic`, message and stack) and keeps the queue in its saved body (`pending`) with a
   blocking store write, on a thread of its own, waited for at most 2 s, so a stuck writer can't hold the crash. The
   next start puts it back in the queue (dropped if stats are off by then). A failure record that only says "panicked"
   is left to the hook.
4. **The saved body is the user's:** wisp.telemetry's row (`plugin:wisp.telemetry:meta`: the install id, the
   switch, the feedback waiting to be sent, a crash's queue) is on a topic of its own so the store keeps it, and
   `capabilities.json` denies that topic to `agent:session` (and so every agent set), `apikey` and `preview`, whose
   bare `topic:read` would read it.
5. **Agents' errors:** a turn that ends with `errorText` goes as `$exception` `type: AgentError` with the run's props
   (agent, model, mode, effort, followUp), as v1.
6. **The UI sends no errors**: v1 had no window `error` / `unhandledrejection` reporting, so v2 adds none.

## Places it reaches (ADR-0040)

Wherever wisp runs: the app, `wisp serve`, the grid's boxes and an agent's MCP calls all go through the same kernel
dispatch and routes, so every server error is seen once at the edge whoever called.

## Edge cases (each with a test)

- A root command's internal error goes once, scrubbed (home folder and paths cut): `a_server_error_at_the_edge_goes_as_one_scrubbed_exception`.
- A nested internal error is told once, by its root: same test.
- A route's 5xx goes with its pattern, never its values: `a_route_that_answers_5xx_goes_with_its_pattern_never_its_values`.
- A route whose command fails is one error, the route's; a 503 is none: `a_route_that_runs_a_failing_command_is_one_error_and_a_503_is_none`.
- Two panics in one run are both kept: `a_panic_is_kept_with_the_queue_and_sent_by_the_next_start`.
- Only the user reads the saved body: `only_the_user_reads_the_saved_body`; it survives a restart: `the_install_id_and_the_off_switch_survive_a_restart`.
- A refusal sends nothing; the off switch sends no exception: `a_refusal_is_an_answer_and_stats_off_send_no_exception`.
- `DO_NOT_TRACK` sends no exception: `do_not_track_sends_no_exception`.
- A panic is kept with the queue and sent once by the next start: `a_panic_is_kept_with_the_queue_and_sent_by_the_next_start`.
- A kernel "panicked" record is left to the hook: `a_failure_names_its_command_or_route_and_a_panic_is_left_to_the_hook`.

## Consequences

Any plugin can watch server errors (a log sink, a local error page) without a private API. A watcher that blocks
slows the failing call only. The hook is process-wide: several kernels in one process (tests) each get the panic.
