---
title: Your repo's .wisp folder
description: Give your repo a .wisp/ folder so its agent setup and the tasks that built it live next to the code.
sidebar:
  order: 3
---

`wisp init` gives your repo a `.wisp/` folder. It holds two things:

- **Your agent setup, as code**: which agents, crew, skills, MCP servers, rules and workflows this repo uses
  (`*.wisp.yml` files). See [Formulas](../../concepts/formulas/).
- **The tasks that built the code**: one file per task, with what was asked, what the agent answered, its
  transcript and the commits it made. See [The .wisp folder](../../concepts/wisp-folder/).

```sh
cd your-repo
wisp init
git add .wisp && git commit -m "chore: wisp setup"
```

`wisp init` writes `.wisp/wisp.yml`, adds a short note to `AGENTS.md` (and `CLAUDE.md` when there is one) so
agents know about the folder, and fetches and locks any modules the setup uses.

## Which tasks the repo keeps

`.wisp/wisp.yml` says it:

```yaml
wisp: 1
history: { track: ask }   # auto | ask | manual
```

- `ask` (the default): when a task in this repo is shipped or marked done, its answer offers **Save to repo**.
- `auto`: every such task is saved.
- `manual`: nothing is saved unless you save it, with **Save to repo** or `wisp track <id>`.

A saved task is written to `.wisp/tasks/` for you to commit, scrubbed of secrets and private paths. Or press
**Push** beside what was saved (⋯ → **Push to repo** later): on a task whose work isn't shipped yet, the files are
committed on its branch and pushed; on one already merged, they go out as a pull request from `wisp/history-<id>`,
without touching your checkout. Commits that
Ship it makes carry a `Wisp-Task:` trailer, so the code and the task point at each other.

## Use it

```sh
wisp log                      # the tasks that built this repo, newest first
wisp log src/auth             # the ones that touched a path
wisp blame src/auth/login.ts:42   # which task wrote that line, and what it was asked
wisp resume <id>              # a new task that carries on from where that one stopped
```

Agents can read it too. With `history: { track: ask, inject: true }`, a new task starts with the past tasks on the
files its prompt names (the five most relevant), so it knows the decisions already made there.

## Next

- [Track agent work beside your code](../../guides/track/track-agent-work/)
- [Write your setup as code](../../guides/declare/software-factory-as-code/)
