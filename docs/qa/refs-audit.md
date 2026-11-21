# Audit: every reference in a formula, a task file, the CLI, MCP and New task (owner-list row 61)

Owner, 2026-11-19: "Cover these cases: I want `wisp "prompt" --formula`, and do the yml declare files use references?"
and "Make sure everything can be atomic and referenced." Read on `decf81a9`; fixed in the same change (ADR-0111).

**Atomic** = the thing is declared on its own, with an address `<kind>.<name>` (every resource kind is: a section of
named items, a singleton `<kind>.default`, or a map form `settings.<key>`). **Ref** = the field takes a bare name and
`${kind.name}` (any field takes `${…}`; it is replaced by the resource's stored name), and inline where shown.
**Checked** = an unknown name is refused before anything runs, naming what exists. Before = at `decf81a9`.

## What every surface did before

| Surface | Before | Gap | Now |
| --- | --- | --- | --- |
| `.wisp/` folder | `${kind.name}`, variables, locals, count, modules evaluated (ADR-0034) | an unknown `${agent.nope}` said only "isn't declared"; `${module.m.agent.x}` (a module's resource by its address) was refused; a module that uses itself recursed until the stack overflowed | names what exists (`agents has a, b`); a module's resource by address is its stored name; a module that uses itself is refused, naming the way round |
| `wisp apply/diff/validate -f f.yml` | read raw: `${agent.x}`, `variables:`, `locals:`, `count:` and `modules:` were not evaluated (`variables:` was refused as "no plugin owns") | **single files didn't use references** | evaluated as `.wisp/` is (`module::evaluate`); `-var`, `--var-file`, `WISP_VAR_<declared>` |
| `wisp run --formula f.yml "…"` | raw overlay, no references; only an agent profile was resolved | same; a goal it names by name was looked up in the store (not applied: refused) | evaluated; the goal inlined from the formula; every name checked |
| `wisp run <declared-task> --formula f.yml` | **the task's name was read as the prompt** | yes | runs the task the formula declares, `tasks.default` under it, `{params}` from `k=v` and words, its goal inlined, its project by name; nothing applied |
| `wisp "prompt" --formula f.yml[,g.yml]` | **`--formula f.yml` became part of the prompt** | yes | the same overlay as `wisp run --formula`; `--agent` may name the formula's agent; `--compare` runs each under it |
| task file `wisp run -f x.task.yml` | `formula:` only inline (a snapshot copy); only its agent profile used | **no reference to formula files or modules** | `formula:` is a file, `module.x` (its folder's `.wisp/`), inline, or a list; each evaluated; the task's agent, goal, connectors, crew, team, project resolved against them and this wisp; a file named twice or the task file itself refused |
| `wisp mcp` `wisp_launch` | no `formula` | yes | `formula`: a file, a list, or inline |
| a task's `wisp_launch` (in-task MCP) | no `formula`; `agent` an enum of harness ids only | **an agent profile couldn't be named** | `formula` (resolved by `resources.overlay`); `agent` takes a harness or a declared agent |
| New task (the app) | no formula, no task file | yes | `+` → Formula or task file…: the peer reads it (`resources.overlay`), a chip until ✕; a task file fills the prompt and its fields; loaded on press (off the first screen) |
| `resources.plan` (every surface above, Setup → Import) | only task → goal checked (ADR-0110) | **every other name field unchecked** | every field below checked through `KindInfo::names` / `refs`; loops through `crew.team` refused |

## Every kind and every field that names another thing

| Kind (section) | Field | Names | Atomic | Ref before | Checked before | Gap | Now |
| --- | --- | --- | --- | --- | --- | --- | --- |
| agent (`agents`) | `connectors[]` | connector | yes | name, `${}` in `.wisp/` | no | unchecked | checked |
| | `skills[]` | skill | yes | name | no | unchecked | checked |
| | `account` | account | yes (personal) | name | no | unchecked | checked |
| connector (`connectors`) | `agents[]` | agent | yes | name | no | unchecked | checked (a harness or `auto` too) |
| crew (`crew`) | `agent` | agent or harness | yes | name, inline fields | no | unchecked | checked |
| | `machine` | machine | yes | name | no | unchecked | checked (declared, a peer added in the app, `this`) |
| | `team[].crew` | crew | yes | name | no | unchecked; a loop of delegations wasn't refused | checked; a loop refused (`a loop: crew.Ada → crew.Bo → crew.Ada`) |
| | `wakes[].task` / `.workflow` / `.crew` / `.agent` / `.project` | task, workflow, crew, agent, project | yes | name | no (wake cycles only, ADR-0043) | unchecked | checked |
| goal (`goals`) | `judges[]` | agent or harness | yes | name, inline object | no | unchecked | checked (inline objects aren't names) |
| notification (`notifications`) | `to` | channel (or a URL) | yes | name | no | unchecked | checked (a URL isn't a name) |
| | `on[]` | trigger (besides event words) | yes | name | no | unchecked | checked (`finished`, `asks`, `failed`, `limit`, `drain` skipped) |
| | `projects[]` | project | yes | name | no | unchecked | checked (paths aren't names) |
| pipeline (`pipelines`) | `judge`, `review` | agent | yes | name | no | unchecked | checked |
| project (`projects`) | `agent` | agent or harness | yes | name | no | unchecked | checked |
| | `trust.mcp[]` | the project's own MCP servers (its `.mcp.json`) | n/a | — | — | none: not a wisp resource | — |
| task (`tasks`) | `agent` | agent or harness | yes | name | no | unchecked | checked |
| | `goal` | goal | yes | name, `{ name, … }`, inline | yes (ADR-0110) | — | inlined from a formula that isn't applied |
| | `connectors[]` | connector | yes | name | no | unchecked | checked |
| | `crew` | crew | yes | name | no | unchecked | checked |
| | `project` | project | yes | **a path only: a name failed at run time ("Not a project folder")** | no | a name didn't resolve | a name is the project's folder (`runtime.create`, overlay or stored, any case); checked |
| team (`teams`) | `lead`, `members.*.agent` | agent or harness | yes | name | no | unchecked | checked |
| tools (`tools`) | its name | agent, harness or a mode | yes | name | no | unchecked | checked (`plan`, `edits`, `auto` are modes) |
| trigger (`triggers`) | `task`, `workflow`, `crew`, `agent` | task, workflow, crew, agent | yes | name | no | unchecked | checked |
| | `project` | project | yes | name or folder | yes (the trigger's own apply, audit A #14) | — | — |
| workflow (`workflows`) | `project`, `tasks.*.project` | project | yes | name | no | unchecked | checked |
| | `on.trigger` | trigger | yes | name | no | unchecked | checked |
| | `tasks.*.agent` | agent or harness | yes | name | no | unchecked | checked |
| | `tasks.*.on` | machine | yes | name | no | unchecked | checked |
| | `tasks.*.needs[]` | another step of the same workflow | n/a (a step, not a resource) | name | yes (workflows' own check) | — | — |
| capability (`capabilities`) | `channels[]` | channel | yes | name | no | unchecked | checked |
| setting (`settings`) | `channels.project` | project | yes (`settings.<key>`) | name | no | unchecked | checked |
| | `runtime.subagents[].agent` | agent or harness | yes | name | no | unchecked | checked |
| | `review.harness` | a harness (or `off`, `other`) | yes | enum | yes (schema enum) | — | — |
| app, pane (`apps`, `panes`) | `grants[]` | commands and topics | yes | — | — | none: permissions, not resources | — |
| account, budget, channel, harness, machine, plugin, skill, ui | — | nothing they name is another resource | yes | — | — | — | — |

## Task files, CLI flags, MCP, New task

| Where | Field | Names | Before | Now |
| --- | --- | --- | --- | --- |
| task file `task:` | `agent`, `goal`, `connectors`, `crew`, `team`, `project`/`cwd` | agent, goal, connector, crew, team, project | only the agent, from the inline snapshot; the rest unchecked | resolved against the referenced formula and this wisp; refused naming what exists |
| task file | `formula:` | formula files, `module.x`, inline | inline only | any, or a list; loops refused |
| CLI | `--agent` | agent or harness | checked against harnesses and stored profiles | the same, or the `--formula`'s agents |
| CLI | `--goal NAME` | goal | checked by wisp.goal (ADR-0110) | the `--formula`'s goal inlined, else wisp.goal |
| CLI | `--formula` | formula files | `wisp run`, `wisp rerun`, `wisp ls` only | a plain launch and `wisp run <task>` too |
| CLI | `wisp run <name>` | a workflow, a stored task, or (with `--formula`) the formula's task | the formula's task wasn't one | it is |
| MCP `wisp_launch` | `agent`, `goal`, `project`, `machine`, `preset`, `formula` | agent, goal, project, machine, preset, formulas | `formula` missing; in-task `agent` harness ids only | `formula` in both servers; in-task `agent` a declared agent too |
| New task | harness, Goal…, Where, Delegate presets, Formula or task file… | harness/agent, goal, project, machine, preset, formula | no formula | `+` → Formula or task file… |

## Left as they are, on purpose

- A formula from a URL or a gist is read by the CLI (`wisp run --formula https://…`); `resources.overlay` on the server
  reads files on its own computer only (New task sends a picked file's text; a task's `wisp_launch` names files in its
  folder), and refuses a URL saying the CLI reads it.
- In-task `wisp_launch`'s `subagents[].agent` stays an enum of harnesses (a delegate's seat is a harness, as
  Delegate's picker has it); a declared team is named by `team:` in a formula instead.
- In the app, a task file's relative `formula: ./wisp.yml` and `module.x` are read from New task's project folder
  (a picked file has no path in a browser).
