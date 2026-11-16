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
- The app's own port (`run::port_for`, 41000 + hash) is still fixed: the app binds it, not wisp; not changed here.

## Edge cases

| Case | What happens | Test |
|---|---|---|
| The task's own proxy port is held by another program (or reserved by the system) | the next free one of its 60, else any free one; `proxy` names the port it answers on | `proxy.rs` `a_taken_proxy_port_moves_to_a_free_one_and_says_which` |
| The proxy is started again while it runs | the same port, not a second listener | same |
| The candidate order | its own port first, 60 distinct ports, all in 42000–42899 (never an app's 41000–41899), the same each time | `pick.rs` `the_preview_proxy_frames_the_app_and_adds_the_picker_as_v1` |
| The own port is free | it's the one taken (ADR-0049's stable origin) | `proxy.rs` `the_proxy_frames_the_app_adds_the_picker_and_refuses_a_foreign_host` (asserts `port(id)` is what `start` returned) |
| The run is stopped | `port(id)` is none and the port is free again | same |
