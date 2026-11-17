---
title: Run a task and follow up
description: Start tasks from the app, the terminal or a script; follow up, queue, ask on the side, stop and ship.
sidebar:
  order: 1
---

## Start

| From | How |
| --- | --- |
| The app | **New task** (N), type, Enter. |
| A terminal | `wisp "add a dark mode toggle"` in the repo. |
| A script | `wisp "…" -d --json` returns the task's id at once; `--wait` blocks until it ends. |
| Several agents at once | `wisp "…" --compare claude,codex`: one task per agent on the same ask. |

Useful options: `--agent codex --model gpt-5 --effort high`, `--mode plan|edits|auto`, `--cwd DIR`, `--also DIR`
(a second folder the agent may use), `--no-worktree`, `--on studio` (another machine).

## Watch

```sh
wisp ls --state running,waiting
wisp attach <id>          # live: steps, answer, questions; then a reply box (Ctrl-C detaches)
wisp wait <id>            # block until it finishes or asks you something
wisp show <id>            # the latest answer as markdown
```

## Follow up

- **Reply** in the box under the answer, or `wisp reply <id> "…"`. Tab fills a suggested follow-up.
- **Queue**: with *Queue* on (or `wisp reply <id> --queue "…"`), a follow-up waits for the turn to end instead of
  interrupting it. Queued follow-ups can be reordered, edited, combined or sent now.
- **Ask on the side**: `wisp btw <id> "is this safe?"` answers without interrupting the run.
- **Switch agent or model** for the rest of the task: `wisp reply <id> --agent codex "…"`.
- **Later**: schedule a follow-up for a time (the clock button), or send one when a usage window resets.

## Stop, ship, finish

```sh
wisp stop <id>            # ends the turn; queued follow-ups stay
wisp ship <id> --draft    # commit, push, open a draft PR
wisp merge <id>           # squash-merge the PR and sync your local base branch
```

## Try two ways

New task's **+** next to the agent adds a second agent (or a second run of the same one with one thing changed,
like no skills). Each gets its own task and worktree, and **Compare** shows them side by side with what differed.
