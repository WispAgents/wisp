---
title: Share skills, AGENTS.md and MCP servers with your team
description: Give every agent on your team the same skills, rules and MCP servers, in one repo or across many.
sidebar:
  order: 2
---

Your team wants every agent, on every harness, on every laptop, to know the same things: how you test, how you
name things, which MCP servers to use. wisp keeps that in your repo, so it's reviewed like code and everyone gets
it with `git pull`.

## Rules everyone follows: AGENTS.md

Claude Code, Codex, Cursor and the others already read `AGENTS.md` (Claude Code reads `CLAUDE.md`; point it at
`AGENTS.md` with one line, `@AGENTS.md`). Keep your house rules there and commit it. `wisp init` adds a short
note about the `.wisp/` folder to it.

For rules only some agents follow, give the agent profile standing **instructions**:

```yaml
agents:
  reviewer: { harness: codex, mode: plan, instructions: "Review only. Never change files. Correctness first." }
```

## Skills

A **skill** is a how-to an agent can pick up when it needs it: "how we write migrations", "how to release". Write
it as a markdown file (or a folder with a `SKILL.md`), then declare it and give it to the agents that need it.

```yaml
# .wisp/skills.wisp.yml
skills:
  migrations: { description: How we write and test database migrations, path: docs/skills/migrations.md }
  release: { description: How to cut a release, path: .wisp/skills/release }
agents:
  builder: { harness: claude, skills: [migrations, release] }
  helper: { harness: codex, skills: [migrations] }
```

A relative `path` is the repo's (the folder `.wisp/` is in). The `description` is what an agent reads to decide
whether to use a skill, so say what it's for. Each task records which skills it had and which it used.

## MCP servers

Declare an MCP server once as a **connector**. Every agent that should have it gets it, on every harness.

```yaml
# .wisp/connectors.wisp.yml
connectors:
  linear: { url: "https://mcp.linear.app/sse", oauth: true }
  sentry:
    url: "https://mcp.sentry.dev/mcp"
    headers: { Authorization: "Bearer ${secret:SENTRY_TOKEN}" }
  postgres:
    command: npx
    args: ["-y", "@modelcontextprotocol/server-postgres", "${secret:DB_URL}"]
    agents: [builder]          # only these agent profiles get it
    modes: [plan, edits]       # and only in these modes
```

**Secrets never go in the file.** `${secret:NAME}` names one; each person sets the value on their own computer:

```sh
wisp secret set SENTRY_TOKEN            # typed, not shown
printf %s "$DB_URL" | wisp secret set DB_URL
```

Until it's set, `wisp validate` says `secret SENTRY_TOKEN isn't set here (wisp secret set SENTRY_TOKEN)`. For an
`oauth: true` server, each person signs in once in Settings → Connectors; the token stays in their secrets file.

Already have MCP servers in Claude Code's or Codex's own config? Settings → Connectors finds them and imports them
in one click; `wisp export connectors` then writes them out as a formula to commit.

## Many repos: modules

Put the shared setup in its own repo (`acme/wisp-setup`) and use it everywhere as a module:

<!-- example: unchecked -->
```yaml
# .wisp/team.wisp.yml in each repo
modules:
  team: { source: "git::https://github.com/acme/wisp-setup//team?ref=v3" }
```

`wisp init` fetches it and pins it in `.wisp/lock.yml`. Moving to `?ref=v4` is one pull request per repo, reviewed
like any other change. See [Formulas](../../../concepts/formulas/).

## Without git: share through wisp Cloud

```sh
wisp share skill.migrations             # a link: the skill and what it references
wisp share connector.linear --access people:ana,ben
wisp open <link>                        # the other side: see what it adds, then import it
```

A shared setup never carries a secret: it names the ones it needs, and whoever imports it sets their own.
