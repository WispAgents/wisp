---
title: Keep going until it's done (goals)
description: Give a task a goal and judges; wisp sends it back after each turn until the judges agree it's met.
sidebar:
  order: 2
---

An agent often stops before the work is really done. A **goal** fixes that: after each turn, one or more judges
read the answer and decide whether the goal is met. If not, the task gets another turn with what's missing.

## Set a goal

- **New task**: turn on **Goal** and fill in *Done when* ("all tests pass and the build is green").
- **A running task**: type `/goal all tests pass` in the reply box, or use *Set goal…* in its menu.
- **From a terminal**: `wisp goal set <id> "all tests pass" --judges codex,claude/opus/high --vote all`.

## Judges

By default the task's own agent checks its work. Add judges for a second opinion: other harnesses, other models, or
crew members. **Judges** next to *Done when* picks them and how they vote:

- **Majority**: most judges agree.
- **All**: every judge agrees.
- **Any**: one is enough.

Your choice is remembered for the next goals. In a formula:

```yaml
goal: { judges: [reviewer, codex], vote: all, maxRounds: 8 }
```

## While it runs

The task's goal band shows the round, each judge's verdict and why. You can pause the goal, resume it, judge now, or
drop it. When it runs out of rounds without being met, it stops and asks you what to do.

## Triggers on goals

`goal.met` and `goal.failed` are events, so a [trigger](../schedules-and-triggers/) can act on them: ship when the
goal is met, or tell a crew member when it isn't.
