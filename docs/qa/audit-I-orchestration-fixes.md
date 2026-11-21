# Audit I orchestration: fixes (worktree at 10c848cb, uncommitted), 2026-11-20

One row per open item of docs/qa/audit-I-orchestration.md: I1–I16, plus H9, G15, E11 and D6 (partly fixed in H).

How to read the Test column:
- **Seen failing** means the test was run on the old code (the fix taken out) and failed.
- Every other test was written with its fix; its assertion is about output or behaviour the old code didn't have, and
  the audit's repro is the before.
- **Proof** is a scratch wisp in /tmp/qaI-fix on :4977, from a fresh build of this tree, with the fake harnesses (the
  audit's QA wrapper as `claude`, `wisp-fake-agent` as the other six) and `wisp-fake-gh`; WISP_HOME=/tmp/qaI-fix/home,
  HOME=/tmp/qaI-fix/userhome. The commands and what they printed are in /tmp/qaI-fix/proof.log (first pass) and
  /tmp/qaI-fix/proof2.log (after the review's fixes, on the final build).

Decisions (owner's delegate, 2026-11-20) are recorded in ADR-0115 and docs/v2-plan.md §3.

## Majors

| # | Fix | Test | Proof |
|---|---|---|---|
| I1 | `agent:session` (and every set built from it) denies `runtime:exec` ("an actions block's command runs when the user clicks it") and `runtime:delete` (crates/wisp-kernel/capabilities.json), so a run's MCP neither lists nor runs `runtime_exec`/`runtime_delete`. ADR-0115 supersedes those two entries of ADR-0112 §3 and holds the agent's whole write list in a fenced block. From the review: it also denies `runtime:projectTest` (a test command a factory's checks run without asking) and `machines:call` (a raw call to another computer's wisp), and `machines.route` holds a routed command to what the caller's own set may run here (plugins/machines/host/src/commands.rs `held_to_caller`: it runs there as this computer's link). | `serve_mcp_tools::an_agent_can_neither_run_a_command_through_runtime_exec_nor_delete_a_task` (a Plan task; raw token and MCP; no PWNED.txt; `machines.route {cmd: runtime.exec}`, `machines.call`, `runtime.projectTest` refused; the user's click still runs); `every_write_an_agent_reaches_is_one_adr_0115_lists` (the set, and every listed write tool, against the ADR's block); `an_agents_capability_set_writes_only_its_own_work` updated | proof.log "## I1": `runtime_exec` not among the run's tools, "Unknown tool: runtime_exec", no PWNED.txt, raw `runtime.exec` with the run's token → forbidden "…runs when the user clicks it". proof2.log: `machines.route {cmd: runtime.exec}` → forbidden "runtime.exec isn't something this task may run, here or on another computer"; `machines.call` → forbidden |
| Note | The user's harness lowering a safety guard asks first (ADR-0115 §4): `wisp-mcp-core::guard::lowers` (pure: `worktrees.projectAsk`/`worktree` off, `git.maskEnvFiles` off, webhooks/channels on, `budget.limitHold` later/off, `machine.batteryHold` lower, `runtime.setupAnswerSets` added, `worktrees.trust`; and always `runtime.exec`, `plugins.install`/`trust`, switching a plugin on, `machines.call`, a routed guarded command); values read as the kernel stores them (`false`, `no` → `off`). wisp.mcp's generated tool (for `user:*` or a box's `box`) makes a held task "Your harness wants to …" that asks Allow/Deny, waits, runs exactly the write on Allow, and settles the task; while it asks the harness may read the task but not answer, reply to, stop or change it; a guarded write with `peer` is refused. The runtime marks the question (`ask.guard`): a follow-up never answers it, and only the user's own hands (`user:*`, `box`, a channel relaying their tap) do. Raising a guard applies at once. | `guard.rs::turning_a_guard_off_from_the_harness_needs_the_users_allow`, `raising_or_keeping_a_guard_and_other_settings_apply_at_once`; `serve_mcp_tools::the_users_harness_lowers_a_safety_guard_only_on_the_users_allow` (ask; the harness's own answer and reply, a follow-up, and another task's agent all refused; Deny; Allow; raise; another setting; `false`; `peer`) | proof.log "## GUARD": the task "Your harness wants to stop asking before trusting…", the setting still `on` while it asks, the harness's `wisp_reply` refused, `wisp reply <task> Deny` → the harness reads "The user said no in wisp, so nothing was changed.", still `on`. proof2.log: `{"value": false}` asks too; `runtime.followUp Allow` to it → refused "…not a follow-up. Nothing was sent."; `wisp reply <task> Deny` → "✓ answered"; still `on` |
| I2 | A group's word or a plugin's name followed by none of its verbs is a slip however many words follow (`words::unknown_verb`, `plugin_slip`), with the close verb (`UnknownVerb::close`: "did you mean pause?") and the `wisp -- "…"` way out; `wisp held <id> <action>` too (`held_slip`). Words a prompt may start with (`ship`, `run`, `plan` …) with two or more words, one quoted argument, and anything after `--` stay tasks. Tests that encoded "two or more words are a prompt" under a group word were changed (serve_cli_verbs ×3, words_tests ×4, args_tests ×1). | `words_tests::a_misspelled_verb_after_a_command_word_is_refused_with_the_close_verb` (the audit's seven lines); `serve_audit_i::a_misspelled_verb_after_a_command_word_launches_nothing_audit_i2` (real binary, nothing launched; quoted and `--` launch) | proof.log "## I2": each of the six → exit 2, "(did you mean pause?)" (off, set, cancel, stop, start), the `wisp -- "…"` line; 0 tasks before and after |
| I3 | Root cause: not a drain race but the interrupt path. With `runtime.followUp` at its default (interrupt), a follow-up that came once a queued follow-up's turn had its conversation interrupted that turn (stopped after ~50 ms) and went ahead of the ones waiting; and the CLI printed "✓ queued" for it. Now (plugins/runtime/core/src/queue.rs) a turn the queue started is never interrupted by a later follow-up (`run_for_follow_up`; runtime.rs/finish.rs keep `from_queue`, cleared on another start, a failed start and delete), and a follow-up that would interrupt but can't yet waits as an interrupting one (`now`), so a later one never goes ahead of it. A turn you started (the task's own, or a follow-up sent while it was idle) still takes v1's interrupt (narrowed after the review), Send now still interrupts, and `wisp reply` says "it interrupts the running turn" when it does (`FollowedUp.interrupts`, additive). | `core/tests/queue.rs::quick_follow_ups_each_run_once_in_order_and_no_queued_one_is_stopped` (model: 300 random interleavings of 25 follow-ups); `host/tests/followup_race.rs::quick_follow_ups_without_queue_each_run_to_the_end_in_the_order_sent_audit_i3` (**seen failing** with `run_for_follow_up` taken out: a queued turn stopped and the 26 never all done; stable over 5 runs) | proof2.log "## I3": one task, then 25 `wisp reply`s: 26 turns, "I3 long thread:done g1:done … g25:done", in order (proof.log's first try sent them to another task by a script slip: its 25 also ran done, in order) |
| I4 | New task sends the bar's model, thinking and account only when no picked file says what runs (`file-runner.ts::barArgs`, HomeView.tsx). | `packages/shell/test/file-runner.test.ts::a_picked_task_files_agent_runs_its_own_model_never_the_bars_audit_I4` (the `runtime.create` body); `serve_formula_refs::a_picked_task_file_runs_its_agents_model_not_the_bars_audit_i4` (that body, codex argv `-m gpt-5 … "high"`, no opus) | tests |

## Minors

| # | Fix | Test | Proof |
|---|---|---|---|
| I5 | wisp.factory's `before:followUp` on a run's own task with nothing asked refuses it: "This factory run is stopped: nothing was sent." (or that its stations do the work); exit 1, no turn. `serve_factory`'s default-factory e2e asserted the old "Not started" turn and was changed to this. | `serve_factory::a_reply_to_a_stopped_run_is_refused_and_ls_says_stopped_audit_i5_g15`; `factory/host/src/tests.rs::a_follow_up_or_a_stop_with_nothing_there_says_why_audit_i5_i12` | proof.log "## I5 / G15": `factory stop`, then `wisp reply <run> Approve` → "✗ This factory run is stopped: nothing was sent.", exit 1; `wisp ls` → "…  stopped  Factory · default  proj …" |
| I6 | `runtime.setup.ask` for `runtime.projectAdd` checks the folder first (async stat): "No folder at …: nothing was asked." | `serve_mcp_tools::an_agents_project_add_for_a_missing_folder_is_refused_before_any_card_audit_i6` | tests |
| I7 | A member's `on` takes v1's kinds and the triggers' names (`automations_api::v1::wake_on`), a word that wakes nothing is refused (crew core); `crew update` says only what changed against the member read first (`said.rs`). | `v1.rs::a_members_on_takes_v1_kinds_and_trigger_names_audit_i7`; `said.rs::crew_update_says_only_what_changed_audit_i7_i12`; `serve_audit_i::crew_on_sets_wakes_and_update_says_only_what_changed_audit_i7` | tests |
| I8 | `wisp run <task> --formula F` drops `-d`/`--detach` from the task's values; `run -f` and `run … --formula` take `--wait`. | `serve_formula_refs::run_task_under_a_formula_takes_detach_and_waits_through_judging_audit_i8_i11` | tests |
| I9 | `--effort` outside minimal…max, a mode or a level as `--model`, and `--agent auto` with Auto off are refused before launching, in words (`words::bad_launch_words`, `scripted::unknown_agent`). | `serve_audit_i::a_bad_effort_a_mode_as_a_model_and_auto_while_off_launch_nothing_audit_i9` | proof.log "## I9": the three refused in words; 0 tasks |
| I10 | A user's `wisp_launch` gives a git checkout's task a worktree, as `wisp "…"` does; `worktrees.worktree` (New task's In place) is New task's own default (plugins/mcp/host/src/launch.rs). | `serve_audit_i::the_cli_and_mcp_agree_on_where_a_task_runs_audit_i10` | tests |
| I11 | `--wait` and `wisp wait` stop when the task asks, is done, or its goal is judged (`scripted::settled`). | `scripted.rs::wait_goes_through_a_goals_judging_audit_i11`; the I8 e2e | tests |

## Smells

| # | Fix | Test | Proof |
|---|---|---|---|
| I12 | Sentences for `goal set`, `crew create`, `review view` (`said.rs`); `trigger log` says when in words; `factory stop` on a task that isn't a run says so (`drive::stop_refusal`); "back at work: no longer paused". | `said.rs::goal_set_crew_create_and_review_view_say_sentences_audit_i12`, `crew_update_says_only_what_changed_audit_i7_i12`; factory host test | tests |
| I13 | Words for every field audit H and I named (`crew create`'s 16, `runtime fork --id/--title`); `crew create` opens with what it does. | `serve_audit_i::no_help_page_carries_developer_notes_or_opens_with_its_wire_name_audit_i_e11` (the field checks) | tests |
| I14 | Every "no X named Y" says the close name (`resources_core::overlay::named_nothing`, used by validate, the overlay and the plan); task-file refusals print tidy paths (`tasks_cli::tidy_path`); an agent's formula that can't be read gives one error (the task's names aren't checked against nothing). Three neighbours' messages updated (serve_goals, serve_formula_refs, serve_mcp_tools). | `overlay.rs::a_name_that_names_nothing_says_the_close_one_audit_i14`; `tasks_cli.rs::a_refusal_says_a_path_as_a_person_writes_it_audit_i14`; `serve_mcp_tools::an_agents_formula_reads_only_inside_its_own_folder` (one error) | tests |
| I15 | A crew wake says how to change it (`wisp crew update <m> --wakes …`, or pause); `trigger test` on an off trigger says it's off; a trigger's runs are titled by their prompt (`automations/host/tests/fire.rs` asserted the old title and was changed). | `serve_audit_i::triggers_say_how_to_switch_a_crew_wake_that_they_are_off_and_runs_are_titled_by_prompt_audit_i15` | tests |
| I16 | A connector's card shows its name as stored and that it was changed (`setup::connector_name`, the same rule as wisp.connectors' `slug`, ends trimmed: "saved as qa_phone (it asked for qa-phone …)"); a failing check is said in words (`connectors_core::agent::error_words`). | `setup.rs::a_connectors_name_is_shown_as_it_will_be_stored_audit_i16`; `agent.rs::a_failing_check_is_said_in_words_audit_i16` | tests |

## Carried over

| # | Fix | Test | Proof |
|---|---|---|---|
| H9 | `crew assign` fields have words; `later plan --kind` is `task|reminder`, `--at`/`--in` take text (`decl::kind_of`: `number \| string` is text, and `--at 1795…` still reads as ms); `later plan --prompt "…" [--folder …]` plans a task without a JSON body, and its example uses it. | the E11 sweep's field and `later plan` checks; `serve_audit_i::later_plan_takes_a_prompt_and_a_time_in_words_audit_h9` | tests |
| G15 | `wisp ls` prints a factory run's own state (`scripted::state_of`). | `scripted.rs::ls_says_a_stopped_factory_run_is_stopped_audit_g15`; the I5 e2e | proof.log "## I5 / G15": `factory stop`, then `wisp reply <run> Approve` → "✗ This factory run is stopped: nothing was sent.", exit 1; `wisp ls` → "…  stopped  Factory · default  proj …" |
| E11 | `decl::public` drops a doc's leading wire name (`` `goal.set`: ``, `` `x`'s args: ``, `` `a` / `b`: ``, `` `x` (on the machine): ``) and owner's-delegate asides; the 25 docs with notes it can't drop safely were rewritten (crew, runtime, machines, resources, stuff, worktrees, cli api docs). | `serve_audit_i::no_help_page_carries_developer_notes_or_opens_with_its_wire_name_audit_i_e11` (all 461 pages; **seen failing**: 109 of 461, then 22, then 1, while fixing); `decl.rs::a_doc_opens_with_words_not_its_wire_name_audit_i_e11` | tests |
| D6 | `automations.task.set/remove/run`, `channels.update/extra` describe their arguments (`described_command!`, `command!(…, ts = …)`). | `serve_audit_i::no_listed_mcp_tool_takes_only_args_audit_i_d6` (no listed tool is `{args}` alone; `channels_update {"bogus":1}` names the field) | tests |

## Review (a delegate's read of the diff, after implementing)

10 findings, all fixed with a test each:
1. An agent's `machines.route {cmd: "runtime.exec"}` ran on a paired computer as the link: routed commands are held to the caller's set (`held_to_caller`).
2. An agent's `runtime.projectTest` set a command a factory's checks run without asking: denied.
3. `{"value": false}` slipped past the guard: values coerced as the kernel stores them.
4. A grid box's user (`box`) skipped the guard: counted.
5. The guard question could be answered by a follow-up (`later.add`) or, it said, by an agent: the runtime marks it and only the user's own hands answer it; an agent's token answers only its own task (the kernel's scope, now asserted).
6. Other harness tools ran code without a card (`runtime_exec`, plugin install/trust/enable, `machines_call`/`route`): they ask too. Not taken: connectors added from the harness (`wisp_connector_add`, v1's feature for the user's own harness, ADR-0112/0113) stay direct, as ADR-0115 says.
7. A guarded write with `peer` bypassed it, or changed the wrong machine: refused.
8. The `runtime.launch` mode rule was dead (no default mode lives there): removed.
9. I3 first protected every follow-up's turn, which changed what the interrupt setting means: narrowed to turns the queue started; `from_queue` cleared on a failed start and on delete.
10. `connector_name` kept a leading `_` that `slug` trims: the same rule now.

## Found by the proof run

- `wisp reply <task> Deny` on a guard question sent a follow-up (the CLI answers only setup cards by their id), which the
  runtime refuses there: the CLI now answers a guard question by its id too. Test:
  `serve_audit_i::wisp_reply_answers_the_question_a_guard_change_asks_adr_0115`.

## Not fixed

None.
