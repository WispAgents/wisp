# ADR-0079: Long threads: a task's Notes, the note before each follow-up, the Bash output offload and the thread's health

- Status: accepted
- Date: 2026-11-17
- Source: owner, 2026-11-15 and 2026-11-17 ("make sure you implement the long thread that we discussed"); the plan in
  `docs/v2-long-threads.md`; naming in `docs/v2-plan.md` §3 *Long threads naming* (D-0050). v1's intent:
  `~/Downloads/wisp-long-threads` (`server/services/threads.js`, `server/threads.js`, `server/offload-hook.mjs`,
  `web/js/threads.js`, `test/threads.test.js`). Follows ADR-0003 (plugins), ADR-0004 (one write path), ADR-0033 (the
  `.wisp/` repo history), ADR-0040 (meet users where they work).

## Context

A long agent thread gets worse as it goes: earlier facts and replaced instructions stay in the context, long tool
output pushes the useful part out, and compaction drops the *why* and what already failed. The fix in the research
(wisp task `d8ee0a207d52`): **the conversation is disposable, the task's state is durable.** v1 is building it; v2
ports the intent, in a plugin, with v1's bugs fixed (`docs/v1-bugs-fixed.md`).

## Decision

### 1. `wisp.threads`, a plugin (`plugins/threads/{api,core,host}`)

- **api** (`wisp-threads-api`): the types (ts-rs, `bindings/threads/`) and four commands, each an MCP tool through
  `wisp mcp` (registered commands are) and in the agent's capability set (`agent:session`: `threads:read`,
  `threads:write`):
  - `threads.notes {id}` → `Notes` (read): every section in order, its text, rev, who wrote it and when; the Notes'
    rev, the rev the agent last read (`seen`), and `lead` when the task is a subagent (see 4).
  - `threads.notesUpdate {id, section, text, by?}` (write): **rewrites one section whole, never appends.** `by` is
    `agent` or `you` (default `you`: the UI and the CLI). Unknown section: `invalid`, naming the real ones. The
    same text again writes nothing (no new rev).
  - `threads.revisions {id, section?}` (read): every version kept, newest first, with who wrote it.
  - `threads.health {id}` (read): see 6.
- **core** (`wisp-threads-core`, sans-IO, pure): the sections (`notes.rs`), the note (`note.rs`), health
  (`health.rs`) and the offload's rewrite and summary (`offload.rs`).
- **host** (`wisp-threads`): tables, commands, the points it contributes to, the repo file, clean-up on delete.

### 2. The Notes

Seven sections, fixed: **goal** (what done looks like) and **state** (where it stands, what is next) are one text;
**constraints**, **decisions** (with why), **failed** (what was tried and why it didn't work), **open** (questions)
and **files** are lists, one item a line. A list is kept clean on write: bullets and numbers stripped, repeats
dropped (case and punctuation ignored), the newest 20 items kept, each item at most 400 characters (a one-text
section 1 200).

**Tables** (forward-only, owner `threads`, ADR-0004): `threads_notes` (one row per task and section: text, rev, by,
at), `threads_revisions` (every version), `threads_meta` (the Notes' rev, the rev the agent last read, the task's
turns when they were last written, its turns now), `threads_health` (the last health). Every row is on its task's
own topic, `session:<id>` (as wisp.goal's check records), so whoever has the task open gets them with it.

**Who wrote it, per section.** v1 kept one `by` for the whole ledger, so an agent's write after yours hid that you
had changed something. v2 keeps `by` per section, and "you changed them since the agent last read them" is: the
Notes' rev is past `seen` and a section you wrote is newer than it.

**The repo file.** A task tracked in its repo (ADR-0033, `ext["wisp.history"].tracked`) also gets
`.wisp/tasks/<id>/notes.md` on each change, written through `history.store@1` (no file written behind the store's
back: the history plugin owns the folder). The Notes travel with the code and with `wisp blame`.

### 3. The agent's tool, `wisp_notes` (`mcp.tool@1`, audience agent)

Call it with nothing to read the Notes; with `set: {section: text, …}` to rewrite sections (a list may be sent as an
array; `""` clears one). It answers with what changed, the rev, and the Notes as they are now. On a task's first
turn the agent is told how to keep them (v1 protocol.js), with the tool's name as its harness spells it.

### 4. Subagents read their lead's Notes

A task with a `parent_id` (a subagent) reads its lead's Notes (`threads.notes` answers them, with `lead` set) and
**can't write them** (`invalid`: "say in your report what should change"): fan-out shares one set of decisions and
no worker rewrites them. Each of its runs starts with its lead's goal, constraints, decisions and what failed
(`<lead-notes>`, at most 4 000 characters).

### 5. The note before each follow-up (`runtime.prompt@1`, a new `before`)

`PromptPart` gains an additive async `before(TurnNote { session, turn, tools }) -> String` (default empty). The
runtime awaits each contribution for at most 2 s (a slow plugin never holds a run up; it is left out and logged),
never on a compaction turn, and puts what they say in front of the user's words (so it is at the end of the
conversation and the prompt cache before it still holds). Built-ins get no private API: any plugin can add a note.

wisp.threads' note, only for a run with wisp's tools (`tools` is the prefix its harness gives them, e.g.
`mcp__wisp__`):

- `<task-notes rev="N">`: the Notes as they are now (at most 6 000 characters, cut with a pointer to the tool), said
  to be newer than anything earlier in the conversation; "the user edited them since your last turn" when they did.
  The rev read is kept as `seen`.
- No Notes yet: on turns 2, 6, 10… a one-line nudge to write them.
- **Supersession:** when your message takes something back (24 phrases such as "actually", "instead", "scratch
  that", "never mind", "no longer", "anymore", matched on word boundaries in its first 2 000 characters, or a
  message opening with "No,", "Wait:" or "Stop."), a nudge to rewrite
  that section whole so the old version is gone. Pure code, no model call.

### 6. Health, worked out at each turn's end

On `turn.ended`, from runtime's own reads (`runtime.turns`, the last 80; `runtime.activity` of the last three
working turns): no model call, no new event. Counted from the last fresh thread (a turn whose
`ext["wisp.threads"].fresh` is true; Fresh thread is the next job):

| Signal | Heavy (1) / Worn (2) |
|---|---|
| context share of the last turn that reported it | ≥ 60 % / ≥ 80 % |
| compactions | 2 / 3+ |
| working turns in this thread | 30 / 60+ |
| the same error in the last three turns (signature: lower case, paths, hashes and numbers taken out) | 3× → 2 |
| the last three turns all failed (error, or a blocked/partial hero) | 1 |
| edits undone (an edit's before/after equal an earlier edit's after/before on the same file) | 2+ → 1 |

A score of 1–2 is `heavy`, 3+ is `worn`. The reasons are listed in words ("context 85% full", "compacted 3×"),
plus "Notes not updated in N turns" when they are 8+ turns old in a thread of 8+ turns. Saved as the task's
`threads_health` row (on `session:<id>`) and answered by `threads.health` (worked out on the spot if never saved).

### 7. The Bash output offload: `wisp hook offload`

Claude Code shows only the first part of a long command output, where failures never are. wisp.threads tells every
run where its long output goes (`runtime.run@1` env: `WISP_OFFLOAD_DIR` = `<wisp home>/threads/out/<task>`;
`WISP_OFFLOAD_MIN`, default 8 000 bytes, at least 2 000). The env names are in `wisp-runtime-api::points`, so any
harness plugin can act on them; no new "command hooks" point is needed.

- **plugins/claude** adds a `PreToolUse` hook on `Bash` to the run's one `--settings` (beside the gate's ask rules)
  when the run has the folder and isn't a lean side run: `"<this wisp binary>" hook offload`, 10 s timeout.
- **`wisp hook offload`** (Rust, in the wisp binary, before the async runtime starts: no Node) reads the hook's
  input and answers `updatedInput` with the command wrapped: it runs in bash with its output to
  `<folder>/out-<time>-<pid>-<n>.txt` and its own exit status kept; on exit **`wisp hook offload-show <file> <min>`**
  prints it whole and removes the file when short, else the first 1 500 bytes, up to 25 lines that look like errors
  (with their line numbers; "0 failed" is not one), the last 3 500 bytes, and the path to grep the rest from. Cuts
  never split a UTF-8 character. If the summary step fails, the output is shown whole (`cat`).
- **Not for** background commands (`run_in_background`, a trailing `&`, `nohup`, `disown`), empty commands,
  non-Bash tools, a command already wrapped, or a run without the folder. Any error: no answer, so the command runs as
  it is.
- The folder goes with the task (`session.deleted`), as do its Notes, versions, meta and health rows.

Other harnesses: Codex, opencode and the rest get the env too; each harness plugin adds its own hook when its CLI has
one (a later job, listed per harness in the plan).

### Where it reaches (ADR-0040)

- **Inside wisp:** the Thread tab (UI: the next job, after the owner picks a mock, D-0056; variants in
  `docs/v2-long-threads.md`).
- **The agent's harness:** `wisp_notes` (MCP, any harness wisp runs or a wisp-init'ed repo), the note before each
  follow-up, the offload hook (Claude Code now).
- **CLI:** `wisp notes [<task>] [--set section=text …] [--json]`; no task: the shell's `WISP_TASK`. "wisp notes on
  the refactor" stays a new task (it routes only on a task id, `--set` or nothing).
- **git:** `.wisp/tasks/<id>/notes.md` in a tracked repo.
- **TUI:** the health line (later). **Formulas:** seeding a task's goal and constraints (later, ADR-0026 kinds).

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| A section is rewritten whole, never appended; each version kept with who wrote it | `plugins/threads/host/tests/notes.rs` `a_section_is_rewritten_whole_with_each_version_kept_and_who_wrote_it` |
| A list loses bullets, numbers and repeats; the same text again is not a new rev; an unknown section names the real ones | same test, and `core/src/notes.rs` `a_list_section_keeps_items_one_a_line_without_marks_or_repeats` |
| A subagent reads its lead's Notes, can't write them, and its run starts with the lead's decisions | `a_subagent_reads_its_leads_notes_and_cant_change_them` |
| The note: the Notes now, your edit since the agent read them (said once), the take-back nudge; nothing for a run without wisp's tools; first-turn guidance | `the_note_before_a_follow_up_carries_the_notes_your_edit_and_the_take_back_nudge` |
| Any take-back phrase in any case anywhere in the first 2 000 characters is a supersession; ordinary words are not | `core/tests/note.rs` `any_take_back_phrase_anywhere_in_any_case_is_a_supersession`, `messages_made_of_ordinary_words_are_not` (seeded generators) |
| The note's parts: rev, the user's edit, the no-Notes nudge's cadence | `core/tests/note.rs` `the_note_carries_the_notes_what_you_changed_and_the_nudge` |
| Health: ok → heavy → worn from context, compactions, turns and the same error | `core/tests/health.rs` `ok_then_heavy_then_worn_from_context_compactions_turns_and_the_same_error_again`; host `health_counts_the_threads_context_and_compactions` |
| A fresh thread starts the count again; old Notes say so | `a_fresh_thread_starts_the_count_again_and_old_notes_say_so` |
| Undone edits and three failed turns count | `edits_the_agent_undid_and_three_failed_turns_count` |
| An error's signature ignores numbers, paths and hashes | `an_errors_words_without_numbers_paths_or_hashes_are_its_signature` |
| Background, empty and already-wrapped commands run as they are; only foreground Bash is answered | `core/tests/offload.rs` `background_commands_empty_ones_and_wrapped_ones_run_as_they_are`, `the_hook_answers_only_for_bash_in_the_foreground` |
| Long output shows its start, its error lines (not "0 failed") and its end, cut between UTF-8 characters | `long_output_shows_its_start_its_errors_and_its_end` |
| The rewrite runs in real bash and keeps the command's own exit status | `the_rewrite_runs_in_bash_with_the_commands_own_status` |
| Short output is shown whole and its file removed; long output's file stays; a missing file shows nothing | `crates/wisp/src/offload_cli.rs` `short_output_is_shown_whole_and_its_file_removed_long_output_stays` |
| Claude gets one `--settings` with the hook beside the gate's rules; none for a lean run or without the folder | `plugins/claude/host/src/launch.rs` `a_run_with_an_offload_folder_gets_the_bash_hook` |
| The folder is under wisp's home, and a deleted task leaves no file and no row | `a_run_gets_its_offload_folder_and_a_deleted_task_leaves_nothing` |
| Writing Notes needs `threads:write` (default deny) | `writing_notes_needs_the_permission` |
| `wisp notes` parses a task, `--set` pairs and `WISP_TASK`; renders markdown or "No notes yet." | `crates/wisp/src/notes_cli.rs` `notes_takes_a_task_and_set_pairs_or_the_shells_task`, `the_notes_render_as_markdown_or_say_there_are_none` |

## Consequences

- Each follow-up of a run with wisp's tools carries up to ~6 KB more prompt; it sits after the cached part.
- Every Bash call in a Claude run starts `wisp` once more (the fast path: no runtime, no config read).
- Next jobs: the Thread tab UI (after the owner's pick), Fresh thread (`threads.fresh`, setting the `fresh` flag),
  the compaction audit (`defaults.compactAudit`), offload hooks for the other harnesses, formulas seeding Notes, the
  TUI health line.
