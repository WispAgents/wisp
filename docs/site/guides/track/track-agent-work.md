---
title: Track your agents' work beside your code
description: Keep every task that wrote your code in the repo, find the task behind any line, and pick up where it left off.
sidebar:
  order: 1
---

When agents write most of a codebase, `git log` tells you *what* changed but not *what was asked*, *what the agent
was thinking* or *what it tried first*. wisp keeps that next to the code: each task that built the repo is a file in
`.wisp/tasks/`, linked to its commits.

## 1. Turn it on

```sh
cd your-repo
wisp init                 # .wisp/ with history tracked in "ask" mode
git add .wisp && git commit -m "chore: track agent work"
```

```yaml
# .wisp/wisp.yml
wisp: 1
history: { track: ask, inject: true }
```

- `track: ask` offers **Save to repo** on each task you ship or mark done; `auto` saves them all; `manual` only
  what you save yourself.
- `inject: true` starts each new task with the past tasks on the files its prompt names.

## 2. Save tasks

- In the app: **Save to repo** on the task's answer or its menu.
- From a terminal: `wisp track <id>` (`--level spec|report|full` for how much; full is the default).
- Old work too: a task from before the folder existed can be saved the same way.

The files land in `.wisp/tasks/` and `.wisp/history/` for you to commit with the code, or in the same PR. Everything
is scrubbed first: keys, tokens, `.env` values and private paths never reach the repo.

## 3. Link commits to tasks

Ship it already adds `Wisp-Task: <id>` to each commit it makes. For commits an agent makes itself:

```sh
wisp hook install         # a prepare-commit-msg hook: commits made in a task's run get the trailer
```

## 4. Bring in work done outside wisp

Ran Claude Code or Codex in a terminal? Bring those sessions in as tasks:

```sh
wisp adopt --all --since 7d --track     # the last week's sessions in this repo, saved
wisp skill install --agent claude       # from now on, Claude Code hands each session to wisp when it ends
```

`history: { adopt: ask | auto | off }` says what happens to a session that ends.

## 5. Use the history

```sh
wisp log                        # every tracked task, newest first
wisp log src/auth               # the tasks that touched a path
wisp log --agent codex --grep cache
wisp blame src/auth/login.ts:42 # the task behind that line: what was asked, its summary, its PR
wisp resume <id>                # carry on that task, on today's code
wisp rerun <id>                 # replay it from the commit it started on
```

In the app, the **Repo** view lists the same tasks, and each opens with its transcript.

## Why it's safe to commit

- **No merge conflicts**: one file per task, named by its id.
- **Small**: specs and reports are short text; transcripts are compressed.
- **Reviewable**: a PR shows which tasks wrote it, and anyone can read what was asked.
