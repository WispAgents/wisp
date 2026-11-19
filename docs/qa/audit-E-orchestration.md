# Audit E: orchestration, every action end to end (worktree HEAD e0cb033e), 2026-11-19

Re-runs audit D's table (docs/qa/audit-D-orchestration.md) and re-checks each of D1–D12 at HEAD e0cb033e. Then looks
for new faults in the same areas, and in what D left untested: review by another harness (row 20) and Check logs
(row 44d).

## Setup

- **Server.** A scratch `wisp serve` on **:4962**, built at HEAD into /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins`).
  - `wisp`, `wisp-fake-agent` and `wisp-fake-gh` were copied into /tmp/qaE1/opt and /tmp/qaE1/bin and run from there, so another session building into the shared target dir couldn't swap them.
  - The worktree's only uncommitted edits are design/v1-reference images, so this build is HEAD's code.
  - Environment: WISP_HOME=/tmp/qaE1/home, HOME=/tmp/qaE1/userhome, WISP_WEB_DIR=/tmp/qaE1/web (a fresh `vite build` of apps/web at HEAD).
- **Server warnings.** wisp doesn't read RUST_LOG. Its level comes from WISP_LOG, which defaults to `warn` (docs/dev.md), so every WARN went to serve.log.
- **Fake harnesses.** Audit D's setup, moved to /tmp/qaE1:
  - codex, opencode, cursor-agent, copilot, grok and agy are the wisp-testkit fake, set through WISP_*_BIN.
  - claude goes through audit D's QA wrapper (/tmp/qaE1/bin/claude, cues `PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`/`#delegate`, `#limit`, `#hang`).
  - `wisp-fake-gh` is WISP_GH_BIN, with its state in /tmp/qaE1/state/gh.json (D's two issues; no PRs at the start). A failing check was added later for the Check logs test.
  - `FAKE_GATE` (/tmp/qaE1/gate) holds a run whose prompt starts `gate <name>` until that file exists.
- **Projects.** All are fresh git repos with D's files, made at the start of this audit (/tmp/qaE1/mkproj.sh).
  - **proj**: math.js and its test, whose bare origin is /tmp/qaE1/remote.git.
  - **proj2, proj5–proj8**: each commits `.claude/settings.json` (a PreToolUse hook `echo hi`) and `.mcp.json` (server `local`), for project trust. One fresh, untrusted copy per hold test.
  - **proj3**: commits `.wisp/worktree.json` `{"setup":"touch setup-ran; exit 3"}`.
  - **proj4**: origin `https://github.com/acme/repo.git`, for issues.
- **No outbound traffic.** DO_NOT_TRACK=1, WISP_TELEMETRY=0 and WISP_UPDATE_CHECK=0. The PostHog, grid, Linear, Slack, Discord and Telegram URLs pointed at 127.0.0.1:9.
- **No real harness.** The server's only children were the fakes.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800.
  - Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation.
  - D's scripts were reused, with lib.mjs changed so every exit path, including a thrown error or the global timeout, closes the browser first.
- **Interfaces used.** The CLI, `/api/cmd`, `/api/mcp` as the user and as a run (its `x-wisp-session`/`x-wisp-token`), the stdio `wisp mcp`, and the UI.
- **Artifacts.** In /tmp/qa-keep/shots-qaE1/: screenshots, qaE1-serve.log and qaE1-calls.jsonl.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass, with finding E1 | In a project wisp knows, each fake got the prompt in its own `proj-<id>` worktree and the project stayed clean. Before `wisp add`, all 9 CLI tasks ran in place in /tmp/qaE1/proj and edited it (E1) |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome, WebKit) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit`, in place; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox` |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`, from the CLI and from the UI slider ("High thinking"); `--compare codex:gpt-5` → `-m gpt-5` |
| 4 | Worktree vs in place | CLI (`--no-worktree`), UI (Where menu) | pass | `--no-worktree` runs in proj. Where › In place stayed sticky for the next UI launch (ADR-0108, deliberate) |
| 5 | Compare | CLI, UI (Chrome) | pass | claude + codex, each in its own worktree; UI 2 ms, toast "B25C compare ui is ready (2 runs)" |
| 6 | Try (subagents=haiku) | CLI, UI (Chrome) | pass | CLI: A and B in separate worktrees, B has `subagentModel: haiku`, "differs: subagentModel → haiku"; `--try` without `--agent` is refused with its reason. UI: Haiku pick 2 ms, send 3 ms, `runtime.trial` |
| 7 | Attach files | UI (Chrome, WebKit) | pass | stdin has `<attachments>`; argv has `--add-dir …/stuff/files/<hash>` |
| 8 | Goal and judges | CLI (`wisp goal set --kick`), UI | pass | PASSAFTER2 → `wisp show`: "◎ Goal: PASSAFTER2 sums work · met after 2 rounds" (D10 fixed). UI goal launch 2–8 ms |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | "Add delegate › With the lead" adds Codex; chip "1 delegate"; launch 3 ms |
| 9b | Delegate a job to the team | per-run MCP | pass | The delegate ran: toast "B delegated: run the tests is ready". Ada's job's `delegate` tool has `subagent` enum `["bob"]` (row 32) |
| 10 | Follow-up queue: queue, combine, edit, drag reorder, cancel (with confirm) | CLI, UI (Chrome, WebKit) | pass | Queue 1 ms (Chrome), 3–4 ms (WebKit). Drag q3 to the top → server `q3, q1, q2` on both engines. Combine 3, edit 8 (`runtime.queue.text`), cancel asks "Cancel “D10C q4”? · Keep it · Cancel it" (3 ms, confirm 1 ms). CLI `queue order` and `queue cancel` change the server order |
| 11 | Stop keeps the queue | CLI, UI | pass | "Stop Claude Code? Your queued follow-ups stay, held until you send one." (1 ms); `queueHeld: true` after the stop, from the CLI and the UI |
| 12 | Send now | CLI, UI | pass | UI 6 ms; the item runs first, then the rest drain in order (a1, then c3, b2) |
| 13 | Start later | UI (Chrome) | pass | Toast "Starts 17:39"; the list's "Scheduled 1" section shows "B26C later ui · Scheduled task · proj · 17:39" |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI | pass | `wisp show` lists "Which database, Postgres or SQLite?" with numbered options and the reply command; `wisp reply … SQLite` → the agent got "The user answered: SQLite" |
| 15 | Usage limit | CLI | pass, with finding E8 | `wisp show`: "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent." The launch line still said "✓ … finished" (E8) |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass | Own worktree; 17 ms. `runtime_fork` over MCP makes a fork in its own worktree |
| 17 | Rerun… | UI (Chrome) | pass | "Starts a new task in a new worktree of proj on this computer."; own worktree; 13 ms |
| 17b | `wisp rerun <id> -d` | CLI | pass | Own worktree; no `rerun-*` folder left behind |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass | A new task, 11 ms |
| 19 | /btw | UI, HTTP MCP | pass | UI 17 ms; copilot's side run has its own `--session-id` and the task so far in its prompt. MCP `runtime_btw {question}` answers `pending` |
| 20 | Review by another harness | Ship, UI, API | **partial** (E5) | With `review.harness: other`, shipping a codex task started a review by Claude in the PR's worktree. The fake's answer isn't a review, and `review.view` says why. The PR strip says only "Not reviewed" before and after Review again (7 ms) |
| 21 | Ship: commit & open PR | UI, CLI | pass | Ship menu 7–8 ms, item 10 ms, toast "Opened PR #2"; CLI "✓ shipped: …/pull/1". A branch with no changes gets the disabled button and "No uncommitted changes · 0 commits on the branch" |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass, with finding E9 | "Squash-merge PR #2? Then wisp fast-forwards your local base branch and removes the worktree, unless it has uncommitted work." (2 ms; go 4 ms). The worktree was kept with precious.txt; the toast quotes a cut-off git sentence (E9) |
| 22b | `wisp ship` + `wisp merge` | CLI | pass, with finding E9 | "✓ merged PR #1 · ! Local main was not updated: … · The worktree was removed; its branch stays." (D9 fixed) |
| 23 | Restore to before latest turn | UI | pass | Confirm "Restore to before this turn? …", then the toast "The folder is back to before this turn. Undo"; 3 ms |
| 24 | Bring in | UI | **partial** (E6) | With few worktrees the offer shows 669–685 ms after opening the pane, and Bring in takes 3 ms. With 50 worktrees, a new stray worktree didn't show in 40 s, or on a second open after 30 s. It appeared well over a minute after it was made |
| 25 | Worktree removal | UI (Settings › Worktrees) | pass | "Remove this worktree? Its 2 uncommitted changes can’t be brought back."; 8 ms |
| 26 | Crew: add | CLI | pass, with findings E11, E12 | `--cron "every minute"` → `* * * * *`; `--on done` → `on task.done`. No empty fields (D8 fixed). It printed "next 1m ago" (E12); `crew create --help` shows developer notes (E11) |
| 27 | Crew: give a job | CLI, UI, HTTP MCP | pass | Each job in its own worktree; UI 9 ms; `crew assign` prints `session <id> · Ada · Job · running`; MCP `crew_assign` too |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass, with finding E7 | Cron every minute: 28 check-ins, all in `proj-ada-check-in` (tidied when clean, ADR-0108). Check in now 19 ms. On a paused member it queues silently, then says "Ada is already checking in." (E7) |
| 29 | Crew: pause / resume | UI, API | pass | `paused` toggles; 8 ms |
| 30 | Crew: tint | UI | pass | Pick 3 ms; Save 21 ms → `bg: teal`, and the list turns teal |
| 31 | Crew: wakes on "I mark a task done" | API (`runtime.setState done`) | pass | "Bob · Job started by trigger:crew.bob.1", codex, in its own worktree |
| 32 | Crew: Org link → the job gets its delegate | CLI | pass, with finding E13 | `crew update ada --team '[{"crew":"bob","why":"reviews"}]'`; Ada's job's `delegate` has `subagent` enum `["bob"]`. The team table lists Bob's agent as "auto" (E13) |
| 33 | `wisp ls --crew ada` | CLI | pass | Lists Ada's jobs and check-ins with `proj` in the project column (D7 fixed) |
| 34 | Recipes: new, edit, run, delete | UI | pass | Create 1 ms; Run 5 ms, the dialog's Project is proj, the run is in a worktree, toast "E34-Fix-Thing is ready"; Edit saves; Delete asks "Delete the recipe “E34-Fix-Thing”? It goes for good…" (1 ms) |
| 35 | Triggers: new schedule, fires, on/off, edit, delete | UI, API | pass | D's script made "uitrig" every minute; it fired 3 times into `proj-trigger-uitrig` (tidied when clean); the switch took 22 ms and is `enabled: false` on the server; the edit saved; delete asks first, 1 ms; the list is empty after |
| 36 | Notes, Fresh thread | CLI | pass | `wisp notes <id> --set goal=…`; `wisp fresh` runs without `--resume`, with the Notes in its brief |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | not re-checked | — |
| 37 | User MCP (`wisp mcp`, stdio): launch | MCP | pass | 250 tools; `wisp_launch {cwd}` runs in a worktree |
| 38 | HTTP MCP: fork, crew_assign, btw | MCP | pass | The fork is in its own worktree, the crew job runs in its own worktree, btw answers `pending` |
| 38b | HTTP MCP: every listed tool is callable | MCP (user and run) | **partial** (D6, E10) | No-argument calls now work (`crew_list {}`, `later_plans {}` …). 82 of 246 tools still name no fields. 25 run calls with `{}` fail with serde words ("missing field `sessionId`"). The documented `args` form is ignored for `crew_agent_list` (E10). No "doesn't declare" refusals; the 2 hits were tool docs |
| 39 | A task launched by an agent | per-run MCP | pass | `wisp_launch {worktree:true}`, `{project:"proj"}` and `{cwd, effort:"high"}` each run in a worktree; effort reaches codex |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 4 ms; the task stays running |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | **fail** (E2) | No horizontal overflow on Home, a task or the crew page. The trust-hold card collapses on both engines (E2) |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured action took 1–22 ms. Two exceptions: Check in now on a paused member took 62 ms (E7), and "Logs for test" opens the check outside wisp |
| 43 | `wisp help runtime "queue order"` | CLI | pass | Prints the verb's help |
| 44a | Try from the UI | UI (Chrome) | pass | See row 6 |
| 44b | A held turn's "Trust and start" | UI, CLI | pass, with finding E3 | UI 8 ms → the turn ran (`done`); CLI `wisp held <id> start` → "✓ started", and a second task on the same repo isn't held (D1 fixed). The card's layout and missing file names are E3 |
| 44c | Start from an issue | UI (Chrome) | pass | proj4's picker lists "#7 Sum is wrong for negatives · bug" and "#8" (7 ms); picking #7 (6 ms) then Send → "Fix #7: Sum is wrong for negatives" in a proj4 worktree |
| 44d | Check logs / Ask to fix (a failing check) | UI | **fail** (E4) | The PR strip says "1 check failing" and the PR tab lists "✕ test" with only "Logs for test", which opens the check's page outside wisp. There is no Ask to fix |
| 44e | A held turn's "Skip" | CLI | pass | "● Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." (D3 fixed) |
| 44f | A held turn's "Run without them" (new) | CLI | pass | "✓ started without the repo's own programs"; claude got `--setting-sources user` and `--strict-mcp-config` |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI | pass | All 26 turns ran, in the order sent (`f1 … f25`), the queue empty. (Sent in parallel they ran in arrival order; not counted) |
| 46 | Crew check-ins and schedule triggers over time | CLI, git | pass | ~28 minutes at every minute: every check-in in `proj-ada-check-in`, every trigger run in `proj-trigger-uitrig`; no growth |
| 47 | A project's worktree setup (`.wisp/worktree.json`) | CLI, API | **fail** (D5, part) | Over `runtime.create` with mode `auto` the setup runs (`setup-ran`), fails and holds the turn with why. From the CLI, even with `--mode auto`, `prepared.held` lists the setup but the turn runs straight to the harness, and the setup never runs |
| 48 | Server warnings (serve.log) | — | pass | 6 WARNs in ~130 tasks: 4 from my own bad calls, 1 from the failing setup (row 47), and 1 "crew check-in … Ada is already checking in." (E7) |

**Counts:** 58 rows (D's 57 plus 44f).
- **Pass:** 51, of which 8 carry a lesser finding (rows 1, 15, 22, 22b, 26, 28, 32 and 44b).
- **Partial:** 3 (rows 20, 24 and 38b).
- **Fail:** 3 (rows 41, 44d and 47).
- **Not re-checked:** 1 (row 36b).

**Feedback (ms, pointerdown → first DOM change).**
- Launch: 3 (Chrome), 8 (WebKit); compare 2, team 3, goal and later 2–8.
- Queue: 1 (Chrome), 3–4 (WebKit).
- Combine 3, edit 8, cancel 3 (confirm 1), stop 1, send now 6.
- Fork 17, rerun 13, side 11, btw 17, restore 3.
- Ship 7–8, ship item 10, merge item 2, merge go 4, review again 7.
- Bring in 3, worktree remove 8.
- Crew: open 12–13, job 9, resume 8, check-in 19 (62 on a paused member), tint pick 3, tint save 21.
- Recipe: create 1, run 5, delete 1.
- Trigger: switch 22, delete 1.
- Done 4, Trust and start 8, Try pick 2 and send 3, project picker 7, issue picker 7, issue pick 6.

## Audit D's findings, re-checked

| D# | Finding | Status at e0cb033e | Evidence |
|---|---|---|---|
| D1 | Start now on a trust hold never starts it | **fixed** | UI Trust and start (8 ms) and CLI `wisp held <id> start` both run the turn; the next task on that repo isn't held |
| D2 | The trust hold doesn't say what it would run, no "Run without them" | **fixed** (see E3) | The card says "This repo asks to run its own programs. Held before it started: these would run on this computer, as you, whatever the mode.", lists "Hook PreToolUse (Bash) echo hi" and "MCP server local echo x", and offers Skip this run · Run without them · Trust and start. Its layout, and the file names and Open file still missing, are E2 and E3 |
| D3 | Skip on a trust hold gives a usage-limit reason | **fixed** | "Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." |
| D4 | `wisp show` says nothing about a hold | **fixed** | "◷ Held before it started: …", each item with its file, and `wisp held <id> start|without|skip` |
| D5 | A project's worktree setup never runs or holds | **partly fixed** | Fixed over `runtime.create` with a mode (the setup runs and fails → held "setup", with "exit 3"). Not from the CLI: with no `--mode`, `--mode auto` or `--mode edits` the setup is listed in `prepared.held`. In Edits the turn is held for trust; in Full auto it runs without the setup, and nothing says so |
| D6 | MCP tools without an argument schema; calls without `args` fail | **partly fixed** | `{}` now works for no-argument tools. 82 of 246 tools still have only `{args, peer}`, with no field names. Missing fields still answer serde words ("crew.get: missing field `id`"). `stuff_get {"id":"x"}` still answers `null`. `runtime_fork {"prompt": …}` silently drops the unknown field |
| D7 | `wisp ls --crew` leaves the project column empty | **fixed** | `proj` shown |
| D8 | `wisp crew create/get/update` dumps every internal field | **fixed** | Only filled fields; `team` prints as a table |
| D9 | `wisp merge` says only "✓ merged" | **fixed** | It says whether local main was updated and whether the worktree was removed (see E9 for the git sentence) |
| D10 | The CLI doesn't show a task's goal | **fixed** | `wisp show`: "◎ Goal: … · met after 2 rounds"; `goal set` without `--kick` says "next Checked when the task's next turn ends; --kick checks its last turn now." |
| D11 | Empty-list words differ | **fixed** | "No tasks." for `--crew bob` and for `--state running` |
| D12 | `automations.triggers` declared but not registered | **fixed** | No longer declared (the only WARN is from my probe) |

**D totals:** 10 fixed (D1–D4 and D7–D12) and 2 partly fixed (D5, D6).

## New findings

### E1. [major] A CLI task in a git repo that wisp doesn't know as a project runs in place and edits the checkout
- **Steps.**
  1. A fresh git repo (/tmp/qaE1/proj), not yet added to wisp.
  2. From it: `wisp "E1 hello codex" --agent codex` (and the other 6 harnesses, with and without `-d`).
- **Actual:**
  - Every run's cwd was /tmp/qaE1/proj itself (the fakes' records).
  - No worktree was made, and the repo was left with `M math.js` and `?? notes.txt`.
  - `wisp ls` still shows `proj` in the project column, so nothing hints that the run was in place.
  - After `wisp add /tmp/qaE1/proj`, the same launch gets a `proj-<id>` worktree.
- **Expected:**
  - v1 (server/services/launch.js:167, 212–216) gave any git repo a worktree unless the mode was Plan or worktrees were off.
  - ADR-0108 §4 says the CLI starts a worktree in a project.
  - At the least, a CLI launch into a checkout should say it is running in place.
- **Suspect:** `scripted::in_project` (crates/wisp/src/scripted.rs:77) returns early when the cwd is under no registered project, so `worktree` is never set.

### E2. [major] On a phone, the trust-hold card collapses to one letter per line
- **Steps.** A task held for project trust (proj7, proj8), opened at 390×844 on Chrome and on WebKit.
- **Actual** (screenshots D-held-chromium-390.png, D-held-webkit-390-p8.png):
  - The items column is 0 px wide. "PreToolUse (Bash) echo hi MCP server local echo x" runs one character per line, with letters overlapping.
  - The card is 2493 px tall on an 844 px screen.
  - Its button row runs from x 63 to 410 on a 390 px screen. WebKit shows a horizontal scrollbar.
  - The buttons sit above the fold, 1600 px up from the items.
- **Expected:** the card fits the width, as audit D's held card did (row 41).

### E3. [minor] On desktop the trust card squeezes its text and leaves out what v1 showed
- **Actual** (D-held-chromium-1280.png):
  - The card's text is pressed into a ~150 px column beside the three buttons, and "Hook" breaks mid-word ("Hoo / k").
  - No item names its file. The CLI shows `.claude/settings.json` and `.mcp.json`; the UI shows neither.
  - There is no Open file.
- **v1** (web/js/features/hold.js:161–174) shows each item with its file and an Open file button.

### E4. [major] No "Ask to fix" for a failing check (v1's Check logs)
- **Steps.**
  1. A task with PR #3; the fake gh reports a failing `test` check (`checks: [{conclusion: FAILURE, detailsUrl: …/actions/runs/42/job/4242}]`).
  2. Open the task, its PR strip and its side panel's PR tab.
- **Actual:**
  - The strip says "Open · 1 check failing". The PR tab lists "✕ test" with one button, "Logs for test", which only opens the check's page outside wisp (no DOM change, no command).
  - Nothing puts the failing check and the end of its log in the reply box.
  - `ship.check_log` (plugins/ship/ui/src/commands.ts:84, "for Ask to fix") is defined and handled on the server (flow.rs:236), but no UI calls it.
- **v1** (web/js/features/panel.js:741–752, `askToFix`) reads `/sessions/:id/check-log` and fills the reply box with "The `test` check failed on PR #3 (…). Find out why, fix it and push." and the log tail.
- **Effect:** a v1 feature is missing (owner: keep every v1 feature).

### E5. [minor] A failed review by another harness shows as "Not reviewed", and Review again shows no progress
- **Steps.** `review.harness: other`; ship a codex task. wisp starts a review by Claude in the PR's worktree. The fake's answer isn't a review.
- **Actual:**
  - `review.view` says `status: failed`, "Claude Code answered, but not with a review wisp could read. Review again to ask once more."
  - The PR strip shows only "⚠ Not reviewed", with the title "Not reviewed".
  - Clicking it sends `review.maybe` (7 ms), but the strip never shows "Reviewing" and still reads "Not reviewed" 5 s later.
  - The PR tab has no review section. v1's panel had one (`xreviewHtml`).
  - The strings for this case exist and go unused: `wisp.ship.rv.retry` "{why}. Click to try again" and `wisp.ship.rv.didntFinish`.
- **Expected:** the why, with Click to try again, and Reviewing while it runs.

### E6. [minor] With many worktrees, Bring in shows a new stray worktree only after a minute or more, and an open pane never updates
- **Steps.** About 50 wisp worktrees; `git worktree add -b stray2 /tmp/qaE1/stray2`; open Settings › Worktrees.
- **Actual:**
  - Not shown in 40 s on the first open, or in 30 s on a second open.
  - `worktrees.view` answered `looking: true` the whole time.
  - The offer row appeared well over a minute after the worktree was made. The next open showed it in 669 ms.
  - With few worktrees, the same offer showed in 685 ms (B#18's fix holds there).
- **Expected:** the look ends in seconds, and an open pane updates when it does.

### E7. [minor] "Check in now" on a paused member queues silently, then says she's already checking in
- **Steps.** Pause Ada; on her page click Check in now, then again.
- **Actual:**
  - First click: 62 ms, no toast, a check-in queued.
  - Second click: "Ada is already checking in.", and a WARN in serve.log (`wisp_crew::jobs: crew check-in … already checking in`). Nothing says she is paused.
  - After Resume, the queued check-in waited for the next cron minute (about 70 s), and Check in now was refused until then.
- **Expected:** say that Ada is paused (or check in now anyway), and start a queued check-in on Resume.

### E8. [minor] The CLI's launch line says "✓ … finished" for a held turn and for a usage-limit failure
- `wisp "E44 held untrusted edits" --mode edits -d` printed `✓ "E44 held untrusted edits" finished`, though the turn was held for trust.
- `wisp "E15 #limit"` printed `✓ "E15 #limit" finished`, though the turn failed at the usage limit (`wisp show`: "✗ … is at its usage limit").
- `wisp show` is right in both cases.

### E9. [minor] When local main can't be updated, ship and merge quote git's sentence cut off
- CLI and UI toast: "Local main was not updated: Your local changes to the following files would be overwritten by merge".
- The file list git gives is cut, the sentence has no full stop, and the capital after the colon is git's.
- Seen with a dirty main checkout (my in-place runs), from `wisp merge` and from the UI's Merge PR.

### E10. [minor] The documented `args` form is ignored by some generic MCP tools
- The generic schema says "the command's arguments, as JSON … (its fields may also be given beside `args`, at the top level)".
- As the user over HTTP:
  - `crew_agent_list {"args":{"session":"x"}}` → "crew.agent.list: missing field `session`".
  - `crew_agent_list {"session":"x"}` works.
- As a run, `crew_agent_list` and `later_agent_schedule` ask for a `session` field, though the run's `x-wisp-session` header already names it. Passing one answers "an agent's token speaks for its own task only".

### E11. [smell] Generated CLI help shows developer notes
- `wisp crew create --help` shows:
  - "`POST /crew`"
  - "the client's id for it (the rail shows it under this id at once): kept when it's an id's shape … Additive (2026-11-16)"
  - "(v1 `bg`) … Additive (2026-11-12)"
  - "checked by `normalize_kit`"
  - "v1 takes any number and floors it"

### E12. [smell] A new every-minute crew member prints "next 1m ago"
- `wisp crew create --name Ada --cron "every minute"` → `next  1m ago`.
- A later `crew get` says `next in 1m`.

### E13. [smell] The crew team table shows a member's agent as "auto"
- After `crew update ada --team '[{"crew":"bob",…}]'`: `team · bob  Bob  auto`. The stored team entry has `agent: "auto"`.
- Bob runs on codex.

### E14. [smell] The side panel's Preview tab is icon-only at 1280
- With the side panel open at 1280×800, the tabs are "Changes 1" (91 px), "#3 ✕" (56), "Shell" (56), then Preview and Thread as 28 px icons.
- HEAD's own commit (e0cb033e) says Changes, PR, Shell and Preview stay labelled at 1280.

### E15. [smell] CLI layout slips
- `wisp ls`: "antigravity" (11 characters) pushes its project column 2 characters to the right of every other row's.
- `wisp show` prints three empty lines before a hold, a usage limit or a question.

## Count

15 new findings (E1–E15):
- **Major (3):** E1, E2, E4
- **Minor (7):** E3, E5, E6, E7, E8, E9, E10
- **Smell (5):** E11–E15
- **Blocker:** none

Audit D's 12: 10 fixed, D5 and D6 partly fixed.

**Open after this round: 17**, the 15 new plus D5 and D6. Row 36b (the agent keeps its Notes) wasn't re-checked.

**NEW findings: 15.**

## Housekeeping
- **Ports and homes.** Only :4962 and /tmp/qaE1 were used. Nothing touched ~/.wisp*, :4477, :4791 or :4800.
- **Stopped.** My server (pid 87679) is stopped (`lsof -ti tcp:4962 -sTCP:LISTEN | xargs kill`); no pkill.
- **Browsers.** Every Playwright browser of mine was closed. A headless Chrome still running at the end belongs to another session's `phone3.mjs`, and was left alone.
- **Gates.** Every gate file was released before the stop. Ada was paused, so no check-in was left queued.
- **Kept.**
  - /tmp/qaE1 (scratch home, projects, records, scripts, and the copied binaries in opt/ and bin/) is kept for repro.
  - /tmp/wisp-target-fixer is kept, as the shared build cache.
  - Artifacts are in /tmp/qa-keep/shots-qaE1/.
- **Disk.** 131 GiB free at the end.
