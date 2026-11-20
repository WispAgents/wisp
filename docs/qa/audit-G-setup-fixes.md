# Audit G, setup: fixes (worktree at dbf3cd55, uncommitted), 2026-11-20

Each item lists its test, then before → after. "Seen failing" means the test ran on the old code and failed. A test
whose API is new (a new function or field) can't run on the old code; for those, the audit's repro is the before. Live
checks ran on a scratch `wisp serve :4961` (home `/tmp/gs1/home`, fake harnesses, web built from this tree). That
server is stopped.

## Security first

| # | Test | Before → after |
|---|---|---|
| GS4 | `wisp-share-core assign::tests` (4: `a_secret_assigned_inline_is_masked_and_its_name_kept`, `names_that_only_look_alike_and_references_are_kept`, `secret_flags_are_masked_where_the_tool_takes_the_value`, `output_masks_assignments_but_not_flags`); `wisp termctx_cli::tests::a_secret_typed_inline_in_a_command_is_masked` | `DB_PASSWORD=hunter2qa npm run migrate` went into the prompt as typed → `DB_PASSWORD=<secret> npm run migrate`. Covered: `export`/`env` assignments, `--password v`, `--api-key=v`, `mysql -pVALUE` (`mysql -p db` is left alone: it prompts, and `db` is the database), `sshpass -p v` and `redis-cli -a v`. `NAME=value` lines in output are masked too. References (`$TOKEN`) and look-alike names (`BYPASS`, `AUTHOR`, `PWD`, `NODE_ENV`) are kept. The name rule is `wisp_share_core::secret_name`; the share's `secret_env` now uses it too, which adds `*PASS`, `AUTH`, `PASSPHRASE` |

## Mediums

| # | Test | Before → after |
|---|---|---|
| GS5 | `wisp-runtime more::setup::tests::a_stale_write_never_takes_a_setup_card_off_its_task` (seen failing), `…::a_setup_card_taken_back_answers_its_agent` (seen failing) | **Cause:** `write_session` saved whatever session copy it was given. A write from a copy read before the card went up (no task lock) dropped `ask`, while `asks` still held its id, so `pollAnswer` said `pending` for good. That matches the audit: `ask: null` with the poll pending. Separately, a turn's end or a stop removed a card silently, and its agent read `cancelled`. **Fix:** `Runtime::keep_open_ask` runs on every write. A question that `asks` still holds for the task, but that the write leaves out, is put back from the stored row, with the task still Waiting. Every real take-back (answer, `cancel_asks`, `unask`, finish, the restart's `take_setup_card`) clears `asks` first. A card taken back by a stop or a turn's end now gives its agent an answer ("…taken back before the user answered it…: nothing changed"), and the turn gets a row saying so. The writer that dropped it in the audit wasn't identified (50 `persist` sites); the guard covers all of them |
| GS1 | `wisp-worktrees --test flows a_folder_that_is_a_project_is_never_offered_to_bring_in` (seen failing) | **Not reproduced.** Tried: a fresh home with a linked worktree, a copy of the audit's own home (`/tmp/qaG2/home`), the plugin switched off and on twice, new tasks and Clean in between. Each time `worktrees_offer` got the row within 5 s and the pane showed one offer (Chrome, 1280). The audit's DB also holds the `repo3-wt2` offer now. **Found beside it, fixed:** with a linked worktree that is also a project (as `repo3-wt` was after action 30), each project offered the other's folder. The main checkout was offered to be brought in as a task. A project's own folder is now never offered |
| GS2 | `wisp-channels --test report a_question_notification_carries_its_options_as_answer_buttons` (seen failing: `left: []`) | A notification (`on: asks`) for a task from elsewhere had no actions → the open ask's options become signed answer buttons named by the ask (`report::answer_buttons`, shared with the channel's own report). An ask only wisp can answer (an approval, a clash, a connector's keys) gets none. ntfy, Telegram, Slack and Discord already render `Msg.actions` |
| GS3 | `wisp-runtime-core setup::tests::a_member_works_in_the_tasks_project_never_its_worktree_setup_card_member_folder`; live | Default folder was the task's worktree (`…/worktrees/repo2-01a1…`) → the project's folder (`ext["wisp.worktrees"].root`), or no folder when the task has only a worktree. A `cwd` under `<home>/worktrees` is refused on add and edit, naming the project's folder. A crew import from a link uses the same folder. Live: `cwd: /private/tmp/gs1/repo` on the card; `cwd: /tmp/gs1/home/worktrees/x` refused |

## Lows

| # | Test | Before → after |
|---|---|---|
| GS6 | `serve_setup_d a_formula_is_read_as_the_ui_reads_it_audit_d` (new lines) | `agents.a1.harness: auto`, `tasks.default.agent: auto` and `tasks.t1.agent: auto` validated with Auto off → refused: "agents.a1: harness: auto: Auto is off (runtime.auto): …" and "no agent named auto", with no `auto` in the "(or …)" list. Auto on: it validates as before. `CrudKind::words_while` (additive, resources api; `monoflow graph impact`: 21 dependents). The runtime still runs an old `agent: auto` on the default harness while Auto is off (the owner's rule, `auto_off.rs`) |
| GS7 | `wisp-connectors pane::tests::a_connectors_last_check_is_read_back_after_a_restart` | After a restart every connector read "Not checked yet" → each row's stored `test` is read back at start, before the first publish |
| GS8 | `wisp-cli-core verbs::tests::a_commands_own_args_field_takes_its_flag_audit_gs8` | `--args /x/mcp-echo.mjs` → "--args takes a JSON object" → it sets the command's own `args` field (`A,B`). A JSON object still sets every field at once |
| GS9 | `wisp update_cli::tests::a_failed_install_says_why_and_exits_1_audit_gs9` | A checksum mismatch printed the whole view and exited 0 → "✗ <error>", exit 1 (`--json` still prints the view, exit 1) |
| GS10 | `serve_cli_verbs every_command_word_is_a_command_a_strict_group_or_a_prompt_and_says_so` (new lines: 9 groups) | `connectors`, `plugin`, … printed the whole usage, `settings` printed "needs more", `machines`/`cloud`/`update` one line → `wisp <group> --help` is byte for byte `wisp help <group>`. `wisp help settings` gives its verbs, not "needs more" |
| GS11 | `wisp-cli-core completions::tests::zsh_and_fish_complete_what_bash_does_audit_gs11`; `zsh -n`, `bash -n` on the generated scripts | zsh and fish had no setting keys or values, no harnesses after `--agent`, and no built-in group verbs; zsh labelled every first word "a wisp command" → the same completions as bash, and no label where there's none to give. fish isn't installed here, so its script was only checked by string |
| GS12 | `wisp-runtime more::setup::tests::an_asking_tasks_row_says_its_question`; live (sidebar and Home, 1280) | **Cause:** `summary_of` never filled `RowFacts.ask`, so a row's `live`, `question`, `options` and `asks` were always empty. "No question asked." was the fake agent's last answer. Now the row reads "Asks: Claude Code wants to add the connector “agent_echo”: …" (a setup card's headline, or the question), with its options and approval flag, as v1's `live` did |
| GS13 | `wisp-runtime-core setup::tests::a_local_connector_shows_the_command_it_runs_verbatim_setup_card_connector` (updated), `shell-task setup-card.test`; live (1280 and 390, light and dark) | "…the connector `agent_echo`: runs `node …`" → "…the connector “agent_echo”: runs node …". The command is still shown in mono on its own line. Cards stored before this change fail their digest on Allow ("This card changed after it was shown …: Ask again"), so they never run |
| F12 | (doc only, no test) | `signin.rs`'s module header said signing in again "makes it current (ADR-0061)" → it is added beside the current one; only the first account, or a sign-in made to put this computer on, becomes current (ADR-0109) |
| E8 | none needed: it already passes | Live on HEAD's own bundle: QA Work → ⋯ → Sign out → reopen ⋯: "Rename · Make it a personal account · Move up · Move down (disabled) · **Sign out (disabled)** · Remove". That is v1's shape (the item is listed and disabled), and `words.ts accountMenu` is covered by `accounts-audit.test.ts`. The audit listed the items' labels without reading their disabled state |

## Gates

- `npm run typecheck`: 0 errors. `npm run check`: 0 errors (soft size warnings only). `npx vitest run`: 495 files,
  2124 tests passed. `node tools/budgets.ts`: passes (initial JS 99.0 KB, first-screen JS 172.1 of 173.0 KB).
- `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -D warnings` clean.
- `--target x86_64-pc-windows-msvc`: clean for `wisp-share-core`, `wisp-cli-core`, `wisp-runtime-core` and
  `wisp-channels-core`. The other touched crates need sqlite, zstd and blake3, whose C builds don't cross-compile on
  this Mac.
- `cargo test --no-fail-fast -p wisp-share-core -p wisp-cli-core -p wisp-runtime-core -p wisp-runtime
  -p wisp-channels-core -p wisp-channels -p wisp-connectors -p wisp-resources-api -p wisp-worktrees -p wisp-grid-client
  -p wisp`: 1049 passed, 0 failed, 3 ignored.
