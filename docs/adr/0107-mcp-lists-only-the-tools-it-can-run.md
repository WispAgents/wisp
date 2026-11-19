# ADR-0107: wisp's MCP server lists only the generated tools it can run

- Status: accepted
- Date: 2026-11-19
- Source: orchestration audit B #4 (`docs/qa/audit-B-orchestration.md`): `/api/mcp` listed 446 tools and a run's MCP
  279; about 250 of them were refused at call time ("wisp.mcp doesn't declare goal:set in its manifest"). Changes
  ADR-0040's edge case `every_command_is_an_mcp_tool`; the rest of ADR-0040 stands.

## Context

- ADR-0040 lists one generated tool per catalogue command, filtered by the **caller's** capability set only.
- A tool call runs through wisp.mcp's code, so the kernel also checks wisp.mcp's **own** set (its manifest's
  permissions, ADR-0003 capability routing). Its manifest grants `connectors crew events:tail/why later machines plugins
  runtime ship stuff telemetry:track watch`; every tool of the other 26 plugin groups (goal, automations, git, history,
  settings, review, worktrees…) was listed and then refused, so an agent was offered tools that can never work.
- v1's MCP lists only tools that run.

## Decision

1. A generated tool is listed only when **both** sets allow its command: the caller's (as before) and wisp.mcp's own
   (`Ctx::may(command.permission)`, `plugins/mcp/host/src/lib.rs` `extras_for`). Listed ⇒ callable.
2. Reaching a plugin's commands from a harness is a grant in `plugins/mcp/host/wisp-plugin.json`, added on purpose,
   one plugin group at a time (this ADR adds `threads:read` and `threads:write` for the agent's `wisp_notes`, audit B #3).
   The caller's set still narrows every call.

## Consequences

- `/api/mcp` no longer offers goal, automations, git, history, settings, review, worktrees and the other ungranted
  groups' commands. Giving one to the user's harness (ADR-0040) is now a manifest grant plus its test, not automatic.
- Tests: `crates/wisp/tests/serve_mcp_callable.rs` `every_listed_tool_is_one_wisp_mcp_may_run`, and
  `crates/wisp/tests/serve_setup.rs` `the_catalogue_is_wisps_mcp_tools` (every command both sets allow is a tool).
