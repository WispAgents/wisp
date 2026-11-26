# ADR-0135: Plan is enforced read-only for every harness, not by the prompt

- Status: accepted
- Date: 2026-11-26
- Source: Plan audit (user-plan-readonly, opencode-plan-bash-writes).

## Context

- Plan that runs commands (v1 #794, `HarnessRun::plan_commands`, `wisp_runtime_core::plan::runs_commands`) launched
  Claude Code with `--permission-mode bypassPermissions` and only its file-editing tools off. Nothing but the Plan
  note kept a `sed -i`, a `> file`, a `git commit` or a `rm` out of the project, the added folders or `$HOME`.
- opencode's Plan was `--agent plan` alone, over the user's own permission rules: an inline `"edit": "allow"`, a
  `"*": "allow"` or a grant let it edit, and its shell ran anything. `plan_commands` never reached it, so a trigger's
  or a channel's Plan task had the same shell as yours.
- ADR-0113 and ADR-0115 counted a Plan task among the agents "without a shell" ("Claude Code's Plan runs no
  command"). Since #794 that's wrong: Plan *runs* commands. What holds is that Plan **cannot write**, and only once
  this ADR's rules are in.

## Decision

1. **Claude Code: an OS sandbox, else its own plan mode.** A Plan run that runs commands carries, in its one
   `--settings`, `sandbox = { enabled, autoAllowBashIfSandboxed, allowUnsandboxedCommands: false, filesystem:
   { denyWrite: [cwd, every --add-dir] } }`: its shell writes only to /tmp (and the run's `{tmp}`); the project, the
   added folders and `$HOME` refuse a write ("operation not permitted"), and no command retries unsandboxed. It keeps
   `bypassPermissions` with the edit tools off, and gets `--strict-mcp-config` (once; untrusted and lean runs have it
   already): only the run's own MCP servers start, never a project's. The Plan note says the writes fail by design and
   not to work around them.
2. **Sandbox availability is an input** (`HarnessRun::plan_sandbox`), set by wisp.claude from the platform: macOS
   (Seatbelt) yes; Linux only with `bwrap` and `socat` on PATH; Windows no. The launch stays plain data. Without it,
   Plan is Claude Code's own `--permission-mode plan` and the agent isn't told its commands run: never an unsandboxed
   bypass. A grid box has no bubblewrap (its container has no user namespaces), so a grid Plan task is the native plan
   mode.
3. **Codex: `sandbox_mode = "read-only"`** in Plan, as it was.
4. **opencode: permission rules that win over the user's.** Plan always has an inline config. After the user's rules
   and the grants, `edit` is `deny`, and `bash` is `"*": "deny"` with allow rules for reads only (`git status/log/
   diff/show`, `ls`, `cat`, `rg`, `grep`, `find`, `pwd`, `head`, `tail`, `wc`), each narrowed again by deny rules for
   a redirect, a pipe, a chain, a command or process substitution (`$(`, a backtick, `<(`, `>(`) and for the flags
   that write or run a program (`find -delete/-exec`, `--output` and `--ext-diff` on `git diff/log/show`, `rg --pre`). opencode takes the last rule that fits and the keys go out sorted, so each deny
   comes after the allow it narrows. Without `plan_commands` (a trigger's, a channel's or a capped task), `bash` is
   `deny`. Edits and Full auto are unchanged.
5. **Wording.** Where ADR-0113 and ADR-0115 say a Plan task runs no command, read "Plan cannot write": its commands
   run, sandboxed or read-only. Those ADRs aren't edited (merged ADRs never are); this one corrects them.

## Consequences

- A Plan agent that tries a write sees it fail; builds and tests that write only under /tmp still run. A build that
  writes into the project (a `target/` or `node_modules/` there) fails in Plan: that's a write.
- On Linux without bubblewrap and socat, and on Windows, Plan can't run commands with side effects (Claude Code's own
  plan mode). Installing them turns Plan-that-runs-commands on.
- opencode's Plan shell is narrower than Claude's: no pipes, no chains, only the listed reads.
- The rules ride on `OPENCODE_CONFIG_CONTENT`, which opencode deep-merges over the global and the project
  `opencode.json`. Neither the order nor the keys a project file's own `permission.bash` or `agent.plan.permission`
  keeps after that merge is checked here: under last-match, a project rule placed after wisp's could still let a
  write through. Follow-up: verify the merge order on opencode, or hand Plan a wisp-owned agent instead of `plan`.
- Reaches: every Plan task, in wisp and from where it was started (MCP, CLI, channels, triggers), on each harness.
