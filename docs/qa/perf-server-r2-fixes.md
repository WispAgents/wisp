# Server performance round 2: the fixes (2026-11-19)

Findings 1 to 8 of `docs/qa/perf-server-r2.md`, measured before and after with that report's own scripts and data
(`/tmp/perf2s`: `burst.py` for #1, its 5k `home-sweep` with the 2 MB task, and the `repo` / `repo2k` clones with 12 and
2,012 tracked tasks for #2 and #6).

**How it was measured.** Both binaries were built with the `budgets` profile (the report's). The *old* binary is the
report's own (`/tmp/wisp-target-perf2s/budgets/wisp`, HEAD `c0d655ad`). The *new* one is this worktree at `673fe4ac`
with these fixes, plus whatever other sessions had uncommitted in it at the time. Old and new ran one after the other,
each on a fresh copy of the same home, never on ports 4477, 4800 or 4791. This Mac was loaded by other sessions'
builds (load average 4 to 27 during the runs), so every number is a median of 7 (blame and the burst: alternating
call by call, or round by round), and the old binary was re-measured alongside each time. Use the old/new ratio, not
the absolute numbers. Scripts: `/tmp/r153-perf/{m.py, blame_srv.py}`, logs `m.log`, `m3.log`, `burst.log`.

## Numbers

| # | measure | r2 report | old today (rounds) | new today (rounds) | change |
|---:|---|---:|---:|---:|---|
| 1 | other client's patch p99 in the 6 s after 10 fresh clients (`burst.py`) | 279.3 ms | 273.3 · 194.7 ms | **105.3 · 104.3 ms** | back at the baseline (p99 104–120 ms before the burst) |
| 1 | 10 fresh clients: each has 4 MB by | 444 ms | 271 · 288 ms | 216 · 223 ms | |
| 1 | 10 fresh clients: server CPU | 1.03 CPU-s | 1.07 · 0.99 | 0.94 · 0.99 | the same work, off the actor |
| 2 | `wisp blame <folder>` (12 tasks · 2,012) | 2.7 s · 3.0 s | 2,299 · 2,316 ms | **246 · 349 ms** | 7–9× |
| 2 | `wisp blame <file>` (12 · 2,012) | 1.2 s · 1.4 s | 1,194 · 1,008 ms | **239 · 255 ms** | 4–5× |
| 2 | `wisp blame file:line` (12 · 2,012) | 107–184 · 282–389 ms | 204 · 387 ms | 181 · 242 ms | what's left is `git blame` itself |
| 3 | `machine.read`, back to back | 90–130 ms | 72 · 70 · 98 ms | **0.7 · 0.6 · 0.3 ms** | answered from the tick's sample |
| 3 | `machine.read`, 12 s apart (the page's ticks run) | — | 81 · 206 · 122 ms | 1.5 · 1.0 · 1.0 ms | the first read of an idle page still samples (69–272 ms) |
| 4 | `runtime.usage` | 38.7–42.0 ms | 35 · 60 · 76 ms | **6.4 · 2.3 · 3.0 ms** | 12–25× |
| 4 | `runtime.history` | 65.3–67.5 ms | 59 · 92 · 82 ms | **42 · 28 · 37 ms** | 1.4–3× |
| 4 | `runtime.agent.stats` | 50.6–51.5 ms | 147 · 257 · 243 ms | 188 · 136 · 182 ms | ≈1.3×; see below |
| 5 | `wisp ls` | 69.6 ms, 485 KB | 69 · 67 · 89 ms, 491 KB | **21 · 18 · 12 ms, 3.7 KB** | the newest 50 |
| 5 | `wisp ls --json` | 78.0 ms, 2.76 MB | 82 · 75 · 98 ms | **20 · 18 · 13 ms, 31 KB** | |
| 6 | `wisp tasks --touching` (2,012 tasks) | 182.8 ms | 614 · 169 · 180 ms | **48 · 46 · 45 ms** | 4× |
| 6 | `wisp tasks --tracked` (2,012) | 230.1 ms | 409 · 168 · 179 ms | **95 · 95 · 86 ms** | 2× |
| 6 | `wisp log <path>` (2,012) | 177.4 ms | 424 · 182 · 182 ms | **51 · 52 · 41 ms** | 4× |
| 6 | the same three with 12 tasks | 22–30 ms | 24–38 ms | 11–26 ms | no `git rev-parse` spawn |
| 7 | `worktrees.view` | 21.9–22.3 ms | 19 · 25 · 24 ms | 20 · 15 · 23 ms | no clear change: see below |
| 8 | `share.preview` (2 MB task, full) | 63.7–81.7 ms | 86 · 210 · 100 ms | 93 · 53 · 72 ms | ≈1.4×, and off the request thread |
| 8 | `share.bundle` (2 MB task, full) | 121.5–128.0 ms | 138 · 317 · 206 ms | 143 · 94 · 136 ms | ≈1.5×, and off the request thread |

Every blame answer (`--json`, 8 targets in both clones) is byte for byte the same from the old and new binaries.

## Server budgets (`spikes/server-budgets`, new binary)

All six pass (exit 0), on this loaded Mac:

| budget | r2 report | new | limit |
|---|---:|---:|---:|
| cold start → first response | 66.6 ms | 76.4 ms | 150 ms |
| idle RSS, settled after the list | 103.2 MB | 96.3 MB | 120 MB |
| task list, first page | 1.18 ms | 1.99 ms | 5 ms |
| patch p99, 10 running, 4 clients | 104.9 ms | 104.6 ms | 150 ms |
| durable append | 285,003/s | 815,897/s | 20,000/s |
| write amplification | 40.2× | 41.7× | 44× (ratchet) |

Every page is at one seq and no task is in two pages (the bench fails otherwise). Every task (11 pages): 122.8 ms
median, which the bench doesn't gate (r2: 46.3 ms; each later page now waits for the one before to be handed over, and
the median's rounds were 62.5 · 249.3 · 271.8 ms on this machine). The full output is in `/tmp/r153-perf/budgets.log`.

## What changed, per finding

1. **A fresh client's list no longer holds a reader or the actor** (ADR-0106, changes ADR-0081 §1). `list`'s pages are
   each their own short read on the blocking pool (`crates/wisp-api/src/v2_pages.rs` `short_reads`), and each reads on
   from the last row with a keyset over the query's own order (`after`), so no row is ever skipped. Every page goes at
   the first read's seq, and later changes come as patches after it. A full page's rows are encoded where they're read
   (`SnapshotAnswer::rows_json`), and the sync machine only copies them into the frame (`wisp_sync::snapshot_frame`).
   Tests: `a_write_between_two_pages_is_in_a_later_page_or_the_patches_after_the_snapshots_seq`,
   `a_snapshot_page_encoded_by_the_host_is_the_frame_its_rows_make`.
2. **Blame reads every commit's message from one `git log --no-walk --stdin`.** On the server this is the new
   `BlameSource::messages` (additive on `history.blame@1`, with a default that keeps other sources working), and the
   task files are parsed on the blocking pool, on every core. In the CLI's local path (`wisp mcp`'s `wisp_blame`,
   `repo_cli.rs`), the commit → task index is built once instead of per commit. Tests: `repo_cli_tests.rs`
   (`blame_names_each_commits_task_from_its_trailer_or_the_file_that_lists_it`),
   `one_logs_records_are_each_commits_message`, plus the existing history blame tests on real git.
3. **`machine.read` answers from the last sample when it's under a tick old** (the ticks run while the page is read),
   with the guard as it is now. On macOS `ps` runs without `-E` (578 KB instead of 1.2 MB), and the environment comes
   from one `ps -E -p` for only the pids not seen before, as Linux reads `environ` once. Test: `the_env_ps_names_each_pids_tag`.
4. **`runtime.usage` and `wisp_stats` take each task's folder and agent from the list in memory**, and `runtime.history`
   its costs. A whole task is parsed straight from the store's text into a `Session` (`rows::sessions_of`), without the
   `Value` tree or the list row. `readopt` reads only the tasks whose body names the session (`Repo::sessions_naming`).
   Tests: `a_task_read_from_text_is_the_task_session_of_reads`, `a_task_adopted_from_a_session_is_found_by_it`.
5. **`wisp ls` shows the newest 50.** `--limit n` shows that many, `--all` everything (archived too), and a note says
   how to see more. Test: `ls_shows_the_newest_fifty_unless_told`.
6. **`tasks --touching`, `log <path>` and `ls --repo --path` parse only the task files that name the path**, every file
   is parsed on every core, and the repo root is found by walking up to `.git` (Unix) instead of spawning git.
7. **`runtime.slots` takes `having`** (additive). Worktrees asks only for the tasks whose body names a worktree slot or a
   compare `group`, and SQLite reads no other body. Test: `slots_having_reads_only_the_tasks_that_name_a_word`.
8. **The scrub skips a string in one pass when no secret pattern matches it** (a `RegexSet`; when one does, the
   patterns run one by one exactly as before). A string is copied only when something in it is taken out, and the
   scrub and the signing run under `spawn_blocking`. The existing scrub tests, including the reference comparison,
   pass unchanged.

## Not done, and why

- **#4 `wisp_stats` (`runtime.agent.stats`)**: what's left (≈140–180 ms here) is reading the event rows of every turn in
  the range (`tool_calls`), not the task decode this finding named. It needs its own index or a summary per turn.
- **#7 `worktrees.view`**: no measurable change. Of its time, `runtime.list` with the .env mask over every row is the
  larger share in this home, and the slots read was already a borrowed parse. The next step is the audit's other
  idea: a plugin caller's list without the text fields, so no mask runs.
- **#8**: the preview isn't reused for the bundle that follows it. A cached preview would need a key that covers the
  caller, the task's revision and every other task's id, and a stale scrub would leak a reference.
- **#1**: the audit's other ideas (only the first page until the client scrolls, one encoding shared by every client
  at the same head) aren't needed to meet the budget. They are listed in ADR-0106.
- **#3**: libproc instead of `ps`, and a cold read of an idle page still runs `ps` once.
