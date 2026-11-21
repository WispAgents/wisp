---
title: The .wisp folder
description: What a repo's .wisp/ folder holds, and how each task in it links to the code it wrote.
sidebar:
  order: 6
---

```text
.wisp/
  wisp.yml                     the folder's own settings (history tracking)
  *.wisp.yml                   your setup: agents, crew, skills, workflows… (formulas)
  lock.yml                     the modules' pinned versions
  modules/                     the modules, fetched by wisp init
  tasks/
    2026/11/<id>-<slug>.task.yml   one tracked task: what was asked, its agent, its commits, its files
    2026/11/<id>-<slug>.report.md  what the agent answered
  history/<id>.jsonl.zst       the task's full transcript, scrubbed and compressed
```

## One file per task

Each tracked task is its own file, named by its id, so two branches never conflict on the same file. The files are
small text you can read and review in a PR: "this PR was written by these three tasks".

## How much is kept

A tracked task keeps its spec (what was asked, with which agent and base commit), its report and its transcript. The
transcript is scrubbed of secrets, `.env` values and private paths before it's written, and compressed (a typical
one is tens of KB). `wisp track <id> --level spec|report|full` chooses how much; `wisp untrack <id>` takes it out.

## Linked to the code

When Ship it commits a task's work, each commit carries a trailer:

```text
Wisp-Task: 01a1bc
```

The task file lists its commits and its PR. So you can go both ways:

- **Code to task**: `wisp blame <file>:<line>` names the task that wrote a line, what it was asked and its PR.
- **Task to code**: `wisp log` lists the tasks that built the repo, `wisp tasks --touching <path>` the ones that
  touched a path.

## Carry on

`wisp resume <id>` starts a new task from where an old one stopped, on today's code: its spec, its report and a
summary of its transcript become the new task's context. Anyone with the repo can do it, not only whoever ran the
task. `wisp rerun <id>` instead replays a task from the commit it started on.
