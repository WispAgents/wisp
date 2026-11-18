# ADR-0097: Long threads in the other harnesses, in formulas and in the TUI

- Status: accepted
- Date: 2026-11-18
- Source: owner, 2026-11-18 ("finish long threads"): the leftovers of ADR-0079 and ADR-0096. Follows ADR-0026 (wisp
  as code), ADR-0027 (the task spec), ADR-0024 (the TUI mirrors the GUI), ADR-0040.

## Context

ADR-0079 sent long Bash output to a file through Claude Code's `PreToolUse` hook; ADR-0096 counted compactions from
Claude Code's stream. The other harnesses, formulas and the TUI had neither. Each harness was checked against its
CLI's own docs, its `--help` where it is installed here (all but Copilot) and wisp's recorded runs (`plugins/<harness>/core/tests/fixtures`).

## Decision

### 1. Per harness

| Harness | Rewrite a command (offload) | Say it compacted | What wisp does |
|---|---|---|---|
| Claude Code | `PreToolUse` `updatedInput`, per run in `--settings` | `compact_boundary` in its stream | ADR-0079, ADR-0096 |
| **Codex** | `PreToolUse` on `Bash` with `permissionDecision: "allow"` + `updatedInput.command` (developers.openai.com/codex/hooks); per run as `-c hooks.PreToolUse=[…]` (checked: the shape loads, a wrong one is refused) | `PostCompact` hook; nothing in `exec --json` (no compaction event in the recorded runs) | **Both, in Full auto only** (below) |
| opencode | a JS plugin (`tool.execute.before`) could go in `OPENCODE_CONFIG_CONTENT` | no compaction event in `run --format json` or the recorded runs | **Skipped**: the plugin loader changed across versions (`plugin` vs `plugins`, directories only) and can't be checked without a run; a later job, with a recorded run |
| Cursor (`cursor-agent`) | `beforeShellExecution` in `hooks.json` (user or project file only) | none | **Skipped**: no per-run hooks (wisp writes no file of the user's or the repo's), and the CLI's support for them is unsettled |
| Copilot CLI | `preToolUse` with `modifiedArgs` (docs.github.com, CLI hooks reference) | `preCompact` (a notice) | **Skipped**: hooks load from `~/.copilot/hooks`, `.github/hooks` or settings files only; `COPILOT_HOME` would move its whole home (sign-in included) |
| Grok Build | `PreToolUse` in `~/.grok/hooks/*.json` or the project's | none found | **Skipped**: file-only hooks, and project hooks need trust |
| Antigravity (`agy`) | `PreToolUse` in `.agents/hooks.json` / `~/.gemini/config/hooks.json`: allow, deny or ask, no rewrite documented | none in its stream | **Skipped**: no rewrite, file-only |

**Codex, in Full auto only.** A hook given on the command line runs only with `--dangerously-bypass-hook-trust`,
which also runs every other enabled hook untrusted, a cloned repo's `.codex/hooks.json` included, outside the
sandbox. In Full auto the run already bypasses approvals and the sandbox (`--dangerously-bypass-approvals-and-sandbox`),
so it adds nothing; in plan and edits it would. So a Full auto run that names its offload folder (`WISP_OFFLOAD_DIR`)
and isn't lean gets:

- `-c hooks.PreToolUse=[{matcher="Bash",hooks=[{type="command",command="'<wisp>' hook offload --codex",timeout=10}]}]`:
  `wisp hook offload --codex` answers in Codex's shape (the allow beside the rewrite; Claude Code's answer has no
  decision, which would skip its own ask). The rewrite runs in the user's shell: tested in bash, zsh and sh.
- `-c hooks.PostCompact=[{hooks=[{type="command",command="'<wisp>' hook compacted",timeout=10}]}]`: `wisp hook
  compacted` adds a line to `<offload folder>/compactions`.
- `--dangerously-bypass-hook-trust`.

**A compaction a hook counted.** At each turn's end wisp.threads reads and removes `compactions` in the task's
offload folder; a count is kept on the turn (`ext["wisp.threads"].compactions`, beside what the slot had), and health
and the compaction audit read it where `stats.compactions` is missing. Any harness with a compaction hook can use it.

### 2. Formulas seed a task's Notes

A task in a formula (`tasks:` in `wisp.yml`, a `-f x.task.yml`, a rerun of its spec) may declare
`notes: { goal, constraints, decisions, failed, state, open, files }` (`wisp_threads_api::SeedNotes`: one text for
goal and state, lists for the rest; an unknown section is refused by `wisp validate`, naming it). Runtime carries it
opaque, as it carries `goal`: `StartArgs.notes` and `TaskSpec.notes` (additive), so a rerun and a shared spec start
with them too. When the task is made (`session.created`) wisp.threads reads its spec (`runtime.spec`) and writes each
section as yours (`by: you`), once: a task that has Notes keeps them. The tracked repo's `notes.md` follows.

### 3. The TUI

Mirrors the GUI's Thread tab. The task's header says how worn its thread is when it isn't Fine, in the GUI's words
(`Worn · context 85% full · 3 compactions`, the same thresholds as its `healthFacts`); `t` opens the **Thread** side
pane (also in `p`'s cycle): that line and the Notes, section by section; `t` again closes it. It reads the
`threads_notes` and `threads_health` rows already on the task's topic; nothing is asked for. (`n` stays "deny a
tool".)

### 4. `wisp notes` and `wisp fresh` end to end

`crates/wisp/tests/serve_threads.rs` runs them on a real `wisp serve` with the fake agent.

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| Full auto with the folder gets both hooks and the bypass; plan, edits, lean and no folder get none | `plugins/codex/core/src/argv.rs` `full_auto_gets_the_long_thread_hooks_and_nothing_else_does`; `plugins/codex/host/src/launch.rs` `a_full_auto_run_with_an_offload_folder_gets_the_hooks` |
| Codex's answer is an allow with the rewrite; Claude's has no decision | `crates/wisp/src/offload_cli.rs` `the_hook_rewrites_a_foreground_bash_command_and_leaves_the_rest` |
| The rewrite keeps a command's output and exit status in zsh and sh as in bash | `plugins/threads/core/tests/offload.rs` `the_rewrite_runs_in_zsh_and_sh_too` |
| Each `wisp hook compacted` adds a line; wisp.threads counts it once, keeps it beside the turn's slot, and audits it | `offload_cli.rs` `each_compaction_adds_a_line_wisp_threads_counts`; `plugins/threads/host/tests/fresh.rs` `compactions_a_hook_counted_are_kept_on_the_turn_and_audited` |
| A spec's notes are written as yours, items trimmed, blanks dropped; written once; none: nothing | `fresh.rs` `a_task_made_with_notes_in_its_spec_starts_with_them` |
| `wisp validate` refuses an unknown section; a declared task's run starts with its Notes; its spec keeps them | `crates/wisp/tests/serve_threads.rs` `a_formulas_notes_then_wisp_notes_and_wisp_fresh` |
| `wisp notes --set`, `wisp fresh --print`, `wisp fresh` (no `--resume`, the brief alone), the next reply resumes | the same test |
| The header's line says what the GUI says, nothing when Fine; `t` opens and closes the Thread pane | `crates/wisp/src/tui/thread.rs` `the_health_line_says_what_the_gui_says_and_nothing_when_fine`; `tui/tests.rs` `the_thread_pane_shows_the_notes_and_the_header_how_worn_it_is` |

## Consequences

- Codex runs in plan and edits keep their long Bash output in the conversation, and their compactions stay uncounted,
  until Codex can trust a hook passed per run. opencode, Cursor, Copilot, Grok and Antigravity: the same until their
  CLIs take a per-run rewriting hook (listed above to recheck).
- `automations-api` depends on `threads-api` (an api on an api: `TaskDecl.notes` is `SeedNotes`).
