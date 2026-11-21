---
title: Tasks
description: What a task is, its turns, its modes, its worktree, its answer, and how it ends.
sidebar:
  order: 2
---

A **task** is one piece of work for one agent. It has a prompt, a project (the folder it works in), an agent and
a model, and a mode. It runs in **turns**: your first message is the first turn, each follow-up is another.

## Modes

| Mode | What the agent may do |
| --- | --- |
| Plan | Read and run commands to look around. No file edits. |
| Edits | Change files. Anything else asks you first. |
| Full auto | The whole job, without stopping to ask. |

Change the mode for the next turn from the reply box (Shift+Tab), or `wisp reply <id> --mode edits "…"`.

## The worktree

With **Worktree** on (the default, Settings → Worktrees), a task gets its own git worktree and branch, made from
your project's base branch. Your checkout is never touched, and several tasks can change the same repo at once.
wisp cleans up idle worktrees after a while (7 days by default), but never one that holds changes or commits that
exist nowhere else.

## The answer

An agent wisp runs ends its turn with a short structured report: what it did, the diff, the tests it ran, a live
preview, questions for you, suggested next steps. wisp draws each part as a card
([answer blocks](../../reference/answer-blocks/)), so you read the result, not the whole conversation. The full
transcript is one click away (*Transcript*).

## While it works

- **Follow up** at any time. By default a follow-up interrupts the turn; with *Queue* on, it waits.
- **Ask on the side** (`wisp btw <id> "…"`) without interrupting.
- **Stop** ends the turn. Queued follow-ups stay.
- A task that needs you (a question, a permission, a usage limit) moves to **Needs you** and, if you set it up,
  tells your phone.

## Goals

Give a task a **goal** ("all tests pass") and wisp keeps sending it back after each turn until judges agree the
goal is met, or it runs out of rounds and asks you. Judges can be other agents and harnesses. See
[Keep going until it's done](../../guides/run/goals/).

## How it ends

**Ship it** commits, pushes and opens a pull request; **Merge** squash-merges it. **Mark done** closes the task.
In a repo with a `.wisp/` folder, a finished task can be [saved to the repo](../wisp-folder/).
