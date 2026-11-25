# ADR-0123: A side discussion has its main task's access, and its writes to the main task's files are proposals there

- Status: accepted
- Date: 2026-11-25
- Source: the side and fork audit, round 1 (items `user-side-full-access`, `user-side-edits-for-approval`,
  `user-fork-full-access`).

## Context

A side discussion (`/side`, `ext["wisp.runtime"].sideOf`) is a task of its own that works in its main task's folder.
Until now its access came from its harness, not from the task it talks over:

- on Claude Code (`HarnessInfo::side_gate`) it ran in Edits behind wisp's gate, and every write **and every command**
  went to `mcp__wisp__approve` as an Allow / Deny card on the side task;
- on every other harness it ran in Plan, read-only: `runtime.approval` answered `SIDE_DENIED`, `approveBlocked`
  refused Allow on it, and Codex ran it with `sandbox_mode=read-only`;
- `runtime.create` gave it only the main task's folder, harness and model: none of its account, thinking, connectors
  or team, so its first run had other MCP servers than the main task's.

The prompt it got (`side_note`) described a clash check ("the user sees a card here") that nothing wrote: no code
made a `clash` ask, and `side_kept` had no caller. A fork had the same gap: `runtime.fork` built its task from
`Session::default()` and kept only `forkOf`, `capped` and `next`.

## Decision

1. **A side runs with its main task's access.** Its mode is the main task's, never below Edits (`side_task::side_mode`),
   Full auto when the main task runs in Full auto, on every harness; it is decided before the run is made, so
   `McpQuery.mode` is the mode it runs in. `plan::runs_commands` is true for it. Codex runs it `workspace-write` with
   the network on (`HarnessRun::side`, `sandbox_workspace_write.network_access=true`), or bypassed in Full auto; a
   channel's cap still wins. `SIDE_DENIED` and `approveBlocked`'s "A side discussion can't allow this" are gone.
2. **It starts with its main task's setup.** `runtime.create` (`create.rs` `side_of_main`) copies the main task's
   account and thinking (on the same harness), the connectors its spec names, its formula's connector overlay, its
   subagents and its cap into the new side's slot. The client's `sideStart` sends nothing more than before.
3. **Only its file writes go to wisp first**, where the harness can stop a write until wisp answers. On Claude Code a
   side's `--settings` asks only about `Edit`, `MultiEdit`, `Write` and `NotebookEdit`; `Bash`, `WebFetch` and
   `WebSearch` are allowed outright, and the approve tool is passed in any mode. A harness says what a tool call writes
   through a new, additive extension: `Harness::file_write(tool, input) -> Option<FileWrite>` (the path and a whole-file
   text, a list of replacements, or an opaque change).
4. **A write to a file the main task changed is a proposal on the main task.** `runtime.approval` on a side
   (`more/propose.rs`) answers with no card: a call that writes no file, or writes one the main task's turns never
   changed, is allowed; one to a file they changed is not made. Its old text (as the file reads now) and new text go
   on the **main** task's slot as `proposals: [{ id, side, sideTitle, path, file, before, after, at }]`, the side's CLI
   reads "proposed to the main task", and the side's turn gets a row saying so. A `.env` file or an opaque change is
   refused with words instead (its text never goes into the row every client syncs, ADR-0050).
5. **The user answers it on the main task.** Two new commands, `runtime.proposal.apply` and `runtime.proposal.discard`
   (`ProposalArgs { id, proposal }`, `uiOnly`, denied to `agent:session`): Apply writes `after` only while the file
   still reads as `before` (else a Conflict, nothing written), drops the proposal, and leaves a note
   (`sideApplied`) the main task's agent reads ahead of its next prompt; Discard drops it. Either way the side's turn
   gets a row. The task page draws each pending proposal as a card (`frame/Proposals.tsx`, its own lazy chunk) with
   the kit's Diff, which side it came from (a button that opens it), Apply and Discard.
6. **A fork keeps its task's setup.** `runtime.fork` copies the parent's whole `wisp.runtime` slot except a named
   deny-list of per-run and per-turn keys (`NOT_FORKED`: `next`, `btw`, `ask`, `blocked`, `grants`, `forLater`,
   `readOnly`, `sideOf`, `proposals`, a compare's `group`/`trial`, a crew job's marks…), the router's slot, and the
   session's `lean` and tags; then `forkOf` and `next = { fork: true }` on top. Its first run has the parent's MCP
   servers and `McpQuery.team`.

## Where it reaches (ADR-0040)

Inside wisp: the side's task page (no more cards for its commands) and the main task's page (the proposal cards). From
the user's harness: a side started with `wisp` or `wisp_launch` runs the same way, and its CLI reads the proposal as
the tool call's answer. `runtime.proposal.*` are `uiOnly`: a proposal is the user's to answer on the main task, never
an agent's (`agent:session` denies both).

## Consequences

- A side on Codex, Copilot, opencode and the rest can now change files: their writes land as they are (nothing can
  stop a write there first), and `side_note` tells such a side so, asking it to leave the main task's files alone.
- The changed set is the main task's own turns' changes; a file it changed only by hand (no turn saw it) is written
  by the side as any other file.
- `side_kept` and both `SIDE_DENIED` texts are deleted. `side_note(of, tool, proposes)` says exactly what happens.
- Tests: `plugins/runtime/host/tests/side_access.rs`, `tests/fork.rs`, `plugins/claude/host/src/launch.rs`
  (`a_side_discussion_asks_wisp_about_its_file_writes_only`), `plugins/codex/host/src/launch.rs`,
  `plugins/runtime/core/tests/plan.rs`, `plugins/runtime/host/src/plugin/create.rs`,
  `packages/shell/test/proposals.test.tsx`.
