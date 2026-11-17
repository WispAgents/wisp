---
title: Build a software factory as code
description: Declare agents that pick up issues, build, review and keep the repo healthy, in your repo's .wisp/ folder.
sidebar:
  order: 1
---

A **software factory** is a set of agents that turn work into pull requests without you starting each one: a
builder picks up issues, a reviewer reviews what the builder opens, a fixer jumps on red CI, and a nightly job keeps
the dependencies fresh. In wisp the whole factory is a few files in `.wisp/`, reviewed and changed in pull
requests like the rest of your code.

This guide builds one step by step. Every field used here is in the [resource kinds reference](../../../reference/kinds/).

## 1. The agents

Say which harness does what, once, and refer to it everywhere else.

```yaml
# .wisp/agents.wisp.yml
wisp: 1
agents:
  builder: { harness: claude, model: opus, mode: auto, skills: [house-style] }
  reviewer: { harness: codex, effort: high, mode: plan, instructions: "Correctness first, then tests, then style." }
  fixer: { harness: claude, mode: auto }
skills:
  house-style: { description: How we write code here, path: docs/house-style.md }
```

## 2. The crew

Crew members are agents that stay, with a role and a brief. They're who the triggers below wake.

```yaml
# .wisp/crew.wisp.yml
crew:
  bob:
    role: builder
    agent: builder
    brief: "Build what an issue asks. Write the test first. Open a PR; never merge."
    sandbox: job          # each job in a worktree of its own
    parallel: 2
  ada:
    role: reviewer
    agent: reviewer
    brief: "Review every PR the crew opens. Request changes or approve; never merge."
```

## 3. The triggers

A trigger starts work when something happens. Its `on` is any of the
[events and trigger sources](../../../reference/triggers/).

```yaml
# .wisp/triggers.wisp.yml
triggers:
  build-labeled-issues:
    on: issue.labeled
    label: agent              # an issue labeled "agent" (GitHub, Linear or Jira, through wisp.issues)
    crew: bob
    prompt: "Build {event.summary}"
    concurrency: queue
    budget: { perRun: 5, perDay: 40 }

  review-crew-prs:
    on: pr.opened
    crew: ada

  fix-red-ci:
    on: ci.failed
    followUp: true            # a follow-up to the task whose PR failed, with the failing log
    cooldown: 30m
    rate: 5/h
```

`when: { days: mon tue wed thu fri, hours: "9-18" }` keeps a trigger to working hours; `approval: true` makes a job
wait for your yes before it starts.

## 4. Goals: done means done

A task with a goal is sent back after each turn until its judges agree the goal is met.

```yaml
# .wisp/goal.wisp.yml
goal: { judges: [reviewer], vote: majority, maxRounds: 6 }   # every goal's judges, unless a task says otherwise
tasks:
  green-build:
    prompt: Make the build pass
    agent: builder
    goal: { text: "npm test passes and the build is green" }
```

## 5. A nightly workflow

A workflow is steps with dependencies, run by hand (`wisp run nightly`) or by a trigger.

```yaml
# .wisp/nightly.wisp.yml
triggers:
  every-night: { on: schedule, cron: "every day at 2", workflow: nightly }
workflows:
  nightly:
    budget: { usd: 20 }
    tasks:
      deps: { agent: builder, prompt: "Update the dependencies that have patch releases. Run the tests." }
      audit: { agent: reviewer, needs: [deps], prompt: "Review the dependency update: ${{ tasks.deps.output }}" }
```

## 6. Guardrails

```yaml
# .wisp/budget.wisp.yml
budget: { stopAt: "75", taskUsd: 10 }   # automations stop at 75% of a usage window; $10 at most per task
```

Settings → Usage also has *Stop a run silent for*, which stops a run that sits with no output.

## 7. Check it, then turn it on

```sh
wisp validate      # the schema and the references
wisp plan          # what will change
wisp apply         # turn it on
wisp graph         # what refers to what
```

Commit `.wisp/`. Anyone who clones the repo and runs `wisp apply` gets the same factory. Change it the way you
change code: a branch, a pull request, a review.

## Watch it run

- The list shows each job as a task you can open, follow up and ship.
- `wisp ls --crew bob` lists one member's jobs, `wisp ls --workflow nightly` a workflow's.
- In a repo that [keeps its history](../../track/track-agent-work/), every job is saved next to the code it wrote.
