# ADR-0117: a paged first look's last page carries what changed since the first, and a shared row's patch goes once

- Status: accepted
- Date: 2026-11-22
- Supersedes: ADR-0106 §2 (one seq for every page)
- Source: the states and events audit of 2026-11-22 (`docs/qa/audit-states-events.md` §1.1, §1.4): task updates on
  desktop and phone stopped until a reload.

## Context

ADR-0106 reads each page of a paged topic (`list`) in its own short read and sends every page at the first read's
seq; the patches after that seq bring the client up to date. `list` is ordered by `updated_at`, so a task that works
while a client loads its list moves *behind* the keyset cursor: no later page has it, and the client gets a `patch`
for a row it never received. The client marks `list` drifted and resubscribes; its `unsub` and `sub` raced as two
unordered POSTs, and when `sub` landed first the server dropped the topic for good: no task updates until a reload.

A `session` row is on two topics (`list` and `session:<id>`). Its change went out once per topic in one frame; the
client keeps one cell per row, so the second `remove` (or `add /-`) of a patch failed and drifted `list` the same way.

## Decision

1. **The last page reads on to now.** The last page of a short-read snapshot (`v2_pages.rs` `read_short`) also reads
   the topic's changes since the first page's seq in its own read, adds their rows (and deletions) to the page, and
   goes at the seq it read to. Earlier pages keep the first seq. The sync machine stamps every page's frame with the
   first page's seq (`Sub::page_seq`) and drops buffered changes the last page already holds.
2. **A non-idempotent change goes once per connection.** At one seq, `Patch` and `Append` ops of the same row go on one
   live topic only (the longer-lived one, `Topic::rank`), never once per topic (`SyncServer::committed`).
3. **A peer's sync POSTs go in order.** `packages/client/src/peers.ts` sends one peer's `/api/events` posts one after
   another, and reopens the stream when one fails while it is still live.

## Consequences

- A row may come up to three times in one first look (a page, the page it moved to, the last page's delta); clients
  key rows by id, as ADR-0106 already required.
- The last page's read is one more short read; no reader is held across pages.
- Tests: `crates/wisp-api/tests/v2_store.rs`
  `a_write_between_two_pages_is_in_a_later_page_or_the_patches_after_the_snapshots_seq` (updated to this contract).
