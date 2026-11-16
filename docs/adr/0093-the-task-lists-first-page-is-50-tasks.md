# ADR-0093: The task list's first page is 50 tasks

- Status: accepted
- Date: 2026-11-16
- Source: docs/qa/bugs.md B24. Amends ADR-0081's number (the first page was 100); the paging, the order, the 500-task
  pages after it and the 5 ms budget (`tools/budgets.ts` `SERVER_BUDGETS.listMs`, ADR-0011) are unchanged.

## Context

ADR-0081 sends the list's first look in pages, the first sized for the first screen: 100 tasks, "a 1440 px window
draws ~40 rows". On the shared Linux runners that page now decides PR CI: 5.38–5.84 ms medians (d58d7621, 430ab974;
the second after ADR-0092 took 15 rounds), 103 KB for 100 tasks (1.27 KB a task). Where the time goes (local bench,
`budgets` profile): the page's query 0.1–0.2 ms; the rest is the bytes (the rows' JSON, the egress views, the frame,
the socket, the client's parse).

The task rows in the browser and the desktop app are v1's feed rows (three lines, ~64–80 px): a 1440×900 window shows
~11 of them, a 2160 px tall one ~26; the phone apps fewer. Home's Needs you lists at most five.

Measured (the same machine, 5 000 tasks, `cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml`):

| first page | its bytes | list, median of 5 rounds | rounds |
|---|---|---|---|
| 100 tasks | 103 KB | 1.95–3.02 ms (three runs) | 2.84 – 3.04 · 1.83 – 2.36 |
| 50 tasks | ~52 KB | **1.27 ms** | 1.26 · 1.27 · 1.28 · 1.38 · 1.68 |

## Decision

The first page of `list` is the **50** latest unarchived tasks (`crates/wisp/src/serve.rs` `list_pages`); the rest
follow 500 at a time as before, all at one seq. The bench's in-process breakdown reads the same 50.

## Consequences

- The first screen of every layout is still whole from the first page (26 rows at most); the rest lands as before
  (~50 ms for every task), when the list repaints once more (ADR-0081: the first page, then every task).
- A task older than the 50 latest that belongs in an open section (a Needs you row from last week) shows with the
  rest, ~50 ms later, as one past the first 100 did before.
- About 45% less time on the gated measure, so CI's runners have headroom under 5 ms; the payload per row is unchanged
  (B24's other lead: a slimmer list row, a wire change, not taken).

## Places it reaches (ADR-0040)

Every client's first look at the list: the browser and desktop app, the phone apps, peers, `wisp tui`. Nothing to
change in them: pages are joined as before.

## Edge cases and their tests

The paging rules are ADR-0081's and its tests (`crates/wisp-api/tests/v2_store.rs`), which take the page size as a
parameter; this changes the number the server passes. The gate (`spikes/server-budgets`) reports the first page's
task count in its table.
