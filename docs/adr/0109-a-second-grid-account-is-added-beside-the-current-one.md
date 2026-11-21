# ADR-0109: a second grid account is added beside the current one; signing in to put this computer on makes it current

- Status: accepted
- Date: 2026-11-19
- Supersedes: ADR-0061 §2's sentence "Signing in adds an account and makes it current" (the rest of ADR-0061 stands)
- Source: setup audit F (`docs/qa/audit-F-setup.md`) F12

## Context

ADR-0061 says signing in adds an account and makes it current. Since audit A (c0d655ad), `wisp.grid-client`'s
`signin::done` adds a second account beside the current one (`Accounts::add_beside`), so this computer stays on the
grid it was on; only the first account, or a sign-in made to put this computer on (Sign in and add it), becomes current.
The behaviour is the one users expect (adding an account to look at its shares must not move this computer to another
grid), but no ADR recorded it, and `done`'s own doc comment still said "made current". Audit F found the mismatch.

## Decision

1. **Signing in** adds the account (or renews it in place). It becomes current when there is no current account, or
   when the sign-in was made to put this computer on that grid (`add_computer`). Otherwise it is added beside the
   current one, and the user switches to it (Settings → Cloud, `wisp cloud use`).
2. Everything else in ADR-0061 stands: one current account, each token under its own key, `remove` making the first
   one left current.

## Consequences

- `wisp cloud signin` with an account already current leaves `wisp cloud accounts` showing the old one with `●`; the
  CLI and Settings say the new one was added.
- The code's comments say this rule (`signin.rs` `done`).

## Places it reaches (ADR-0040)

The app (Settings → Cloud), the CLI (`wisp cloud signin`, `wisp cloud accounts`, `wisp cloud use`), unchanged.

## Edge cases

- First sign-in with no account: current (test `grid-client` signin tests, unchanged).
- Sign in and add this computer while another account is current: the new one is current, this computer goes on it.
- Renewing the current account's sign-in: it stays current.
