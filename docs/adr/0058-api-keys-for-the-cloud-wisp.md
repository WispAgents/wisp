# ADR-0058: API keys for scripting the cloud wisp: the grid's `/api/keys`, `grid-client.keys*`, and a key only opens `<handle>`

- Status: accepted
- Date: 2026-11-14
- Source: F-mach-15 (docs/v1-features.md): "Given an API key, when I call `https://<handle>.<domain>/api/…` with
  `Bearer wgk_…`, then it works without a browser." v1: grid/accounts.js (API keys), grid/server.js `manageKeys`,
  server/services/api-keys.js, web/js/features/api-keys.js, test/grid.test.js "API keys".

## Context

The grid's trust table (ADR-0014, `apps/grid/src/trust.rs`) already named the `Key` credential (`box` only) and the
`grid:keys` / `edge:keys` reaches, and grid.db already had v1's `api_keys` table, but nothing made, checked or revoked
a key: no route, no command, no screen. The box's wisp already maps `x-wisp-proxy-via: key` to the `apikey`
capability set (crates/wisp-api auth.rs), but the edge never sent it.

## Decision

1. **Wire shape, the grid's (v1's, plus `address`):** with a device token (`grid:keys`),
   - `GET /api/keys` → `{keys: [{id, name, hint, createdAt, lastUsedAt}], address}`, newest first; `address` is the
     user's own wisp (`instance_url(handle)`), where a key is sent (new: v1's page derived it);
   - `POST /api/keys {name}` → 201 `{id, name, hint, createdAt, lastUsedAt, key}`: the only answer with the key. A name
     is 1–60 characters (Unicode scalar values), one line. 20 live keys at most (409). At most 30 creates a minute per
     user (429, `http::Limiter`);
   - `DELETE /api/keys/<id>` → `{ok: true}`, or 404 `No such key.` (unknown, another user's, revoked already).
   Every answer is `no-store`. A key on these routes is refused with the trust table's words (403 `a key can’t manage
   keys`), a box's secret and the grid's own cookie likewise (403); no credential is 401.
2. **At rest:** the key's SHA-256 (base64url, v1's `sha256`, so a live grid.db's keys keep working) and its last four
   characters. `audit` gets `apikey.create` / `apikey.revoke` with the key's id only. `last_used_at` is written at most
   once a minute per key.
3. **The edge:** a `Bearer wgk_…` that proves nobody else is checked only at `<handle>.<domain>` (`Credential::Key`,
   `may(Key, Box)`), and only for that handle's own user (another user's key: 403). Everywhere else a key is nobody
   (401): computers, sandboxes, previews, the tunnel and the grid's own API. A request let in by a key is marked
   `x-wisp-proxy-via: key` by the edge itself (a request extension, set after the caller's own `x-wisp-proxy-*` headers
   are dropped), so the box's wisp speaks for it with the `apikey` set, which can't manage keys.
4. **The `apikey` set gains `edge:health`** (crates/wisp-kernel/capabilities.json): `/api/health` is how the pane says
   to try a new key, as v1's did.
5. **Commands (wisp.grid-client):** `grid-client.keys` (read-only, `NoArgs → Keys`), `grid-client.keysCreate`
   (`KeyArgs{name} → NewKey`) and `grid-client.keysRevoke` (`IdArgs → Value`), calling the grid with this computer's
   device token. `keysCreate` is `uiOnly`: the key is shown once to a person, and through an agent it would sit in a
   transcript. None of the agent sets names `grid-client:*`, and `apikey` doesn't either. A key's id is checked
   (hex, ≤ 32) before it reaches the grid's path; a 404 on list or create says the grid is older.
6. **UI:** Settings → Cloud → API keys (signed in): kit only, `t()`. Create clears the box and shows the row at once
   (Making…); the key is held in one local signal, shown with Copy and a `curl … $WISP_KEY …/api/health` line, and
   dropped on Done or when the pane unmounts. Never in the entity store, drafts, settings or storage. Revoke asks
   first, then the row goes at once; a refusal puts it back and reloads the list.

## Edge cases (each with its test)

| Case | Test |
|---|---|
| the key is said once; lists carry neither it nor its hash; only the hash is at rest; audit has the id only | `apps/grid/tests/keys.rs` `a_key_is_made_once_listed_without_its_secret_and_revoked`, `keys.rs` `a_key_is_kept_as_its_hash_and_last_four_and_said_once` |
| a name is needed, 60 characters at most (60 emoji fit), one line | `keys.rs` `names_are_one_trimmed_line_of_at_most_60_characters`; route test above |
| 20 live keys; a revoke frees a place; counted and made in one transaction | `keys.rs` `twenty_live_keys_at_most_and_a_revoke_frees_a_place` |
| creates are rate-limited per user, not per grid | `apps/grid/tests/keys.rs` `making_keys_is_rate_limited_per_user` |
| a key opens its own wisp only; a typo, a key without its last character or in the query string is 401; another user's wisp is 403; a computer and the grid's API are 401; a key can't manage keys | `apps/grid/tests/keys.rs` `a_key_opens_its_own_wisp_only_and_is_401_once_revoked` |
| a revoked key, or a disabled user's, opens nothing; another user can't revoke it | `keys.rs` `a_revoked_key_or_a_disabled_user_opens_nothing_and_another_user_cant_revoke` |
| last used: set on first use, held for a minute, then moves | `keys.rs` `a_key_is_kept_as_its_hash_and_last_four_and_said_once` |
| the commands send the device token; a bad name or id never reaches the grid; an older grid says so | `plugins/grid-client/host/src/keys.rs` tests |
| end to end: made from wisp, a script calls the box with the key alone, the key can't make keys through its own wisp, the key is in no file of this wisp, revoked → 401 | `crates/wisp/tests/serve_grid.rs` `an_api_key_scripts_the_cloud_wisp_without_a_browser_until_revoked` |
| the pane: row at once, rollback, revoke restore in place, try line names no key | `plugins/grid-client/ui/test/keys.test.ts` |

## Not covered yet

- **`/_grid/keys` (`edge:keys`)**, keys made in a cloud wisp's own browser with its cookie: v2's edge has no
  `i:<box>` cookie credential yet, so a cloud wisp's Settings can't make keys; a desktop signed in to the grid can.
- A request already streaming (an SSE stream opened with a key) isn't cut when the key is revoked; new requests are 401.

## Places it reaches (ADR-0040)

Inside wisp (Settings → Cloud), the grid's HTTP API (any script with a device token), and wisp's MCP for listing and
revoking (`grid-client.keys`, `grid-client.keysRevoke`); making one is a person's act.
