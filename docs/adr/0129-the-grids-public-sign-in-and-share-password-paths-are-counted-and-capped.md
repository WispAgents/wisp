# ADR-0129: The grid's public sign-in and share-password paths are counted and capped, and a device code keeps where it was asked from

- Status: accepted
- Date: 2026-11-26
- Source: the grid audit, round 3 (items `doc5`, `doc7`): a share's password could be guessed as fast as the grid
  hashed it, each guess a scrypt on an async worker; anyone could ask for device codes without limit, and the approve
  page showed only the name the device chose for itself.

## Context

`/s/<id>` (the form) and `/api/shares/<id>` (`x-wisp-share-password`) checked a password with scrypt (N=2^14, r=8:
about 50 ms of CPU and 16 MB) inline on a tokio worker, with no count: a script got a guess per hash, and a few in
parallel stalled every other request on the grid. A "guess" of any length was hashed.

`POST /api/device/code` is public. Nothing counted it, rows stayed a day past their expiry, so one address could fill
`device_codes`. The approve page said "**<name>** asks to use your wisp": the name is whatever the asker sent, in the
grid's own bold, and nothing said where the request came from, so a phished approver had nothing to go on. `/activate`
looked codes up without a count.

## Decision

1. **Share passwords.** A non-owner's password on a password share counts, before any hashing, against
   `sharepw|<id>|<client ip>` (10 a minute) and `sharepw|<id>` (60 a minute, every address): past either it is 429,
   the locked page saying "Too many tries. Wait a minute and try again." (its form kept), or `{error}` on the API. The
   sharer and the other access kinds count nothing. A password that couldn't be one (empty, longer than
   `PASSWORD_MAX`) is wrong without hashing.
2. **scrypt off the workers.** Checking a password, and hashing a new one for `put`/`patch`, runs on
   `spawn_blocking` holding one of 4 permits of a semaphore on `Grid` (`scrypt`). With none free the answer is 503
   with `Retry-After: 1` at once: no queue. `Shares::put`/`patch` hash inline only when the edge didn't
   (`Access::hashed`), as the CLI and tests call them.
3. **Device codes.** `POST /api/device/code` counts `devcode|<client ip>` (10 a minute, 429). A new code is refused
   with 429 when its address has 20 codes waiting (not consumed, not denied, not expired) and with 503 when the grid
   has 10,000. Expired rows are deleted at each insert (a device polling one hears `invalid_grant`, which the CLI and
   the phone app already take as "start again").
4. **New state: where a code was asked from.** Migration 16 adds `device_codes.requester_ip` and
   `requester_agent` (the user-agent, at most 200 characters), both nullable: rows from before have none.
5. **The approve page** says `A device calling itself “<name>” asks to use your wisp as <handle>` (the name
   escaped, quoted as the device's claim), `Asked from <ip> · <n> minutes ago`, and, when the approver's address
   isn't the requester's, a warning line: "This request came from a different network than you. If you didn’t just
   start it yourself, don’t allow it."
6. **`/activate` look-ups** count `activate|user:<id>`, or `activate|ip:<ip>` signed out: 30 a minute, 429.

Every count is the edge's in-memory `Limiter` (a minute's sliding window), and the address is `http::client_ip`, as the
waitlist's.

## Consequences

- A share password costs a guesser one try every 6 s per address, and at most one a second over all; the grid never
  spends more than 4 cores on scrypt. A share under attack refuses its real readers too for the rest of that minute
  (the sharer always reads it): the price of a per-share cap.
- The limits live in one process's memory: a restart resets them, and a grid with several edges would count per edge.
- A person signing in many devices from one office NAT at once can hit 20 waiting codes; finishing or letting them
  expire frees room.
- The warning compares addresses only: a phone on mobile data approving a laptop's code at home sees it too. It warns,
  it doesn't refuse.
- Not checkable by `tools/check`: these are the grid's behaviour, covered by its tests
  (`share_password_guesses_are_rate_limited`, `hashing_waits_on_the_blocking_pool`, `device_code_is_rate_limited_per_ip`,
  `live_device_codes_are_capped_per_ip`, `expired_device_codes_are_swept`, the approve page's test).
- Places it reaches (ADR-0040): the grid's own pages and API; wisp's CLI sign-in and the phone app's go through the
  same device API unchanged.
