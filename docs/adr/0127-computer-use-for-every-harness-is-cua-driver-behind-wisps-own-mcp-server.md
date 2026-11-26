# ADR-0127: computer use for every harness is Cua Driver behind wisp's own `computer` MCP server

- Status: accepted
- Date: 2026-11-26
- Source: owner, 2026-11-26: "i want a standard way for computer use provided by wisp for all the harness, is there is
  an opensource solution we can use or we developer our own" → "use cua and build the layer"; "make sure to test it
  in grid and sandbox"; "prepare the grid and sandbox with a browser for testing when needed by agents".
- With ADR-0003 (everything is a plugin), ADR-0040 (meet users where they work), ADR-0047 (sandboxes), D-0023 (new
  integrations off by default), D-0055 (pinned, justified dependencies).

## Context

Each harness has its own computer use or none: Claude Code's is Anthropic's API tool, Codex and opencode have none
built in, and a user who wants one wires an MCP server per harness. Driving a screen well is a large job of its own:
screenshots at native resolution, input that doesn't steal focus, accessibility trees (AX on macOS, UIA on Windows,
AT-SPI on Linux), macOS's permission prompts, X11 and Wayland. Cua Driver (trycua/cua `libs/cua-driver`, MIT, Rust)
does all of it and speaks MCP over stdio (`cua-driver mcp`); release 0.62.1 ships signed, checksummed binaries for
macOS, Linux and Windows (x86_64 and arm64).

Measured on the grid (a box of `wisp-grid-user` under gVisor, arm64, 2026-11-26): cua-driver 0.62.1 lists 65 tools;
it screenshots the box's 1280x800 Xvfb screen and clicks; typing needs a window manager (no `_NET_ACTIVE_WINDOW`
without one: `foreground_unavailable`), and Chromium's page shows in the accessibility tree only with a session bus,
the AT-SPI bus and `--force-renderer-accessibility` (1 element without, 18 with). With all three, `type_text` into
the address bar took the browser to a page. The driver's 65 tool schemas are 226 KB of JSON, about 55k tokens a run.

## Decision

1. **wisp doesn't drive screens; Cua Driver does.** A new plugin, `wisp.computer` (`plugins/computer/{api,core,host}`),
   is the layer around it. wisp is tested with one driver version (`DRIVER_VERSION`, 0.62.1); the grid image pins the
   same one by its release's SHA256SUMS. The AGPL perception extension (`cua-driver perception`) is never installed
   or offered.
2. **One MCP server for every harness.** While computer use is on, every task's run gets `computer`: HTTP, at
   `POST /api/computer/mcp`, with the task's own token and `x-wisp-session` (a `runtime.run@1` part, as wisp's own
   server is given). Every harness that takes MCP servers per run gets it with no harness-specific code. The harness
   never talks to the driver: wisp answers.
3. **wisp's policy, per call, from the server's side.** The gateway (`wisp-computer-core::gateway`) answers
   `initialize`, `ping` and `tools/list` itself and judges each `tools/call`:
   - a Plan mode task (its mode as the runtime last started it, never a header) sees and calls only the tools whose
     `readOnlyHint` is true;
   - `policy::DENIED` is never offered in any mode: the driver's own config, updates and installs, the perception
     extension, replays, the agent cursor's looks, recordings, raw mouse buttons (19 tools; a box's run sees 46 of
     65, one on the user's computer 44, without the clipboard);
   - another task's token is a 403 (the edge's `agent:session` rule); `computer:*` is in `agent:session`, nowhere in
     a capped set.
   What passes goes to the driver and its answer comes back as it is: a screenshot stays an MCP image.
4. **The driver starts on the first call, not the first list.** Until then `tools/list` is the pinned driver's own
   list (`core/data/tools.json`), so a run that never looks at the screen starts nothing (on a box that's a screen
   and a browser it doesn't pay for). Started, the live list replaces it and each call is judged by it again. One
   driver per wisp, in the plugin's scope; it starts again on the next call when it died. Its product telemetry and
   update check are off (`CUA_DRIVER_RS_TELEMETRY_ENABLED=0`, `CUA_DRIVER_RS_UPDATE_CHECK=false`).
5. **Off by default** (D-0023): `computer.enabled` in Settings, or `WISP_COMPUTER=on`. `computer.status` (CLI, MCP,
   Settings) says whether it's on, which driver, its version, and what to do when it can't work. The driver is
   `WISP_COMPUTER_DRIVER`, else `cua-driver` on the path, else `~/.local/bin/cua-driver` (where Cua's installer puts
   it). On a Mac without Cua's app (`/Applications/CuaDriver.app`, which holds the grants), wisp runs `cua-driver mcp
   --direct` so macOS asks for the grants for wisp instead of failing.
6. **A grid box and a sandbox come ready.** The image has the driver, a window manager (matchbox, no title bars),
   the session and accessibility buses, and Chromium with its accessibility on (`wisp-desktop`), and sets
   `WISP_COMPUTER=on`: the box is the sandbox. `WISP_COMPUTER_DRIVER=wisp-computer` brings the screen up before the
   driver; `WISP_COMPUTER_BROWSER=wisp-browser-mcp` gives every run that isn't in Plan mode `browser` too
   (Playwright on that same Chromium), so an agent tests a page with page tools or with the screen, and the user
   watches it in the task's browser view.

## Consequences

- Every harness gets computer use, in wisp or outside it (`wisp run`, a task started from a channel), with one
  policy and one place to switch it off. A harness with its own computer use keeps it; wisp's is added beside it.
- Cost: the tool list is about 42k tokens per run while it's on (167 KB for the 44 tools of a local run). Claude
  Code defers large MCP tool sets; others load them whole. Trimming the driver's own descriptions is left to a
  later ADR (they're the driver's contract with the model).
- A new external binary: the grid image pins it by checksum; on a laptop the user installs it (`INSTALL_HINT`),
  and `computer.status` says so. A newer driver is a wisp change: `DRIVER_VERSION`, the image's checksums and
  `data/tools.json` move together.
- Not yet: approvals per action on the user's own computer (today the switch is the consent), recordings into the
  task, a Settings pane of its own (the switch is in Settings → All), Windows' boxes.
- Tests: `plugins/computer/core` (policy, gateway, framing), `plugins/computer/host` (lookup, `--direct`),
  `crates/wisp/tests/serve_computer.rs` (a real `wisp serve`, the fake agents and a stand-in driver: Claude Code's
  and Codex's runs get it, a screenshot comes back as an image, Plan mode, the deny list, a forged token, off by
  default and switched on in Settings).
