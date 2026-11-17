---
title: Your first task
description: Start a task, watch it work, follow up, and ship what it wrote as a pull request.
sidebar:
  order: 2
---

A **task** is one piece of work you hand to an agent: "fix the flaky login test", "add a dark mode toggle". wisp
gives it its own copy of your repo (a git worktree), runs the agent there, and shows you what it did.

## Start it

In the app, press **New task**, pick the project, type what you want, and press Enter. From a terminal, in your repo:

```sh
wisp "fix the flaky login test"
```

Under the box you can choose:

- **The agent and model.** Claude Code, Codex, Cursor and the rest, each with its own models and effort.
  *Auto* lets wisp pick for you.
- **The mode.** *Plan* only reads and proposes. *Edits* changes files and asks before anything else. *Full auto*
  does the whole job without stopping.
- **Worktree.** On by default: the task works in its own folder and branch, so your checkout stays as it was and
  several tasks can run at once.

## Watch it

The task opens beside the list. While it works you see what it's doing right now. When it's done you get its
answer as cards: what changed, the diff, the tests it ran, screenshots, what you might do next.

From a terminal:

```sh
wisp ls                 # every task and its state
wisp attach <id>        # follow one live
wisp show <id>          # its latest answer as markdown
```

## Follow up

Type in the reply box under the answer. If the answer suggested follow-ups, the first shows faded in the box:
press **Tab** to use it. A follow-up sent while the task works interrupts it, or waits its turn if you switch
*Queue* on.

```sh
wisp reply <id> "also cover the logout path"
```

## Ship it

**Ship it** (or `wisp ship <id>`) commits the task's changes, pushes its branch and opens a pull request. *Write
it with <agent>* has the task's agent write the PR title and description from the diff, following your repo's
pull request template. wisp then follows the PR's checks and reviews on the task.

When you're happy, **Mark done**. `wisp merge <id>` squash-merges the PR and updates your local branch.

## Next

- [Give your repo a `.wisp/` folder](../your-repo/), so every task is kept next to the code it wrote.
- [Run on more than one computer](../../guides/run/machines/).
