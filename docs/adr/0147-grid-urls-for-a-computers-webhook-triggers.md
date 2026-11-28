# ADR-0147: A computer's webhook triggers are reached from the internet by a grid URL the person makes, naming that computer and carrying a token

- Status: accepted
- Date: 2026-11-28
- Source: the owner asked "Can we have a webhook trigger using the grid to machines? So user setup a trigger on
  webhook which will generate a url to call and the trigger work on webhook requests", then "The url should have the
  computer that execute", then "Make sure user can generate tokens to use and send no public urls of course". Follow-up the same day: "Hold webhook calls on the grid while the computer is offline". Builds on
  ADR-0043 (the `webhook` trigger source, `POST /api/triggers/<name>`), ADR-0010 (a computer's tunnel), ADR-0014
  (the grid's trust table) and ADR-0058 (API keys: said once, kept as a hash).

## Context

- A `webhook` trigger (ADR-0043) answers `POST /api/triggers/<name>` on its own wisp, behind its `secret` or HMAC
  `header`, and only once `automations.webhookTriggers` is on. That port is on a laptop behind NAT: GitHub, Sentry or
  Linear can't reach it, so a webhook trigger worked only on a machine with a public address.
- Every computer on the grid already holds a tunnel (`<computer>--<handle>.<domain>`), but everything through it is the
  computer's person (the tunnel's mark), so an unsigned call must never ride it.
- The grid already lets unsigned callers in by an unguessable path (`/_in/<token>`, `/_route/<token>`, ADR-0146's
  connector events). A trigger's name is guessable (`deploy`), so it can't be the key.

## Decision

1. **The URL:** `https://<computer>--<handle>.<domain>/_hook/wgh_<43 base64url chars>`. The host names the computer
   that runs the trigger; the token (256 random bits) is the only way in. A URL is made for one trigger on one
   computer, by wisp on that computer (its device token): `POST /api/hooks {trigger}` → 201 with `url`, said once.
   `GET /api/hooks` lists the calling computer's live URLs (id, trigger, the token's last four characters, made, last
   called); `DELETE /api/hooks/<id>` revokes one. At most 50 live per computer, 30 makes a minute per user.
2. **State:** the grid's `hook_tokens` table (migration 18): the token's SHA-256 and `hint`, never the token, which is
   in no later answer, no log and no `audit` row (only the URL's id). Removing the computer removes its URLs
   (`ON DELETE CASCADE`); a disabled user's URLs open nothing. wisp keeps the URL nowhere: no row, topic or log line.
3. **Trust (ADR-0014):** two new reaches: `grid:hooks` (a device only; a box's secret, a key, a cookie are denied:
   anything on a box can read its secret, so it never makes a URL that starts work on a laptop) and `trigger`
   (`/_hook/<token>`, the `webhook` credential only).
4. **The call:** at `<computer>--<handle>/_hook/<token>`, before any sign-in. A token unknown, revoked, or another
   computer's is the same 404. The body is capped at 1 MB (413) before the tunnel. The grid forwards
   `POST /api/triggers/<trigger>` (percent-encoded, one segment, the call's query kept) with the caller's own headers
   (its signature, `x-wisp-secret`, its delivery id), without the grid's cookie, a grid bearer (`wgd_`, `wgk_`), any
   `x-wisp-*` mark or `x-forwarded-*`, and marks it `x-wisp-route: hook`.
5. **Offline: held** (`hook_held.rs`, the `hook_held` table, migration 18). A call to a computer that is offline (or
   goes offline while asked) is kept and answered 202 `{held: true}`; once its tunnel is back the grid delivers what's
   held, in order. Rules:
   - **Sealed** under the user's data key (AES-GCM, bound to the computer): the call carries the sender's signature and
     maybe `x-wisp-secret`. A grid without a master key holds nothing (503, `Retry-After: 60`).
   - **Strictly in order:** while anything is held for a computer, a new call waits behind it even with the computer
     online, and nothing passes a call being retried.
   - **What ends a call:** the computer's 2xx or 4xx. A 5xx or no answer is tried again after 2 s, doubling up to 5 min,
     and dropped after 8 tries; the tunnel going mid-way isn't a try.
   - **Caps:** 200 calls and 20 MB per computer, 24 h each. A call over a cap isn't held: 503 with `Retry-After`, so the
     sender keeps it.
   - **Once:** a call with a delivery id (`x-wisp-delivery`, `x-github-delivery`) is held once per URL.
   - **Revoked:** revoking a URL drops what it held; removing the computer or URL removes it (`ON DELETE CASCADE`).
6. **The computer's side:** grid-client's tunnel server takes `x-wisp-route: hook` on `POST /api/triggers/<name>`
   only (404 otherwise) and hands it to wisp here **without the tunnel's mark**, so the open route sees no token and
   the caller is `public`. The trigger still checks its secret or HMAC header and answers its own 404 when webhook
   triggers are off: the token gates the grid, the secret gates the trigger.
7. **No public URL by name:** `POST <computer>--<handle>/api/triggers/<name>` stays what it was, a request into the
   computer that asks for a sign-in.
8. **Where it reaches (ADR-0040):** the app (a saved webhook trigger's form: **Make a grid URL**, shown once with
   **Copy URL**, each live URL by its last four characters with **Revoke**); the CLI (`wisp trigger url <name>`,
   `wisp trigger urls [<name>]`, `wisp trigger url-revoke <id>`); MCP (`grid-client.hooks` and `.hooksRevoke`;
   `.hooksCreate` is `uiOnly`, like `keysCreate`: the URL is its own key, and through an agent it would sit in a
   transcript). Commands: `grid-client.hooks {trigger}` → `TriggerHook[]`, `.hooksCreate {trigger}` → `NewHook`
   (`url`), `.hooksRevoke {id}`.

## Edge cases (each has a test)

1. `a_token_is_said_once_kept_as_its_hash_and_opens_its_computers_trigger` (grid `hooks.rs`): the hash in the table,
   nothing in `audit`, one character off opens nothing, a trigger's name is never a token.
2. `a_revoked_token_opens_nothing_and_only_its_computer_revokes_it`, `a_computer_holds_at_most_fifty_live_urls`,
   `removing_the_computer_removes_its_urls`, `a_trigger_name_is_one_path_segment`.
3. `a_webhook_call_carries_no_grid_credential_or_mark_and_keeps_its_signature`,
   `a_grid_bearer_is_dropped_and_a_callers_own_bearer_stays`, `the_trigger_is_one_encoded_segment_and_the_query_goes_along`
   (grid `edge_hook.rs`).
4. `trigger_urls_are_made_by_a_device_and_called_by_their_token_only` (the trust table).
5. `a_trigger_url_reaches_its_computers_trigger_by_its_token_and_nothing_else_does` (`apps/grid/tests/trigger_hooks.rs`):
   offline calls held (202) and delivered in order once the tunnel is up, the first delivery's 503 retried, a delivery
   id held once, nothing readable in the table, the forwarded path and mark, a wrong token or another computer's address 404, over 1 MB 413, a
   name at the computer's address asks for a sign-in, revoked 404, another computer can't revoke it.
6. `a_hook_call_reaches_one_trigger_only_and_as_no_one` (grid-client core): never the tunnel's mark, no `x-wisp-token`,
   `x-wisp-key`, cookie or Origin; `%2F` in the name is refused.
7. `hook_held.rs`: `a_held_call_is_sealed_and_opens_for_its_user_and_computer_only`,
   `a_delivery_id_is_held_once_and_a_call_without_one_each_time`, `past_a_cap_a_call_is_not_held_and_a_day_old_one_is_gone`,
   `revoking_a_url_drops_what_it_held_and_no_master_key_holds_nothing`, `retries_wait_longer_each_time_up_to_five_minutes`.
8. `a_grid_url_starts_a_computers_webhook_trigger_until_revoked` (`crates/wisp/tests/serve_grid.rs`, two real
   processes): a URL needs the computer on the grid; the trigger's secret is still checked through the grid; the URL
   starts the task (`triggeredBy: trigger:deploy`); with the computer off the grid a call is held and starts its task
   once it's back; revoked, 404; the token is never in wisp's database.

## Consequences

- Any computer's webhook trigger works from the internet with no open port, and the URL says which computer runs it.
- Two keys for a generic caller (the URL's token and the trigger's `x-wisp-secret`); a signed sender (GitHub, Sentry,
  Linear, PagerDuty) sends its signature as before, so it needs only the URL.
- A lost URL can't be shown again: make a new one and revoke the old.
- The grid keeps calls (sealed) for up to a day: a call made while the laptop sleeps runs when it wakes, which may be
  later than its sender meant. A sender that wants it now sees 202 `{held: true}` and can tell.
- Not here yet: the grid's own account page listing a computer's URLs and what's held for it.
