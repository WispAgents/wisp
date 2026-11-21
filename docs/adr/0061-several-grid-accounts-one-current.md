# ADR-0061: wisp.grid-client keeps several (grid, account) sign-ins, one current, each token in secrets under its own key

- Status: accepted
- Date: 2026-11-14
- Source: F-mach-21 (docs/v1-features.md): "Given several grids or accounts, when I add an account, then each is
  listed." v1: web/js/features/cloud.js (`cloudPane`, Add an account), server/services/cloud.js, server/machines.js
  `addGrid`/`remove` (#427 "several grids and accounts in the phone app and on the desktop").

## Context

v2's grid client (ADR-0010, 0038, 0041) kept one account: one `account` doc in `grid_client_doc`, one token at
`grid-client/token`, and `St::account()` read by every call to the grid (shares, sandboxes, teams, keys, this
computer's tunnel, the publish destination). Signing in again replaced it. v1 had no account record at all: each
sign-in was a `grid` machine with its token under its machine id, so several grids and several people on one grid
were simply several cloud computers, all live, none "current"; whatever needed one account took the first.

Two bugs came with the one-account shape and with v1's: the cloud box was found by its grid alone, so a second
account on the same grid renewed the first one's box with the second one's token (v2), and signing out removed every
peer of the grid, another account's included (v2), or only the cloud box, leaving that account's computers in
Machines holding its token (v1, `machines.remove` of the box only).

## Decision

1. **State shape** (`plugins/grid-client/host/src/state.rs`): `grid_client_doc` holds `account:<grid>#<handle>` per
   account (the old `Account` body plus `added`, ms) and `current` (`{key}`). A key is `<grid>#<handle>` (a grid is an
   origin, a handle never holds `#`; the grid's answer with one is refused). In memory, `Accounts { list, current }`
   is the pure part: `add` (renewed in place if signed in to again, and made current), `put`, `update` (dropped if
   signed out meanwhile), `switch`, `remove` (the first one left becomes current), `on_grid`. Every change goes
   through `St::change`, which writes the docs that changed and `current` in one store write.
2. **Current:** `St::account()` and `St::signed_in()` read the current account, so shares, sandboxes, teams, keys,
   replies and the publish destination go through it unchanged. Signing in adds an account and makes it current.
   A share link on any grid signed in to opens as that grid's account (the current one if it's there).
   **This computer's tunnel is the current account's:** switching (or signing out of the current one) closes it and
   dials the new current account's grid if this computer was on there.
3. **Tokens:** each account's device token is in secrets at `grid-client/token/<grid>#<handle>` (`~/.wisp/secrets.json`
   `{"grid-client": {"token/<key>": …}}`), never in a row, the change feed, a topic or the Settings → Cloud row.
4. **Peers:** each account's cloud box, computers and sandboxes are wisp.machines peers carrying the `grid` and
   `handle` they came from (Machines and Run on list them as before). The cloud box is found by its previous id or by
   (grid, handle, transport `grid`); a new one is named `Cloud`, or `Cloud (<handle>)` when one is called that
   (v1 `addGrid`). `sync_computers` lists every account's computers; one account the grid refuses doesn't stop the
   others. Signing out of one account removes exactly its peers: its box, and those with its grid and handle.
5. **Commands** (wisp.grid-client, `api/src/lib.rs`): `grid-client.accounts` (read-only, `NoArgs → AccountInfo[]`),
   `grid-client.accountSwitch` and `grid-client.accountRemove` (`AccountArgs{key} → Status`). `grid-client.signin`
   adds; `grid-client.signout` signs out of the current account. `Status` gains `accounts: AccountInfo[]`
   (`{key, grid, handle, machine, current}`, additive, defaulted). All are MCP tools like every command (ADR-0040);
   no agent capability set names `grid-client:*` (as ADR-0058).
6. **Migration** (forward-only, `grid-client` v2): the `account` doc becomes `account:<grid>#<handle>` and `current`,
   in SQL, in one step. Its token moves at the first start after it (`St::load`): `grid-client/token` is copied to the
   migrated account's key and deleted, only when exactly that one account is there and its new key is empty.
   The Settings → Cloud row (`grid_client_cloud`, owner `grid_client_view`) gains an `accounts` JSON column (v2).
7. **UI** (Settings → Cloud, kit only, `t()`): an Accounts group, one row per account (handle; `on <host>`; a
   `Current` pill), each with Switch (disabled on the current one, so the columns never move) and Sign out (asks
   first, v1's words); this computer on the grid under them; then Add an account (v1's chip and lede), which opens
   the sign-in box at once, with Cancel. Switch and Sign out are optimistic (`switched`/`removed` in
   `ui/src/accounts.ts`), rolled back with a toast on refusal. An older server's row (no `accounts`) shows its one.
8. **CLI:** `wisp cloud accounts | switch <account> | remove <account>`; an account is its key, `handle@host`, or a
   handle signed in to on one grid only.

## Edge cases (each with its test)

| Case | Test |
|---|---|
| add, re-add in place, switch, unknown switch, remove non-current/current/last | `plugins/grid-client/host/src/accounts.rs` `accounts_add_switch_and_remove_keep_one_current` |
| a stale `current` falls back to the first; order is sign-in order; one current | `accounts.rs` `a_stale_current_falls_back_to_the_first_and_the_list_keeps_its_order` |
| signing out of one account takes only its own peers (v1 left its computers; v2's port took the other account's) | `accounts.rs` `signing_out_of_one_account_takes_only_its_own_peers` |
| a second account on one grid gets its own cloud box, named with its handle | `peers.rs` `a_second_account_on_one_grid_gets_its_own_cloud_box_named_with_its_handle` |
| two grids and two people on one: each listed, tokens keyed per grid and account, each one's box and computer, switch, sign out of one and of the current, no token on disk in the store, kept across a restart | `plugins/grid-client/host/tests/accounts.rs` `signed_in_to_two_grids_and_twice_to_one_each_is_listed_with_its_own_token_and_peers` |
| the one-account shape moves (docs and token), imports untouched | `tests/accounts.rs` `the_one_account_shape_moves_to_several_with_its_token` |
| end to end with two real local grids, the CLI, and `secrets.json` | `crates/wisp/tests/serve_grid_accounts.rs` `two_grids_and_two_people_on_one_each_listed_switched_and_signed_out_of_alone` |
| `wisp cloud switch ada` on two grids asks for `ada@host` | `crates/wisp/src/cloud_cli.rs` `an_account_is_named_by_handle_handle_at_host_or_key` |
| the row's accounts; Switch/Sign out at once; an older server's row | `plugins/grid-client/ui/test/accounts.test.ts` |
| the mock answers Status and AccountInfo | `apps/web/test/mock-commands.test.ts` `Cloud accounts answer Status and AccountInfo…` |

## Consequences

- One tunnel at a time: this computer is on the current account's grid only. Being reachable on two grids at once
  would need one dialer per account; not covered yet.
- Agents can't switch accounts (no agent set names `grid-client:*`), so an agent's share goes through whichever
  account the person made current.
- The topbar's account menu still names one grid account (the current one).

## Places it reaches (ADR-0040)

Inside wisp (Settings → Cloud), the CLI (`wisp cloud accounts|switch|remove`), and wisp's MCP tools
(`grid-client.accounts`, `.accountSwitch`, `.accountRemove`) for the person's own harness.
