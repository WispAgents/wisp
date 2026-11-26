---
title: Use wisp from your agent or editor
description: Give Claude Code, Codex or any MCP client wisp's tools, so it can start tasks, read history and hand off work.
sidebar:
  order: 1
---

You don't have to leave the agent you already use. wisp serves its tools over MCP, so Claude Code, Codex, Cursor or
any MCP client can start wisp tasks, hand work to your crew, read past tasks and check wisp's state, without you
opening wisp.

## One command

```sh
wisp skill install --agent claude     # in your repo
wisp skill install --agent codex
```

For Claude Code this writes:

- `.mcp.json`: wisp's MCP server (`wisp mcp`) for this repo.
- `.claude/skills/wisp/SKILL.md`: when and how to use wisp.
- `.claude/settings.json`: a `SessionEnd` hook, so each session you end is handed to wisp
  ([kept in the repo](../../guides/track/track-agent-work/) if you track history).

For Codex it adds `[mcp_servers.wisp-history]` to `~/.codex/config.toml`. Codex has no end-of-session hook; bring sessions
in with `wisp adopt --all --since 1d`.

`wisp skill` prints the skill without installing it, to read first or use with another agent.

## Any MCP client

Point it at `wisp mcp` (stdio):

```json
{ "mcpServers": { "wisp": { "command": "wisp", "args": ["mcp"] } } }
```

## What it can do

From your agent (`wisp mcp`): start a task on any agent, model or machine (`wisp_launch`), list and read tasks
(`wisp_list`, `wisp_get`), wait for one (`wisp_wait`), follow up (`wisp_reply`), stop it (`wisp_stop`), and see
which agents are set up (`wisp_agents`).

An agent running *inside* a wisp task gets more: asking you a question (`ask_user`), attaching a PR (`wisp_pr`),
showing a live page beside the task (`wisp_watch`), the crew and connectors, and coming back later
(`resume_later`). All of them are in [the reference](../../reference/mcp-tools/).
