---
title: Call wisp and your crew from Slack, Linear and more
description: Start tasks, talk to crew members and answer questions from Slack, Discord, Telegram, email, GitHub, Linear and your phone.
sidebar:
  order: 5
---

You don't have to open wisp to use it. Message it from where your team already talks, hand it issues from your
tracker, and answer its questions from your phone. Every one of these starts or follows up a real task you can open
in wisp later.

| Where | What you can do |
| --- | --- |
| Slack, Discord, Telegram, email | Start a task, talk to a crew member, follow up, answer, stop. |
| GitHub comments | `/wisp …` on an issue or PR starts or follows up a task; the answer comes back as a comment. |
| Linear, Jira, GitHub issues | Start a task from an issue in New task (*Issue*), or have a label start one by itself. |
| Your phone | wisp's app or ntfy: questions and finished tasks, with buttons to answer, reply, mark done or stop. |
| Your own agent or editor | wisp's MCP server: see [Use wisp from your agent](../../../integrations/your-agent/). |

## Set up a chat channel

In the app: Settings → Phone, pick the channel and paste its token. Or declare it, so the whole team has the same
setup:

```yaml
channels:
  team-slack:
    transport: slack
    enabled: true
    token: "${secret:SLACK_BOT_TOKEN}"
    allow: [ana, ben]          # who may talk to it
    cap: edits                 # the most a message may start (never Full auto)
notifications:
  slack-news: { to: team-slack, on: [finished, asks, failed] }
```

Then `wisp secret set SLACK_BOT_TOKEN`. A channel is off until you switch it on, and it only answers people it
allows.

## Talk to it

Write the way you would in New task. Options go at the front and end at a colon:

```text
fix the flaky login test
@ada review PR 42
in api on studio with opus high plan: why is the cache slow?
```

- `@ada` hands the job to the crew member Ada.
- `in api` picks the project, `on studio` the machine, `with opus high` the model and effort, `plan` the mode.
- Reply in the thread to follow up. Buttons under a question answer it.

Commands: `/help`, `/projects`, `/models`, `/crew`, `/machines`, `/status`, `/new`, `/stop`, `/done`, `/btw` (ask
on the side without interrupting).

## From GitHub

Switch on GitHub comments in Settings → Phone. Then, on an issue or a pull request in a repo you have open in wisp:

```text
/wisp fix this
/wisp @ada review this PR
```

Only you can start work this way; anyone else is ignored. On a public repo, the comment wisp posts back is a short
fixed line unless you allow full reports.

## From Linear, Jira or GitHub issues

**By hand:** New task → *Issue* lists your open issues; picking one fills the prompt.

**By itself:** a trigger starts a task, or wakes a crew member, when an issue gets a label:

```yaml
triggers:
  agent-label:
    on: issue.labeled
    label: agent
    crew: bob
    prompt: "Build {event.summary}"
```

A webhook works too, from any tool that signs its calls. Linear signs with the `Linear-Signature` header:

```yaml
triggers:
  from-linear:
    on: webhook
    header: Linear-Signature
    secret: "${secret:LINEAR_WEBHOOK_SECRET}"
    prompt: "Triage this Linear event: {event.summary}"
```

## On your phone

wisp's phone app (or [ntfy](https://ntfy.sh)) tells you when a task finishes, asks you something, or goes silent while
it spends your plan, with buttons to answer, reply, mark it done or stop it. Set it up in Settings → Phone.
