# wisp v2 server performance audit, round 1 (2026-11-18)

Worktree `/Users/mokh/.wisp/worktrees/wisp-8fb64ec939b0`, HEAD `1b971518` (branch for PR #756). Machine: this Mac
(Apple Silicon, macOS 15.6). Read-only: no repo file was edited; every measurement ran on scratch homes under
`/tmp/qa-keep/perf/` (never `~/.wisp` or `~/.wisp-dev`), with the fake harness CLIs only (no paid run).

Builds: `cargo build --locked --profile budgets -p wisp -p wisp-testkit` (7 min, `CARGO_TARGET_DIR=/tmp/wisp-target-perf`),
plus a symbolized copy of the same profile for `sample` (`CARGO_PROFILE_BUDGETS_STRIP=none
CARGO_PROFILE_BUDGETS_DEBUG=line-tables-only`, `/tmp/wisp-target-perf-sym`).

Scratch environment: `/tmp/qa-keep/perf/env.sh` (the bench's own cleared env: fakes first on PATH, `WISP_*_BIN` fakes,
no telemetry, no update check). The 5k home is the bench's migrated `cold-home` (5,000 tasks: 2,500 done, 1,250 error,
1,250 review). Helpers: `srv.sh start|stop <home>`, `cmd.py <cmd> <args> [n]` (POST /api/cmd), `cold.py`,
`idlecpu.py`, `longcpu.py`, `cli.py`, `samp.py`.

## 1. The server budget bench (as CI runs it)

`cargo run --locked --profile budgets --manifest-path spikes/server-budgets/Cargo.toml -- --keep` **does not run on
HEAD**: `--locked` refuses because `spikes/server-budgets/Cargo.lock` is stale (finding 2). To still get the numbers I
ran the identical spike from a copy (`/tmp/qa-keep/spikecopy`, every top-level entry symlinked back to this worktree,
the spike's lock updated there only, `--offline`), on the same `budgets` binaries. Full log:
`/tmp/qa-keep/perf-bench.log`; kept folder `/var/folders/sy/…/T/wisp-server-budgets-71611`.

| budget | measured (this Mac) | budget | headroom | rounds |
|---|---:|---:|---:|---|
| cold start → first response | 41.5 ms (median of 5 rounds) | 150 ms | 108.5 ms (72 %) | 40.9 · 41.5 · 42.2 · 41.4 · 43.5; single restarts 39.3–107.3 |
| idle RSS (settled, after the list) | 104.1 MB | 120 MB | 15.9 MB (13 %) | 101.9 MB before the list |
| task list (`fresh=list`, first page, 50 tasks) | 0.83 ms | 5 ms | 4.17 ms (83 %) | 0.83 · 0.85 · 0.85 · 0.82 · 0.81 |
| patch p99 (10 running, 4 clients) | 106.8 ms | 150 ms | 43.2 ms (29 %) | 104.6 · 106.8 · 110.0; p50 83.7 ms |
| durable append | 1,064,699 events/s | 20,000/s | 53× | spikes/events |
| write amplification | 40.0× | 44× | 4.0× (9 %) | WAL 4.64 MB + db 0.47 MB over 0.13 MB kept |

Runner calibration 7.61–8.92 ms (1.2×): a clean run, no round marked †.

Notes on the table: patch p50 of 84 ms is ADR-0083's 100 ms shared coalescer window (`plugins/runtime/host/src/drive.rs:34`),
by design; p99 has 43 ms left for everything else. Write amplification and idle RSS are the tight ones (9 % and 13 %).
The bench measures idle RSS before any `POST /api/cmd` and before any second client, so it can't see findings 1 and 6.

## 2. Findings

### 1. The edge keeps the last 10,000 command answers in memory, with no size bound: 60 `runtime.list` → 1.0 GB  (critical)

- **Measured:** fresh server on the 5k home, settled at 46 MB physical footprint. `runtime.list` ×20 → 362 MB, ×40 → 708 MB,
  ×60 → **1.0 GB** (≈17 MB kept per call, a 2.3 MB answer held as a `serde_json::Value` tree). With the CLI:
  10 × `wisp ls` → 46 → 176 MB, 227 MB a minute later, **RSS 192 MB: over the 120 MB idle budget** after normal use.
  50 full share previews of a 1.97 MB task: 76 → 240 MB.
- **Repro:** `source /tmp/qa-keep/perf/env.sh; srv.sh start <copy of home-b>; python3 cmd.py runtime.list '{}' 20`, then
  `vmmap --summary <pid> | grep 'Physical footprint:'`. Or run `wisp --port <p> ls` 10 times.
- **Cause:** `crates/wisp-api/src/sync.rs:20` `IDEM_KEPT = 10_000` and `Idempotency::once` (sync.rs:91–113) keep every
  answer's `Done = (u16, Value)` for the last 10,000 idempotency keys, whatever its size and age; every `POST /api/cmd`
  goes through it (`crates/wisp-api/src/own.rs:79–98`). Every CLI call uses a fresh key (`crates/wisp/src/client.rs:150`),
  so nothing is ever replayed from it, and it only grows.
- **Impact:** server memory grows with every command answered, up to 10,000 answers (GBs with list-sized results). The
  idle RSS budget is met only because the bench never sends a command before it measures.

### 2. CI's server-budgets step fails before it measures: the spike's `Cargo.lock` is stale  (high)

- **Measured:** `cargo run --locked --profile budgets --manifest-path spikes/server-budgets/Cargo.toml` → exit 101, "cannot
  update the lock file … because --locked was passed". The lock needs `ts-rs` added under `wisp-resources-api`,
  `wisp-sandbox-api` and `wisp-telemetry-api` (diff of the lock a non-locked build wrote: 3 lines).
- **Repro:** the command above from the worktree root (it's the command in `.github/workflows/suite.yml:149`).
- **Cause:** `b4a78bc7` (8 h ago) added `ts-rs` to those three api crates; `spikes/server-budgets/Cargo.lock` (its own
  workspace, `spikes/server-budgets/Cargo.toml:12`) was last updated in `7f2416b3` (17 h ago).
- **Impact:** the server budget gate measures nothing on PR #756's head. Any regression since then (findings 3–5
  included) can't fail CI, and the step itself fails CI.

### 3. `worktrees.view` reads every task one by one: 178–547 ms for a 162-byte answer  (high)

- **Measured:** `worktrees.view` on the 5k home: 474 · 375 · 547 · 404 · 276 ms, then 183 · 178 · 188 ms warm; 162 B
  answered. A boot profile puts `RuntimePort::tasks` and its `runtime.get` dispatches at ≈400 of ≈2,300 busy samples.
- **Repro:** `srv.sh start …/home-b; python3 cmd.py worktrees.view '{}' 5`.
- **Cause:** `plugins/worktrees/host/src/runtime_port.rs:67–110`: `runtime.list` (all 5k, archived too), then
  `runtime.get` for each task, in sequence, to read its worktree slot. Each `get` is a store read through the blocking
  pool plus the .env mask (`wisp_runtime::env_view`). Called by `Cleanup::view` (`cleanup.rs:257`) in the request
  (`commands.rs:148–157`), and the same command then spawns `look_for_offers` + `refresh` (`lib.rs:150–157`, another
  full walk), and the scan it starts ends in `changed.notify_one()` (`cleanup.rs:165`) → `refresh` again: **3 walks of
  5k reads per pane open**. `fill_kids` (runtime_port.rs:60) is O(tasks × children) on top.
- **Impact:** opening the Worktrees pane waits ~0.2–0.5 s at 5k tasks and grows linearly; it's also the core of findings 4 and 5.

### 4. The Machine page's process view walks every task every 10 s while anything runs (≈10 % of a core)  (high)

- **Measured:** idle server, 5k home: a 0.9–1.3 CPU-s burst every 60 s (CPU 2.17 → 3.04 s at t≈60 s, 3.11 → 4.41 s at
  t≈120 s, nothing between). `machine.read` (the page's read) 233–542 ms for 567 B. Page open (a read every 15 s):
  **+4.3 CPU-s in 45 s**.
- **Repro:** `python3 longcpu.py …/home-b` (idle bursts); `python3 cmd.py machine.read '{}' 5` (latency).
- **Cause:** `plugins/machine/host/src/procs.rs:106–118`: the sample runs every tick (`this_mac.rs:21` `SAMPLE_MS = 10 s`)
  when a run is live, one ended in the last 3 min (`AFTER_MS`) or the page was read in the last 30 s (`READ_MS`,
  `machine/core/src/this_mac.rs:23–25`), else once a minute (`IDLE_MS`, procs.rs:33). Each sample:
  `runtime.list` (procs.rs:151), `ps`, footprints, then `worktrees.view` (procs.rs:174 → finding 3, its 3 walks
  included), plus a `runtime.get` per named task for facts (procs.rs:184–196). `machine.read` (procs.rs:330–338) runs
  the whole sample inside the request.
- **Impact:** with any task running, wisp burns ~10 % of a core continuously at 5k tasks (more with more tasks), and the
  Machine page answers in ~0.25–0.5 s.

### 5. Every restart spends ~2 CPU-s in the 2 s after its first response re-reading every task  (medium)

- **Measured:** 5k home, after the first 200: CPU 0.32 s at t=0.25 s, 1.89 s at 1 s, 2.60 s at 2 s, then flat
  (repeat starts 1.94–2.6 CPU-s; a home's first start after a copy 3.6 CPU-s). 40 short-lived blocking-pool threads.
  Profile (stopped at launch, `sample` attached, resumed): ≈1,050 of ≈2,300 busy samples in `wisp_runtime::repo::load_session`
  (= `runtime.get`), reached mostly from worktrees' `RuntimePort::tasks` (start `refresh`), machine's first sample
  (`worktrees.view` from `wisp_machine::procs`) and ship's boot.
- **Repro:** `python3 idlecpu.py …/home-b`; profile: `python3 samp.py`, then `/tmp/qa-keep/perf/boot-sample2.txt`.
- **Cause:** start-time walks, each `runtime.list` + one `runtime.get` per task: worktrees `on_start` → `refresh`
  (`plugins/worktrees/host/src/lib.rs:266–269`, finding 3), machine's first sample (finding 4) and
  `plugins/ship/host/src/wire.rs:177–185` (`boot`: `runtime.get` for every one of the 5k rows, to find the few with a
  worktree; review fixed the same pattern with a list-row field, `plugins/review/host/src/lib.rs:57–63`). Evidence for
  ship: the bench's cold-start logs are 430–607 KB of `runtime.get: wisp.runtime is off` warnings, 3,745 in
  `serve-cold-3-2.log`: that many `get`s were still queued when the bench stopped the server ~40 ms after its first response.
- **Impact:** the cold-start budget doesn't see it (it's after the first 200), but the UI's first reads land in this
  window, and it costs ~25k store reads per start at 5k tasks (linear in tasks). Bisecting by plugin wasn't possible:
  see the side note on switch-offs.

### 6. Every new client is pushed the whole task list: 4.9 MB at 5k tasks, of which 46 KB is the timed first page  (medium)

- **Measured:** one client `GET /api/events?…&fresh=list,session:<id>` on the 5k home: 34 frames, 4.68 MB. Frames 2–12
  are the `list` snapshot's 11 pages (46 KB, then 10 × ~463 KB); the session snapshot is 7 KB and the patches
  0.4–9 KB each. Four clients on `list` + 10 streaming tasks: per client 4,934 KB list snapshot, 74 KB session snapshots,
  284 KB of patches (118 patches). The bench's own "clients read 493 frames, 58,847 KB" is exactly 3 rounds × 4 clients
  × 4.9 MB. Server side, every task as one read is 33.6 ms (bench, "every task").
- **Repro:** `curl -s -N --max-time 14 -H 'Accept: text/event-stream'
  "http://127.0.0.1:<port>/api/events?proto_min=2&proto_max=2&build=perf&client_id=c1&fresh=list" > s.sse`, then split
  on blank lines (script in this report's history: `/tmp/qa-keep/perf/stream4-*.sse`).
- **Cause:** `crates/wisp-api/src/v2_pages.rs:61–91` (`read_paged`): after the first page it reads and emits every page of
  both queries (not archived, then archived: `crates/wisp/src/serve.rs:134–137`) at connect. ADR-0081 sped up the
  first page; the rest still follows at once, for every client.
- **Impact:** each fresh connect (a tab, the desktop app, a phone or a grid tunnel reconnecting without a cursor) costs
  ~34 ms of server CPU and 4.9 MB on the wire at 5k tasks, linear in tasks, archived ones included. Patch traffic is 6 %
  of a client's bytes.

### 7. Share preview scrubs every string and key against every other task's id: 109–137 ms on a 2 MB task, in the request  (medium)

- **Measured:** a task with 1,003 steps (1.93 MB): `share.preview` spec 5–7 ms, report 8–14 ms, **full 109–137 ms**
  (1.97 MB answer). Small tasks 3–11 ms. Each full preview also leaves ~3 MB behind (finding 1).
- **Repro:** `python3 cmd.py share.preview '{"id":"<big>","level":"full"}' 3` (the big task is `/tmp/qa-keep/perf/big.id`
  in `home-b`).
- **Cause:** `plugins/share/core/src/lib.rs:205–243`: for every string in the bundle, and every object key
  (`scrub`, lib.rs:246–255), the secret regexes, the env values, `folder_forms` rebuilt and sorted per string
  (lib.rs:169), then `others` (lib.rs:41–75): for each of the ~5k other task ids, a `find` plus a fresh `String` rebuild.
  So the cost is strings × 5k ids, on the async request thread (no `spawn_blocking`). The ids come from a full
  `runtime.list` per preview (`plugins/share/host/src/calls.rs:56`), and the steps are read one turn at a time
  (`plugins/share/host/src/lib.rs:90–98`); `share.bundle` runs the preview again (lib.rs:152–153).
- **Impact:** grows with transcript size × task count; at 20 MB of history it's over 1 s of one worker's CPU.

### 8. The sign-in repair re-checks every failed task on every start, before the first response  (low)

- **Measured:** cold start median 36.6 ms on the 5k home; the same home with its 1,250 error tasks marked
  `halt: failed` (no turns, so each check returns early): **47.4 ms (+10.8 ms, ≈9 µs a task)**. A real failed task
  also reads its last turn (presentation included).
- **Repro:** `cp -R home-pristine home-failed; sqlite3 home-failed/wisp.db "update runtime_session set
  summary=json_set(summary,'$.fields.halt','failed') where state='error'"; python3 cold.py home-failed 7` (vs
  `cold.py home 7`).
- **Cause:** `plugins/runtime/host/src/runtime.rs:118–127` loops over every failed task in `Runtime::start` (awaited by
  the plugin's `on_start`, before `wisp_api::serve`, `crates/wisp/src/serve.rs:305–337`), each `repair_signed_out`
  (`finish.rs:218`) taking the task lock and reading its last turn. Nothing records that a task was checked, so it
  repeats on every restart. The `working` loop right after (`runtime.rs:142–151`, a `repo.session` per running lead)
  has the same shape.
- **Impact:** cold start grows with the number of failed tasks, forever. The bench's seed never sets `halt: failed`,
  so it can't see it.

### 9. Thread health re-reads the last three turns' full steps at every turn end  (low)

- **Measured:** `threads.health` on a thread with one 1.96 MB turn: 6–8 ms; `runtime.activity` of that turn alone 7 ms.
  At a turn end it reads up to 80 turns plus the full steps of the last 3 work turns, so a long, busy thread pays
  ≈3 × (one turn's steps) + 80 turn rows at every turn end, and each turn's steps are read on 3 consecutive turn ends.
- **Repro:** `python3 cmd.py threads.health '{"id":"<big>"}' 3`; `python3 cmd.py runtime.activity '{"id":"<big>","turnId":"<turn>"}' 3`.
- **Cause:** `plugins/threads/host/src/health.rs:52–80` (`work_out`: `Turns` limit 80, then `ActivityOf` per turn for
  the last 3 work turns), then `notes::load` and a writer transaction even when nothing changes (`save`, health.rs:93–107);
  run from the `TurnEnded` subscriber (`plugins/threads/host/src/lib.rs:122–134`), and recomputed from scratch on every
  `threads.health` read (health.rs:111–113).
- **Impact:** subscriber-side (not on the stream's path), but it scales with step volume per turn and adds a
  writer transaction to every turn end.

### 10. `wisp ls` and `wisp show last` read the whole 2.3 MB list  (low)

- **Measured:** CLI against the 5k server: `wisp ls` 81 ms, printing all 5,000 rows (485 KB); `ls --all` 76 ms;
  `wisp show <id>` 7 ms but `wisp show last` 60 ms. One-word slips (`tsks`, `frob`, `status`), `crew start`, `help`
  and `runtime`: 10–11 ms; `--version` 7.5 ms; the offload hook (`wisp hook offload`, run on every Bash tool call) 7 ms.
- **Repro:** `python3 cli.py`.
- **Cause:** `runtime.list` with no limit is the whole list as JSON (2.3 MB, 34 ms over HTTP for 5k); `show last` resolves
  "last" through it. Each call also adds ~13–17 MB to the server (finding 1).
- **Impact:** small alone; it matters because every one of these CLI calls grows the server (finding 1).

## 3. Checked and fine

- Claude Code's default-model reads (`plugins/claude/host/src/default_model.rs`): async file reads, `claude auth status`
  run once per detect and cached; no process per request.
- grid-client `last_grid`: in memory, written only when it changes (`state.rs:306–313`). The grid dialer loops only
  while a computer is on (`computer.rs:139–166`).
- The offload hook takes a fast path before the async runtime starts (`crates/wisp/src/offload_cli.rs:10`, `main.rs:57`): 7 ms.
- The CLI's catalogue is fetched only for words that aren't the CLI's own (`crates/wisp/src/main.rs:248–262`); slips answer in ~10 ms.
- Share preview of a task with no history: 3 ms; notes, health and `runtime.get` on small tasks: < 1 ms.
- Harness detect at start is spawned off the start path (`plugins/runtime/host/src/plugin/mod.rs:121–137`).
- Python's first `urllib` request costs ~11 ms (its macOS proxy lookup); `curl` answers `/api/status` in 0.3 ms. First
  samples of each `cmd.py` loop are that, not the server.

## 4. Side notes (not performance, not counted)

- **A plugin switched off doesn't stay off across a restart:** `wisp plugin disable wisp.ship` answers "✓ wisp.ship off",
  but after a restart `wisp plugin ls` shows it active again (same for `wisp.worktrees`). Repro: `/tmp/qa-keep/perf/without.sh`
  then `plugin ls` after a restart.
- `wisp show "Fix the flaky auth test"` (quoted words, not an id) launched a task titled "show Fix the flaky auth test"
  instead of refusing.

## Count

**10 findings**: 1 critical (1), 3 high (2, 3, 4), 3 medium (5, 6, 7), 3 low (8, 9, 10). All six server budgets pass
on this Mac, but only when run from a copy: CI's step can't run on HEAD (finding 2).
