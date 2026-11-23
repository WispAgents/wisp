---
title: Workflows
description: Chain tasks into a DAG with dependencies, conditions, matrix fan-out, inputs and a spend budget.
sidebar:
  order: 10
---

A **workflow** runs a directed acyclic graph (DAG) of **steps** declared under `workflows:` in a
[formula](../formulas/). Each step is a [task](../tasks/) with its own prompt, agent, mode, goal or worktree, and steps
run in parallel as soon as the steps they depend on finish:

1. **Dependencies (`needs`) and outputs (`${{ … }}`)**: a step waits for the steps in its `needs`, then reads their
   results from their answer blocks (`${{ tasks.tests.output.failed }}`, `${{ tasks.tests.output.failures }}`,
   `${{ tasks.tests.output.summary }}`).
2. **Conditions (`if`)**: a step runs only when its `if` expression is truthy
   (`if: "${{ tasks.tests.output.failed > 0 }}"`); otherwise it is marked **skipped** and downstream steps carry on.
3. **Fan-out (`matrix`)**: a step with `matrix: { agent: [claude, codex] }` expands into one child task per
   combination (`audit[agent=claude]`, `audit[agent=codex]`), and downstream steps receive the list of every
   combination's output.
4. **Inputs and bounds**: `inputs` parameterise a run (`wisp run release-check version=2.1.0`), `retries`, `timeout`
   and `optional` govern individual steps, and `concurrency` and `budget: { usd: … }` cap how many steps run at once
   and how much the run may spend.

## One task, one step per child

A workflow run is one parent task (`<name> (workflow)`) in the list, held so it never runs an agent itself. Its steps
(and matrix combinations) are its child tasks (`<name> · <step>`). The parent records each step's state (`waiting`,
`running`, `done`, `failed`, `skipped`), attempts, output and cost, and ends in **Review** when every required step
succeeded (or **Error** when one failed).

## Workflows and factories

Use a [factory](../factories/) when **one task** should go through a line of stations (`spec → build → checks →
review → ship`) and **loop back** to build when a command check or review fails.

Use a **workflow** when a job is a **forward graph of distinct tasks**: run tests, fix failures only when tests fail,
fan out an audit across several harnesses in parallel, and summarize the findings.

## Checked before anything runs

```yaml
wisp: 1
agents:
  builder:  { harness: claude, model: opus, mode: auto }
  reviewer: { harness: codex, effort: high, mode: plan }
workflows:
  release-check:
    inputs:
      version: null
    concurrency: 2
    budget: { usd: 20 }
    tasks:
      tests:
        agent: builder
        prompt: "Run the test suite for ${{ inputs.version }} and report any failures."
      triage:
        needs: [tests]
        if: "${{ tasks.tests.output.failed > 0 }}"
        agent: builder
        retries: 1
        timeout: 15
        prompt: "Fix these failures: ${{ tasks.tests.output.failures }}"
      audit:
        needs: [tests]
        matrix: { agent: [claude, codex] }
        mode: plan
        prompt: "Audit the diff for ${{ inputs.version }} on ${{ matrix.agent }}."
      notes:
        needs: [triage, audit]
        agent: reviewer
        optional: true
        prompt: "Write release notes for ${{ inputs.version }}: ${{ tasks.audit.output }}"
```

`wisp validate` checks the graph before a run can start: unknown step names, dependency cycles, undeclared `inputs.` or
`matrix.` keys, and any `${{ tasks.<step>… }}` read where `<step>` isn't in `needs:` (directly or transitively) are
refused up front.

What each field takes is in the [resource kinds reference](../../reference/kinds/). How to declare, trigger and run
workflows from the app, the phone, the TUI and the CLI is in [Workflows](../../guides/run/workflows/).
