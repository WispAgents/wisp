---
title: Agents and harnesses
description: Harnesses are the coding agent CLIs wisp runs; agent profiles are your named ways of running them.
sidebar:
  order: 3
---

## Harnesses

A **harness** is a coding agent's own command line: `claude`, `codex`, `opencode`, `cursor-agent`, `agy`, `grok`,
`copilot`. wisp doesn't replace it. It starts it in the task's folder, reads what it does, and shows you the
result. Your sign-in, your plan and your settings stay the harness's own.

Settings → Agents lists the harnesses wisp found, their version and models, and lets you:

- **Sign in** through the harness's own login, or add a key for one that takes keys.
- **Add another account** (Claude Code, Codex): wisp picks the account for each run and skips one that's at its
  usage limit.
- **Update** a harness when a newer version is out, the way you installed it.
- **Ask before running a changed harness**: wisp pins each harness's program and holds a run if it changed.

## Agent profiles

An **agent** in a formula is a named way of running a harness: which harness, model and effort, which mode, which
skills and MCP servers, and standing instructions.

```yaml
agents:
  reviewer: { harness: codex, effort: high, mode: plan, instructions: "Review for correctness first." }
  builder: { harness: claude, model: opus, skills: [testing], connectors: [linear] }
skills:
  testing: { description: How this repo tests, body: "Run the tests before you say it's done." }
connectors:
  linear: { url: "https://mcp.linear.app/sse", oauth: true }
```

A task, a crew member, a team seat or a goal judge names a profile (`agent: reviewer`) or a harness directly.

## Auto

Pick **Auto** and wisp's router chooses the harness and model for each task from what it is and what you have.
New task shows what Auto would pick before you send. Tune it with a `router:` in your formula.

## Teams

A **team** is a lead agent and the subagents it may hand work to: wisp's own delegates (each a task of its own,
on any harness) and the harness's own subagents (Claude Code's). The task's **Team** tab shows each one live: what
it's doing, then what it came back with.
