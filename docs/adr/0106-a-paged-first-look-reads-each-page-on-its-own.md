# ADR-0106: A paged first look reads each page on its own, and its rows are encoded where they are read

- Status: accepted
- Date: 2026-11-19
- Source: the server performance audit, round 2 (`docs/qa/perf-server-r2.md` #1: ten fresh clients at once pushed
  an already connected client's patch p99 from 105 to 279 ms). Changes ADR-0081 §Decision 1 ("in one read
  transaction"); the rest of ADR-0081 stands.

## Context

- ADR-0081 reads a paged topic's every page (`list`: 5,000 tasks, 11 pages, 4.9 MB) inside **one** read transaction,
  so every page is at one seq and no row is in two pages or none. That holds one of the store's **4** reader
  connections for the whole topic; ten cursor-less clients take all four, and every other read (commands,
  `runtime.get`, the CLI) waits on the pool.
- Each page's rows were serialised into the `snapshot` frame by the sync machine, inside the one actor task that owns
  it (`v2_actor.rs`): while it encoded 10 × 4.9 MB, nobody's patches went out.

## Decision

1. **Short reads.** A paged source whose queries are all ordered (`list`'s are: `updated_at`, then `id`) reads each
   page in its own read transaction (`v2_pages.rs` `short_reads`), and reads on from the last row it handed over with a
   keyset over the query's own order (`after`: a shorter prefix of the order equal and the next column past it), never
   an offset. The next page is read once the previous one is handed over. A query without an order keeps ADR-0081's
   one transaction.
2. **One seq.** Every page goes at the **first** read's seq. A row that changed after it is read as it is now in a later
   page, or arrives in the patches after that seq, which the client applies on top in order: it ends with every row as
   it is. A row a change moved from one query to a later one (archived meanwhile) may come twice; clients key rows by id.
   Nothing is lost: an offset could skip a row when one before it went, a keyset can't.
3. **Rows encoded where they are read.** A full page's rows are serialised to JSON in the snapshot task
   (`SnapshotAnswer::rows_json`, additive) and the machine only copies them into the frame (`wisp_sync::snapshot_frame`,
   the bytes `json(&Snapshot)` would write). A delta keeps its rows as values.

## Consequences

- A burst of fresh clients holds a reader for one page at a time (~0.3 ms, 500 rows), not the whole list, and the
  actor's work per page is a copy. The numbers are in `docs/qa/perf-server-r2-fixes.md`.
- `spikes/server-budgets` still requires every page at one seq and no task in two pages on its idle server; both hold.
- Tests: `crates/wisp-api/tests/v2_store.rs`
  `a_write_between_two_pages_is_in_a_later_page_or_the_patches_after_the_snapshots_seq`, and
  `crates/wisp-sync/tests/server.rs` `a_snapshot_page_encoded_by_the_host_is_the_frame_its_rows_make`.
- Not done here (the audit's other ideas): first page only until the client scrolls, and one encoding shared by
  clients asking at the same head.
