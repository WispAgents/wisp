# ADR-0132: the Steward keeps a task's PR going until it's green: it fixes failing checks and new review threads within a limit, and another vendor's model reviews each push

- Status: accepted (owner, 2026-11-26: "make sure it's a toggle in the ship modal and implement it")
- Date: 2026-11-26
- Source: Check-in 50 (2026-11-26), idea 1. Claude Ship (2026-11-23, HN 50618513, 90 points, 36 comments) adds a
  PR Steward that fixes failing CI, conflicts and review feedback. It is Team/Enterprise only, GitHub only, and
  has no ZDR/HIPAA. Its `/ship` shadows the user's own skills (CC#111946, +5). HN asks for review by another vendor:
  "Claude and Codex in an adversarial loop", "layer model families".

## Context

wisp already has every part of this loop except the loop itself:

- `wisp.ship` follows each task's open PR: every 60 s, or every 10 s while it settles (`ship-core::follow`). With
  `ship.fixConflicts` on, it sends the task a follow-up to merge its base. It first checks the conflict with git
  (`merge-tree`), sends at most once per PR head and 3 times per task (`MAX_CONFLICT_FIXES`), and pushes the
  result itself.
- `wisp.review` reviews a PR with another harness when its head changes (`review.maybe`,
  `review-core::pick_reviewer`). "Another" only means a different harness id. Copilot reviewing Claude Code's PR
  on Copilot's Claude default is the same model checking its own work.
- The refusal card already knows whose model a harness runs (`shell-task/ui/src/cards/vendors.ts`, v1
  `vendors.js`). That table exists only in the UI.
- A failing check is only an automation source (`ci.failed`, wisp.github) and a red row on the PR card. Nothing
  sends it back to the task. Review comments from people are never read: `PrSummary.review` keeps only GitHub's
  overall decision.

So today a user watches the PR, copies the failing log or the reviewer's comment into a follow-up, and waits.
Claude Ship does that for the paid plans of one vendor, on GitHub, and reviews with the same model. wisp can do it
on any plan and any harness, locally, with a reviewer from another vendor.

## Decision

1. **The Steward is a mode of `wisp.ship`'s follow loop, not a new plugin.** It runs on the poll that already
   exists, with no new timer. The setting is `ship.steward`: `off` (the default, D-0023), `fix` (checks, threads,
   conflicts) or `fix+review` (the same, plus the review loop in rule 5). A task can override it in its spec
   (`steward:` in `wisp.yml` / task specs, ADR-0027), with `wisp ship --steward[=fix|review|off]`, with the MCP tool
   `ship_steward {id, mode}`, or from the PR card. When the Steward is on, it includes `ship.fixConflicts`: the
   existing conflict fix becomes one of its causes, with the limits it already has.

2. **Three causes, each read on the PR's current head:**
   - **Failing checks.** One or more checks have settled to `fail` and none are `pending`. wisp reads the failed
     jobs' logs through the existing `ship.check_log` read (`gh run view <id> --log-failed`): the last 200 lines of each failed job, 16 KB in total, with
     ANSI codes stripped and secrets masked as GitHub masks them. It also reads the check names and their links.
     It does not read checks the repo marks as not required, unless the user turns
     `ship.steward.optionalChecks` on.
   - **New review threads.** GraphQL `reviewThreads`: unresolved, not outdated, created or answered since the last
     Steward pass. Each one is read with its path, line, the diff hunk and the whole conversation. Threads written
     by bots count only when the bot is listed in `ship.steward.bots` (empty by default). A thread whose last
     message is the task's own author (the user) answering the reviewer counts as news.
   - **Conflicts.** The same as today: git is asked first, and the existing prompt is used.

3. **One follow-up per pass, with every cause it found.** The prompt is built in `ship-core::text`, like
   `conflict_prompt`. It lists the failing checks with their log tails, then the threads quoted with `path:line`,
   then the conflict steps. It asks for the smallest change that fixes each one. It also says: don't push (wisp
   does), don't resolve or answer threads on GitHub, and ask the user when a fix needs their decision. wisp
   pushes the new commits as the conflict fix does: a plain push, never a force push. A rejected push stops the
   Steward (rule 6).

4. **Limits, so a moving target can't run up the bill.**
   - At most one pass per cause per PR head.
   - At most 3 passes per cause per task (the conflict limit, shared).
   - At most 6 passes per task in total.
   - Each pass is unattended work, so it goes through `budget.hold` like an automation. A plan near its line
     holds the pass and the card says so.
   - A task that is running, waiting for an answer, shipping, archived or starting is skipped, the same set the
     conflict fix skips (`follow::Busy`).
   - **No retries.** The Steward never re-runs a failed check to make it green (D-0053's rule, applied to the
     user's CI). Before it spends a turn, wisp checks the same check on the base branch's latest commit. If it
     fails there too, the card says "fails on `main` too" and nothing is sent.

5. **Review by another vendor (`fix+review`).**
   - The vendor table moves to Rust (`runtime-core::vendor`, the same regexes and `HOME` map). The UI copy is
     checked against it by a golden (`test/goldens/vendors.json`), so the two can't drift.
   - `pick_reviewer` gets the setting `other-vendor`: a ready harness whose configured model's vendor is known
     and is not the author's vendor. Failing that, it takes the first model the harness lists from another
     vendor, pinned on the review run. If no harness qualifies, the review is skipped with that reason; it never
     falls back to the same vendor. `fix+review` uses `other-vendor`. Plain `review.harness` keeps today's
     meaning.
   - After each push the Steward makes, and on the PR's first head, `review.maybe` runs as it does today. Its
     **high** findings, constraint breaks included, are a fourth cause for the next pass. They count against the
     same limits and are quoted like threads. Medium and low findings stay on the review card.
   - The loop is adversarial in this way only: the reviewer never edits code and the author never grades itself.

6. **When it stops.** Green means: every required check passes, no thread is unresolved, and (with `fix+review`)
   the last review has no high finding. Then the card shows "Green · reviewed by <harness> (<model>)" and the
   task goes to Needs you, unread. **The Steward never merges.** Merging stays the user's (Merge on the card,
   `wisp ship`, `ship_merge`).
   The Steward also stops, and the task goes to Needs you with the reason, when:
   - a limit is reached;
   - the same failure comes back twice: the same check with the same log fingerprint, a hash of its error lines
     with numbers and paths normalized;
   - the failure is on the base too;
   - the agent asks a question;
   - a push is rejected;
   - the budget holds the pass;
   - the PR is closed, merged or turned back into a draft.

7. **State: one additive record.** `ShipTask.steward: Option<Steward { mode, step, passes: {checks, threads,
   review, conflicts}, last_head, seen_threads, fingerprints, why }>`. `ConflictFix` stays as it is (wire
   compatible) and its count is mirrored into `passes.conflicts`. After a restart, a pass that was pushing is
   marked failed, as the conflict fix is today (`after_restart`). Changes go through ship's commands in one
   transaction (ADR-0004). Thread ids and fingerprints are capped at 200 per task.

8. **GitHub first, behind a port.** Reading checks, logs and threads goes through `ship`'s `gh` module behind a
   `ForgePort` trait (`checks_failed`, `failed_logs`, `open_threads`). A later GitLab or Gitea forge implements the
   trait. No other part of the loop knows the forge.

## As built (2026-11-26)

The owner asked for the Steward as a toggle in the Ship it dialog. Where the build differs from the rules above,
this section is the decision:

- **The toggle.** Ship it has a **Keep it green** box and, under it, **Another vendor reviews each
  push**. They start from the task's own Steward, else from Settings → Ship's `ship.steward` (`off` by default).
  `ship.ship` takes `steward: fix | review | off` and switches the Steward on once the PR is there. The PR row has
  the one control that fits: **Stop Steward**, **Start Steward again** or **Keep green**. It also has a line saying
  what the Steward is doing. The CLI flag is `wisp ship <id> --steward fix|review|off`. The MCP tool is the
  `ship.steward` command's. A separate `wisp steward` word was not added, because the flag covers it.
- **Which checks count.** gh's `statusCheckRollup` doesn't say which checks are required, so every failing check
  counts and `ship.steward.optionalChecks` doesn't exist.
- **Bot threads** never count, and `ship.steward.bots` doesn't exist. A thread whose only comments are from bots is
  left out, and bots' comments are dropped from the threads that are sent.
- **Green ignores threads already sent.** Only a person resolves a thread, so a thread sent once and still
  unresolved doesn't hold back green. An answer in it makes it news again.
- **Waiting.** The Steward waits until every check has settled and until the conflict fix (which goes first) is
  done. Then one pass carries every cause.
- **Starting again after a stop resets the counts and fingerprints.** The threads already sent stay sent. Without
  the reset, the Steward would stop again at once.
- **A pass whose turn changed nothing** stops the Steward and says so (the agent may have answered instead).
- **Review by another vendor.** `review.maybe` takes `pick` (the setting, this time only). wisp.review also reads
  the ship slot's `steward.mode`, so whoever asks (a turn's end, a new head) gets `other-vendor` while the
  Steward reviews. `other-vendor` is not a value of the `review.harness` setting. The vendor table is
  `review-core::vendor` (not runtime-core: only review picks by vendor). The golden is
  `plugins/review/core/tests/goldens/vendors.json`, generated from the UI's `vendorOf`, and each side has a test
  that reads it.

## Consequences

- One loop instead of the user's copy and paste, on any harness and any plan, with no cloud and no team plan. The
  cost is the turns. The limits (6 passes per task, 3 per cause) and `budget.hold` bound it, and a repeated failure
  stops it early.
- Moving the vendor table to Rust touches `@wisp/protocol` only through the golden. The UI keeps its own
  DOM-free copy for the cards.
- Reading `--log-failed` adds one `gh` call per failed job and pass. Nothing is read while a check is pending or
  when the Steward is off, so `ship`'s poll cost doesn't change for everyone else.
- Out of scope: the Steward answering or resolving threads on GitHub (that's publishing on the user's behalf;
  a later ADR may add it behind its own switch), re-running checks, rebasing, and merging.
- UI: the PR card gets a Steward line (step, passes used, Stop) and the green line. Per D-0056, the mock with
  screenshots goes to the owner before it's built, made from the existing kit parts (`Banner`, the PR panel's rows).

## Places it reaches (ADR-0040)

- Inside wisp: Settings → Ship → **Steward**; the PR card's line and Stop; the review card says which vendor
  reviewed.
- From the harness: the MCP tools `ship_steward {id, mode}` (new) and `ship_view` (it gains the `steward` record); the steward step is part of
  `runtime_turns` for each pass.
- CLI: `wisp ship --steward`, `wisp steward <task> on|off|status`.
- As code: `steward:` in task specs and `wisp.yml`, plus `ship.steward*` in exported settings.
- CI: the existing `ci.failed` source can wake a stopped Steward (`automations` action `ship.steward.resume`).
  The GitHub Action needs nothing new.
- Chat channels: the stop and green lines go out as the task's notifications already do.

## Edge cases and tests

`ship-core` (pure, model-based where there's a state machine):

- `steward_skips_pending_checks_and_runs_once_per_head`
- `steward_caps_passes_per_cause_and_per_task`
- `steward_stops_when_the_same_fingerprint_fails_twice`
- `steward_sends_nothing_when_the_check_fails_on_the_base_too`
- `steward_never_reruns_a_check`
- `threads_count_only_unresolved_not_outdated_and_new_since_the_last_pass`
- `bot_threads_count_only_when_listed`
- `the_users_reply_in_a_thread_is_news`
- `one_follow_up_holds_every_cause_in_order_checks_threads_conflict`
- `log_tail_is_clipped_to_16kb_ansi_stripped_and_masked`
- `after_restart_a_pushing_pass_is_failed`
- `a_closed_merged_or_draft_pr_stops_the_steward`
- `busy_tasks_are_skipped_as_the_conflict_fix_skips_them`

`review-core`:

- `other_vendor_never_picks_the_authors_vendor`
- `other_vendor_pins_a_listed_model_from_another_vendor`
- `other_vendor_with_none_skips_with_the_reason`

Golden:

- `vendors.json` checks that `runtime-core::vendor` and `shell-task` `vendorOf` agree on every fixture model id.

Host (fake `ForgePort`, fake harness):

- A red check leads to one follow-up, a push, then green: the card is green and nothing merges.
- A high review finding sends one more pass; a medium one doesn't.
- `budget.hold` holds a pass and the card says why.
- A rejected push stops the Steward with the reason.
