---
title: Schedules and triggers
description: Run tasks on a schedule, on events (CI, issues, PRs, chat, webhooks) or after other tasks.
sidebar:
  order: 3
---

A **trigger** starts work when something happens: a time, a webhook, a CI failure, a labeled issue, a task that
finished. It can start a task with its own prompt, run a declared task or workflow, wake a crew member, or follow up
the task the event came from. All the events are in the [reference](../../../reference/triggers/).

In the app: your avatar → Automations. Or declare them:

```yaml
triggers:
  weekday-triage:
    on: schedule
    cron: "every weekday at 9"
    tz: Europe/Zurich
    prompt: "Look at the issues opened since yesterday and label them."

  tests-on-change:
    on: files.changed
    debounce: 2m
    prompt: "Run the tests for what changed and report failures only."

  after-done:
    on: task.done
    if: "event.payload.project == 'api'"
    prompt: "Write release notes for {event.summary}"
```

## Keep it in bounds

| Field | What it does |
| --- | --- |
| `concurrency` | `skip` (default), `queue`, `replace` or `parallel:N` when it fires while a run is going |
| `rate` | at most this many runs: `5/h`, `20/d` |
| `debounce` | events within this window become one job |
| `cooldown` | after a failed run, wait this long |
| `when` | only these days and hours; outside them `skip` or `queue` |
| `budget` | `{ perRun: 5, perDay: 40 }` in dollars |
| `approval` | the job waits for your yes before it starts |

## Webhooks

```yaml
triggers:
  sentry:
    on: webhook
    header: Sentry-Hook-Signature
    secret: "${secret:SENTRY_HOOK}"
    prompt: "A Sentry alert: {event.summary}. Find the cause; don't fix yet."
```

Each webhook trigger has its own URL (Automations shows it). With `header`, the call must be signed with the secret,
which is never sent, only checked.

## Scheduled tasks

A declared task can carry its own schedule:

```yaml
tasks:
  deps:
    prompt: Update the dependencies with patch releases and run the tests
    schedule: "every day at 2"
    worktree: true
```

`wisp run deps` runs it by hand too. Workflows chain several tasks: see
[Build a software factory as code](../../declare/software-factory-as-code/).
