# ADR-0133: Find in a task searches its step rows, and the server searches its stubbed turns, masked

- Status: accepted
- Date: 2026-11-26
- Source: UI audit round 4 (R4-3, with R4-5's per-turn open state).

## Context

- A task page had no find. The browser's own (⌘F) can't do the job: a step's output and a folded cluster's steps
  are drawn only when opened (Timeline renders detail and group children only when expanded), and the turn window
  (`turn_window`, v1 #788) sends every turn older than the latest 8 as a stub with no events.
- What the page shows of a step that read a .env file is masked (ADR-0050); a find must never match the value it
  hides.

## Decision

1. **mod+f is wisp's** (`BUILT_IN_KEYS`, so a user plugin can't bind it). With a task open it opens Find in task, a bar
   over the turns (`FindBar` in the kit, `TaskFind` in the shell, its own chunk, loaded on first use); with none
   open the browser's find runs as before. A phone reaches it from the task's menu (Find in task).
2. **The loaded turns are searched in the client, in their step rows** (`task-find.ts`): the rows the transcripts are
   drawn from, case-insensitive, over a step's label, whole title, command, output and a message's text, never the
   DOM. An output is read as the step shows it (`stepOutput`): masked unless that step was revealed.
3. **The stubbed turns are searched on the server: `runtime.searchTask`** `{id, query, turnIds?, limit?}` →
   `{hits: [{turnId, seq, snippet}], more?}`, read-only, `uiOnly` (an agent reads a task with `wisp_session`). It
   reads those turns' stored steps, masks them as the wire does (`env_mask::mask_steps_known` with what the task read,
   when Hide .env values is on) and only then matches (`runtime-core::step_find`, pure). At most 200 hits. Only the
   named task's turns are read, whatever ids are named (the capability is checked against the task).
4. **Going to a hit** opens its turn (the ledger's, asked whole through `turnOpener` when a stub; its server hit is
   replaced by the turn's own once it lands), turns the transcript on, opens its cluster and step through the turn's
   open store (`step-open.ts`, R4-5), scrolls it into view and marks the words with the CSS Custom Highlight API
   (`wisp-find`, `wisp-find-current`), a ring on the step where the API is missing. Nothing in the page is rewritten.

## Consequences

- One new wire command (additive): its args and answer are ts-rs bindings; older servers answer `not_found`, and Find
  then counts the loaded turns only.
- Reaches: the task page (desktop and phone). Agents and the CLI already read whole tasks (`wisp_session`, `wisp tasks`).
