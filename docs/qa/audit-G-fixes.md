# Audit G: fixes (worktree at 9102236a, uncommitted), 2026-11-20

Each item: the test that covers it, then before → after. "Seen failing" means the test was run on the old code and
failed; the others were written with their fix (the audit's own repro is the before).

## CI (de1e88db)

| Item | Test | Before → after |
|---|---|---|
| (a) `wisp factory list/names --definitely-not-a-flag` exit 0 | `serve_cli_verbs no_verb_path_with_a_bad_flag_ever_becomes_a_task` (seen failing in CI) | accepted → refused. The factory commands are typed (`plugins/factory/api`: `List`/`Names` take `{ }`; `Remove`/`Stop` take their arguments' types, so `wisp factory stop <id>` takes the id) |
| (b) 3 of 21 pages' formula examples don't validate | `serve_docs every_formula_example_in_the_docs_validates` (seen failing: 3 pages) | concepts/factories.md and guides/run/software-factory.mdx declare `planner`, `reviewer` and `critic`; guides/declare/software-factory-as-code.mdx no longer declares `tasks.default` twice (the factory block names its own task) → 21 of 21 pass |

## Majors

| # | Test | Before → after |
|---|---|---|
| G1 | `wisp-resources overlay::an_agent_reads_a_formula_in_its_own_worktree_under_wisps_home` (seen failing) | `ok.yml` in `~/.wisp/worktrees/x` read as "a secrets file" → read. A task folder inside wisp's home opens only itself; the rest of the home, `secrets.json` and `.env*` stay refused, including through a link out |
| G2 | `wisp-goal --test formula_judge` (2 tests, both seen failing) + `wisp-goal-core a_judge_that_cant_judge_stops_the_goal_once` | judge `critic` ran as harness "critic", 4 rounds of "couldn't judge" → resolved from the task's formula snapshot (`runtime.snapshot`; the goal plugin now has `runtime:snapshot`). A judge that can't run asks once ("The goal can't be judged: …"), with no follow-up |
| G3 | `automations/ui trigger-edit: the switch turns a trigger off and back on` + Chrome on a real `wisp serve` | On sent no `enabled`, which apply keeps → `enabled: true/false` is always sent; the switch moves within 60 ms and rolls back on a refusal. Live: off → `false`, on → `true` |
| G4 | `serve_mcp_tools a_setup_card_is_answered_with_wisp_reply_and_allow_is_never_queued_audit_g4`, `wisp-channels --test answer a_typed_reply_under_a_setup_card_…`, `shell setup-reply.test` | `wisp reply <id> Allow` queued as a prompt → answers the card ("✓ You allowed: …"; words: "✓ You denied: … ("why")"). `wisp show` names Allow/Deny. `runtime.followUp` refuses a card's answer word from any client. The app's reply box under a card sends `runtime.answer`. A chat's typed reply answers for the linked person only |

## Minors

| # | Test | Before → after |
|---|---|---|
| G5 | `factory-core a_spec_you_approve…` (context asserted), `factory/ui band.test` | question context "" → the spec (1,500 chars) as the ask's context (`wisp show`, phone, channels); the band's Spec row shows it ("… · waiting on you") |
| G6 | `serve_mcp_tools the_users_http_wisp_launch_takes_a_goal_and_runs_in_a_worktree_audit_g6` | HTTP user `wisp_launch` dropped goal/formula/factory/worktree and ran in place → the agent's launch path with no asking task; the schema lists the fields |
| G7 | `serve_mcp_tools goal_and_factory_tools_reach_both_mcp_servers_audit_g7` | no `goal_*` or `factory_*` tools → wisp.mcp grants `goal:*` and `factory:*`; agents get `factory:read` (capabilities.json). `factory_stop` is the user's only |
| G8 | `serve_formula_refs an_invalid_formula_is_refused_before_a_task_launches_audit_g8` | launched with the agent dropped → refused with validate's "agents.critic.brief: unknown field". `resources.overlay` runs the schema check, and `wisp "…" --formula` (it ignored `--formula` entirely) applies the formula |
| G9 | `serve_formula_refs a_declared_task_under_a_formula_runs_in_its_own_worktree_audit_g9` | ran in place → a worktree in a project; `worktree: false` still runs in place |
| G10 | `wisp-resources --test names a_picked_task_file_reads_its_formula_beside_it_audit_g10`, `resources-core a_refusal_lists_eight_names…` | `./team.yml` read at the root, raw OS error, 32 names → the same file (name and text) found in the project gives its folder; otherwise plain words; at most 8 names "and N more" |
| G11 | `automations --test fire a_schedule_in_words_fires_audit_g11` (seen failing), `trigger::an_on_that_names_no_source_is_refused…` | `cron: "every minute"` fired 0 → fires. `on: every minute` accepted → refused, naming the sources. Then (round 2): `wisp diff` after apply on `project: proj` said "1 to change" → unchanged (`serve_automations_audit …audit_a_14`, extended, seen failing: the stored spec was the path). The trigger keeps `project: proj` as written; the engine reads it as the project's folder when it loads (retried each tick until the projects are there). A task's and a workflow's `project:` checked the same way in that test: they were already kept as written |
| G12 | `cli-core a_switch_takes_true_or_false_after_it_audit_g12` (seen failing) + live CLI | `--paused false` read "false" as the member → `paused: false` (also `=false`, `off`, `--no-paused`). By name worked already. Then: `wisp crew pause|resume <member>`, by name or id (`serve_cli_audit_g crew_pause_and_resume_…_audit_g12`, seen failing: printed the usage) |
| G13 | `shell factory-stop-delete.test` + band dialog | Stop the run stopped at once → asks "Stop the factory run?" (task head and band ⋯). Delete on a run says its stations' tasks and clean worktrees go too |

## Smells

| # | Test | Before → after |
|---|---|---|
| G14 | `goal/ui suggest.test: a name typed whole shows no menu…` | the first Enter picked the same text → no menu for an exact name, so Enter sends |
| G15 | `factory-core the_runs_words_…_audit_g15`, `serve_factory` (ship output, `wisp ls`), `factory/ui band.test` (endWhy) | "round(s)" → "1 round"/"3 rounds"; "exited 1:." → "(it printed nothing)"; ship failure once, not twice; "merged: https://…/pull/2" → "merged PR #2"; `wisp stop` on a waiting run → "you stopped it"; `wisp ls` → "Factory · default". Then: an ended run's task says Finished/Failed/Blocked (a `runtime.summary@1` field may set the row's `status` when the task's own turns say none; `runtime-core a_plugins_status_is_the_rows_…_audit_g15`, `factory-core an_ended_runs_task_says_how_it_ended_audit_g15`) and the band's state pill shows only while the run goes (`factory/ui band.test`, seen failing). Past the review limit with no verdict: "The review gave no verdict after 1 round: What the reviewers said: …" (`factory-core the_review_limit_says_no_verdict_…_audit_g15`, seen failing) |
| G16 | `wisp show_says_the_goal…` (+ G2's core test) | "after 1 rounds" → "after 1 round"; `wisp show` no longer repeats the question. Then (the owner's delegate's decision): `wisp goal set <task> nevr` → "no goal named nevr: goals has green-build, never; use --text \"…\" for words", nothing set; one word is a name, `--text` (or several words, which no name is) is words (`serve_cli_audit_g goal_set_…_audit_g16`, seen failing; `wisp cli::goal_set_sends_one_word_as_a_name_audit_g16`). The API is unchanged: the CLI sends the word as `name` |
| G17 | `resources-core formula`, `kernel developer_notes_…` | "it starts with `wisp: 1`" → "(a formula starts with `wisp: 1`)"; "1 resources" → "1 resource"; `runtime answer --help` fields have words; `crew update --help` opens with what it does; `factory stop <id>`. Then: `wisp fresh <id> -d` / `--detach` starts the thread, as `rerun` takes `-d` (`serve_cli_audit_g fresh_takes_detach_…_audit_g17`, seen failing: exit 2 and the usage) |
| G18 | `serve_mcp_tools …audit_g7` (unknown tool), `runtime-core setup` | "Method not found: tools/call" → "Unknown tool: X"; `{"name":"Eve"}` → "…: it takes member" |
| G19 | `termctx-core a_new_pane_on_a_reused_tty_lists_only_its_own_commands_audit_g19` | a closed pane's commands → the hooks record `pid` (zsh/bash `$$`, fish `$fish_pid`); the CLI keeps its ancestor shell's lines. Older pid-less logs are read as before. Users re-run `wisp shell-init` to get the pid |
| G20 | `factory-core` (Merged.local), `serve_factory` (e2e shows it) | "merged: …" only → "merged PR #1 · Local main was not updated: … Commit or stash them, then pull." |
| G21 | `serve_mcp_tools a_setup_card_goes_with_its_agent_on_a_restart_audit_g21` | the card outlived its agent → taken off at the restart; the turn says "its card asking to … was taken back: nothing changed" |
| G22 | Chrome at 1280, light and dark | 3-line hints, flyout at y=3 → hints clamped to 2 lines (kit `menu-item-hint`); the flyout 8 px from the top (580 px tall with 12 goals) |

## Audit E and D items

| # | Test | Before → after |
|---|---|---|
| E5 | `ship/ui pr.test: a failed review says so and Review again shows Reviewing at once_audit_E5` | "Not reviewed" → "Review failed" ("Review waits" when held), its why in the title; the click's patch reads "Reviewing" |
| E6 | `worktrees --test look_publish a_look_over_sixty_worktrees_ends_whatever_du_takes_audit_e6` (seen failing: cut at its 10 s limit) | `du` (120 s) came before the worktree checks inside a 90 s look, so every look was cut and restarted (`looking: true, last: null`) → `du` runs in its own task and writes the sizes when it ends; the look checks all 60 worktrees and ends |
| E11 | `kernel developer_notes_are_left_out_of_a_doc_audit_e11` (extended) | `(ADR-…)`, `(audit …)`, `(review …)`, `(owner-list …)` and "v1's …" are left out of `--help` and tool descriptions. Then: a catalogue-wide check, `serve_cli_audit_g no_commands_help_carries_a_developers_note_audit_e11` (seen failing: 5 left: `crew.create` id "for a frame", `goal.set` judges "(B49: …)" and a broken "Additive (…, B44: …)", "v1's" in `connectors.agent.list`, `router.set`, `runtime.import`). `public()` drops "(B49: …)", "(#31 …)" and a whole `Additive (…)` (kernel test extended); the three "v1's" docs and `crew create`'s id are reworded at the source |
| E14 | `ui controls-tabs.test fit_drops_the_labelled_tabs_icons_before_any_tab_loses_its_words_audit_e14` (seen failing) + Chrome on the mock (seed `ask`) at 1280 and 1440, light and dark | Reproduced in the mock at 1280: panel 396 px, tab row 297 px, "Changes 2" and "PR #812 ✓" labelled, then Shell, Preview, a pinned app and Thread as 28 px icons. The tight row needs ~322 px with "PR #2" and one Thread tab (the audit's), 297 + 8 there. The kit's `fit` gets one more step before a tab loses its words: the labelled tabs drop their icons (`data-bare`, −17 px each). The audit's case fits (254 of 305). After: the mock keeps Changes 2, PR #812 ✓ and Shell labelled (Shell was an icon); its Preview is still an icon, 6 px short, because that row is wider than the audit's (a 91 px PR label and two minor tabs). 1440 is unchanged (every tab labelled, with icons). The ⋯ beside the tabs stays: the 2026-11-18 UX decision put it there |
| D6 | `mcp-core an_unknown_field_is_refused_naming_the_fields_audit_d6`, `kernel an_empty_struct_is_an_object_of_no_fields_audit_d6`, live `tools/list` | 60 tools with only `{args, peer}` → 3 (`factory_set`, `resources_plan`, `resources_read`). The causes: empty structs (`Record<symbol, never>`) and bare `Value` read as "not an object"; stuff, connectors and plugins arguments had no TS type. A field a closed type doesn't take is refused ("stuff_get has no field idd: it takes id"), not dropped |

## Gates (all run)

- `npm run typecheck`: 4 programs, 0 errors.
- `npm run check`: 0 errors.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `npx vitest run`: 2,122 passed, 1 failed (strings-split). That one was fixed and its file re-run (63 passed); the full suite was not re-run.
- `cargo test -p` over 26 crates (wisp, its serve tests, serve_cli_verbs and serve_docs included): 1,466 passed, 2 failed (serve_factory, serve_mcp_callable, both expecting the old behaviour). Both were updated and re-run green.
- `node tools/budgets.ts`: pass, no ratchet raised. Desktop first-screen JS 172.1/173.0 KB, CSS 28.8/30.0 KB.
- `npm run build -w apps/docs`: 56 pages.

## Round 2: the items left open (worktree at 29d51509, uncommitted), 2026-11-20

G11, G12, G15 (both), G16, G17, E11 and E14 above, each with its test. Gates, all run on the final tree:

- `npm run typecheck`: 4 programs, 0 errors.
- `npm run check`: 0 errors.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean; after the review's fixes, again on the three crates
  they touched (wisp, wisp-kernel, wisp-automations): clean.
- `npx vitest run`: exit 0, 495 files, 2,124 passed.
- `cargo test --no-fail-fast -p wisp -p wisp-kernel -p wisp-automations -p wisp-factory-core -p wisp-factory -p wisp-runtime-core -p wisp-runtime-api -p wisp-goal-api -p wisp-crew-api -p wisp-router-api -p wisp-connectors-api`: exit 0, 1,011 passed, 0 failed, 1 ignored (66 serve binaries).
- `node tools/budgets.ts`: pass, no ratchet raised. Desktop first-screen JS 172.1/173.0 KB, CSS 28.8/30.0 KB.

A review of the diff found three, fixed: `public()` left ".:" after "Additive (…):" and "." for a doc that was only the
note (kernel test extended); a trigger load could race an apply's and a failed name was dropped by apply (the loads now
take turns, and any load that can't read a name marks the engine, so the tick loads again); the "use --text" hint was
added to every error, now only to "no goal named".
