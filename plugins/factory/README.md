# wisp.factory

Software factories (ADR-0114). v1 had none: its closest thing was a goal's judges sending a task back, and a review
by another harness once a PR was open.

## Intent

- A factory (`factories:` in a formula, the `factory` kind, a built-in `default`) runs one task through stations, in
  this order, each its own agent with its own prompt: spec (read-only; you approve it) → build (its own worktree; one
  agent or several side by side) → checks (commands in the build's folder) → review (APPROVE or CHANGES, counted by a
  vote) → ship (a PR you merge).
- A failed check, a review that isn't approved, or a failed build goes back to `loop.back_to` (build, or spec) as the
  next round: a follow-up to the same task with what failed. Past `loop.max` it asks you (`then: ask`) or ends failed
  (`then: stop`).
- A run is a held parent task whose children are its stations (ADR-0028's shape). `runtime.create`'s `factory` is read
  by `before:create`, which makes the task held, with no worktree and no terminal. A `before:run` guard refuses any turn on
  it. The driver writes where it stands to `ext["wisp.factory"]` and the list row (`runtime.summary`'s `factory`).
- The decisions are pure (wisp-factory-core's machine). The host does the IO: `runtime.create`, `followUp`, `ask` and
  `pollAnswer`, `sh -c` for checks, and `ship.ship` / `ship.merge`.
- UI: a band on `shell.task:band` (lazy) and New task's picker row (packages/shell `factory-pick.ts`). TUI: one line
  in the task header.

## Not yet

- A crew member naming a factory for its jobs (a trigger or a declared task can).
- Cutting a single running turn by cost: harnesses report a turn's cost when it ends, so the budget cuts a station's
  other tasks and the run (ADR-0114 rule 12), not the turn itself.
