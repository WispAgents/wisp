# wisp v2 server performance audit, round 2 (2026-11-19)

> Findings 1–8 are fixed: before and after in [perf-server-r2-fixes.md](perf-server-r2-fixes.md).

Worktree `/Users/mokh/.wisp/worktrees/wisp-8fb64ec939b0`, HEAD `c0d655ad`. Machine: this Mac (Apple M2, 8 cores,
macOS 15.6). Read-only: this file is the only repo change. Every measurement ran on scratch homes under `/tmp/perf2s/`
(never `~/.wisp*`, never ports 4477, 4800 or 4791), with wisp-testkit's fake agent and fake gh, and no real harness.

**Build.** `cargo build --locked --profile budgets -p wisp -p wisp-testkit` into `CARGO_TARGET_DIR=/tmp/wisp-target-perf2s`
(release with thin LTO, the profile CI's bench uses). Line tables were kept (`CARGO_PROFILE_BUDGETS_STRIP=none
CARGO_PROFILE_BUDGETS_DEBUG=line-tables-only`) so `sample` can name functions. One build: 45 min 19 s. The bench
spike then built into the same target dir. `samply` and `cargo flamegraph` aren't installed, so profiles are macOS
`sample` (1 ms) run while a command loops. The busiest wisp frames were read with their `file:line`.

**Data.** The 5k home is the bench's own migrated `cold-home` (5,000 tasks: 2,500 done, 1,250 error, 1,250 review),
copied fresh for each experiment. Streams are 10 tasks on the fake claude (`FAKE_STREAM=2`, `FAKE_STREAM_MIX=agent`:
a real agent's steps, 2 a second each). Clients are `/api/events?fresh=list,session:<each of the 10>`. The big task
for per-task commands has 1,000 steps (1.96 MB of activity), like round 1's.

**The machine was loaded.** The load average was 6.5 on 8 cores, from someone else's `node` (≈120 %), Chrome and Steam,
and the disk was 94 % full. Wall-clock numbers today run 1.5–2.3× round 1's for the same binary. To separate code from
machine, I ran round 1's binary (`/tmp/wisp-target-perf/budgets/wisp`, still on disk) against round 2's on copies of
the same home, and I report CPU-seconds where they can stand in for wall time.

**Instrumentation caveat.** `vmmap --summary` suspends the target process while it reads it. My first 30-minute run
sampled memory with it every 30 s, and every sample stalled the stream (p99 800 ms, max 4.0 s, in clusters exactly
31 s apart). The latencies below leave out the 7 s around each `vmmap`. Later latency runs use `ps` only.

Scripts: `/tmp/perf2s/{w.py, cold.py, ab.py, idle10.py, stream30.py, outbox.py, burst.py, sweep.py, cli.py, prof.py,
hot.py}`. Raw output sits next to them (`*.out`, `prof-*.txt`, `stream-s30.json`). The bench log is
`/tmp/perf2s/bench.log`, and its kept folder is `/var/folders/sy/…/T/wisp-server-budgets-58185`.

## 1. Numbers

| measure | budget | r1 (2026-11-18) | r2 (today) | note |
|---|---:|---:|---:|---|
| cold start → first response (bench) | 150 ms | 41.5 ms | **66.6 ms** | rounds 56.5 · 61.1 · 81.6 · 66.6 · 86.0† ms. A/B on the same home today: r1's binary 95.7 ms median, r2's 88.9 ms, so the change is the machine, not the code |
| idle RSS (bench, settled after the list) | 120 MB | 104.1 MB | **103.2 MB** | 95.2 MB before the list. Physical footprint of an idle server after 10 min: 41.6 MB |
| task list, first page (bench) | 5 ms | 0.83 ms | **1.18 ms** | every task (11 pages, 4.52 MB) 46.3 ms, not gated |
| durable append (bench) | 20,000/s | 1,064,699/s | **285,003/s** | no change to `crates/wisp-store` or `spikes/events` since r1: disk and load |
| write amplification (bench) | 44× (ratchet) | 40.0× | **40.2×** | WAL 4.67 MB + db 0.48 MB over 0.13 MB kept. Still 9 % headroom |
| patch p99, 10 running, 4 clients (bench, 10 s × 3) | 150 ms | 106.8 ms | **104.9 ms** | p50 88.0 (the 100 ms coalescer window, ADR-0083) |
| patch p99 over **30 min** streaming | 150 ms | — | **114.2 ms** | 144,000 of 144,000 stamps; each 5-min window's p99 111.4–116.8 ms, p99.9 134 ms, max 160 ms |
| memory over 30 min streaming | — | — | **flat** | footprint 48.4 MB at start, 66.7 MB at 5 min, 73.5 MB at 15 min, 69.4 MB at 30 min (max 78.8). CPU 0.8 % of a core. db 16 → 55 MB |
| CPU idle for 10 min (5k home) | — | 0.9–1.3 CPU-s every 60 s | **0.50 CPU-s in 570 s (0.088 % of a core)** | no bursts; r1 #4 fixed |
| CPU in the seconds after start | — | ≈2 CPU-s | **0.12 CPU-s** at the first response, 0.18 at 30 s | r1 #5 fixed |
| memory after 60 × `runtime.list` | — | 1.0 GB | **77 MB, flat** (and flat after 30 full share previews) | r1 #1 fixed |
| sync resume (60 reconnects from cursor while streaming) | — | — | welcome p50 1.6 ms, p99 3.1 ms; 60 of 60 resumed; 0 of 3,570 stamps missed | 72 KB per resume |
| 40 clients resuming at once | — | — | all welcomed in 20.4 ms | |
| a fresh (no cursor) client | — | 4.9 MB | **4.91 MB** | r1 #6 still open: finding 1 |
| 10 fresh clients at once | — | — | **1.03 CPU-s; the other client's patch p99 105 → 279 ms** | finding 1 |
| resume from `cursor=1` | — | — | resume, 5.4 MB in 0.77 s, 0.13 CPU-s; others' p99 unchanged (106 ms) | |
| a client that never reads (40 s) | — | — | RSS 146 → 150 MB; then it got its 7.5 MB snapshot, no resync | |
| `wisp ls` (5k) | — | 81 ms | **69.6 ms**, 485 KB printed | finding 5 |
| `wisp show last` | — | 60 ms | **10.2 ms** | r1 #10, half fixed |
| `wisp show <id>` | — | 7 ms | 9.3 ms | |
| `wisp tasks --touching <path>` | — | — | 30.0 ms (12 tasks) · **182.8 ms (2,012 tasks)** | finding 6 |
| `wisp blame file:line` | — | — | 107–184 ms (12 tasks) · 282–389 ms (2,012) | finding 2 |
| `wisp blame <file>` / `<folder>` | — | — | **1.2 s / 2.7 s** (12 tasks) · 1.4 s / 3.0 s (2,012) | finding 2 |

† the bench's own calibration marked the runner slowed for that round (1.6× its fastest), so the median left it out.

### The slowest 20 commands (5k home, median of 5 unless noted)

All 157 read-only commands in `cli.catalogue` ran with guessed args (59 refused args I couldn't guess, all in < 1 ms),
plus the per-task ones on the big task and 17 common writes. The 10 GET routes in `/api/openapi.json` all answer in
≤ 0.6 ms.

| # | command | median | answer | why |
|---:|---|---:|---:|---|
| 1 | `share.bundle` (big task, full) | 121.5–128.0 ms | 1.97 MB | preview + signing: finding 8 |
| 2 | `machine.read` | 90.4–129.7 ms | 566 B | `ps -E`: finding 3 |
| 3 | `share.preview` (big task, full) | 63.7–81.7 ms | 1.97 MB | finding 8 |
| 4 | `runtime.history` | 65.3–67.5 ms | 1.33 MB | finding 4 |
| 5 | `runtime.agent.stats` | 50.6–51.5 ms | 990 B | finding 4 |
| 6 | `runtime.create` | 36.5–48.8 ms | 117 B | the CLI spawn and the durable commit, in the request (checked, fine) |
| 7 | `runtime.usage` | 38.7–42.0 ms | 750 B | finding 4 |
| 8 | `machines.reachable` | 39.5–42.0 ms | 274 B | external processes (Add-machine dialog only) |
| 9 | `runtime.list {}` | 31.0–38.6 ms | 2.37 MB | the whole list as JSON (`limit: 50`: 2.9 ms) |
| 10 | `worktrees.view` | 21.9–22.3 ms | 161 B | finding 7 (r1: 178–547 ms) |
| 11 | `share.preview` (big task, report) | 19.0 ms | 2.7 KB | |
| 12 | `worktrees.files` | 15.2–18.2 ms | 65 B | |
| 13 | `worktrees.viewLast` | 15.5 ms | 162 B | |
| 14 | `runtime.activity` (the big turn) | 15.3 ms | 1.96 MB | |
| 15 | `mcp.tools` | 13.7 ms | 220 KB | |
| 16 | `worktrees.done.list` | 11.2–11.7 ms | 61 B | |
| 17 | `resources.write {}` | 10.1 ms | 199 B | |
| 18 | `router.view` | 8.7 ms | 4.8 KB | |
| 19 | `resources.export` | 7.3 ms | 193 B | |
| 20 | `runtime.agent.sessions` | 6.4 ms | 2.6 KB | |

I left out `runtime.quick`: it waits for its run to finish (5.0 s here, the fake's 5 s stream). All the other writes
answer in ≤ 2.5 ms: rename, pin, archive, setMode, setState, tag, delete, followUp, fork 5.6 ms, crew.create, stuff.save.

## 2. Round 1's findings now

| r1 | what | now |
|---|---|---|
| 1 | answer cache kept 10,000 answers (60 lists → 1.0 GB) | **fixed**: 77 MB flat after 60 lists |
| 2 | the spike's stale `Cargo.lock` | **fixed**: `cargo run --locked` runs |
| 3 | `worktrees.view` read every task one by one, 178–547 ms | **fixed** (22 ms). A linear residue remains: finding 7 |
| 4 | the machine view walked every task every 10 s | **fixed** for idle (0.088 % of a core). `machine.read` itself is still slow: finding 3 |
| 5 | ≈2 CPU-s after every start | **fixed**: 0.12 CPU-s; the cold-start logs are 4 KB (were 430–607 KB of `runtime.get` warnings) |
| 6 | every new client is pushed the whole list (4.9 MB) | **open**: finding 1, now with its cost to other clients measured |
| 7 | share preview scrubbed every string against every id, 109–137 ms | **improved**: 64–82 ms on a loaded machine. Finding 8 |
| 8 | sign-in repair before the first response | **fixed**: spawned after start (`runtime.rs:118–135`). It still re-runs on every start, off the path |
| 9 | thread health re-read three turns' steps at every turn end | **fixed**: `threads.health` 0.5 ms on the big task |
| 10 | `wisp ls` / `show last` read the whole list | **half**: `show last` 60 → 10 ms; `ls` still prints all 5,000 (finding 5) |

## 3. Findings

### 1. A fresh client's whole-list read holds a store reader and the sync actor: 10 fresh clients push others' patch p99 to 279 ms  (high)

- **Measured:**
  - **One fresh client:** `fresh=list` gets 4.91 MB at 5k tasks. The bench's 12 clients read 58.8 MB, which is 3 rounds × 4 clients × 4.9 MB.
  - **10 fresh clients at once, while 10 tasks stream** (`burst.py`): each had its first 4 MB after 444 ms, the server
    spent **1.03 CPU-s**, and the already-connected reference client's patch latency went from **p99 105.1 ms**
    (max 105.9) to **p99 279.3 ms** (max 284.5 ms) for the next 6 s. That is over the 150 ms budget.
  - **40 fresh clients at once** (`outbox.py`): 5.42 CPU-s, 7.98 MB each (list + 10 sessions), and footprint
    43 → 108 MB. It settled at 87–105 MB and stayed there over 6 more rounds (allocator high-water, not a leak).
  - **For contrast:** a resume from a cursor costs 1.6 ms and 72 KB, and even `cursor=1` (5.4 MB of replay) left the
    reference p99 at 106 ms.
- **Repro:** `python3 /tmp/perf2s/burst.py <copy of home-pristine>`, then read "ref in the 6 s after 10 fresh clients".
- **Profile** (`prof-fresh.txt`, 17 rounds of 10 fresh clients):
  - The busiest wisp frames are `v2_pages.rs:124` (10,247 samples), the store's reader `store.rs:266–291` (8,504),
    `json_out.rs:33` (5,715), the sync actor's `feed` (`v2_actor.rs:128`, 3,715) and serde_json's deserialise and
    re-serialise (`de.rs`, `wire.rs:44`).
  - There are 8,793 samples in `__psynch_mutexwait`.
- **Cause:**
  - **The read:** `crates/wisp-api/src/v2_pages.rs:61` `read_paged` reads the topic's every page inside one
    `store.read_async` (`v2_pages.rs:118–127`). That holds one of the store's **4** reader connections
    (`crates/wisp-store/src/store.rs:76`, `reader()` at `store.rs:283`) for the whole 5k-row read. Ten at once take all
    four, and every other read (commands, `runtime.get`, the CLI) waits on the pool's mutex.
  - **The encoding:** each page's rows come out of SQLite as JSON text (`query_json`), are parsed into `serde_json::Value`
    rows, and are serialised again per client by the single sync actor (`v2_actor.rs:124` `feed` → `wisp_sync` handle →
    `wire.rs:44`). While it encodes 10 × 4.9 MB, the actor emits nobody's patches.
  - **Why it's still there:** r1 #6. ADR-0081 sped up the first page, but the other 10 pages still follow at once.
- **Impact:** any burst of cursor-less connects stalls live streaming for everyone and pins up to 108 MB: tabs opening,
  the desktop app and a phone at once, or a grid tunnel or proxy dropping every client without its cursor. It grows
  linearly with tasks, archived ones included (`crates/wisp/src/serve.rs:134–137` reads both queries).
- **Suggested fix:**
  - Send the first page at connect and the rest on demand (scroll, search) or behind live patches at a low priority.
  - Read the remaining pages as separate short reads (keyset on `updated_at, id`), releasing the reader between pages.
  - Encode each `(topic, seq, page)` once and share the bytes between clients that ask at the same head.
  - Keep the store's JSON as `RawValue` instead of parsing and re-encoding it.

### 2. `wisp blame` spawns one `git show` per commit and re-parses every task file per commit: 1.2 s for a file, 2.7 s for a folder  (medium)

- **Measured:** in a clone of this repo (12 tracked tasks):
  - `wisp blame crates/wisp/src/main.rs` takes **1,223 ms** (34 distinct commits); `wisp blame crates/wisp/src` takes
    **2,709 ms**; `packages/ui/src/app` takes 2,377 ms.
  - With 2,012 tracked tasks: 1,381, 3,032 and 2,740 ms. Even `blame file:line` takes 107–184 ms (282–389 ms at 2,012).
  - Spawning the same 87 `git show -s --format=%B <sha>` takes 2,027 ms (23.3 ms each). One
    `git log --format=%H%x00%B -- crates/wisp/src` takes 151 ms. `git blame --porcelain` of all of main.rs takes 142 ms.
- **Repro:** `cd /tmp/perf2s/repo && wisp --port <p> blame crates/wisp/src` (or `python3 /tmp/perf2s/cli.py <port> <home>`).
- **Cause:**
  - **One process per commit:** `crates/wisp/src/repo_cli.rs:206` `tasks_of_commit` runs `git show -s` for every
    blamed commit. It is called per commit at `repo_cli.rs:259` (file and lines), `:271` (folder: every commit of
    `git log -- <path>`) and `:294` (ranges).
  - **A full re-parse per commit:** for every commit without a `Wisp-Task:` trailer (86 of 87 here), it calls
    `tracked(root)` (`repo_cli.rs:61`), which reads and YAML-parses every `.task.yml`. That is O(commits × tasks), on
    top of the `tracked()` that `blame` already did at `repo_cli.rs:220`.
- **Impact:** `wisp blame` is the answer AGENTS.md and the wisp skill send agents to before every change (also
  `wisp_blame` over MCP, `repo_cli.rs:568`). It takes seconds on real folders, linear in history, and multiplied by
  tracked tasks.
- **Suggested fix:**
  - Read every commit's body in one `git log --no-walk --format=%H%x00%B <shas…>` (or the folder's own `git log` with
    `%B`).
  - Build the commit → task index once from the `tracked()` already in hand.
  - Look each commit up in that index.

### 3. `machine.read` runs `ps -E` over every process on the machine: 90–130 ms a read, and every 10 s while a task runs  (medium)

- **Measured:** `machine.read` takes 90.4 ms median under profile, 129.7 ms in the sweep, for a 566 B answer. In the
  profile the server is idle: it waits on a child process. `ps -E -axww -o pid=,ppid=,rss=,%cpu=,etime=,command=` on
  this Mac takes **78.7 ms** median and prints **1.17 MB** for 613 processes. Without `-E` it takes 55.9 ms and 577 KB.
- **Repro:** `python3 /tmp/perf2s/prof.py machine.read '{}' 6`, then time the `ps` line above.
- **Cause:** `plugins/machine/host/src/procs.rs:125` (`Procs::ps`, Mac) spawns `ps -E` (every process's environment, to
  find wisp's task tag). Then `procs.rs:158` takes `wisp_proc::footprints` for every pid. `sample` (`procs.rs:139`) does
  both on each `machine.read` (`procs.rs:358`, in the request) and on each 10 s tick while a run is live or for 3 min
  after one (`procs.rs:109`, `this_mac.rs` `SAMPLE_MS`).
- **Impact:** the Machine page waits ~0.1 s per read, and every 10 s while anything runs a 1.2 MB `ps` is spawned and
  parsed. Its CPU is in the child, so wisp's own CPU numbers don't show it.
- **Suggested fix:**
  - List processes with libproc (`proc_listpids`, `proc_pidinfo`); `footprints` already uses `proc_pid_rusage`.
  - Read an environment (`KERN_PROCARGS2`) only for pids not seen before, as the Linux path does with `environ_wanted`.
  - Let `machine.read` answer from the last sample when it's younger than `READ_MS`.

### 4. Commands that need two fields of each task decode every task in full: `runtime.usage` 42 ms, `agent.stats` 51 ms, `history` 68 ms  (medium)

- **Measured** (5k home): `runtime.usage` 38.7–42.0 ms (750 B), `runtime.agent.stats` 50.6–51.5 ms (990 B),
  `runtime.history` 65.3–67.5 ms (1.33 MB). In each profile, `Repo::sessions` (`repo.rs:134`) and the store's
  `decode_row`/`decode_value` (`wisp_store` `entity.rs:238–253`) are 75–85 % of the busy samples: 1,309 of ~1,550 for
  usage, 1,280 of ~1,500 for agent.stats.
- **Repro:** `python3 /tmp/perf2s/prof.py runtime.usage '{}' 5` (and `runtime.agent.stats '{"session":"s00003"}'`).
- **Cause:** `plugins/runtime/host/src/repo.rs:131` `sessions()` reads every session row and decodes its whole body
  into a `Session`. Its callers mostly need a projection:
  - `plugin/reads.rs:24` (`runtime.usage`) needs id → cwd.
  - `plugin/agent_tools.rs:112` (`wisp_stats`, the agent's tool) needs agent and cwd.
  - `more/fork.rs:116` (`readopt`, run by the Claude Code `SessionEnd` adopt hook) looks for one task by
    `ext["wisp.adopted"].session` or `resume_id`. I didn't time `readopt`; it pays the same full decode.
  - `plugin/specs.rs:78` (`runtime.history`) needs each spec, then reads `summaries()` again for costs.
- **Impact:** linear in tasks, on commands an agent calls (`wisp_stats`) and on every adopt. Each is ~30–40 ms of one
  blocking thread at 5k tasks.
- **Suggested fix:** use the in-memory list (`lock_list` already has id, agent and cwd) or a column projection
  (`query_json` with `select`, as `summaries()` and `slots()` do). Give `readopt` a direct lookup (an indexed
  `json_extract` or a column) instead of a scan.

### 5. `wisp ls` still prints all 5,000 tasks  (low)

- **Measured:** `wisp ls` 69.6 ms (485 KB on stdout), `ls --all` 86.0 ms, `ls --json` 78.0 ms (2.76 MB). Each call asks
  the server for `runtime.list` with no limit: 31–39 ms and 2.37 MB per call (`limit: 50` takes 2.9 ms).
- **Repro:** `python3 /tmp/perf2s/cli.py <port> <home>`.
- **Cause:** r1 #10's `show last` was fixed, but `ls` still reads and prints the whole list.
- **Suggested fix:** show the newest 50 by default (`--all` for everything, `--limit n`), passing `limit` to
  `runtime.list`.

### 6. `wisp tasks --touching`, `--tracked` and `log` parse every task file on every call  (low)

- **Measured:** `tasks --touching packages/ui/src/app/Task.tsx` takes 30.0 ms with 12 tasks and **182.8 ms with 2,012**.
  `--tracked` takes 23.3 → 230.1 ms, and `log <path>` 22.2 → 177.4 ms: about 75 µs per task file. About 20 ms of each
  call is spawning `git rev-parse --show-toplevel` to find the repo (`/usr/bin/git` spawns cost 23 ms each here).
- **Repro:** `cd /tmp/perf2s/repo2k && wisp tasks --touching packages/ui/src/app/Task.tsx`. `repo2k` is this repo plus
  2,000 task files synthesized from its 12.
- **Cause:** `crates/wisp/src/repo_cli.rs:61` `tracked()` walks `.wisp/tasks` and YAML-parses every `.task.yml`
  (`wisp_resources_core::formula::documents`) before `filtered()` (`repo_cli.rs:345`) looks at `files`. `root()`
  (`repo_cli.rs:26`) spawns git each call.
- **Suggested fix:** skip files whose text doesn't contain the path before parsing them, or keep an index (path → ids)
  keyed by the folder's mtime. Find the repo root by walking up to `.git` instead of spawning git.

### 7. `worktrees.view` still reads every task's whole body to find the few with a worktree: 22 ms at 5k  (low)

- **Measured:** `worktrees.view` takes 21.9–22.3 ms for 161 B (r1: 178–547 ms). `worktrees.viewLast` takes 15.5 ms.
  In the profile, `runtime_port.rs:111` (`runtime.list`, every row run through the .env mask at `env_view.rs:350`) and
  `runtime_port.rs:126` take 2,122 samples, and `json_out.rs:33` (`runtime.slots`) takes 4,144. Each view also starts
  `look_for_offers` (`lib.rs:147`) and `refresh` (`lib.rs:154`), which is another view.
- **Cause:** `plugins/worktrees/host/src/runtime_port.rs:104–129` lists all 5k tasks, archived too, then
  `runtime.slots` (`plugins/runtime/host/src/repo.rs:124`: `select id, body` of every session) to read three ext slots.
- **Suggested fix:** filter in SQL to the tasks that have a `wisp.worktrees` slot (`json_extract(body, …) IS NOT NULL`)
  and join only those to the list. Skip the .env mask for a plugin caller that reads no text fields. Coalesce the
  refresh a view starts with the view itself.

### 8. Share preview and bundle still scrub on the request thread: 64–82 ms and 121–128 ms on a 2 MB task  (low)

- **Measured:** `share.preview` at level full takes 63.7–81.7 ms (1.97 MB answer); level report takes 19.0 ms.
  `share.bundle` takes 121.5–128.0 ms. In the bundle profile, `wisp_share_core` `lib.rs:217–221`
  (`scrub_value` → `scrub_with`) is 3,000 of 3,239 busy samples. Bundle minus preview, ≈40 ms, is the signing.
- **Repro:** `python3 /tmp/perf2s/prof.py share.bundle '{"id":"<big>","level":"full"}' 6` (the big task's id is in
  `/tmp/perf2s/big.id`).
- **Cause:** `plugins/share/core/src/lib.rs:169` `scrub_with` runs per string and per key. Each one does the secret
  regexes, a `replace` pass per env value, a `replace_path` per folder form and a replace per id set. It runs in
  `plugins/share/host/src/lib.rs:103` `preview` on the async worker, not `spawn_blocking`. `make` (`lib.rs:177`)
  repeats the whole preview for a bundle the dialog just previewed.
- **Suggested fix:** run it under `spawn_blocking`. Use one multi-pattern pass (Aho-Corasick over env values, ids and
  path forms) per string instead of one pass per pattern. Reuse the preview's bundle when `share.bundle` follows it
  with the same args.

## 4. Checked and fine

- **The 30-minute stream:** no memory growth (footprint plateau 65–79 MB), p99 steady at 111–117 ms in every 5-minute
  window, every stamp delivered to every client, 0.8 % of a core for 10 live runs and 4 clients. The db grew 39 MB for
  36,000 steps.
- **Idle:** 0.088 % of a core over 10 min. An idle server's footprint is 41.6 MB; its RSS creeping 55 → 100 MB is
  `__TEXT` pages faulting in (140 MB resident of 284 MB of code and shared libraries), not heap.
- **The sync outbox:**
  - **Resume:** 60 resumes from a cursor were all `resume: true`, welcomed in 1.6 ms (p99 3.1 ms), with no stamp lost
    or doubled.
  - **Bursts:** 40 simultaneous resumes were welcomed in 20 ms.
  - **Stalls:** a reader that stalls for 40 s costs ~4 MB of RSS and then gets its snapshot.
  - **Old cursors:** a cursor from seq 1 resumes with a 5.4 MB replay that disturbs nobody.
- **Writes:** every common write answers in ≤ 2.5 ms (fork 5.6 ms). `runtime.create` (36–49 ms) waits on the CLI spawn
  and the durable commit (`run.rs:612` `spawn_run`, the store coalescer's fsync), well inside the UI's optimistic
  answer.
- **Per-task reads on the 2 MB task:** `runtime.get` 0.5 ms, `runtime.turns` 0.4 ms, `runtime.activity` 15 ms (1.96 MB),
  `threads.health` 0.5 ms, `threads.notes` 0.8 ms, `runtime.search` 4.7 ms.
- **CLI basics:** `wisp --version` 6.7 ms; slips answer in ~12 ms.
- **The bench's `--locked`** build works (r1 #2).

## 5. Side notes (not counted)

- **The bench is sensitive to whatever else runs on a developer Mac:** append rate dropped 3.7× and cold start rose 1.6×
  with no change to the code that runs, and only the cold-start round with a 1.6× calibration was marked †. Only CI's
  Linux runner should be trusted for those numbers; locally, compare against an older binary on the same home (as
  `ab.py` does).
- **`vmmap` and `footprint` suspend the process they read.** Never sample memory with them during a latency run (my first
  30-minute p99 was 800 ms because of it).

## Count

**8 findings:**

| severity | findings |
|---|---|
| high | 1 (r1 #6, still open, now shown to break the patch budget for connected clients) |
| medium | 2, 3, 4 |
| low | 5, 6, 7, 8 |

Of round 1's 10 findings, 7 are fixed (1, 2, 3, 4, 5, 8, 9), #10 is half fixed (finding 5), #7 improved (finding 8)
and #6 is open (finding 1). All six server budgets
pass, and patch latency also holds over 30 minutes, until a burst of fresh clients connects.
