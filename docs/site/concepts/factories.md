---
title: Factories
description: Run a task through stations (spec, build, checks, review, ship) instead of one agent, looped back to build until it passes.
sidebar:
  order: 9
---

A **factory** runs a task through a line of **stations** instead of handing it to one agent. Each station is its own
agent with its own prompt, and each hands its work to the next:

1. **Spec**: an agent writes the spec (what it must do, its constraints, how we'll know it's done, the plan) without
   changing a file. You approve it, change it in your own words, or stop.
2. **Build**: an agent builds it to the spec, in a worktree of its own. Give it two agents and both build it; the first
   whose checks pass goes on.
3. **Checks**: commands run in the build's folder (`npm test`, `npm run lint`). A gate is code, not an agent's word.
4. **Review**: one or more reviewers read the change and each ends with APPROVE or CHANGES. Their votes count as a
   goal's judges do: majority, all or any.
5. **Ship**: the PR opens from the build's worktree. You merge it, or it merges by itself if you say so.

A failed check or a review that asks for changes goes **back to build** with what failed, as the next round: the same
build agent gets a follow-up, so it keeps its context and its worktree. After `max` rounds (3 by default) the run asks
you whether to keep going, or stops if the factory says `then: stop`.

## One task, one row

A factory run is one task in the list. Its stations are its children, and a band at the top of the task shows each
station as a step: where it is ("Review · round 2 of 3"), what each station gave, and what waits on you. Questions
(approve the spec, merge the PR) are the task's own question card, so you answer them from the app, the phone, the
TUI, the CLI or a chat channel.

## `default` is always there

With no setup, `--factory` (or **Factory** in New task's picker) runs the built-in `default`: a spec you approve, the
build in its own worktree, the project's `test` command as the checks, one review, and a PR you merge, back to build
at most 3 rounds. Each station runs on the task's agent, or your default harness. Declare your own under `factories:`;
one named `default` replaces the built-in one.

```yaml
factories:
  careful:
    stations:
      spec:   { agent: planner, approve: human }
      build:  { agents: [claude, codex] }
      checks: { run: [npm test, npm run lint], timeout: 15 }
      review: { agents: [reviewer, critic], vote: all }
      ship:   { pr: true, merge: human }
    loop: { back_to: build, max: 3, then: ask }
    budget: { usd: 20 }
```

What each field takes is in the [resource kinds reference](../../reference/kinds/). How to run a task through one,
everywhere you work, is in [Run a task through a software factory](../../guides/run/software-factory/).

## What it builds on

A factory uses what wisp already does: each station is an ordinary task (budgets cap it, worktrees hold its changes),
the review counts votes as goals do (a task's goal is what every reviewer judges against, and a declared goal's judges
review when the station names none), every wait is a question on the task, and shipping is wisp's own Ship and Merge.
A run from an agent (`wisp_launch`) never runs a station above its own mode and never merges without you.
