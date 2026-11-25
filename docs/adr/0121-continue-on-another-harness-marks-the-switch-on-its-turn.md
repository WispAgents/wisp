# ADR-0121: Continue on… hands a task to another harness through one follow-up, and its turn carries the switch

- Status: accepted
- Date: 2026-11-25
- Source: the harness audit, round 2 (item `user-handoff`): a task moved to another harness only by picking one in
  the reply box or pressing a refused or limited turn's offer; nothing showed what went along, how big it was, or,
  afterwards, where in the Timeline the harness changed.

## Context

A follow-up that names another harness already switches the task (`run.rs` `apply_opts`, `handoff::carry_over`) and
writes the turns the new harness missed ahead of the prompt (`handoff_note`, v1 handoff.js). That note is all the
missed turns up to a 24k-character budget. A user handing over a long task can't see it, can't keep it small, and the
Timeline shows the new harness's name on the turn with nothing marking the change.

## Decision

1. **One way to hand a task on.** Continue on… (the task's menu, the palette's This task, ⇧C on a task, `/continue …`
   in the reply box, and the refused and usage-limit cards' harness buttons) opens one dialog
   (`shell-task.continue`): the shared runner picker (`RunnerPicker`, the same component as New task and the reply
   box), then the preview, then Continue. Confirming sends **one** `runtime.followUp` with `run: { agent, model }` and
   the new `handoff`; it is the existing switch path (no `runtime.setAgent` first, which would leave the turn with no
   switch to mark). Cancel sends nothing.
2. **What carries over is a choice.** `FollowUpArgs.handoff` (additive): `summary` writes out only the latest turn the
   harness missed (its report and files) with the user's rules (What still holds), counting the rest; `full` is the
   existing note. Absent means `full`, so every older caller behaves as before. The dialog's default is `summary`.
3. **The preview is built by the hand-off's own code.** `runtime.handoffPreview { id, agent }` (read-only, `uiOnly`:
   an agent hands a task on with `runtime_followUp`) runs the same `handoff_note` for both choices over the task's
   turns as the hand-off would see them, and answers the latest hero summary, the files its reports list, how many
   turns the harness missed, how many rules go along, and each choice's size (turns written out, tokens at 4
   characters a token). The note is never sent to the UI, only its size: it may quote work the user hasn't opened.
4. **The turn carries the switch.** When a turn starts on another harness than the task's last, the runtime writes
   `turn.ext["wisp.runtime"].switch = { from: { agent, model }, to: { agent, model }, context: "summary" | "full" }`
   on that turn (with `fresh` when both apply). It is the turn's own record, written in the same transaction as the
   turn; no new event. `with_handoff` reads `context` from it, so the note sent is the one previewed.
5. **The Timeline draws it.** A new slot, `shell.turn:head` (over each turn, its key as the id), takes shell-task's
   divider: "Switched from Claude Code (Opus 5.5) to Codex · context: summary". The kit's `Divider` takes a `label`.

## Consequences

- A hand-off is visible before (its size and contents) and after (the divider), and can be kept small.
- The refused and limit cards no longer send at once: they open the dialog on the harness and model they offered,
  so a hand-off always shows what it carries.
- Places it reaches (ADR-0040): the app (task menu, palette, key, reply box, the turn cards); an agent or a user's
  harness through `runtime_followUp`'s `run` and `handoff`; the switch mark is on the turn every peer and
  `runtime.turns` reader already gets.
