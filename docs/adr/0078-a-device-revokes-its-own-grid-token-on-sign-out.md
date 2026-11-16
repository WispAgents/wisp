# ADR-0078: A device revokes its own grid token on sign-out (`POST /api/device/revoke`)

- Status: accepted
- Date: 2026-11-16
- Source: round 300, the security gap round 299 left (docs/v2-plan.md §4.4 step 14 *Left*). Follows ADR-0014 (the
  grid's control plane), ADR-0061 (several grid accounts) and ADR-0065 §2.3-4 (wisp Cloud on the phone). Restores v1's
  route (grid/server.js `/api/device/revoke`, mobile/www/app.js Sign out).

## Context

- Sign out on the phone (`mobile/web/src/cloud-accounts.ts`) and on the desktop (`grid-client.signout`,
  `grid-client.accountRemove`) only forgot the device token locally. The token stayed valid on the grid (its
  computers, tunnels, API keys, shares) until the person removed the device on the grid's account page, so a token
  copied off a phone before sign-out kept working.
- v1 had the route; v2's grid (`apps/grid`) ported the account page's Remove (`/devices/:id/revoke`) but not this, and
  the port dropped v1's `device.revoke` audit line from the account page's Remove too.

## Decision

1. **`POST /api/device/revoke`** on the grid's own address, authenticated by the device token (`Bearer wgd_…`)
   **being revoked**: there is no id in the request, so a token can only ever revoke itself (a body naming another
   device is ignored). Reach `grid:device` (ADR-0014's table): a Box secret, an API key or a session cookie is refused.
2. **Atomic**: the device's `revoked_at`, its audit line and its computer's removal are one transaction
   (`Accounts::revoke_own`, sharing `revoke_in` with the account page's Remove); the removed computers' tunnels close.
3. **Idempotent**: the device is found by its token's hash whether revoked or not; asking again answers `200
   { ok: true, revoked: false }` (the first answer may have been lost on a bad network). A token the grid never issued
   is `401`.
4. **Audited as the account page's Remove**: both write `device.revoke {"id": <device id>, "by": "device" | "account"}`
   (never the token or its hash), and only when the device was signed in until then.
5. **Clients tell the grid before they let go of the token, written ahead.** On Sign out the phone and the desktop
   first set the token aside **as a secret** (phone: `SecureStore` slot `wisp.cloud.revoke.<n>`, listed in web
   storage under `wisp.phone.cloud.revoke` without the token; desktop: `secrets.json` key `grid-client/revoke/<id>` =
   `<grid>\n<token>`), never in a row, and take it off the account's key (so an app killed mid sign-out still tells
   the grid, and a sign-in again as the same account meanwhile keeps its new token); then they call the route, and
   the phone forgets the account's computers after it. Told is the grid's own `200 {ok: true}`, 401/403 (already
   gone) or 404 (a grid from before the route can't be told; its account page removes the device); anything else
   (offline, 5xx, 429, a captive portal's page, a proxy's 3xx/421, no answer in 10 s) leaves it set aside. The account
   is signed out locally at once all the same. The phone tells it again on every refresh (start, back in the app;
   a sealed store that fails to answer keeps the entry) and its toast says the grid will be told; the desktop tells it
   again at start and after each sign-in (in the background).

## Places it reaches (ADR-0040)

The phone app's Sign out (Computers → wisp Cloud card), the desktop and browser's Settings → Cloud Sign out and
`wisp cloud signout` (both `grid-client`), and any script holding a device token (the route is plain HTTP).

## Edge cases and their tests

- A revoked token is refused on its next call; its computer leaves the account's list:
  `apps/grid/tests/device_revoke.rs` `a_revoked_token_is_refused_on_the_next_call`.
- A token can't revoke another device's token (body naming another id or token ignored; unknown, `wgk_` and empty
  tokens 401): `a_token_cant_revoke_another_devices_token`.
- Twice is fine, one audit line; the account page's Remove of a device already gone adds none: `revoking_twice_is_fine`.
- Phone: Sign out revokes first, then forgets: `mobile/web/test/cloud.test.ts`
  `sign_out_calls_revoke_then_forgets`; offline queues it, the toast says so, the next refresh tells the grid:
  `sign_out_offline_queues_the_revoke`; set aside before the grid is asked: `a_sign_out_is_written_ahead`; a sign-in
  again during the tell keeps its token and computers: `signing_in_again_while_the_grid_is_told_keeps_the_new_sign_in`;
  a sealed store failing once keeps the entry: `a_sealed_store_that_fails_once_keeps_the_queued_revoke`; which
  answers count as told, and a grid that never answers: `tell_revoked`.
- Desktop: told on sign-out; down, the token waits in secrets (not the store) and the next start tells it:
  `plugins/grid-client/host/tests/accounts.rs`
  `signing_out_tells_the_grid_and_one_it_couldnt_tell_is_told_on_the_next_start`.

## Consequences

- A sign-out on a phone that never reaches its grid again leaves the token valid until the account page removes the
  device (as before this ADR); the phone keeps trying on every refresh.
- The desktop's Sign out answers after the grid does (at most 10 s when the grid hangs); the command's answer doesn't
  say whether the grid was told (a warning in the log does). Saying so in Settings → Cloud needs a field in
  `grid-client`'s `Status` (a contract change), left for later.
- Signing in again as the same account replaces the token locally without revoking the old one on the grid; that's
  a separate gap (the device row stays listed on the account page).
