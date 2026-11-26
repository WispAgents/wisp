# ADR-0136: an agent's sandbox writes are held to its own task

- Status: accepted
- Date: 2026-11-26
- Source: sandbox audit r5 (SBX-A1).
- Changes: ADR-0112's and ADR-0115's agent writes (`sandbox.drop`, `sandbox.own` stay on the list, scoped as below). The
  list itself is unchanged: ADR-0126's `agent-writes` block is still the newest.

## Context

ADR-0112 gave an agent `sandbox.drop` and `sandbox.own` so a task's own work (its `/sandbox`, its delegates', a crew
job it handed out) could make, name and drop the sandboxes it runs in; "the kernel's agent scope still holds each of
these to the agent's own task where they name one". They name one only by args the kernel knows (`sessionId`,
`session`, `parentId`, `taskId`): `sandbox.drop {machine}` and `sandbox.own {machine, owner}` name a machine, so they went
through. An agent in task A could delete task B's sandbox (with B's unpushed work in it, or a delegate's that holds work
the user is meant to delete by hand) and re-own any sandbox, `{pending: N}` included, which the sweep then drops.

## Decision

1. When the caller is an agent (`scope::agent_task` is some), wisp.sandbox decides it in the command, sans-IO in
   `wisp-sandbox-core` (`may_agent_touch`, `agent_drop_refusal`, `agent_own_refusal`):
   - **drop:** each sandbox the call names must be its own task's: its `/sandbox` (`{task}`), its judges'
     (`{judges}`), its delegates' (`{lead}`), a crew job's whose job is its task (`{crew, job}`), compared by the task's
     own id (`core::bare`). `{task}` must be its own; `{crew}` takes only its own job's; one a machine here doesn't keep
     (by `sandbox` id) has no owner, so it's the user's. A delegate's still holding work (`make::holds_work`) is
     refused with words that say the user deletes it in Settings → Computers → Sandboxes. Refused: `forbidden`, and
     nothing is dropped.
   - **own:** the new owner names its own task, on a sandbox its task owns already. A launch's (`{pending}`), a crew
     member's own, one with no owner and another task's are never re-owned by an agent, and an agent never writes
     `{pending}`. One shape more: a crew job's sandbox not named yet (`{crew: C, job: ""}`) may be named a job of the
     same member (`{crew: C, job: <task>}`), since crew names a delegated job's sandbox under the lead agent's call.
2. The user (and the kernel, and a plugin's own root call) keeps today's behaviour: any of their sandboxes, by machine,
   task, crew member or sandbox id.
3. `sandbox.drop {sandbox}` is new (additive): one the grid holds that no machine here keeps, deleted on the grid by its
   id (Settings lists those since SBX-A2).

## Consequences

- An agent can't delete or take over another task's sandbox with its run's token (`/api/cmd`, or nested under a tool), nor through a
  plugin it calls (`serve_mcp_tools::an_agent_never_drops_or_reowns_another_tasks_sandbox`).
- A crew job a lead agent delegated, whose start fails half way, can't be dropped under that agent's call (its box is
  `{crew, job: ""}`, no task's yet): it goes with the sweep after `PENDING_MS`, as one never named did before.
- The capability set is unchanged: both commands stay agent writes (`an_agents_capability_set_writes_only_its_own_work`).
