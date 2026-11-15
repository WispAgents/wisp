# Working on wisp v2

From a fresh clone to a running wisp you can debug and test. For a person or an agent. The rules are in
`AGENTS.md`, the map is in `ARCHITECTURE.md`, and the plan is `docs/v2-plan.md`. Every command here was run on
macOS (2026-11-15). `tools/check`'s `commands` rule checks that the commands still resolve.

## 1. Clone to running

**You need:**

- **rustup**, not a distro or Homebrew cargo. `rust-toolchain.toml` pins the version (1.100.0, with clippy and
  rustfmt). The first `cargo` call in the repo installs it. Unset `RUSTUP_TOOLCHAIN` before you run cargo, or
  it overrides the pin.
- **Node ≥ 22.18** (`engines` in `package.json`). CI runs Node 22 and 24.
- For `npm run dev:desktop` only: the Tauri prerequisites (Xcode Command Line Tools on macOS, webkit2gtk-4.1 on
  Linux, WebView2 on Windows).

```sh
npm install           # once: links the TypeScript workspaces
npm run doctor        # one ok/warn/FAIL line per check, with its fix; exit 1 on FAIL
npm run doctor -- --json
```

`npm run doctor` reads and builds nothing, and takes about a second. A FAIL line means:

| FAIL | What's wrong | Fix |
|---|---|---|
| Node | older than `engines` | install Node 24 (`nvm install 24`) |
| npm deps | `node_modules` is missing or out of sync with `package-lock.json` | `npm install` |
| rustup | rustup isn't on PATH | https://rustup.rs |
| RUSTUP_TOOLCHAIN | it's set to a different version than the pin, so rustfmt and clippy won't match CI | unset it (it's a warn if the versions match) |
| git hooks | `monoflow init` left hooks that rewrite shared worktrees | delete them; install monoflow with `node tools/setup-monoflow.ts` |
| desktop (Tauri) | on macOS there are no Command Line Tools, so cargo has no linker | `xcode-select --install` |

Warns and notes don't block anything. For example: set `CARGO_TARGET_DIR` (one shared target dir per machine),
`npm run build` (the web build only matters for `wisp serve` without Vite), and
`cargo build -p wisp-testkit --bins` (fake agents, for running serve tests by hand).

**Run it:**

```sh
npm run dev           # cargo build -p wisp, then wisp serve on :4800 + Vite with HMR on :5180
npm run dev:desktop   # the same + the Tauri window (app id dev.wisp.dev) loading Vite's page
npm run dev -- --help # --port, --vite-port, --home, --no-watch
```

- Open http://localhost:5180. The API is http://127.0.0.1:4800 (`/api/health` answers there). If a port is
  busy, it takes the next free one and says so.
- The home is `~/.wisp-dev` (database, secrets, scratch). A dev run refuses ports 4477 and 4791 and the homes
  `~/.wisp` and `~/.wisp-v2-preview`. The desktop dev app has its own identifier, `dev.wisp.dev`, so it gets its
  own single-instance lock and data dir and never takes over an installed wisp.
- UI edits hot reload. A change under `crates/` or `plugins/*/host` rebuilds and restarts the server on the
  same port and home. If the build fails, the old server keeps running. `r` + Enter rebuilds by hand.
- **To stop it:** Ctrl-C, or `q` + Enter. That stops everything it started, by pid. If something still holds the
  port, look before you kill: `lsof -i tcp:4800 -sTCP:LISTEN`, then kill that pid. Never use pkill or killall:
  they hit other agents' processes.
- To talk to the dev server from the CLI, point the CLI at it: `WISP_HOME=~/.wisp-dev WISP_PORT=4800 target/debug/wisp ls`.

`npm run dev:web` runs Vite alone with the **mock peer** and needs no Rust (see §2).

## 2. Debugging

**Server logs** go to stderr, so in `npm run dev` they show in that terminal. There is no log file under
`~/.wisp-dev`. The level comes from `WISP_LOG` (an `EnvFilter`, default `warn`; `RUST_LOG` is not read):

```sh
WISP_LOG=info npm run dev                         # plugin activation, startup
WISP_LOG=warn,wisp_api=debug,wisp_runtime=trace npm run dev   # per crate: the crate name with _
```

A packaged desktop app starts its own server, which writes `server.log` in the app's data dir
(macOS: `~/Library/Application Support/dev.wisp.app/server.log`). The dev window attaches to the dev server, so
its logs stay in your terminal.

**Spans:** every command runs inside a `cmd{cmd=… set=…}` span (`crates/wisp-api/src/own.rs`), and every HTTP
route inside `route{method=… path=… set=…}` (`crates/wisp-api/src/edge.rs`). Any `tracing::debug!` that fires
inside one prints with that prefix. Few code paths log inside them today, so add the event you need.

**The browser:** use Chrome devtools on http://localhost:5180. The Network tab shows `/api/events` (SSE frames)
and the command POSTs. **In the Tauri window**, debug builds have the WebKit inspector: right-click, then
Inspect Element.

**A debugger on the Rust server:** use `rust-lldb` on the dev server's pid (`npm run dev` prints it in the
`wisp serve up … (pid N …)` line).
`rust-lldb -p <pid>`. On macOS, run `DevToolsSecurity -enable` once first. With Developer Mode off, the attach
waits for a password prompt (that's what happened when this guide was written). Debug builds have full
debuginfo. There is no `.vscode/launch.json` yet, because a CodeLLDB config hasn't been tested.

**The mocks** (`apps/web/dev/mock-*.ts`) are a fake server written in TypeScript (`mock-peer.ts`). It serves
`npm run dev:web` (port 5180, or the next free one), `qa/parity.mjs` and the UI tests. It doesn't run under `npm run dev`, where
`WISP_DEV_API` sends `/api` to the real server; `WISP_MOCK=0` turns it off. `mock-commands.ts` answers commands
and `mock-reads.ts` answers the reads a screen sends when it opens. When a screen sends a command the mock
doesn't have, add it there with the args from the plugin's api crate. Seeds:
`curl -X POST 'http://localhost:5180/__mock/reset?seed=first-run'` (also `computers`, `usage`, `trial`, `cloud`).
`/__mock/fail?cmd=<id>` refuses the next run of a command so you can watch the rollback. `MOCK_SESSIONS=5000`
gives you a long list.

**A store row:** the database is `~/.wisp-dev/wisp.db` (SQLite, WAL). Open it read-only, so nothing writes
behind the store's back:

```sh
sqlite3 -readonly ~/.wisp-dev/wisp.db ".tables"
sqlite3 -readonly ~/.wisp-dev/wisp.db "select seq, topic, entity, id, op from change order by seq desc limit 5"
sqlite3 -readonly ~/.wisp-dev/wisp.db "select id, state, agent, title from runtime_session order by created_at desc limit 5"
```

Each plugin's tables start with its prefix (`runtime_`, `accounts_`, …). `change` records every row a command
wrote.

**CI logs:** `gh run list --limit 5` lists runs, `gh run view <id>` shows one run, and `gh run view <id> --log-failed`
prints only the failing steps. The lead keeps downloaded logs in `target/ci/` (`*-failed.log`, one per job).
Search them with grep instead of downloading them again.

## 3. Testing

Coders run **only the test they're writing**. Full suites run in PR CI (`.github/workflows/suite.yml`) and in the
one batched pass. In agent sessions, the hook `.agents/bin/no-suites.sh` (`.claude/settings.json`) refuses:
workspace-wide `cargo test` or `cargo nextest` (no `-p`), `npm test`/`npm t`, `npm run test`/`verify` (with `--workspace` too), `monoflow verify`,
a full `qa/parity.mjs`/`layout.mjs`/`feedback.mjs` run, and pkill/killall. The reason: several coders share one
machine, and a full suite takes most of an hour of CPU. The batched pass puts `WISP_OUTER_LOOP=1` in front of the
command.

| Area | Example (each one passed when this guide was written) |
|---|---|
| a Rust crate | `cargo test -p wisp-store --test migrate` (7 tests) |
| one test by name | `cargo test -p wisp-claude-core --test context_window the_context_window` |
| a core crate (pure, sans-IO) | `cargo test -p wisp-claude-core --test subagents` (`plugins/*/core/tests/`) |
| a plugin host with a fake harness | `cargo test -p wisp-runtime --test quick_cancel` (runs the real `wisp-fake-agent` through the proc service) |
| a UI vitest file | `npx vitest run packages/ui/test/app-frame.test.ts` |
| a tools test | `node --test tools/test/dev.test.ts` |
| a UI gate on one view | `node qa/parity.mjs --views=home --widths=1440 --themes=dark` |

```sh
cargo test -p wisp-store --test migrate
cargo test -p wisp-runtime --test quick_cancel
npx vitest run packages/ui/test/app-frame.test.ts
node --test tools/test/dev.test.ts
QA_PARITY_PORT=4597 node qa/parity.mjs --views=home --widths=1440 --themes=dark
WISP_OUTER_LOOP=1 node qa/layout.mjs    # outer loop only: the full gate
```

- A crate's tests are in `<crate>/tests/*.rs`; `--test <file>` picks one. The crate names are in each
  `Cargo.toml` (`plugins/claude/host` is `wisp-claude`, `plugins/claude/core` is `wisp-claude-core`).
- Fake harnesses: `crates/wisp-testkit` builds `wisp-fake-agent` and `wisp-fake-gh`. Serve tests build them
  the first time they run. No test needs the network or a real Claude or Codex.
- The `qa/*.mjs` scripts drive system Chrome through playwright-core in `PW_DIR` (default `/tmp/wisp-qa-pw`;
  see `qa/README.md`). Set `QA_PARITY_PORT` if 4590 is taken. A gate exits 1 when a shot differs, and its
  report lands in `qa/out/`.
- Keep the code compiling while you work: `cargo check -p <crate>` and
  `cargo clippy --workspace --all-targets -- -D warnings`. `npm run check` and `npm run typecheck` are fast and
  allowed.

**Goldens** (`test/goldens/`, and each crate's `tests/goldens*`) hold everything a peer, an agent or a released
client reads. A test that fails on a golden shows the diff. If you changed the shape on purpose, rewrite the
golden by running the failing test with `UPDATE_GOLDENS=1` in front (for example
`UPDATE_GOLDENS=1 cargo test -p wisp-protocol --test goldens`), read the diff, and say so in the PR
(`change-a-contract` skill). Never update goldens to make a red test pass.

## 4. Common tasks

Each skill is in `.claude/skills/<name>/SKILL.md`. Any agent can read them.

- **add-command**: something a user, agent, channel or peer can make wisp do (a command, an HTTP route, an MCP
  tool), with its capability and optimistic patch.
- **add-plugin**: a new feature area. It scaffolds `plugins/<name>/{api,host,ui}`, writes the manifest and
  registers it.
- **add-migration**: a new table, column or index, or a change to row or event shapes (forward-only, plugin
  prefix).
- **add-ui-extension**: a screen, pane, button, chip or block renderer built from the kit and checked in Chrome.
- **change-a-contract**: before you touch `@wisp/kernel`, `@wisp/protocol`, an api package, an extension point,
  a `data-part` or a golden. Run `.agents/bin/monoflow graph impact <file>` first.
- **record-decision**: a decision that changes the architecture gets an ADR in `docs/adr/`, in the same PR.
- **release**: bump the workspace version and `npm/wisp-agents/package.json`; `release.yml` publishes.

## 5. Troubleshooting

- **The build is slow, or the disk is full:** each worktree builds into its own `target/` unless you set
  `CARGO_TARGET_DIR`. A workspace build takes about 10 GB. `npm run doctor` shows the free space. Use
  `cargo clean` on old target dirs. Never run two full workspace builds at once on a shared machine.
- **A stale or odd build** (old binary, a test that runs old code): check that cargo ran against the pin.
  `RUSTUP_TOOLCHAIN` left over from a parent process changes the compiler and throws the cache away. Unset it.
- **Port busy:** `npm run dev` moves to the next free port and prints it. A qa script names its port in the
  error (`port 4590 is taken`). Pick another one with `QA_PARITY_PORT`, or stop the holder if you started it.
  Only kill a pid you started.
- **`warning: failed to parse serde attribute … transparent`** during a build is noise from ts-rs reading serde
  attributes it doesn't handle. The bindings are still right. Ignore it.
- **`cargo deny check`** prints about 13 `warning[duplicate]` lines (base64, syn, thiserror, …). These are
  crates two dependencies pin at different versions. What counts is the last line:
  `advisories ok, bans ok, licenses ok, sources ok`.
- **The Tauri lock after Ctrl-C:** stopping `npm run dev:desktop` leaves `/tmp/dev_wisp_dev_si.sock` behind
  (still true on 2026-11-15, `docs/v2-plan.md` §4.6). On macOS the next start finds nothing listening on it,
  removes it and goes on, so it's harmless. Delete it by hand if a start ever says another instance is running.
- **A bare `wisp <word>` starts a task:** the CLI takes any word it doesn't know as a prompt. `wisp tasks` with no
  flags, for example, launches an agent. The list of commands is `wisp help`. To stop a task you started by
  mistake, run `wisp stop <id>`.
- **`npm run check` fails on a doc:** the `commands` rule found a command that no longer resolves (a script, file,
  crate or `wisp` word). Fix the command. If a line can't be checked, add `<!-- cmd-check: skip <why> -->` on it.
- **`npm run dev` shows a blank page:** check the terminal for `wisp serve exited`. Then `curl http://127.0.0.1:4800/api/health`,
  and press `r` + Enter to rebuild.
