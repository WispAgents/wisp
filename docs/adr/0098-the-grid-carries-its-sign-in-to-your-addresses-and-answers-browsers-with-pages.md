# ADR-0098: The grid carries its sign-in to your addresses with a one-time ticket, and answers a browser with a page

- Status: accepted
- Date: 2026-11-18
- Source: QA round 2 (2026-11-18), the web Grid's 11 findings (B1–B11). Follows ADR-0014 (the grid's trust table),
  ADR-0041 (the grid is the hub), ADR-0064 (a preview's own sign-in, the same ticket shape), ADR-0078. Restores v1
  grid/server.js `/go`, `boxEdge`/`computerEdge` `/_grid/auth`, `common` (assets), `watchers` (`/events`).

## Context

- **Open** on Home went to `<handle>.<domain>` with nothing: the grid's cookie is host-only (and must be: a page a
  box serves would read it), so the box saw nobody and a browser got `{"error":"Sign in first."}`. v1 carried the
  sign-in with a ticket; the port had dropped it.
- The grid's images, `/events`, robots.txt and Caddy's TLS check (`/_grid/tls-ask`) were never ported, and every
  failure a browser hit (a typo'd path, a dead share link, an offline computer) was raw JSON.
- Smaller v1 carry-overs QA tripped on: a form's note lived in the URL (`?m=`) and came back on every reload; Remove
  took a computer off the grid in one click; every `.input` was upper-cased (v1 styled its only field, the device
  code, and v2 added others); the signed-in header's Grid badge differed from the landing's.

## Decision

1. **`/go` hands the sign-in over** (`handoff.rs`, `edge_handoff.rs`): signed in at the grid, `/go`
   (`?computer=` / `?sandbox=`, `&path=`) makes a one-time ticket (60 s, scoped `box:<label>`) and answers `302
   <address>/_grid/auth?t=&r=`. There the ticket, once, becomes the address's own host-only cookie `wisp_box`, a
   session scoped `i:<label>` (30 days), then on to `r` (a same-site path). Credential `cookie` in ADR-0014's table,
   which already allowed `box`, `computer` and `sandbox` and denied `preview`; no row changes.
   - A spent, expired, forged or other-address ticket sets nothing and goes back to `/go` for a fresh one (which
     needs the grid's sign-in). Tickets and sessions are per label, so neither moves between your wisp, a computer
     and a preview.
   - `Accounts::session` (the grid's pages) now takes only `scope = 'grid'`, never an address's or a preview's.
   - Signed out at an address: a page opened in a browser goes to `/go?…&path=<where it was>`; a fetch hears 401.
     A non-GET with that cookie from another Origin is 403. `/_grid/signout` there ends it.
2. **A browser gets a page, anything else JSON**, decided by the request (`Accept: text/html` and not a fetch, or a
   posted form), never the path: 404, a dead share (v1's "isn't shared any more, or its link has a typo"), an offline
   computer (v1's text), and any other failure (`edge_answer.rs`). API clients see the same JSON as before.
3. **What every address answers before routing** (`edge_common.rs`): `/_grid/a/<name>` from `apps/grid/assets`, built
   into the binary (one file to deploy); `/favicon.svg|ico` on the grid's own address; robots.txt; Caddy's TLS ask from
   loopback only.
4. **`/events`** streams `event: change` to a signed-in page when its computers or cloud wisp change (tunnel up or
   down, hostd's change events), a ping every 25 s, as v1.
5. **Pages:** `/` signed out is the landing with its sign-in (v1); a form's note rides a one-minute cookie the next
   page shows once and drops; Remove (a computer, a device) asks once in place (the button turns into the question,
   a second press sends; without script it just sends); only the device-code field is upper-cased; the waitlist
   dialog's footer stays in view; one Grid badge (the landing's); a page named after the grid is titled once;
   Activate has the Home/Account nav; Account says how a dev account signs in.

6. **Add a cloud machine** (v1 `/machines/waitlist`, `wishMachines`): `POST /machines/waitlist` (session + CSRF) keeps
   what the person waits for (`more`, `bigger`, `gpu`, a note) in `machine_wishes`, audited (`machines.wish`,
   `machines.unwish`); nothing picked takes them off. JSON for the dialog, a redirect with a note for a plain form. Home
   shows it, and the operator reads it with `wisp-grid waitlist` (v1 `wisp grid waitlist`: the seat waitlist, then who
   wants more machines). Home's Machine line shows the container's limits (boxd's defaults when grid.json sets none, as
   v1); a process box has none, so it says only "sleeps when idle".
7. **What Home says of the cloud wisp is kept** (v1 server.js `hostd.on('change')`): each hostd change writes
   `instances` (state, started and stopped at, stop reason, last activity, next scheduled task). hostd reads each
   running box's `/api/status` every `checkMs` (v1 `tick`) for its `nextWakeAt`; a box put to sleep with a task
   scheduled wakes for it 30 s early (v1 `#scheduleWake`), and after a restart the grid hands the kept times back
   (`restore`). A wake that starts the box doesn't cancel itself (the timer leaves the box before the start ends timers).
8. **No box outlives the grid** (v1 cli.js `serve`, hostd.js `close`): `wisp-grid dev` and `serve` stop every box on
   Ctrl-C or SIGTERM (dev only heard Ctrl-C, so `kill` left its boxes running), a start under way gives up and kills its
   process, nothing new starts (503 "The grid is shutting down."), and the rows say "grid stopped". A grid killed
   with SIGKILL still leaves them, as v1 did.

## Consequences

- A box's wisp never sees the address cookie (the edge drops `cookie` towards a box, as before).
- Tests: `apps/grid/tests/go_handoff.rs` (hand-off, replay, expiry, other address, computer, signed out),
  `tests/pages_qa.rs` (B2–B11), `tests/machines_waitlist.rs`, `tests/box_lifetimes.rs` (the test binary stands in for a
  box), `src/handoff.rs`, `src/assets.rs`, `src/wishes.rs`, `src/instances.rs` and `src/hostd_wake.rs` unit tests.

## Edge cases

- `a_replayed_or_expired_ticket_sets_no_cookie`, `open_on_a_computer_hands_the_sign_in_to_that_computer_only`,
  `an_address_session_signs_in_there_and_nowhere_else`, `signed_out_at_the_wisp_a_browser_goes_for_a_ticket_and_a_fetch_hears_401`.
