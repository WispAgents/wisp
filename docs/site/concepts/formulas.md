---
title: Formulas (your setup as code)
description: A folder of .wisp.yml files that declares agents, crew, machines, skills, MCP servers, rules and workflows.
sidebar:
  order: 5
---

A **formula** is your wisp setup written as files. In a repo it's the `.wisp/` folder: wisp reads every
`*.wisp.yml` (or `.json`, `.toml`) in it as one setup, the way Terraform reads every `.tf` in a folder. Split it
however you like: `agents.wisp.yml`, `crew.wisp.yml`, `workflows/release.wisp.yml`.

```yaml
# .wisp/agents.wisp.yml
wisp: 1
agents:
  reviewer: { harness: codex, effort: high, mode: plan }
  builder: { harness: claude, model: opus }
skills:
  testing: { description: How this repo tests, path: docs/testing.md }
connectors:
  linear: { url: "https://mcp.linear.app/sse", oauth: true }
```

Every kind and field is in the [resource kinds reference](../../reference/kinds/). `wisp schema` prints the same as
JSON Schema, for your editor.

## Addresses and references

Every resource has an address `<kind>.<name>`: `agent.reviewer`, `crew.ada`, `connector.linear`. Any field can
refer to another resource or one of its fields:

```yaml
tasks:
  nightly: { prompt: Update the dependencies, agent: "${agent.reviewer}" }
```

References set the order things are created in, and a cycle is refused. `wisp graph` prints them, and
`wisp mv agent.reviewer agent.critic` renames one and rewrites every reference to it.

## Variables, locals, outputs, count

```yaml
variables:
  model: { type: string, default: opus }
  helpers: { type: number, default: 2 }
locals:
  harness: claude
agents:
  critic: { harness: "${local.harness}", model: "${var.model}" }
  helper:
    count: ${var.helpers}            # helper-0, helper-1
    harness: codex
    instructions: "helper ${count.index}"
outputs:
  critic_model: { value: "${agent.critic.model}" }
```

Set a variable with `-var model=sonnet`, a `--var-file`, or `WISP_VAR_model`. `wisp output` prints the outputs.
**Workspaces** keep one set of variables each: `wisp workspace new prod`, `wisp workspace select prod`, and
`${workspace}` is its name.

## Modules

A module is a formula folder you reuse, with inputs and outputs: a "review team" or a "nightly maintenance" setup
shared across repos.

<!-- example: unchecked -->
```yaml
modules:
  team: { source: ../shared/review-team, inputs: { model: haiku } }
  pack: { source: "git::https://github.com/acme/wisp-modules//pack?ref=v1.2" }
```

`wisp init` fetches them into `.wisp/modules/` and pins them in `.wisp/lock.yml`; `wisp init -upgrade` moves
them. A module's resources are addressed `module.team.agent.reviewer`.

## Plan, apply, export

```sh
wisp validate         # schema and checks
wisp plan             # what applying .wisp/ would change
wisp apply            # do it (asks first; --yes to skip)
wisp destroy          # take out everything it declares
wisp export           # what's set up here (in the app too), as files
```

wisp's store is the state: `wisp plan` compares your files with what wisp has now, so a change made in the app
shows up as drift. `wisp export` writes it back to files, and `wisp import <address>` adopts one thing made in the
app into the files.

## One-off overlays

`wisp run --formula f.yml "task"` runs one task under a formula and leaves nothing behind: handy for trying a setup
before you apply it.
