# ADR-0086: Preview's proxy takes the next free port, and says which it took

- Status: accepted
- Date: 2026-11-16
- Source: Windows CI on e978c065 (`the_proxy_frames_the_app_adds_the_picker_and_refuses_a_foreign_host` failed: the
  task's proxy port couldn't be bound). v1 server/runs.js `pickPort`. Amends ADR-0049 decision 1 (the port) and its
  edge case *The proxy's port is taken*.

## Context

ADR-0049 put a task's proxy on exactly `42000 + hash(id) % 900` and gave up (no proxy, no Pick) when that port couldn't
be bound. A port can be held by another program, and on Windows whole ranges are reserved (Hyper-V / WinNAT,
`netsh int ipv4 show excludedportrange`), so for some task ids Preview never got its proxy. v1 didn't have this bug: its
`pickPort` tried the task's port, then the next 59 in the span, then any free one. v2's port dropped that fallback.

The port is derived from the task so the proxy's origin (`http://localhost:<port>`) is the same each run: the app keeps
its localStorage, cookies and service workers across restarts (v1: "so the app keeps its storage"), and two tasks
don't share an origin.

## Decision

1. **The proxy tries, in order:** the task's own port, then the next ones in the 42000–42899 span (wrapping), 60 in
   all (`wisp_terminal_core::pick::proxy_ports_for`, pure, the same order each time), then a port the system picks
   (`127.0.0.1:0`). The first that binds is the proxy's. A free own port keeps ADR-0049's stable origin; a taken one
   gives the same alternate each time while the conditions stay the same.
2. **The port a proxy runs on is the one it bound, and only the running proxy says it:** `Proxies::start` returns it,
   `Proxies::port(id)` reads it, and `RunInfo.proxy` / `RunStarted.proxy` carry it. Nothing recomputes it from the id
   (`proxy_port_for` is only the first try). The panel already frames `http://localhost:<proxy>`, so the UI is unchanged.
3. No proxy (the frame shows the app itself, no Pick) only when loopback takes no listener at all.

## Consequences

- Preview's Pick works whatever the task id and whatever holds ports on the machine.
- When the own port is taken, the app shows under another origin that run: its storage from earlier runs isn't seen
  (as v1).
- The app's own port has the same fallback (below, *The app's port too*).

## Edge cases

| Case | What happens | Test |
|---|---|---|
| The task's own proxy port is held by another program (or reserved by the system) | the next free one of its 60, else any free one; `proxy` names the port it answers on | `proxy.rs` `a_taken_proxy_port_moves_to_a_free_one_and_says_which` |
| The proxy is started again while it runs | the same port, not a second listener | same |
| The candidate order | its own port first, 60 distinct ports, all in 42000–42899 (never an app's 41000–41899), the same each time | `pick.rs` `the_preview_proxy_frames_the_app_and_adds_the_picker_as_v1` |
| The own port is free | it's the one taken (ADR-0049's stable origin) | `proxy.rs` `the_proxy_frames_the_app_adds_the_picker_and_refuses_a_foreign_host` (asserts `port(id)` is what `start` returned) |
| The run is stopped | `port(id)` is none and the port is free again | same |

## The app's port too

The same flaw sat on the app's port: `terminal.run.start` gave the app `PORT=41000 + hash(id) % 900` whatever held it,
so with that port taken (another program, a Windows excluded range) the dev server failed to start, or started
elsewhere while `RunInfo.port` / `RunStarted.port` and the row's mark still named the derived one. v1 ran `pickPort`
for the app as for the proxy (`APP_BASE`), passing over ports its own other runs held, and found where the app really
listened from the first local address it printed, else `PORT`.

1. **The app's port is chosen at each start, as the proxy's:** the task's own, then the next ones in 41000–41899
   (wrapping), 60 in all (`wisp_terminal_core::run::ports_for`, pure), then any port the system gives. A port is free
   when no other task's app has it (given and not ended: one still starting counts, its program may not have bound
   yet), loopback binds it, and nothing answers on it on `127.0.0.1` or `[::1]` within 150 ms (v1 `listenable` +
   `answers`: a dev server on `[::1]`, or one whose `SO_REUSEADDR` lets the bind through, is still seen). wisp only
   probes; the app binds it.
2. **The chosen port is recorded (`Registry::claim_app_port`) and read back:** the app's `PORT`, `RunStarted.port`
   (started or already running), `RunInfo.port` while it runs, and the row's mark (`ext["wisp.terminal"].run.port`, set
   at start, kept as it was at exit or Stop) all read the record; nothing recomputes it from the id (`run::port_for` is
   only the first try, and what `RunInfo.port` says before any run). Choosing and recording is one step under one lock, so two starts never get one port; a second Run for a task that started while it probed answers the running app's port. Hanging the run up (Stop, its `DELETE` route, a restart) forgets it.
3. Where the app really listens stays as ADR-0049 had it: the first local address it prints (`Registry::local_port`),
   else the port it was given; the proxy targets that, and the panel's Open link reads the printed address.

| Case | What happens | Test |
|---|---|---|
| The task's own app port is held by another program (or reserved) | the app gets the next free one of its 60 as `PORT`; Run's answer, `run.info`, a second Run and the row's mark all say it | `crates/wisp/tests/serve_terminal.rs` `a_taken_app_port_still_runs_the_app_and_every_caller_says_the_real_port` (fails on the fixed port) |
| Another task's app has the port, still starting | not given to a second task | `registry.rs` `a_taken_app_port_moves_to_the_next_free_one_and_is_the_one_recorded` |
| The own port free again (after Stop) | it's the one taken again (the stable origin) | same |
| The candidate order | own first, 60 distinct, all in 41000–41899 (never a proxy's 42000–42899), the next ones wrapping | `run.rs` `preview_runs_the_worktree_run_else_the_dev_script_with_a_stable_port` |
| The app exits or is stopped | the row's mark keeps the port it was given (`running: false`) | `serve_terminal.rs` `a_tasks_running_app_is_marked_on_its_row_until_it_stops_or_exits` |
