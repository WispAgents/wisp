# ADR-0088: Other plugins' per-turn writes take fewer commits, and the write-amplification ratchet goes to 44×

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *Make patch latency and write amplification pass*, §7 *Write amplification 46-47x vs
  the 48x ratchet*; ADR-0085's option 2, chosen by the lead (owner's delegate). Supersedes ADR-0085's number (48×).
  Amends ADR-0044 (*Durable subscriptions*: when a cursor that rode no write is saved) and the search index's
  write path (`crates/wisp-store/src/search.rs`, ADR-0004).

## Context

ADR-0085 left the realistic stream (10 tasks, a real agent's steps at 2 a second for 10 s, 4 clients) at 44.7–45.7×
against a 48× ratchet: run noise (±1.5) nearly ate the margin. 543 of its 1 363 WAL frames were commits that wrote no
step. Nobody knew whose: the spike only counted frames.

**Who writes what** (`spikes/server-budgets`' new per-writer breakdown, below; this round's baseline run, 45.7×, 204
commits, 1 366 frames): every frame's page named by the b-tree it belongs to (SQLite's `dbstat` on the database once
the server stopped), every commit by the tables it touched (but the change feed's and page 1, which all touch):

| commits | frames | writer | what |
|---:|---:|---|---|
| 86 | 629 | `runtime_event` | the activity window (ADR-0083): the steps, the gate's payload |
| 10 | 314 | `runtime_session`+`runtime_turn`+`event`+FTS | each turn's end; **59 of them the session title's FTS rows, rewritten though the title never changed** |
| 29 | 191 | `events_view`+`event_cursor` | wisp.events' feed: one write per durable event (3 per turn end, logged in one commit) |
| 44 | 44 | `event_cursor` alone | 7 durable subscriptions × each burst: a caught-up cursor saved in its own commit, even right after its handler's write had saved it |
| 10 | 64 | `router_route`+`event_cursor` | wisp.router learning from the turn's end |
| 20 | 82 | `budget_meta`, `budget_acc` | wisp.budget at a turn's end: the spend, then the learned run costs, two writes |
| 5 | 42 | `runtime_turn` (+`runtime_event`/`budget_acc`) | the turn's ext and last steps |

The stream's 30 durable events are 3 a turn end (`task.finished`, `turn.ended`, `turn.finished`), heard by 7 durable
subscriptions (automations' engine, crew's two, events' feed and sinks, router, telemetry). Nothing wrote per step
outside the window: every one of these is per turn. They weigh more here than in real use (a 20-step turn; real ones
average 71 steps), but each is a real commit a real turn pays.

## Decision

1. **Search text unchanged, index untouched.** `search::reindex` gets the row before the write; an upsert or a patch
   that leaves every indexed column as it was (by the text the index holds) touches no FTS page. A new row, a delete,
   or a changed column reindexes as before.
2. **Idle cursors are saved together** (ADR-0044 amended). A durable worker that catches up no longer writes its
   cursor in its own commit: it hands it to the kernel's one `Saver`, which writes every waiting cursor in one
   transaction (`EventLog::set_cursors`, additive with a default that loops `set_cursor`; the store's is one write)
   `SAVE_WITHIN_MS` (1 s) after the first one waiting. Each worker keeps its latest delivered cursor there as it goes
   (no write), so switching a plugin off saves what its subscriptions delivered before `set_enabled` returns (wisp's
   stop switches every plugin off before it closes the store), one save at a time (a flush that finds nothing waits
   for one in flight). A worker started again resumes from a cursor still waiting there, not only the stored one. A
   save the store refuses keeps its cursors and tries again a second later. A handler whose write carried its cursor (ADR-0044's
   *effectively once*) isn't saved a second time: `deliver` now says whether the cursor reached the log. A new
   subscription's first cursor is still saved once (where a restart resumes it). Every 256 deliveries still saves.
3. **Batch subscriptions** (`Setup::on_durable_batch`, additive): the next event and those already in after it
   (matching, one after another, up to 64) come in one call; the handler writes once, its cursor riding that write at
   the last of them. A batch that fails without having written is delivered again one by one (3 tries each, parked
   alone, as before). Gaps come alone. A commit's durable events now enter the ring together and raise its head once
   (`Ring::insert`), so a worker never wakes between them. wisp.events' feed is the first: a turn end's 3 events are
   one `events_view` write.
4. **wisp.budget's turn end is one write**: the run costs Ask near the limit learned (`budget_meta` `runs`) ride the
   spend's save (`runs_dirty`), learned before it; a failed save keeps them for the next.
5. **The ratchet is 44×** (the lead raised the coder's 42 to 44: CI's Linux runners read ~1.5× above the Mac, and a gate
   that fails on noise is a flaky test) (`tools/budgets.ts` `writeAmplification`, the spike's `WRITE_AMPLIFICATION`, the constants
   test agrees): the six runs below read 35.9–39.4× (mean 37.8); 42 is the worst + 2.6, the mean + 4.2 (ADR-0085 kept
   +2.3 over its worst). 15× stays the bulk-append budget (the runtime bench).
6. **The spike reports where the WAL went** (`spikes/server-budgets/src/wal.rs`, reported, not gated): commits and
   frames by writer, then frames by table, after the patch latency line.

### What a crash replays now (ADR-0044's promise)

Unchanged for a handler that writes: its effect and its cursor commit together, so a crash before replays it and
after doesn't. For a handler that writes nothing for an event (telemetry's `turn_ended` always; sinks with none configured; any
other handler on an event it has nothing to do for), a crash (kill -9, power
loss) within 1 s of catching up replays those events at the next start, where before it replayed only the one in
flight. ADR-0044 already asks handlers to be idempotent (the envelope carries `seq` and `key`); the one visible
effect is telemetry: a turn's end may be counted twice after such a crash. A clean stop and a plugin switched off and on replay nothing:
the save lands before `set_enabled(false)` returns. Under an injected clock (`FakeClock` in tests) the 1 s save waits
for the clock, as every kernel timer does; a switch-off still saves at once.

## Places it reaches (ADR-0040)

PR CI's *server budgets* step and anyone's terminal (the breakdown prints there too). No wire, MCP, CLI or UI change:
`events:*` topics get the same rows, the search finds the same rows, cursors mean the same seqs. A plugin's durable
handler may now opt into batches (`on_durable_batch`), the same API for built-ins and users' plugins.

## Edge cases and their tests

- An upsert or patch of a column the index doesn't hold leaves the index's pages as they were; a title change and a
  delete still move it — `crates/wisp-store/tests/search.rs`
  `a_write_that_leaves_the_indexed_text_as_it_was_leaves_the_index_untouched` (fails without decision 1).
- Two subscriptions that write nothing, two events: both cursors in one `set_cursors`, none one at a time, at 2; a
  new start replays nothing — `crates/wisp-store/tests/durable_events.rs`
  `idle_cursors_of_every_subscription_are_saved_together_within_a_second`.
- Switched off: both cursors are in the store when `set_enabled(false)` returns —
  `a_subscription_switched_off_saves_its_cursor_at_once` (it caught a flush returning while another's write was in
  flight: hence one save at a time).
- Switched off and straight back on, then one more event: each subscription hears only the new one —
  `switched_off_and_straight_back_on_replays_nothing`.
- A commit's 3 events reach a batch subscription in one call and one write, the next alone; the cursor rode each write
  and isn't written again — `a_batch_subscription_gets_a_commits_events_in_one_call_its_cursor_riding_its_one_write`.
- A batch that fails goes again one by one, each written, the cursor at the last —
  `a_batch_that_fails_is_delivered_again_one_by_one`.
- ADR-0044's own (restart replays from the cursor without loss or a second effect, a crash between a handler's write
  and its cursor can't happen, lag, gap, park and retry, switched off and on) pass unchanged: the same file, 17 tests
  (8 runs in a row green).
- A turn's end saves the spend and the learned run in one commit, and the run is kept —
  `plugins/budget/host/tests/plan_free.rs` `a_turns_end_saves_its_spend_and_the_learned_run_in_one_commit`.
- The breakdown: a WAL's frames split into its commits, a torn tail left out; a commit named by its tables but the
  feed's — `spikes/server-budgets/src/wal.rs` tests.
- The feed's topic end to end (`events:*` resumes from a client's cursor, lifecycle events read from outside, sinks
  retried) — `crates/wisp/tests/serve_events.rs`, 5 tests, unchanged.

## Consequences

Measured (2026-11-16, M-series Mac, `budgets` profile, `spikes/server-budgets`):

| | write amp. | commits | frames | patch p99 |
|---|---:|---:|---:|---:|
| before (ADR-0085's code, this round's baseline) | 45.7× | 204 | 1 366 | 103.8 ms |
| decisions 1–4, head raised per event | 39.1× | 135 | 1 149 | 103.4 ms |
| decisions 1–4, a commit's events raise the head once (4 runs) | 35.9 / 37.3 / 38.1 / 37.7× | 117–128 | 1 045–1 117 | 103.5–104.1 ms |
| the final build: + a stop saves what it delivered, before `set_enabled` returns (2 runs) | **39.4 / 38.5×** | 131–134 | 1 130–1 158 | 103.4–104.3 ms |

The last two runs differ from the four before in the activity window only (90–94 commits, 653–696 frames, against
80–87 and 604–633: how the 100 ms windows fall), not in the writers this ADR touches; it's the run noise the ratchet's
margin is for.

Each cut, by its writer's lines in the same breakdown (per-writer frames are exact; whole-run numbers carry the ±1.5
noise, so the cuts weren't run one at a time):

| writer | before | after |
|---|---|---|
| a turn end's session FTS (decision 1) | 59 frames in 10 commits | 0 |
| lone `event_cursor` commits (decision 2) | 44 commits, 44 frames | 0 in the window (≤ 1 a second, one frame, all subscriptions) |
| `events_view` (decision 3) | 29 commits, 191 frames | 4–10 commits, 40–108 frames (some ride the turn-end commit) |
| wisp.budget (decision 4) | 20 commits, 82 frames | 10 commits, 50–65 frames |

The other budgets are unchanged (cold start ~48 ms, idle RSS ~104 MB, the list ~1.3 ms, append ~1M/s).

- **What's left** (all six runs): the activity window 604–696 frames (~57 %); each turn's end 250–272 frames in 10
  commits, most of it the turn row and its change row carrying the answer (the turn row keeps it twice: `answer` and
  `body.text`, ~2.9 KB each here) and its FTS; router 50–58; budget 50–65; `events_view` 40–108. Next levers, not done:
  the turn row keeping its answer once (a row-shape change clients read), and the store's writer group-committing
  writes queued at the same moment (the router's, budget's and feed's, all woken by one turn end, each its own commit
  today; ADR-0074/0075's staging would hold per write inside one transaction).
- Linux (CI) hasn't measured this build (every number here is the Mac's). If CI reads over 42×, the next round reads
  its breakdown (printed in the job's log) before changing the number.
- Left, small: a non-writing handler that returns in the instant its plugin is switched off may be noted after
  `set_enabled`'s save; its worker's own save then races the store's close, and that one event may replay at the next
  start. A save the store keeps refusing is retried every second for as long as the kernel lives (a warning each).
- `on_durable_batch`'s handler gets a `Vec`; its call's `cause` is the batch's last seq (events it emits record that);
  the every-256 save counts a batch's events, not its calls.
