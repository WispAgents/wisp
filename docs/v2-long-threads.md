# Long threads in v2: plan

Source: wisp task `d8ee0a207d52` ("Long agent threads degrading"): the research (`long-agent-threads.md`, 8 tools
ranked) and its second turn, which is building them in **v1** in `~/Downloads/wisp-long-threads` (uncommitted on
2026-11-15: `server/services/threads.js`, `server/offload-hook.mjs`, `web/js/threads.js`, `web/js/features/thread.js`,
`design/long-threads.md`, changes in every harness adapter, the runner, `mcp-ask.js`, the panel and settings).

**Why:** a long thread degrades (context rot, multi-turn lock-in: −39 %, wrong facts and replaced instructions that
stay, tool-output bloat, compaction that drops the why and what failed). The fix: **the transcript is disposable,
the task's state is durable.**

**Rule for this plan:** port the v1 intent once v1's PR merges (docs/v1-sync.md), not before: v1 is still being
built and its names and behaviour may change. Until then: this plan, the ADR draft, and the UI design for the
owner's approval (§3: design first, owner approves, then build).

## What v1 is building (the intent to port)

| Piece | v1 | What it does |
|---|---|---|
| **Task ledger** | `web/js/threads.js` SECTIONS; `wisp_ledger` MCP tool; Thread tab | Goal · Constraints · Decisions (+why) · Tried and failed (+why) · Where it stands · Open questions · Key files. Sections are **rewritten, never appended**. Kept by the agent (`wisp_ledger`) and by you (the Thread tab). A subagent reads its lead's ledger. Revisions; "seen" = what the agent last read |
| **Note** | runner turn note | In front of each follow-up: the ledger as it is now (at the end of the conversation, so the prompt cache before it holds), a nudge when your message changes something said earlier (supersession), and what the last compaction lost |
| **Fresh thread** | `freshBrief`, Fresh thread action | A new conversation on the **same task** from a brief (ledger + recent diff) you can read and edit first; the old conversation stays linked |
| **Health** | `threads.js` health | How worn the thread is, from data wisp already has: context size, compactions, turns since fresh, the same error again and again, edits reverted. The Thread tab's meter; offers Fresh thread when worn. No model calls |
| **Compaction audit** | `services/threads.js` audit, `Runner.probe` | After a compaction, a throwaway copy is asked what it remembers of goal/constraints/decisions/failed; a cheap model says what was lost ("Kept 6 of 8"); what was lost goes in front of the next follow-up. Setting `defaults.compactAudit` |
| **Output offload** | `server/offload-hook.mjs`, Claude `PreToolUse` on Bash | Long command output goes to a file; the agent sees the start, the error-looking lines, the end and the path (Claude Code shows only the first 2 KB of a long output, where failures never are). Not for background commands. `WISP_OFFLOAD_MIN` (8000 bytes) |

Research items v1 doesn't build yet (keep as later): P7 merge check of subagent results against the ledger's
decisions, P8 the long-thread eval harness.

## v2 design (to confirm in an ADR)

**A plugin: `wisp.threads`** (`plugins/threads/{api,core,host,ui}`, Rust host + core, Solid UI), everything through
api packages, default deny.

- **Data:** its own tables (`threads_ledger`: task id, section, text, rev, by, at; `threads_audit`: task, turn,
  kept/lost). One write path, every row in `change`. A task tracked in a repo also writes the ledger to
  `.wisp/tasks/<id>/ledger.md` (ADR-0033), so it travels with the code and with `wisp blame`.
- **Commands** (each with its MCP exposure, ADR-0040): `threads.ledger` (read), `threads.ledgerUpdate(section, text)`
  (rewrite one section; the agent's `wisp_ledger` tool; the UI's edits), `threads.fresh(task, brief?)` (makes the
  brief, then `runtime` starts a new conversation on the same task with the old one linked), `threads.health`
  (a topic: per task, from runtime's turn events: tokens, tool-output share, compactions, turns since fresh,
  repeated error strings, reverted edits).
- **The note:** a runtime extension point that contributes text in front of a follow-up (check whether runtime
  already has one: `plugins/runtime/core/src/prompt/`; if not, add the extension point: built-ins get no private
  API). Supersession detection is pure core code (no model call) with property tests.
- **Compaction audit:** needs a harness event "compacted" through the harness api (Claude Code reports it; others:
  check each harness plugin) and a probe run (`runtime.quick`-style, throwaway, no tools). Off by a setting.
- **Output offload:** a harness extension point "command hooks"; `plugins/claude` installs a PreToolUse hook that
  runs **`wisp hook offload`** (Rust, in the wisp binary: no Node in v2). Other harnesses: whatever each supports
  (codex/opencode hooks or wrapper), listed per harness in the ADR. Files under the task's scratch, removed with it.
- **Subagents** read their lead's ledger (v1), so fan-out shares decisions.
- **Meet users where they work:** the `wisp_ledger` MCP tool works from any harness in a wisp-init'ed repo; CLI
  `wisp ledger <task>` (show / `--set section=…`), `wisp fresh <task>`; the TUI shows the health line.
- **Wisp as code:** a formula can seed a task's ledger (goal, constraints) declaratively.

**Naming (owner, 2026-11-15):** *Ledger* stays the task history (ADR-0068). The state file is **Notes**, shown in a
**Thread** tab; the tool is `wisp_notes` (v1's `wisp_ledger`), the CLI `wisp notes <task>`, the tables `threads_notes`,
the repo file `.wisp/tasks/<id>/notes.md`. Everywhere below, read "ledger" (v1's word) as Notes.

**UI (design first, owner approves):** a Thread area on the task page (desktop: a tab in the side panel as v1;
phones: in the task menu, platform look): the sections, each editable inline; the health meter (one line: "Worn ·
3 compactions · same error 4×"); Fresh thread (opens the brief to edit, then starts); the audit result on the
turn ("Kept 6 of 8"). A mock with 2 variants goes to the owner before any build.

## Order

1. **Now:** this plan; ADR draft (`docs/adr/0078-long-threads.md`, status Proposed): naming settled (Notes, Thread tab).
2. **When v1's PR merges:** read its final code and tests, list its bugs (port intent, not bugs), then:
   1. `wisp.threads` data + `wisp_ledger` (MCP) + CLI + the note (no UI yet) — the core.
   2. Output offload (`wisp hook offload`) in the Claude plugin, then others.
   3. Health topic.
   4. UI: the mock → owner's pick → build (desktop, then phones).
   5. Fresh thread.
   6. Compaction audit.
3. **Later (research P7, P8):** the merge check for subagents, and the long-thread eval harness in `spikes/`.

One coder at a time (§3), each round one of the steps above.
