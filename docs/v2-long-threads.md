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

## Thread tab: the mock for the owner to pick (D-0056)

Nothing is built yet. Both variants use only kit components (`SidePanel` tabs, `Section`, `Field`/`TextArea`,
`Chip`, `Meter`, `Button`, `Sheet`, `Menu`) and v1's sizes. They read `threads.notes`, `threads.health` and
`threads.revisions` (all on the task's `session:<id>` topic, so no extra subscription) and write with
`threads.notesUpdate` (optimistic patch on the section row, rolled back with a toast on reject). Strings go through
`t()`. Health shows as **Fine**, **Getting long** or **Worn** (`ok`, `heavy`, `worn`).

**Variant A: a section list, as v1's Thread tab** (desktop: a fourth tab, *Thread*, in the task's side panel, after
Files; phone: *Thread* in the task menu opens a full-height sheet with the same content)

```
┌ Thread ─────────────────────────────── Worn ▮▮▮▯ ┐
│ context 85% full · compacted 3× · same error 4×   │   ← health line (Meter + reasons), hidden when Fine
│ [ Fresh thread ]  (next job; disabled until then) │
├───────────────────────────────────────────────────┤
│ Goal                                 agent · 2m   │   ← who wrote it last + when; hover: History (revisions)
│ Import CSV files into the ledger                  │   ← click to edit inline (TextArea), Enter saves, Esc cancels
│ Constraints                            you · 1h   │
│ • No new dependencies                             │
│ • Keep the public API                             │
│ Decisions · Tried and failed · Where it stands ·  │   ← empty sections fold into one "+ Add" row
│ Open questions · Key files                        │
└───────────────────────────────────────────────────┘
```

**Variant B: the Notes as one document, with a health card on top** (desktop: the same *Thread* tab; phone: the task
menu's *Thread* opens a page in the task's stack, platform back)

```
┌ Thread ───────────────────────────────────────────┐
│ ┌ Worn ──────────────────────────────── ⓘ ┐       │   ← one card: level chip, reasons as chips, Fresh thread
│ │ [85% context] [3 compactions] [error 4×] │       │
│ └──────────────────────────────────────────┘       │
│ Notes · rev 12 · updated by the agent 2m ago  [Edit]│
│ ## Goal                                            │   ← read view: the same markdown the agent gets
│ Import CSV files…                                  │
│ ## Constraints                                     │   ← Edit turns the whole document into fields per section;
│ - No new dependencies                              │     Save writes only the sections that changed
└────────────────────────────────────────────────────┘
```

Trade-offs: A edits one section in place and shows who wrote each (closest to v1, more rows on a phone). B shows
exactly what the agent reads and is calmer at a glance, but editing is a mode. **The lead's suggestion: A** (v1's
look, one-click edits, and "you wrote it" per section is the v1 bug this port fixes). Screenshots come with the build,
from the kit on a dev page, once a variant is picked.

## Order

1. **Done:** this plan; ADR-0079 (accepted, 2026-11-17).
2. **Step 1 (2026-11-17, done):** `wisp.threads` (`plugins/threads/{api,core,host}`): the Notes tables and
   revisions, `threads.notes` / `notesUpdate` / `revisions` / `health`, `wisp_notes` (MCP), `wisp notes` (CLI), the
   repo's `notes.md`, subagents read their lead's Notes, the note before each follow-up (`runtime.prompt@1`
   `before`), the offload (`wisp hook offload` + Claude's `PreToolUse`), the health row. v1's bugs avoided are in
   `docs/v1-bugs-fixed.md`.
3. **Next:**
   1. UI: the owner picks a mock above → build the Thread tab (desktop, then phones) in `plugins/threads/ui`.
   2. Fresh thread (`threads.fresh`, the editable brief, the `fresh` flag health counts from).
   3. Compaction audit (`defaults.compactAudit`).
   4. Offload hooks for the other harnesses; formulas seeding Notes; the TUI health line.
4. **Later (research P7, P8):** the merge check for subagents, and the long-thread eval harness in `spikes/`.

One coder at a time (§3), each round one of the steps above.
