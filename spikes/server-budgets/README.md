# server budgets (ADR-0011, ADR-0080)

```sh
cargo build --profile budgets -p wisp -p wisp-testkit
cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml   # [--wisp=<bin>] [--fake=<bin>] [--summary=<file>] [--keep]
```

Seeds a home with 5 000 tasks through wisp.runtime's writer (`schema::install`, `repo::save_session`), starts the real
`wisp serve` on it (fake harness CLIs, a cleared environment, no network) and gates it on `tools/budgets.ts`
`SERVER_BUDGETS`; exits 1 on a miss, 2 when it can't run, 3 when a timed budget was over while most of its rounds ran
on a slowed runner (*too noisy to judge*, below):

- cold start: process start → the first `200` from `GET /api/status`, each round the median of 5 restarts (on a copy of
  the migrated home), gated on the median of 5 rounds ≤ 150 ms
- idle RSS: the server's resident set once settled (2 s within 1 MB), after a client read the list ≤ 120 MB
- the task list: `GET /api/events?fresh=list` (the UI's first read) → the last byte of the `list` snapshot's first page
  (what the first screen draws, ADR-0081), each round the median of 12 reads (after 2 warm-ups) by a fresh client of the
  idle server, gated on the median of 5 rounds ≤ 5 ms; the time to its last page (every task), the pages and the bytes
  are reported. A task in two pages, or pages at two seqs, fails the run.

**Rounds and the runner's calibration (ADR-0089).** Shared CI runners have slow spells (a cold start of 424 ms among
85–115, 30 list reads with a p90 of 67 ms, one stalled commit window deciding a p99): a timed budget gated on one
measure failed on them. So cold start and the list take turns, 5 rounds of each spread over the job, the stream runs 3
rounds (fresh tasks, fresh clients), and each budget is the median of its rounds. Before and after each round the bench
times a fixed workload in its own process (200 loopback TCP round trips to an echo thread, then 4 xorshift passes over
64 KB, each written and read back; median of 5): nothing of wisp's is in it, so it tracks the runner only (on Linux
each round also reports the hypervisor's steal, `/proc/stat`, not judged). A round whose bracket ran over 1.5× the job's fastest
calibration is marked † and left out of its median while the clean rounds are most; over budget with most rounds
marked is *too noisy to judge*: exit 3, an `::error` that says so, never a pass. Noise only ever slows a measure, so a
pass on a noisy runner is a pass. The table shows every round's value and the calibration's range.

It also prints where the list's time goes (the first page's query, every task in one query, and the change-feed replay a
first look made before ADR-0081), measured in process. And where the idle RSS is (ADR-0087, reported, not gated): on
Linux `/proc/<pid>/smaps` sorted into the binary's pages, other files, `[heap]`, glibc's other arenas, thread stacks
and the rest of the anonymous memory, plus the thread count (macOS: the thread count), before the list and once
settled; then the same home on glibc's own malloc settings (`MALLOC_ARENA_MAX=<8 × cores>`, which turns
`wisp_proc::tune_heap` off), before and after the same list reads, so one run shows what ADR-0087's settings save. PR CI runs it on Linux (`.github/workflows/suite.yml`, *server budgets*) and puts the table in the job
summary. `--keep` keeps the bench's folder (the home and each server's log).

On the same server, once those are read (ADR-0082):

- patch latency: 10 tasks on the fake claude (wisp-testkit's fake agent, `FAKE_STREAM` + `FAKE_STREAM_MIX=agent`),
  gated until 4 clients (`fresh=list` + all 10 `session:<id>`) have their first looks, then a real agent's steps (see
  *The stream's shape*), 2 a second each for 10 s, every one stamped; each client's first sight of each stamp is a
  sample; 3 rounds, each 10 new tasks and 4 new clients (800 samples a round, 2 400 in all): the median of the rounds'
  p99 ≤ 150 ms, and a stamp a client never got, in any round, is a miss
- write amplification: in the last stream round, the WAL pinned by a read while they stream, then the database's
  growth once the server stopped, over the JSON of the steps the 10 turns kept (the runtime bench's payload) ≤ the
  ratchet (`SERVER_BUDGETS.writeAmplification`)
- append rate: `spikes/events`' `append_rate` (its library), 100 000 events 1 000 a transaction ≥ 20 000/s

Then where the stream's WAL went (ADR-0088, reported, not gated; `src/wal.rs`): every frame's page named by the b-tree
it belongs to (SQLite's `dbstat` on the database once the server stopped), every commit by the tables it touched (but
`change`, `change_topic` and page 1, which every commit touches), as commits and frames per kind of commit, then frames
per table. A writer that commits alone per turn or per step shows as its own line. `--keep` also keeps `stream.wal`,
the frames the stream wrote.
`--stream=<lines a second>:<ms>:<agent|text>` runs another shape by hand (`20:6000:text` is ADR-0082's first stream:
400 B of text a line); the gate is the default.

## The stream's shape (ADR-0085)

What a real agent's turn keeps, once wisp has parsed it, measured by this spike's second bin:

```sh
cargo run --release --manifest-path spikes/server-budgets/Cargo.toml --bin steps -- ~/.claude/projects/<dir>…
```

It reads Claude Code's own session logs (one line per content block, as `stream-json` prints them, each with its
timestamp), feeds each main-thread line to wisp's Claude parser (`wisp-claude-core`) and each event to the runtime's
`activity::apply` (`drive.rs`'s path), and sizes the JSON of each step a turn keeps (the gate's payload). It prints
sizes and counts, never text. On 2026-11-16, over the 1 647 logs of the tasks wisp ran on this repo
(the `~/.claude/projects/` folders of its worktrees, 3 872 turns, 275 374 steps):

| bytes a kept step | n | mean | p50 | p90 | p99 |
|---|---:|---:|---:|---:|---:|
| every step | 275 374 | 689 | 290 | 1 941 | 5 403 |
| tool call | 96 151 (35 %) | 820 | 455 | 1 628 | 7 202 |
| tool result (clipped at 2 000 chars) | 94 394 (34 %) | 962 | 740 | 1 993 | 2 099 |
| thinking (mostly redacted: 19 B) | 59 146 (21 %) | 33 | 19 | 19 | 280 |
| text | 23 954 (9 %) | 738 | 148 | 2 433 | 9 304 |

| rate | p50 | p90 | p99 | max |
|---|---:|---:|---:|---:|
| steps a second over a turn (prompt → last step) | 0.2 | 0.5 | 0.8 | 2.4 |
| steps a second in a turn's busiest 10 s | 0.8 | 1.3 | 1.8 | 3.3 |
| ms between steps | 1 607 | 9 827 | 124 405 | |
| kept bytes a second over a turn | 152 | 260 | 461 | 1 152 |

A running task hands a 100 ms commit window (ADR-0083) a step in 1 % of its windows, 1.1 steps when it does (p99 2).
The repo's parser fixtures (`plugins/*/core/tests/fixtures`) are hand-written and tiny (a handful of lines a run, no
timestamps), and the dev homes (`~/.wisp-dev*`) held 56 events: neither is a workload. Codex's logs are another format
(its `exec --json` stream is what wisp parses), so the evidence is Claude Code's.

So the fake streams (`FAKE_STREAM_MIX=agent`, `MIX` in `crates/wisp-testkit/fixtures/fake_agent.rs`) a 20-step cycle:
the 20 ventiles' midpoints of every step's size (19 19 19 19 86 114 146 180 222 264 327 413 487 563 681 922 1 350 1 863
1 988 2 925 B, mean 630), the kinds in their share (4 thinking, 7 Bash calls, 7 results each after its call, 2 texts),
each line padded so the step wisp keeps is that size (a stamp makes the smallest 62–108 B: measured mean 640 B). At 2
steps a second per task, above the busiest 10 s of 99 % of real turns and G3's harness rate, for 10 s: 200 stamps,
800 samples. The 400 B text lines at 20 a second (ADR-0082) were 25× the p99 turn's rate and one kind of step.
