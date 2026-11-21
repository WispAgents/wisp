# ADR-0085: The write-amplification gate streams a real agent's steps, and the change feed writes less per commit

- Status: accepted; its open question answered by the lead (owner's delegate, 2026-11-16): option 1, ratchet at 48×,
  then option 2 next
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *Make patch latency and write amplification pass*, §7 *Write amplification 30x vs 15x*.
  Follows ADR-0082 (the gate) and ADR-0083 (one shared 100 ms window). Amends ADR-0082's stream (decision 2) and
  `crates/wisp-store/src/schema.rs` migration 1's AUTOINCREMENT (kept by its guarantee, not by the keyword).

## Context

- ADR-0083 left write amplification at 30.5–31.8× (budget 15, ADR-0011) on ADR-0082's stream: 10 tasks, each writing
  20 lines a second of 400 B text for 6 s. The cost is pages per commit, so the number depends on the steps the fake
  streams; nobody had measured what real turns keep.
- **What real turns keep** (`spikes/server-budgets`' `steps` bin: Claude Code's own session logs through wisp's Claude
  parser and the runtime's `activity::apply`; 1 647 logs of the tasks wisp ran on this repo, 3 872 turns, 275 374
  steps; README *The stream's shape*): a kept step is 689 B on average, p50 290, p90 1 941, p99 5 403 (tool calls 35 %,
  results 34 %, thinking 21 % (mostly redacted, 19 B), text 9 %). A turn writes 0.2 steps a second at its median, 0.8 at
  its p99, 2.4 at most; its busiest 10 s are 0.8 (p50), 1.8 (p99), 3.3 (max) a second; 1 607 ms between steps at the
  median. A running task has a step in 1 % of its 100 ms windows. ADR-0082's stream was 25× the p99 turn's rate, one
  kind of step, and so put ~5 events in each commit where a real one puts 1–2. The repo's parser fixtures are
  hand-written and tiny, and the dev homes held 56 events: neither is a workload.
- At real sizes and rates the fixed cost of a commit is all there is: each of the 10 tasks' steps commits nearly
  alone, so every commit pays a leaf in each structure keyed by task, the change feed's tail, `sqlite_sequence`
  (AUTOINCREMENT's high-water mark, rewritten by every insert) and page 1.

## Decision

1. **The gate streams a real agent's steps** (`FAKE_STREAM_MIX=agent` in wisp-testkit's fake claude): a 20-step
   cycle whose sizes are the 20 ventiles' midpoints of the 275 374 steps (19 … 2 925 B, mean 630; measured 640 with
   the stamps), in their kinds' shares (4 thinking, 7 Bash calls, 7 results each after its call, 2 texts), each line
   padded so the step wisp keeps is that size. 2 steps a second per task (above the busiest 10 s of 99 % of real turns;
   G3's harness rate) for 10 s; 10 running, 4 clients as before. Every step is stamped, so patch latency samples tool
   calls and results too (200 stamps, 800 samples). `--stream=<per_s>:<ms>:<agent|text>` reruns another shape by hand
   (`20:6000:text` is ADR-0082's); the gate is the default. The evidence is reproducible with the `steps` bin.
2. **The change feed has no AUTOINCREMENT** (kernel migration 4, a table rebuild). Its guarantee stays: a seq is never
   handed out twice in an epoch. Retention never deletes the newest change row (`retention::prune`: `upto ≤ head − 1`),
   so `max(seq)`, SQLite's base for the next rowid, never goes down; `head_seq` is `max(seq)` (the b-tree's last cell).
   A database whose high-water mark is past its rows (retention had emptied the feed under AUTOINCREMENT) loses its
   epoch in the same migration, so every cursor from before takes a snapshot.
3. **An append's change row stores its rows' keys, not the rows** (`[{"id": 812}, …]`): the rows are in their table
   already. Listeners (the live path) still get the whole rows; `changed_since` only ever read the ids; `changes_since`
   reads an append's rows back from their table (a row deleted since is left out). Rows stored whole before stay as
   they are and read back the same way.
4. **Not done, measured**: dropping `runtime_event_turn` (nothing else serves a turn's reads: `repo::steps`, the task
   window, the tool stats) saves 4 % (44.7–45.7 → 43.4–43.7×) and would need a turn's first seq stored and three reads
   rewritten. A 1 s window (results waiting for it too) gives 31.3× and a 1 s p99.

## Places it reaches (ADR-0040)

PR CI's *server budgets* step and anyone's terminal (`steps` too). The store's on-disk shape (kernel v4) for every
user; no wire, MCP or UI change: what clients get live and on resume is unchanged.

## Edge cases and their tests

- The fake's mix: the cycle's kinds in order, every line stamped in order, a result naming the call before it, the
  largest step padded to its size, the turn still ending — `crates/wisp-testkit/tests/testkit.rs`
  `the_fake_claude_streams_a_real_agents_steps_in_the_mix`.
- A shape typed by hand: parsed, or refused (exit 2) — `--stream` (`Shape::parse`), the default is `STREAM`.
- Real logs: an ISO stamp becomes unix ms, a broken one is skipped — `steps` bin
  `an_iso_stamp_is_unix_ms_and_garbage_is_none`; a sidechain line, a non-JSON line, a line before any prompt are skipped.
- Everything ages out, round after round: the newest row stays and the next seq is new each time (fails on the
  feed without the `head − 1` cap: the table empties and seq 1 comes back) — `crates/wisp-store/tests/mutation.rs`
  `retention_never_takes_the_newest_change_so_no_seq_comes_back`; the row cap and 7-day tests now keep that newest row
  (`the_row_cap_wins_when_it_keeps_fewer_and_seqs_are_never_reused`: head 10, oldest 10, next 11).
- A v3 feed upgrades with its rows, seqs and epoch; the next seq follows its newest; no AUTOINCREMENT is left —
  `crates/wisp-store/tests/migrate.rs` `kernel_v4_drops_the_feeds_autoincrement_keeping_its_rows_seqs_and_epoch`.
- A v3 feed retention had emptied (high-water 12, no rows) gets a fresh epoch —
  `kernel_v4_gives_a_feed_retention_emptied_a_fresh_epoch`.
- An append stores the keys, listeners hear the rows, the feed reads them back, a row deleted since is left out —
  `crates/wisp-store/tests/property.rs` `an_appends_change_row_stores_the_keys_and_reads_back_the_rows`; the two
  properties (replay = tables; heard = read back, but for rows deleted since) over random runs.

## Consequences

Measured (2026-11-16, M-series Mac, `budgets` profile, `spikes/server-budgets`; frames from `--keep`'s `stream.wal`):

| | realistic stream (gate) | ADR-0082's stream (`20:6000:text`) |
|---|---:|---:|
| before (ADR-0083's code) | 55.2× (1 615 frames, 216 commits) | 31.5× |
| + no AUTOINCREMENT | 48.8–51.4× | 29.9× |
| + an append stores keys (this ADR) | **44.7–45.7×** (1 363 frames; db growth 0.71 → 0.49 MB) | **26.2×** |
| (not done) + no `runtime_event_turn` | 43.4–43.7× | |
| (not done) 1 s window, results wait too | 31.3×, p99 1 004 ms | 12.0× |

Patch p99 stays 103–105 ms (800 of 800), the other budgets unchanged.

- Kernel v4 copies the whole change feed once, at the first start after the upgrade (≤ 1M rows by retention, after
  the upgrade's `VACUUM INTO` snapshot): a slower first start for a large home, never again. Retention may keep one
  row past 7 days (the newest) while nothing is written. `changes_since` reads an append's rows as they are now, a
  later patch already in them: its only callers are tests; a future one that replays the feed must read appends as
  current rows.

- **The realistic number is ~3× the budget, and no trim closes it.** Of the 1 363 frames, 543 (2.2 MB, 17× the
  payload on their own) are commits that write no step: per-turn work of other plugins (57 commits of the event log's
  cursors alone, the events view, router, budget, search index). The 94 activity commits write 8.7 pages each for 2.4
  steps (~1.5 KB): `change_topic` 1.8, `runtime_event` 1.6, `change` 1.35, `runtime_event_seq` 1.1,
  `runtime_event_turn` 1.1, page 1 0.5. A step that commits alone costs at least its table's leaf and the feed's
  (8 KB for ~700 B: 12× before any index), and real steps come 1.6 s apart, so they do commit alone.
- The 15× came from G1's one session streaming 2 MB in 5 000 events (13×), where hundreds of steps share a commit;
  the runtime's bench of that load still passes (9.3×). It doesn't transfer to 10 tasks at real rates. In absolute
  terms the realistic stream writes 5.8 MB in 10 s for 10 tasks at 2 steps a second (~25 KB a kept step); a heavy
  user at real rates (0.3 a second) writes ~2.5 GB in an 8-hour day of 10 tasks.

## Answered: the budget (the lead, 2026-11-16)

**Option 1, then option 2.** `writeAmplification` is 48× on the realistic stream from now (the gate goes green and
catches any structure every step touches); the next round does option 2 (fewer per-turn commits, ~17× of the 45×)
and lowers the number to what it measures, in its own ADR. 15× stays the bulk-append budget (the runtime bench, 9.3×).

### The question as it was put

The budget (ADR-0011: only an ADR changes it; `tools/budgets.ts` is untouched here). Until it's answered the gate
keeps 15× and stays red. The options, with what each costs:

1. **(recommended) Ratchet at the realistic load**: `writeAmplification: 48` on this stream (measured 44.7–45.7, run
   noise ±1.5), lowered by each trim that lands; the 15× stays the budget of a bulk append (the runtime bench's one
   2 MB session, 9.3×). Cheapest; it still catches a structure every step touches (+1 page a commit ≈ +3×).
2. **Fewer per-turn commits first** (the event log's cursors riding the next write, budget/router/search writes
   batched per turn): the 543 frames are the biggest lever left (~17×); their owners' ADRs (ADR-0044's delivery) change.
3. **Live apart from durable**: patches go out from the buffer and the store commits once a second; 31× measured, a
   crash loses up to 1 s of steps clients saw, ADR-0074/0075 change. Alone it doesn't reach 15× either.
