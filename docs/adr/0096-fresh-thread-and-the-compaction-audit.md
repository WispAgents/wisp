# ADR-0096: Fresh thread and the compaction audit

- Status: accepted
- Date: 2026-11-18
- Source: owner, 2026-11-17 (long threads, the next steps after ADR-0079); the plan in `docs/v2-long-threads.md`.
  Follows ADR-0079 (long threads: Notes, the note, health), ADR-0003 (no private API) and ADR-0040. v1's intent:
  `~/Downloads/wisp-long-threads` (`server/runner.js` `fresh`, `probe`; `server/services/threads.js` `audit`;
  `web/js/threads.js` `freshBrief`).

## Context

ADR-0079 keeps a task's state outside its conversation. Two things were left:

1. When the thread is worn, start a **new conversation on the same task** from that state, without losing the task,
   its folder, its history or the link to the old conversation.
2. After a **compaction**, know what the compacted conversation lost of the goal, constraints, decisions and failures,
   and tell the agent.

## Decision

### 1. Fresh thread

- **`threads.freshBrief {id}`** (read) makes the brief: `# Fresh thread`, the Notes one heading level down (else
  what was asked first, at most 3 000 characters), the branch and the files the task's turns changed (each once, the
  latest 40), and the last two finished turns (request and answer, 3 000 characters each). At most 24 000 characters.
  It writes nothing; `diff` lists the files.
- **`threads.fresh {id, brief?}`** (write) starts it: your edited brief (trimmed; empty is refused), or the brief as
  made now. A subagent's thread is its lead's (refused).
- **Runtime does the restart, through its public command.** `runtime.followUp` gains an additive `fresh: bool`. Such
  a follow-up:
  - is refused while a turn runs, while follow-ups wait, or while a question waits on your answer (it would answer
    the question with the brief); a CLI only lingering after its result is let go, as for any follow-up;
  - clears the task's `resume_id`, so the turn resumes nothing (`fresh_start`);
  - marks its turn `ext["wisp.runtime"] = { fresh: true, after: <the conversation it left> }`: the old conversation
    stays linked, on the turn where the new one starts;
  - hears no hand-off note: `with_handoff` reads only the turns from the last fresh one on, so even a task that
    switched harnesses earlier doesn't get the old turns ahead of the brief. The turn after resumes the new
    conversation.
- wisp.threads then marks the turn as its own too (`ext["wisp.threads"].fresh`, `runtime.turn.setExt`) and counts
  the Notes as read (the brief has them). **Health** counts from the last turn either plugin marked fresh. **The note**
  on a fresh turn is the first-turn guidance only (no `<task-notes>`: the brief has them).
- **CLI:** `wisp fresh [<task>] [--print] [-f brief.md | -f -] [--json]`: `--print` shows the brief to edit, `-f`
  starts from your file (or stdin), neither starts from the brief as made. Bare: the shell's `WISP_TASK`. It routes
  only on a task id or nothing ("wisp fresh look at the parser" stays a task). **MCP:** both commands (registered).

### 2. The compaction audit (setting `threads.compactAudit`, off by default)

- **The signal.** A turn's stats gain an additive `compactions` (`TurnStats.compactions`, from a harness's
  `stats.compactions`): how many times the conversation compacted during the turn. **Claude Code** counts each
  main-thread `compact_boundary` (a `/compact`, and the CLI compacting on its own mid-turn); a subagent's boundary
  isn't counted. The other harnesses (Codex, opencode, Cursor, Copilot, Grok, Antigravity) report no compaction in
  their parsers or recorded runs, so they never trigger it; each adds the count when its CLI says it. Gate G6 drops
  the new key from the five recorded compaction runs (`meta.drop`) and checks everything else as v1 parsed it.
- **Health** counts a turn's `compactions` beside `/compact` turns (a `/compact` turn counts once).
- **The probe.** `runtime.probe {id, question, timeoutMs?}` (new, additive): the question asked of a **fork** of the
  task's conversation (as /btw), answering only (`lean`: no tools, no MCP; plan mode), awaited (default 120 s, at
  most 300 s), and kept nowhere: no turn, no /btw entry. A harness that can't fork, or a task with no conversation:
  `conflict`. Any plugin can use it.
- **The audit.** At a turn's end, when the setting is on and the turn finished with `compactions > 0`: in the
  background (the plugin's scope), one at a time per task, the Notes' goal, constraints, decisions and failures
  become items (`Label: item`, at most 24); the probe asks v1's recall question; each item is **kept or lost by a
  word match** (core `audit::score`: kept when 60 % of its content words, three letters or more, common words out,
  plurals and possessives folded, are in the answer, in any order or case). **No model call**, unlike v1's second
  run: it costs nothing past the probe, gives the same answer every time and needs no free model. It can't tell a
  contradiction from a repeat; the note says the items "may" have been dropped and that they still hold.
- **Stored per compaction** in `threads_audit` (migration 2, `<task>|<turn>`, on `session:<id>`): `status`
  (`running` → `done` or `skipped` with `why`), `kept`, `total`, `lost`, `told`. `threads.audits {id}` lists them
  newest first; a `running` one older than 10 minutes (a restart cut it off) reads as skipped, "It didn't finish."
  Removed with the task.
- **The next follow-up** carries `<compaction-lost>` with the lost items, once (then `told`); not on a fresh turn.

### Where it reaches (ADR-0040)

MCP (`threads.freshBrief`, `threads.fresh`, `threads.audits`, `runtime.probe` as registered commands), the CLI
(`wisp fresh`), the agent's note, the Thread tab (being built in `plugins/threads/ui` from this api: the brief to
edit, Fresh thread, "Kept N of M" on a compaction's turn), Settings (the switch).

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| The brief has the Notes one level down, the files once, the last two turns; writes nothing | `plugins/threads/host/tests/fresh.rs` `the_brief_has_the_notes_the_files_changed_and_the_last_turns`; core `tests/fresh.rs` `the_brief_has_the_notes_the_work_and_the_last_turns` |
| No Notes: what was asked first; the brief and each part capped, cut between characters | core `without_notes_it_starts_from_what_was_asked_first_and_it_is_capped` |
| Your edited brief goes as written; none: the one made now; empty refused; a subagent refused | `fresh_starts_a_new_conversation_from_the_brief_and_marks_its_turn` |
| A fresh turn resumes nothing, hears no hand-off even after a harness switch, links the old conversation; the next resumes the new one | `plugins/runtime/host/tests/fresh.rs` `a_fresh_follow_up_starts_a_new_conversation_linked_to_the_old_one` |
| Refused while a turn runs; nothing queued | `a_fresh_follow_up_is_refused_while_a_turn_runs` |
| A fresh turn's note is the guidance only; health's compactions start again from it | `a_fresh_turn_hears_how_to_keep_the_notes_and_health_counts_from_it`; core `compactions_inside_a_turn_count_as_compact_turns_do` |
| Claude Code's boundaries are counted per turn; none when nothing compacted; v1's goldens still match | `plugins/claude/core/tests/compactions.rs` `a_turns_compactions_are_counted_in_its_stats`; `g6.rs` |
| The probe asks a fork, keeps no turn and no /btw; a harness that can't fork is refused | `a_probe_asks_a_copy_of_the_conversation_and_keeps_nothing` |
| The audit is off by default; a turn that didn't compact isn't audited; on: kept N of M, lost items in the next note once; a refused probe is skipped with why | `the_audit_is_off_by_default_and_on_says_what_the_compaction_lost_once` |
| An audit a restart cut off (`running` past 10 minutes) reads as skipped, "It didn’t finish." | `plugins/threads/host/src/audit.rs` `an_audit_a_restart_cut_off_reads_as_skipped` |
| The scorer: an item repeated in any order and case is kept; never mentioned or an empty answer: lost; items are the four sections, capped | core `tests/audit.rs` (seeded generators) |

## Consequences

- A fresh thread costs nothing extra; the audit costs one short forked run per compaction, only when switched on.
- The word match will sometimes call a reworded fact lost; the note is a reminder, not a correction.
- Next: the Thread tab (ui-task), the other harnesses' compaction counts and offload hooks, formulas seeding Notes.
