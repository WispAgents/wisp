# ADR-0091: Ask near the limit before a send is made; a refusal says why

- Status: accepted
- Date: 2026-11-16
- Source: owner, 2026-11-16 ("near the limit every send fails", "send anyway doesn't work"); docs/qa/bugs.md B4; v1
  features/hold.js, limit-hold.js, launcher.js (the 409 `hold`).

## Context

v1 asked before a send went: the page checked the rule from the harness list and showed a row in the box (*Claude
Code's weekly window is at 91%… Start anyway · Wait until the reset · ×*), the draft staying; the server answered 409
with `hold` when the page couldn't tell. v2 asked only when the turn started (`before:run`), after the task existed, so
a send near the limit made a task that showed *Failed/Blocked* with words meant for agents (*Send it again with
confirmLimit: true*), and a held run's *Start now* ran the guard again without your yes, so it was held again.

## Decision

1. **The server asks before anything is made.** `wisp.budget` intercepts `runtime.create` and `runtime.followUp`
   (`BeforeCreate`, `BeforeFollowUp`) for a send you make (not an automation, a subagent, an answer to the task's
   question, or Auto: its pick is made in the first turn, where `before:run` still asks). A send past your line is
   refused with the veto reason `budget_hold`; no task or turn is made. `confirmLimit` (Start anyway, the CLI's
   `--yes`) goes and moves that window's line up a step until its reset, as before.
2. **`budget.ask`** answers the hold's numbers (window, label, used, line, where this send would take it, reset) for
   the row.
3. **A refusal carries its reason.** The protocol's `reject` gets an optional `reason` (additive; golden updated): a
   veto's interceptor code. `/api/cmd`'s error JSON has it as `error.reason`. The client's `CommandError.reason`.
4. **The prompt boxes ask first.** A plugin UI declares `sendHolds: [{ reason, check }]` (the shell's
   `ShellContext.sendHolds`); New task and the reply box ask every check before they send (Send shows pending
   meanwhile) and show the first hold in the kit's `HoldRow` (v1's .l-hold under New task's bar, .c-hold over the reply
   box's text), keys as v1 (Enter goes, not in its first 300 ms; ⌥Enter waits; Esc keeps editing). A send the server
   refuses with a reason a check answers gets the same row, not a toast. Budget's check loads on the first send.
5. **Start now is a yes.** `runtime.held` / `runtime.release` by a person (caller set `user…`) passes `confirmLimit`;
   wisp releasing it at the reset doesn't. The started turn is the held turn itself (same id, same place), started
   straight away, never written to the queue first, so the page updates the card in place.

## Places it reaches (ADR-0040)

The app (New task, reply box, held cards), the CLI and MCP (the refusal's words say how to go ahead: `--yes`,
`confirmLimit`), the API (`error.reason`, `budget.ask`).

## Edge cases and their tests

- A send past the line makes no task; the same with `confirmLimit` runs; a follow-up is asked too and makes no turn;
  `budget.ask` gives the row's numbers — `crates/wisp/tests/serve_budget.rs`
  `a_run_that_would_take_the_window_past_your_line_is_asked_about_and_a_yes_goes`.
- Start now on a held automation: runs as the same turn, one turn, not held again — same test.
- The reject's reason on the wire — `crates/wisp-protocol/tests/goldens.rs`, `crates/wisp-sync/tests/server.rs`.
- The row's keys and places — `packages/ui/test/hold-row.test.tsx`; its sentence as v1's —
  `plugins/budget/ui/test/hold.test.ts`.
- Seen in the app frame by frame (desktop, light and dark): Send pending in the first frame, the row at ~60 ms, no task,
  no toast, the box kept; Esc; Start anyway runs; a follow-up's row; Start now on a held job: the card becomes the
  running turn in the first frame, no gap.

## Left

Wait until the reset in the reply box (its Start later never reads the time it picks: B14); the server's own ask for
Auto before the task is made (B15).
