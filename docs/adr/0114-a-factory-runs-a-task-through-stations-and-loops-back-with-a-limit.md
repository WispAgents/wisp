# ADR-0114: a factory runs a task through stations (spec, build, checks, review, ship) and loops back to build with a limit

- Status: accepted
- Date: 2026-11-20
- Source: owner, 2026-11-20 (`docs/owner-list.md` row 65): "add the concept of a factory where the user creates a
  software factory (search how software factories are designed); when running a task you can pass it through the
  factory instead of one agent". The lead's research (Augment, Addy Osmani, LeadDev, Mastra, TrueFoundry): a factory is
  stations in a line, each its own agent with its own prompt and context (spec → build → deterministic checks → review →
  ship); the gates are code (tests, lint), not prompts; people approve at a few points (the spec, the merge); a failed
  check or review goes back to build with the findings, up to a limit, then asks a person; every hand-off is kept and
  readable; the spec is the bottleneck.

## Context

wisp could already run each part of a factory: an agent builds in its own worktree (ADR-0108), a goal's judges send work
back until they agree (ADR-0110), review by another harness reads a PR (`wisp.review`), `wisp.ship` opens and merges it,
a question holds a task for a person (`runtime.ask`), budgets cap a task, and a workflow runs a DAG of task specs as one
parent task (ADR-0028). What it couldn't do was put them in a line for one task: a workflow has no loop (a DAG never
goes back to build when a check fails), its steps are only agents (a check is an agent asked to run the tests, not the
test itself), and nothing waits on a person's yes in the middle. Users wrote the line by hand: a builder, then
`wisp "review …"`, then a follow-up with the review pasted in.

## Decision

1. **A factory is a kind** (`factories:` in a formula, `factory` resources, atomic and referenced like every kind,
   ADR-0111): its stations, its loop and a budget. A **station** is one of five, always in this order; a factory lists
   the ones it has (build is required):

   ```yaml
   factories:
     default:
       stations:
         spec:   { agent: planner, approve: human }        # writes the spec, read-only; you approve it
         build:  { agent: builder, worktree: true }         # or agents: [claude, codex] to build it twice
         checks: { run: [npm test, npm run lint] }          # commands: a gate is code
         review: { agents: [reviewer], vote: majority }     # each says APPROVE or CHANGES: …
         ship:   { pr: true, merge: human }                 # opens the PR; you merge it
       loop: { back_to: build, max: 3, then: ask }
       budget: { usd: 20 }
   ```

   - `spec`: `agent`, `prompt` (extra instructions), `approve: human | none` (default `human`). Runs in Plan mode.
   - `build`: `agent` or `agents` (two or more build side by side, each in its own worktree; the first whose checks
     pass goes on), `worktree` (default true in a project), `mode` (default the run's).
   - `checks`: `run: [commands]`, each `sh -c` in the build's folder, in order, `timeout` minutes each (default 10).
     With no `run`, the project's own `test` command; with neither, the station says it has nothing to run.
   - `review`: `agent` or `agents`, `vote: majority | all | any` (default majority, the goal's words), `prompt`. Each
     reviewer reads the build's folder in Plan mode and ends with one line, `APPROVE` or `CHANGES: <what to fix>`.
   - `ship`: `pr` (default true), `draft`, `merge: human | auto | none` (default human).
   - `loop`: `back_to: build | spec` (default build), `max` rounds (default 3), `then: ask | stop` (default ask).
   - An agent a station leaves out is the task's agent if it named one, else the user's default harness (New task's
     last pick, as every launch that names none, owner decision 2026-11-17).
   - **A built-in `default`** works with no setup: spec (approved by you) → build → checks (the project's test) →
     review (one reviewer) → ship (a PR you merge), `loop: { back_to: build, max: 3, then: ask }`. A factory you
     declare named `default` replaces it.
2. **A run is a parent task whose children are its stations** (ADR-0028's shape). `runtime.create` takes
   `factory: <name>` (additive on `StartArgs` and the task spec, so reruns, shares and `wisp task spec` keep it);
   wisp.factory's `before:create` reads it and changes the args (it writes nothing): the parent is made held, never runs
   an agent and makes no worktree; an unknown name is refused naming the factories there are. A `before:run` guard
   refuses any turn on a run's own task (a follow-up typed there, Start now on its held prompt): its stations do the
   work, and a follow-up to a station's task is how you add to it. A formula's `factories:` (`--formula`, a task file's
   `formula:`) are the run's before the applied ones, nothing applied (ADR-0111's overlay gains the section). When the task exists,
   the run's driver (wisp.factory) makes each station's child with `runtime.create` (`parentId`,
   `lineage.workflowRun`, `ext["wisp.factory"] = { run, station }`) and writes where it stands on the parent
   (`ext["wisp.factory"]`: the factory, its stations with their state, task, round and output, the round of the
   limit, what it waits on) and on the list row (`runtime.summary`'s `factory` field).
3. **The decisions are pure** (`wisp-factory-core`, a machine: an event in, the next step out): what starts, what a
   station's result means, when to loop back and when to stop. The host only does the IO.
4. **Hand-offs are durable and readable**: the spec task's answer is the spec every later station is given; the build's
   worktree is where checks run, reviewers read and ship ships; each station's output (the spec, the check commands
   with their exit code and last lines, each reviewer's verdict) is kept on the parent. Going back to build is a
   **follow-up to the same build task** with the findings, so its agent keeps its context and its worktree; a review
   after the first is a follow-up to the same reviewers.
5. **The new piece is the loop**: a failed check or a review that isn't approved goes back to `back_to` with what
   failed, as round 2, 3, … up to `max`. Past it, `then: ask` asks you on the parent ("Checks still fail after 3
   rounds. Keep going?", One more round / Stop, or your words as what to do next) and `then: stop` ends the run failed,
   saying why. `back_to: spec` sends the findings to the spec task, which you approve again, then the build follows up
   with the new spec.
6. **It reuses, never duplicates**:
   - **workflows**: the run's shape (a held parent, children with `lineage.workflowRun`, the state on the parent).
   - **goals** (ADR-0110): the review's `vote` is the goal's rule (majority, all, any); a task's goal is what the spec
     and every reviewer judge against; when the review names no agents, a declared goal's judges are the reviewers and
     its vote the vote, and its `maxRounds` is the loop's `max` when the factory has no `loop`.
   - **holds and approvals**: every wait on a person is `runtime.ask` on the parent, so it is the question card in the
     app, on the phone, in the TUI and on a channel, and a `task.asked` fact for triggers and crew.
   - **ship**: `ship.ship` on the build task opens the PR; `ship.merge` merges it.
   - **budgets**: each station is an ordinary task, so wisp.budget's caps hold it; the factory's `budget.usd` stops
     the run before a new station once the stations together have cost that much.
   - **worktrees**: the build's worktree is a task's worktree (ADR-0108), tidied and merged as one.
7. **An agent's factory run is held to the agent, station by station** (ADR-0111, ADR-0112; review r194 factories #1). A
   task an agent's token makes keeps that agent's limits (`ext["wisp.runtime"].heldTo`: its mode, cap, connectors,
   crew, account, team and machines, the set `child.rs` checks). `runtime.create` holds a task made under such a task
   to the same limits by the same check (`child::inherit`, `hold` without the fields a plugin sets on its own task: its
   `ext`, its parent, a worktree key) when a plugin or the kernel makes it as a step of that task's own run
   (`lineage.workflowRun` naming its parent: a factory's stations), so they never exceed what the starting agent may.
   Never for the user's own calls (their page, CLI, harness through `wisp mcp`, a linked channel, an API key): a Side
   discussion, follow-up, fork or rerun of an agent's task keeps the user's mode (review r194 final pass #1). A mode asked for nothing is the parent's, never Full auto, and the
   machine asks no station for more than the run's own mode. `merge: auto` becomes `human` for an agent's run. A
   factory is the user's setup: an agent's formula can't declare one (it isn't in `AGENT_FORMULA_KINDS`).
8. **A verdict is exactly the reviewer's last line** (review r194 factories #2): its own final non-empty line, the
   presentation block left out, trimmed, any case: `APPROVE`, or `CHANGES: <what to fix>`. Anything else (a negation,
   a quote, the word mid-line, text a builder left in files the reviewer read and quoted) is no verdict, and never a yes.
9. **Checks run the built code, as you** (review r194 factories #3). A gate runs code, so it runs the build's: in its
   worktree, `sh -c`, with wisp's environment minus wisp's own variables (`WISP_*`), the harnesses' homes and every
   variable whose name says token, secret, password, auth, credential or key (`check_env`). What they print is masked
   by wisp.runtime's `.env` rules for the build's task before it is kept or shown. When the build didn't run in Full
   auto (its commands needed your yes) and the run isn't in a sandbox, the station asks first on the run: "Run `npm
   test` on the built code?", Run / Skip / Stop, every round.
10. **A run stops at once** (review r194 factories #4). Stop (`factory.stop`) kills a running check's process tree
   then and there, and stops every station's task. Deleting a run stops it the same way and deletes its stations'
   tasks; each station's worktree is removed as the worktrees plugin removes one: never with uncommitted work in it
   (that one stays, as Merge keeps one, for Worktrees to show).
11. **A turn on the run's own task is refused, failing closed** (review r194 factories #5): a `before:run` guard
   (`Failure::Closed`) refuses it for every run this wisp knows, and until the runs are read at start it reads the task
   and refuses one it can't read.
11a. **The factory's `before:create` fails closed** (review r194 final pass #3): `Failure::Closed`, so a task that names a
   factory is never made as a plain agent's task because the guard broke; a panic in it is refused in plain words ("wisp
   couldn't read the factory this task names, so nothing was started"), and a release build, which aborts on panic,
   never goes on without it.
11b. **A check can reach the local server** (review r194 final pass #2, left as designed): it runs with no token, so it
   reaches only what wisp serves to a process on this computer without one, the threat model ADR-0113 records.
12. **The budget cuts a station** (review r194 factories #6): when a station turn's reported cost takes the run past
   `budget.usd`, the station's other tasks stop and the run ends failed, saying so; no new station starts past it.
   Harnesses report a turn's cost only when it ends, so a single running turn can't be cut by cost mid-way, and
   wisp.budget's `taskUsd` is kept but enforced nowhere yet (a separate gap, not this ADR's).
13. **A run picks up after a restart** (the owner's delegate, 2026-11-20). The machine and the turns it waits on are
   kept on the run's task after every step. At start (wisp.factory requires wisp.runtime, so its tasks can be read)
   each run that was going picks up: a station still running is waited on again (its tasks re-attached), a wait on you
   stays the same question, and the steps that do no harm twice (the checks, ship, merge) are done again. A run from
   before the machine was kept says so and ends.
14. **One voice per run** (the owner's delegate, 2026-11-20). The run's task says "Factory · <name>" in the list row,
   its header and its reply box, never a harness. Its stations post no "ready" notice of their own: the run tells you
   when it waits on you, ends or fails (a station's agent asking you still does: that's the run waiting on you).

## Places it reaches (ADR-0040)

- **Formulas**: `factories:` (validate, apply, export, `.wisp/`, Setup → Import, a grid deploy); `factory:` on a declared
  task (`tasks.default` included, so every run of a task can go through one) and on a trigger; each station's agents and
  a task's `factory` are references (`wisp validate` names one that names nothing).
- **CLI**: `wisp "…" --factory <name>` (`--factory` alone is `default`), `wisp run <task> --factory <name>`, a task
  file's `factory:`, `wisp export factories`, and `wisp show` / `wisp ls` print where a run stands.
- **Agents**: `wisp_launch`'s `factory` in `wisp mcp` and in a task's own MCP server (held by rule 7).
- **The app**: New task's harness picker has a **Factory** row next to the harnesses, whose pane lists the factories by
  name (One agent is any harness row); the pill then says the factory. The task page shows the run as one row with its
  stations as steps on a band (where it is, "Review · round 2 of 3", each station's output, Open on each station's
  task, Stop), the question card where it waits on you; the phone shows the same band and card; the TUI prints the
  stations line.
- **Channels and the phone**: a run waiting on you is a question (`task.asked`), answered from anywhere.

## Consequences

- One new plugin, `wisp.factory` (api, core, host, ui, tui), and two additive fields: `StartArgs.factory` and
  `TaskSpec.factory` (wisp.runtime's contract), plus `factory` on a declared task and a trigger (wisp.automations).
- A factory run costs one parent task and one child per station (two or more for several builders or reviewers); a
  loop adds turns, not tasks.
- A checks station runs the user's commands on the built code in its worktree, with no sandbox but none of wisp's
  tokens or secrets, and after your yes when the build needed approvals (rule 9).
- `runtime.create` gains one rule for every plugin (rule 7): a task made under a task an agent started is held to that
  agent. A plugin that made children of an agent's task with more than the agent may (none did on purpose) is now
  refused or held down.
- The first page pays for the factory's words and its UI's index (every plugin UI's index is on `apps/web`'s eager
  glob; the band itself loads through a one-line module, so the page carries no list of its chunks) without moving
  ADR-0105's ratchet: the entry now loads only the platform it runs on (`loadPlatform`: the desktop app's, Android's
  and the iPhone's code come with an import of their own, the browser's starts at once). Desktop first-screen JS went
  from 177,289 to 176,159 B br (173.0 KB budget: 993 B of room); the phone's is 169,837 B (167.0 KB budget).
- Not checkable by `tools/check` (behaviour, not structure): the tests below hold it.

## Edge cases, each with its test

Review r194 (factories) and the owner's delegate's decisions, each with a test that failed first:
- #1 `crates/wisp/tests/serve_mcp_tools.rs` `an_agents_factory_run_never_builds_above_its_parent` (`wisp_launch` and raw
  `runtime.create`, plan and edits parents, with and without a mode; failed: the build station ran in Full auto under a
  Plan parent); `plugins/runtime/core/src/child.rs` `a_task_made_under_an_agents_task_is_held_to_that_agent`; the
  machine's `an_agents_run_holds_its_stations_mode_and_never_merges_by_itself` (no mode asked: Plan).
- #2 the machine's `only_an_exact_last_line_is_a_verdict`.
- Final pass #1 `serve_mcp_tools.rs` `the_users_own_calls_on_an_agents_task_keep_the_users_mode` (failed: the user's
  Side discussion at Full auto was refused) and `plugins/runtime/host/src/plugin/create.rs`
  `only_a_step_of_the_agents_run_inherits_its_hold`; #3 `plugins/factory/host/src/tests.rs`
  `the_factory_create_guard_fails_closed_with_a_plain_reason`.
- #3 the machine's `checks_ask_first_unless_the_build_ran_in_full_auto_or_a_sandbox`,
  `checks_run_without_wisps_tokens_secrets_or_harness_keys`; `serve_factory.rs`
  `checks_ask_first_after_a_build_that_needed_approvals_and_never_see_wisps_secrets`.
- #4 `serve_factory.rs` `stop_and_delete_kill_a_running_check_and_take_the_stations_with_them`.
- #5 `plugins/factory/host/src/tests.rs` `a_turn_on_a_run_is_refused_and_an_unreadable_task_fails_closed`.
- #6 the machine's `a_station_is_cut_when_the_runs_total_passes_the_budget`.
- Resume: the machine's `a_run_picks_up_where_it_was_after_a_restart`; `serve_factory.rs`
  `a_factory_run_picks_up_where_it_was_after_a_restart`.
- One voice: `plugins/shell-home/ui/test/notify.test.ts` `a_factory_station_never_notifies_only_its_run_does`;
  `plugins/shell-list/ui/test/row-two-lines.test.ts` `a_factory_runs_row_says_factory_and_its_name_not_a_harness`;
  `packages/shell/test/factory-pick.test.ts` `a_factory_runs_task_names_the_factory_not_a_harness`.

The first build's:

`plugins/factory/api/src/lib.rs`:
- `a_factory_needs_a_build_and_only_names_the_five_stations`: no build is refused; an unknown station or field is
  refused, naming the five; `back_to` must be a station it has; `vote` and `then` only take their words; checks with an
  empty command is refused.
- `the_built_in_default_is_spec_build_checks_review_ship`.

`plugins/factory/core/src/lib.rs` (the machine):
- `a_spec_you_approve_goes_to_build_with_the_spec`
- `words_instead_of_approve_revise_the_spec_and_ask_again`
- `stop_at_the_spec_ends_the_run_and_builds_nothing`
- `no_spec_station_builds_from_the_prompt_and_approve_none_never_asks`
- `a_failing_check_goes_back_to_build_with_its_output_as_round_two`
- `checks_with_nothing_to_run_are_skipped_and_say_so`
- `a_review_not_approved_by_the_vote_goes_back_to_build_and_reviews_again_as_follow_ups`
- `an_unclear_or_failed_reviewer_never_approves`
- `past_the_limit_then_ask_asks_and_one_more_round_or_words_go_on_and_stop_ends`
- `past_the_limit_then_stop_ends_failed_and_says_why`
- `back_to_spec_revises_the_spec_asks_again_then_follows_up_the_build`
- `a_build_that_fails_loops_back_like_a_failing_check`
- `several_builders_the_first_whose_checks_pass_is_reviewed`
- `ship_opens_the_pr_and_merge_human_asks_before_merging`
- `merge_auto_merges_without_asking_and_pr_false_ships_nothing`
- `a_failed_ship_ends_the_run_failed_with_the_reason`
- `the_budget_spent_starts_no_new_station`
- `a_goal_is_what_reviewers_judge_and_its_judges_review_when_none_are_named`
- `an_agents_run_holds_its_stations_mode_and_never_merges_by_itself`
- `the_view_says_where_it_stands_and_the_round_of_the_limit`

`plugins/factory/host/src/tests.rs`:
- `an_unknown_factory_is_refused_naming_the_ones_there_are`
- `a_factory_task_is_made_held_with_no_worktree_and_keeps_its_factory`
- `a_run_wisp_stopped_in_the_middle_of_says_so_and_ends` (a run from before its machine was kept)

`plugins/automations/host/src/lib.rs`: `a_declared_tasks_factory_reaches_its_run`.

`crates/wisp/src/scripted.rs`: `factory_flag_names_a_factory_and_alone_is_default`.

`crates/wisp/tests/serve_factory.rs` (a real `wisp serve`, the fake harnesses and the fake gh):
`a_task_goes_through_the_default_factory_end_to_end` (spec approved, build, a failing check looping back as round 2,
review approved, the PR opened by the fake gh and merged after your yes, a follow-up on the run's own task refused),
`factory_names_are_references_and_validate_names_what_exists` (and a task file's formula declaring the factory it runs
through), `wisp_launch_runs_a_task_through_a_factory` (and Stop). An agent's run being held is the machine's
`an_agents_run_holds_its_stations_mode_and_never_merges_by_itself`.

`plugins/factory/ui/test/band.test.ts`: `the_band_says_each_station_the_round_and_what_waits_on_you`;
`packages/shell/test/factory-pick.test.ts`: `the_picker_offers_a_factory_and_the_launch_sends_it`.
