# ADR-0081: The task list's first look is read from its table in pages, the first screen first

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *Make the server budgets pass* (the list and RSS rows of ADR-0080 §Consequences).
  Follows ADR-0005 (topics, cursors, snapshot pages), ADR-0011 (list query ≤ 5 ms on 5k sessions), ADR-0050 (egress
  views), ADR-0080 (the gate).

## Context

- `spikes/server-budgets` (ADR-0080) measured the UI's first read, `GET /api/events?fresh=list`, on 5 000 tasks:
  **76.6 ms** median to the snapshot's last byte (budget 5), and idle RSS **146 MB** after list reads (95 before; budget
  120). The cold start already passed (64 ms).
- `list` had no full-read source, so a first look replayed the change feed (`changed_since("list", 0)`, ~54 ms) where
  one query of the session table takes ~4.5 ms. Worse, once retention pruned the feed the topic read **empty**: a new
  client of an old home saw no tasks.
- Giving `list` a plain source (one query) brought it to **48.7 ms**. Timed inside the server (2026-11-16, Mac,
  `budgets` profile), the rest of a 4.5 MB snapshot is JSON: the query 7–8 ms, parsing its rows into values ~17 ms, the
  egress views (ADR-0050, .env masking) ~8.5 ms, the snapshot message and its frame ~13 ms. No full snapshot of 5 000
  tasks fits 5 ms, and building one keeps ~50 MB of short-lived allocations resident.
- The protocol already has snapshot pages (`Snapshot.more`, ADR-0005), joined by both clients (packages/sync,
  wisp-sync's client); nothing sent them. The UI paints from its disk cache and resumes from its cursor
  (`first-paint.ts`), so `fresh=list` is a first load, a new epoch or a cursor retention dropped: the times the list is
  blank until the snapshot lands.

## Decision

1. **A paged source** (`wisp_api::v2_store::PagedSource`, `StoreSnapshots::with_pages`, additive): a topic's full
   snapshot is one kind's rows from its table, read by `queries` in order, each paged with `limit`/`offset`, in **one
   read transaction**, so every page is at one seq and no row is in two pages or in none. A resume whose cursor the
   feed still holds reads the change feed as before (one delta, never paged). Other sources' kinds on the same topic go
   in the first page.
2. **`list` is paged** (`crates/wisp/src/serve.rs` `list_pages`): wisp.runtime's `session` rows, the **100** latest
   unarchived first (the `(archived, updated_at)` index: ~0.1 ms; ties broken by id, so every page cuts one order),
   then the rest **500** at a time, the archived last.
   100 covers the first screen of every layout (a 1440 px window draws ~40 rows).
3. **Pages go as they are read** (`Snapshots::snapshot_pages`, a provided method: one page by default, additive): the
   store's reader hands each page over while it reads the next, each leaves through the egress views, and the actor
   (`v2_actor.rs`) gives the machine one `SnapshotAnswer` per page. The sans-IO server (`wisp-sync`) flushes a page at
   once instead of at the 50 ms flush rate, and **never sheds a topic with snapshot pages queued**: past the high-water
   mark they wait until the socket drains (a resync would only ask for the same pages again, and again on a link slower
   than they come). The actor reports each socket's backlog as it is now before every page and every tick, so waiting
   pages go once it drained. A snapshot started over for its load buffer drops its old pages still queued. A read that
   stops after pages went out is read once more whole and that closes the snapshot (at a later seq the client starts
   over with it); only a second failure closes it with the pages sent, logged.
4. **The UI draws the first page** (`@wisp/sync` `ClientOutput` `snapshotPage`, `@wisp/client`
   `EntityWriter.snapshotPage`, both additive): the first page of a snapshot joins the topic at once and drops nothing;
   later pages wait, and the last page brings the whole topic as the `snapshot` it was, which drops what no page had.
   The list repaints twice (the first page, then every task), never once a page. Ops for the topic still wait for the
   whole snapshot. Rust clients (peers, the TUI, the CLI's attach) join pages as before.
5. **The gate** (`spikes/server-budgets`) times the first page's last byte against `SERVER_BUDGETS.listMs` (5 ms,
   unchanged), and reports the time to the last page (every task), the page count and the bytes. It fails a run where a
   task comes in two pages or pages come at two seqs. The number in `tools/budgets.ts` is unchanged; its comment now
   says the first page.

No new wire message or field: `more` exists since ADR-0005; this is the first sender.

Result (2026-11-16, Mac, 5 000 tasks, two runs): list first page **76.6 → 1.3 ms** median; every task 76.6 →
**40 ms** (11 pages; 33 ms before the id tiebreak, which sorts each page's ties); idle RSS **146 → 98–101 MB**; cold
start 65–70 ms (unchanged).

## Places it reaches (ADR-0040)

Every client of the stream: the browser and desktop app and the phone apps (packages/sync), peers, boxes and the grid
(wisp-sync's client), `wisp tui` and the CLI's attach. No MCP tool, CLI command or setting changes; it's the transport.

## Edge cases and their tests

- The first page small, the rest after it, every row once, all at one seq, a page crossing from one query to the next;
  one read of the topic is every page joined: `crates/wisp-api/tests/v2_store.rs`
  `a_paged_topic_sends_a_small_first_page_then_the_rest_every_row_once_at_one_seq`.
- Rows ending exactly on a page close the snapshot with an empty last page; fewer rows than the first page is one page,
  as before: `rows_that_end_on_a_page_close_the_snapshot_with_an_empty_last_page_and_a_small_topic_is_one_page`.
- A cursor the feed holds: one delta of what changed, never paged: `a_paged_topic_resumes_from_a_cursor_as_one_delta_of_what_changed`.
- Retention pruned the feed: a first look still reads every row (it read empty before), and a dropped cursor gets the
  whole topic in pages: `a_paged_topic_reads_every_row_after_retention_pruned_its_change_feed`.
- The paged kind isn't defined yet (its plugin hasn't started): one empty page closes the snapshot:
  `a_paged_kind_not_defined_yet_reads_one_empty_page_that_closes_the_snapshot`.
- A page goes as it comes, not at the flush rate; a resumed topic's cursor holds until the last page:
  `crates/wisp-sync/tests/server.rs` `snapshot_pages_go_in_order_and_the_cursor_holds_for_a_resumed_topic_until_the_last_page`
  (its "waits for the tick" assertion changed deliberately).
- A client slower than the pages (past the high-water mark): pages wait, nothing is dropped for a resync, they go in
  order once it drains: `snapshot_pages_past_the_high_water_mark_wait_for_the_socket_instead_of_a_resync` (the
  spike's reads failed with a resync loop before this rule and the actor's fresh backlog reports).
- The client: the first page shows at once, a later page waits, a page at another seq starts over, ops wait for the
  whole snapshot: `packages/sync/test/client.test.ts` `snapshot pages: the first joins the topic as it lands, …`.
- The store: a page adds its rows and drops none (a cached row stays until the whole snapshot is there), the snapshot
  then drops what no page had, the row count never dips: `packages/client/test/entities.test.ts`
  `a snapshot in pages: each page shows its rows at once and drops none; …`.
- The gate: first page and last page timed, a task in two pages or pages at two seqs fail the run:
  `spikes/server-budgets` `the_list_is_timed_at_its_first_page_and_whole_at_its_last`,
  `a_task_in_two_pages_or_pages_at_two_seqs_is_an_error`.
- A snapshot started over because changes outgrew the load buffer while it loaded: the old read's pages still queued
  are dropped, never sent: `a_snapshot_started_over_for_its_load_buffer_drops_its_old_pages_still_queued`.
- A read that fails after pages went out: read once more whole, which closes the snapshot; a second failure closes it
  with the pages sent and is logged (as a failed read answered empty before). Not tested (needs a store that fails
  mid-read).

## Consequences

- On a first load the first screen draws from the 100 latest unarchived tasks within ~1 ms of the server's answer;
  the rest lands ~40 ms later. Until then a task older than those 100 that sits high in the list (a question asked
  days ago, still in *Needs you*) isn't there yet, and the counts and search cover the first page only. Then
  everything is exact. A resume from the cache (the usual app open) is unchanged.
- The whole list now takes 40 ms instead of 77, and its peak memory is one page's, not 4.5 MB of JSON and its values at
  once: RSS stays ~10 MB over idle instead of ~50.
- A full snapshot on a link slower than the server is paced by the socket instead of resynced. That holds for any
  topic with snapshot pages queued, not only `list`. A stalled socket keeps that connection's queued pages (at most the
  topic, ~4.5 MB on 5 000 tasks, plus its folded ops) until it closes, as the single 4.5 MB frame sat in its queue
  before.
- What's left of a full read is JSON (parse, the views, serialize): a later round could send rows the views leave alone
  as the stored text.
