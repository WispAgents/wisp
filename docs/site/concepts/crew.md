---
title: Crew
description: Long-lived agents with a role, a brief, a notebook and a schedule, that you hand jobs to.
sidebar:
  order: 4
---

A **crew member** is an agent that stays: it has a name and a face, a role ("reviewer", "release manager"), a
**brief** (standing instructions), a **kit** (skills and notes), and a **notebook** it keeps between jobs. Each job
it does is a task of its own, so you can open it, follow up and ship it like any other.

## Giving it work

- **Hand it a job** from its page, or mention it: `@ada fix the flaky test` in the launcher, a reply box or a chat
  channel.
- **Delegate**: a task can hand work to crew members (its Team tab shows them).
- **Check-ins**: `cron: "every weekday at 9"` and it looks at its brief and decides what to do.
- **Wakes**: triggers can wake it on an event (`pr.review_requested`, a CI failure, a chat message). A crew a team
  shares in `.wisp/` wakes on each teammate's computer; `match: { assignees: me }` on an issue trigger keeps each
  issue to the teammate it's assigned to ([Shared with a team](../../guides/run/schedules-and-triggers/#shared-with-a-team)).

## Where it works

`cwd` is its folder and `machine` the computer its jobs run on. `sandbox: job` gives each job a worktree of its
own; `sandbox: own` gives the member one of its own. `parallel` says how many of its jobs may run at once, and
`paused` stops check-ins and jobs until you resume it.

```yaml
agents:
  reviewer: { harness: codex, effort: high, mode: plan }
crew:
  ada:
    role: reviewer
    agent: reviewer
    brief: "Review every PR the crew opens. Request changes, never merge."
    cron: "every weekday at 9"
    sandbox: job
```

## Sharing

A crew member can be shared as a file (`.crew.json`) or through wisp Cloud, with what it references. See
[Share skills, AGENTS.md and MCP servers](../../guides/declare/share-skills-and-mcps/).
