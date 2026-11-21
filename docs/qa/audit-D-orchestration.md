# Audit D: orchestration, every action end to end (worktree HEAD b7598c9d), 2026-11-19

Re-runs audit C's table (docs/qa/audit-C-orchestration.md) and re-checks each of C1–C18, plus B#6, B#18 and B#19, at
HEAD b7598c9d. Then looks for new faults in the same areas, and in what C didn't test: Try from the UI, a held turn's
Start now, issues, long threads over many turns, and the server's warnings in serve.log.

## Setup

- **Server.** A scratch `wisp serve` on **:4961**, built at HEAD into /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins`).
  - Another session was building the same checkout into the same target dir. To keep it from swapping binaries under my server, I copied `wisp`, `wisp-fake-agent` and `wisp-fake-gh` into /tmp/qaD1/opt and ran those.
  - The worktree's only uncommitted edits are design/v1-reference images, so this build is HEAD's code.
  - Environment: WISP_HOME=/tmp/qaD1/home, HOME=/tmp/qaD1/userhome, WISP_WEB_DIR=/tmp/qaD1/web (a fresh `vite build` of apps/web at HEAD).
  - Restarted once, on the same home, to test Try from the UI after the first stop.
- **Server warnings.** The brief asked for RUST_LOG=warn, but wisp doesn't read RUST_LOG. Its level comes from WISP_LOG, which defaults to `warn` (docs/dev.md), so every WARN went to serve.log this time.
- **Fake harnesses.** Audit C's setup, moved to /tmp/qaD1:
  - codex, opencode, cursor-agent, copilot, grok and agy are the wisp-testkit fake, set through WISP_*_BIN.
  - claude goes through audit C's QA wrapper (/tmp/qaD1/claude-qa.mjs, cues `PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`/`#delegate`, `#limit`, `#hang`).
  - `wisp-fake-gh` is WISP_GH_BIN, with its state in /tmp/qaD1/state/gh.json; two issues were added for the issue test.
  - `FAKE_GATE` (/tmp/qaD1/gate) holds a run whose prompt starts `gate <name>`.
- **Projects.**
  - **proj**: /tmp/qaD1/proj, a git repo with math.js, whose bare origin is /tmp/qaD1/remote.git.
  - **proj2**: commits `.claude/settings.json` (a PreToolUse hook `echo hi`) and `.mcp.json` (server `local`), for project trust.
  - **proj3**: commits `.wisp/worktree.json` with a failing setup.
  - **proj4**: origin `https://github.com/acme/repo.git`, for issues.
- **No outbound traffic.** DO_NOT_TRACK=1, WISP_TELEMETRY=0 and WISP_UPDATE_CHECK=0. The PostHog, grid, Linear, Slack, Discord and Telegram URLs pointed at 127.0.0.1:9.
- **No real harness.** The server's only children were the fakes.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800.
  - Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation.
- **Interfaces used.** The CLI, `/api/cmd`, `/api/mcp` as the user and as a run (its `x-wisp-session`/`x-wisp-token`), the stdio `wisp mcp`, and the UI.
- **Artifacts.** In /tmp/qa-keep/shots-qaD1/: screenshots, plus qaD1-serve.log, qaD1-rec/ and qaD1-calls.jsonl.
- **Shell.** The shell is zsh, whose arrays start at 1. One early queue "bug" came from that, not from wisp; it was re-run with literal ids and isn't counted.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | Each fake got the prompt in its own `wisp/task-<id>` worktree; the project stayed clean. `wisp ls` shows `proj` in the project column (C15 fixed) |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome, WebKit) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit…`, in place (Plan's documented default); codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox` |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`, from the CLI and from the UI slider ("High thinking"); `--compare codex:gpt-5` → `gpt-5` |
| 4 | Worktree vs in place | CLI (`--no-worktree`), UI (Where menu) | pass | In place runs in /tmp/qaD1/proj. Where › In place reads "Your project folder, from now on. The CLI keeps worktrees." A CLI launch right after still got a worktree (C4 fixed). Switching back to Worktree took 6 ms |
| 5 | Compare | CLI, UI (Chrome) | pass | claude + codex, each in its own worktree; launch feedback 2 ms |
| 6 | Try (subagents=haiku) | CLI, UI (Chrome) | pass | CLI: A and B are in separate worktrees, B has `subagentModel: haiku`, "differs: subagentModel → haiku"; `--try` without `--agent` is refused with a reason. UI: Compare, then the run's picker on the lead's harness and model, shows "What's different › Subagents: Task's model / Haiku / Sonnet". Picking Haiku (2 ms) and Send (3 ms) sent `runtime.trial`; run B got `haiku`, each run in its own worktree |
| 7 | Attach files | UI (Chrome, WebKit) | pass | stdin has `<attachments>` "…/attach.txt (text/plain, 13 B)"; argv has `--add-dir` |
| 8 | Goal and judges | CLI (`wisp goal set --kick`), UI | pass, with finding D10 | PASSAFTER2 → `ext.wisp.goal {rounds:2, state:met}`. NEVERMET asks "The goal isn't met after 3 rounds… Keep going?" in `wisp show`. UI goal launch: 3 ms |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | "Add delegate › With the lead" adds Codex at once (4 ms); `spec.team=[{agent:codex,name:codex}]`; chip "1 delegate" |
| 9b | Delegate a job to the team | per-run MCP | pass | codex ran in the lead's worktree and returned its report |
| 10 | Follow-up queue: queue, combine, edit, drag reorder, cancel (with confirm) | CLI, API, UI (Chrome, WebKit) | pass | Queue: 1 ms (Chrome), 7 ms (WebKit). Drag q3 to the top → server `q3, q1, q2`. Combine 3 ms, Edit 9 ms (`runtime.queue.text`), Cancel asks first (3 ms, confirm 1 ms). Every queue drained in order after its gate opened |
| 11 | Stop keeps the queue | CLI, UI, stdio MCP | pass | "Stop Claude Code? Your queued follow-ups stay, held until you send one." (1 ms); `wisp_stop` → queue held ("Held after Stop") |
| 12 | Send now | CLI, UI | pass | UI 5 ms; the item runs, then the rest drain in order. On a run that isn't resumable yet (no session id), the CLI says `queued <item id>` and the item moves to the front, as in v1 (`queue::send_now` `Next`) |
| 13 | Start later | UI (Chrome) | pass | Toast "Starts 15:07". The list has a "Scheduled 1" section: "B26C later ui · Scheduled task · proj · 15:07". It ran at 15:07 in a worktree (B#6 fixed) |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI | pass | `wisp show` lists "Which database, Postgres or SQLite?" with options; `wisp reply … SQLite` → the agent got "The user answered: SQLite" |
| 15 | Usage limit | CLI | pass | `wisp show`: "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent.", with no raw stderr (C13 fixed) |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass | Own worktree; 15 ms |
| 17 | Rerun… | UI (Chrome) | pass | The dialog says "Starts a new task in a new worktree of proj on this computer." (C6 fixed); own worktree; 15 ms |
| 17b | `wisp rerun <id>` (also with `-d`) | CLI | pass | `-d` accepted (C12 fixed); own worktree; no `rerun-<sha>` folder left behind and no leftovers (C8 fixed) |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass | A new task with `parentId`, in the parent's folder; 12 ms |
| 19 | /btw | UI, CLI, HTTP MCP | pass | UI 18 ms. Copilot's side run has its own `--session-id` and the task so far in its prompt (C18 fixed) |
| 20 | Review by another harness | — | not re-checked | — |
| 21 | Ship: commit & open PR | UI, CLI | pass | `Wisp-Task:` trailer; the branch is pushed; fake PR #1 and #3; toast "Opened PR #1"; 7–10 ms |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass | Ship › "Merge PR #1…" opens "Squash-merge PR #1? Then wisp fast-forwards your local base branch and removes the worktree, unless it has uncommitted work." (2 ms; C7 and B#19 fixed). Toast: "Merged PR #1. Local main is up to date with origin/main. The worktree … was kept: it has 1 uncommitted change." precious.txt survived |
| 22b | `wisp ship` + `wisp merge` | CLI | pass, with finding D9 | PR #3 merged, local main at its commit, the worktree removed. Under PR #2, the fake gh's push to main isn't a fast-forward, so nothing landed: a fake limitation, not counted |
| 23 | Restore to before latest turn | UI | pass | Confirm, then the toast "The folder is back to before this turn. Undo"; 3 ms |
| 24 | Bring in | UI | pass | "1 worktree from git · In proj" is shown 461 ms after opening the pane (B#18 fixed); Bring in took 3 ms → a task on `stray-branch` |
| 25 | Worktree removal | UI (Settings › Worktrees) | pass | "Remove this worktree? Its 2 uncommitted changes can't be brought back." The folder is gone and the branch stays; 9 ms |
| 26 | Crew: add | CLI | pass, with finding D8 | `--cron "every minute"` → `* * * * *` (C11 fixed); `--on done` → `wakes` |
| 27 | Crew: give a job | CLI, UI, HTTP MCP | pass | Each job in its own worktree; UI 6 ms; `crew assign` prints `session <id> · Ada · Job · running` |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass | Cron every minute (18 check-ins); "Check in now" took 23 ms, 17 → 18. All check-ins share `proj-ada-check-in` (C3 fixed) |
| 29 | Crew: pause / resume | UI, API | pass | `paused` toggles; 10 ms |
| 30 | Crew: tint | UI | pass | Picking Ada tints the list violet. Edit › Task panel › teal, then Save (22 ms) → `bg: teal`, and the list turns teal |
| 31 | Crew: wakes on "I mark a task done" | API (`runtime.setState done`) | pass | Bob's codex job "task.done: D3 model (woken by trigger:crew.bob.1)", in its own worktree. Archive (`runtime.archive`) doesn't wake him: Archive isn't Done |
| 32 | Crew: Org link → the job gets its delegate | CLI | pass | `crew update Ada --team '[{"crew":"bob","why":"reviews"}]'`; Ada's job's `delegate` has `subagent` enum `["bob"]` |
| 33 | `wisp ls --crew ada` | CLI | pass, with finding D7 | Lists Ada's jobs and check-ins |
| 34 | Recipes: new, edit, run, delete | UI | pass | Create 1 ms; Run 5 ms, the dialog's Project is proj, the run is in a worktree; Delete asks "Delete the recipe "Fix-Thing"? It goes for good…", and the list is empty after |
| 35 | Triggers: new schedule, fires, on/off, edit, delete | UI, CLI | pass | Create 3 ms; fired every minute into `proj-trigger-uitrig` (C3 fixed); the switch took 34 ms and is off on the server; delete asks first, confirm 3 ms. `wisp automations trigger ls` has headers and "last run just now" (C14 fixed) |
| 36 | Notes, Thread view, Fresh thread | CLI | pass | `wisp notes --set goal=…`; `wisp fresh` runs without `--resume`, with the Notes in its brief |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | pass | "Updated decisions (rev 1)", and `wisp notes` shows it |
| 37 | User MCP (`wisp mcp`, stdio): launch, queued reply, get, stop, wait | MCP | pass | 250 tools (B#4's stdio part fixed). `wisp_launch {cwd}` runs in a worktree (C5 fixed). After `wisp_stop`, `wisp_get` says `[review] ◐ Stopped` at once (C16 fixed) |
| 38 | HTTP MCP: fork, crew_assign, btw | MCP | pass | The fork is in its own worktree, the crew job runs, and btw answers `pending` |
| 38b | HTTP MCP: every listed tool is callable | MCP (user and run) | **partial** (D6) | 246 tools as the user. No "doesn't declare" or "may not" refusals (the 3 "refused" hits were tool docs containing the word). But generic command tools called without `args` fail with a raw serde error |
| 39 | A task launched by an agent | per-run MCP | pass | `wisp_launch {worktree:true}`, `{project:"proj"}` and `{cwd, effort:"high"}` each run in a worktree; effort reaches codex (C1 fixed) |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 4 ms; the task stays running |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | pass | No horizontal overflow on Home, a task or the crew page; the composer folds until focus; the held card fits |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured action took 1–34 ms (see Feedback below) |
| 43 | `wisp help runtime "queue order"` | CLI | pass | Prints the verb's help |
| 44a | Try from the UI | UI (Chrome) | pass | See row 6 |
| 44b | A held turn's "Start now" | UI, API | **fail** (D1, D2) | Held for project trust. Start now sends `runtime.release` (9 ms), and the same turn is held again. Nothing runs |
| 44c | Start from an issue | UI (Chrome) | pass | proj4's picker lists "#7 Sum is wrong for negatives · bug" and "#8". Picking one fills the prompt with its title, URL and body; Send → "Fix #7: Sum is wrong for negatives" in a proj4 worktree. proj without a GitHub remote says so and offers Linear and Jira |
| 44d | Check logs (a failed CI run) | — | not tested | The fake gh has no run logs |
| 44e | A held turn's "Skip" | UI | **partial** (D3) | The task ends (`done`), with the wrong reason |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI, UI (Chrome, WebKit) | pass | All 26 turns ran, in the order sent (C2 fixed; 10 back to back on another task too). The thread is shown in 284 ms (Chrome) and 487 ms (WebKit), at its latest turn, with no errors |
| 46 | Crew check-ins and schedule triggers over time | CLI, git | pass | About 18 minutes at every minute: one folder each (`proj-ada-check-in`, `proj-trigger-uitrig`), two branches, no growth (C3 fixed) |
| 47 | A project's worktree setup (`.wisp/worktree.json`) | CLI | **fail, to confirm** (D5) | A failing setup neither ran nor held, in Full auto or in Edits |
| 48 | Server warnings (serve.log, WISP_LOG default warn) | — | pass | 2 WARNs in ~100 tasks, both from my own bad calls (`runtime.done`, `automations.triggers`; see D12) |

**Counts:** 57 rows.
- **Pass:** 51, of which 4 carry a lesser finding (rows 8, 22b, 26 and 33).
- **Partial:** 2 (rows 38b and 44e).
- **Fail:** 2 (rows 44b and 47).
- **Not tested or not re-checked:** 2 (rows 20 and 44d).

**Feedback (ms, pointerdown → first DOM change).**
- Launch: 2–6.
- Queue: 1 (Chrome), 7–8 (WebKit).
- Combine 3, edit 9, cancel 3, stop 1, send now 5.
- Fork 15, rerun 15, side 12, btw 18, restore 3.
- Ship 7–10, merge item 2, bring in 3, worktree remove 9.
- Crew: job 6, pause 10, check-in 23, tint save 22.
- Recipe: create 1, run 5, delete 1.
- Trigger: create 3, switch 34, delete 3.
- Done 4, Start now 9, Skip 8, issue picker 7, issue pick 6, Try pick 2 and send 3, project picker 9.

## Audit C's findings, re-checked

| C# | Finding | Status at b7598c9d | Evidence |
|---|---|---|---|
| C1 | `wisp_launch` from a run drops worktree, project and the rest | **fixed** | `{worktree:true}`, `{project:"proj"}` and `{cwd, effort:"high"}` each ran in `home/worktrees/proj-<id>`; codex got `model_reasoning_effort="high"` |
| C2 | Quick follow-ups lost or out of order | **fixed** | 10 back-to-back `wisp reply --queue`: turns 1–10 in order. 1 + 25 fired right after launch: 26 turns `P,1,…,25`, all done, the queue empty |
| C3 | A new worktree per check-in and trigger run | **fixed** | 18 check-ins and ~20 trigger runs over ~18 min: one folder each (`proj-ada-check-in`, `proj-trigger-uitrig`), branches `wisp/ada-check-in` and `wisp/trigger-uitrig` (ADR-0108). Each run is still its own row in the list |
| C4 | In place turns off worktrees everywhere, silently | **fixed** | The menu says "Your project folder, from now on. The CLI keeps worktrees."; a CLI launch after it got a worktree (ADR-0108: sticky by design) |
| C5 | stdio `wisp_launch {cwd}` runs in place | **fixed** | cwd `home/worktrees/proj-01a1f413166b…` |
| C6 | The Rerun dialog names the main checkout | **fixed** | "Starts a new task in a new worktree of proj on this computer." |
| C7 | Ship › "Merge PR #2…" opens nothing | **fixed** | The menu item opens the `ship.merge.confirm` dialog in 2 ms |
| C8 | `wisp rerun` leaves a detached `rerun-<sha>` folder | **fixed** | No `rerun-*` folder; git lists only `proj-*` worktrees |
| C9 | `queue order <id> <order>` positional does nothing | **fixed** | `wisp runtime queue order <id> c3,a1,b2` → server order `c3, a1, b2` |
| C10 | `queue edit --items` needs an undocumented `at` | **fixed** | `--items '[{"id":…,"text":"b2 edited"}]'` → "✓ done"; the edited item first, the rest kept |
| C11 | Crew schedule "every minute" refused | **fixed** | → `cron * * * * *` |
| C12 | `wisp rerun <id> -d` prints usage | **fixed** | "◌ <id> → <new id> (as it ran)" |
| C13 | `wisp show` on a limit task prints raw stderr | **fixed** | Only the message and its hint |
| C14 | Raw values in CLI output | **fixed** | `trigger ls` has headers and "last run just now"; `queue send-now` prints `queued <item id>` |
| C15 | `wisp ls` shows the worktree folder as the project | **fixed** | `proj` (but see D7 for `--crew`) |
| C16 | `wisp_stop` answers before the task stops | **fixed** | `wisp_get` right after: `[review] ◐ Stopped` |
| C17 | Home sends `runtime.defaultModel` ~20 times | **fixed** | 2 per Home load (Chrome request log) |
| C18 | copilot `/btw` resumes the task id | **fixed** | The side run has its own `--session-id 01a1f417aef1…` and "## The task so far" in its prompt |

| B# | Finding | Status at b7598c9d | Evidence |
|---|---|---|---|
| 4 (stdio part) | stdio `wisp mcp` had 11 tools | **fixed** | 250 tools |
| 6 | Start later shows nowhere | **fixed** | The "Scheduled 1" section in the list, with time and project |
| 18 | Worktrees count and Bring in late | **fixed** | Bring in is shown 461 ms after opening the pane. `worktrees.view` answers `looking: true` because each call starts a look (commands.rs:165); the pane is told when it ends |
| 19 | Merge confirm says it removes the worktree; 235 schemas per run | **fixed** | The confirm says "…removes the worktree, unless it has uncommitted work"; the tool list per run is ADR-0107's (deliberate) |

**C totals:** all 18 fixed. B#4's stdio part, B#6, B#18 and B#19 are fixed too.

## New findings

### D1. [major] "Start now" on a turn held for project trust never starts it
- **Steps.**
  1. proj2 commits `.claude/settings.json` (a PreToolUse hook `echo hi`) and `.mcp.json` (server `local`).
  2. `wisp "D44 held untrusted edits" --agent claude --mode edits` from proj2. The turn is `held`, by `wisp.worktrees`, kind `trust`, with both items.
  3. Task page › Start now (UI, 9 ms, sends `runtime.release`); then again over `/api/cmd runtime.release {action:"start"}` (`{"ok":true}`).
- **Actual:**
  - The same turn is `held` again each time. The queue is empty, no fake ever ran, and the state stays `review`.
  - A second task on proj2 is held too, so the yes was never recorded.
- **Expected:** Start now trusts what it was held for (v1: "Trust and start"), and the run starts.
- **Suspect:**
  - `Runtime::release` → `release_held` queues the turn with `opts.released`, then `kick` runs `before:run` again.
  - wisp.worktrees' trust interceptor (plugins/worktrees/host/src/trust.rs `before_run`) checks only `hold` and `untrusted` on the context. It doesn't seem to see the release, so it holds again.

### D2. [major] The trust hold doesn't say what it would run, and offers no "Run without them"
- **Actual:**
  - The card reads "Held before it started · Skip · Start now"; the row says "Held"; the phone view is the same.
  - The hook command, the MCP server and their files are in the turn's `held.data.items` but shown nowhere.
- **v1** (web/js/features/hold.js:161–174, 230):
  - "This repo asks to run its own programs. Held before it started: these would run on this computer, as you, whatever the mode."
  - Each item has Open file.
  - The buttons are **Trust and start**, **Run without them** and **Skip this run**.
- **Effect:** the user is asked to trust programs they can't see, and can't run the task without them.

### D3. [minor] Skip on a trust hold gives a usage-limit reason
- **Steps.** A second proj2 task, held for trust › Skip (8 ms).
- **Actual:** `wisp show`: "● Skipped before it started · Its usage window was near its limit."
- **Expected:** what it was held for (the repo's programs).
- **Suspect:** `Runtime::release` (plugins/runtime/host/src/ops.rs:417) defaults the label to "Its usage" when the hold's data has no `label`. The trust hold has none.

### D4. [minor] `wisp show` on a held task prints nothing about the hold
- **Actual:**
  - `wisp show <held task>` prints only the title line and "ran with … from a88f24b2…".
  - No "held", no reason, and no way to start or skip.
  - The only CLI way is `wisp runtime release <id> <turn-id> <action>`, where the turn id must be dug out of `runtime.turns` and `--action` is JSON.
- **Expected:** what is held and why, and how to start or skip it, as `wisp show` does for a question or a usage limit.

### D5. [major, to confirm] A project's `.wisp/worktree.json` setup never runs or holds
- **Steps.**
  1. proj3 commits `.wisp/worktree.json` `{"setup":"touch setup-ran; exit 3"}` (first `echo …; exit 3`).
  2. Three launches: Full auto twice, Edits once.
- **Actual:**
  - Each worktree has only `a.txt` and `.wisp`: no `setup-ran`.
  - No turn was held ("Its setup" or trust).
  - Each run went straight to the harness and finished.
  - serve.log has no "worktree prepared with problems" WARN, which `Engine::create` logs when a setup step fails (plugins/worktrees/host/src/engine.rs:206).
- **Expected** (v1 project-trust.js:160–162):
  - In Full auto the setup runs at once, and a failure holds the turn with its reason.
  - In other modes an untrusted setup is held for trust.
- **Not ruled out:** a source of worktrees that skips `prepare`.

### D6. [minor] 82 MCP tools have no argument schema, and calling one without `args` fails with a serde message
- **Steps.** `tools/call crew_list {}` (also `later_plans`, `stuff_list`, `machines_list`, `crew_graph`, `machines_hosts` and others), over HTTP as the user, as a run, and over stdio.
- **Actual:**
  - `wisp: crew.list: invalid type: null, expected struct NoArgs`.
  - 82 of 246 tools are generic command wrappers: their schema is `{args: "the command's arguments, as JSON", peer}`, with `args` not required and no field names.
  - `{"args":{}}` works.
  - `stuff_get {"args":{"id":"x"}}` answers `null` instead of saying there's no such item.
- **Effect:**
  - An agent following the schema gets a Rust type error.
  - It can't learn the fields from the schema.
- **Note.** ADR-0107 decides which tools are listed. It doesn't cover their argument schemas.

### D7. [smell] `wisp ls --crew <name>` leaves the project column empty
- `wisp ls --crew ada`: `01a1f421c297…  review  claude  ·  Ada · Job`, with no project.
- Plain `wisp ls` shows `proj` for the same rows.

### D8. [smell] `wisp crew create|get|update` dumps every internal field
- Empty fields are printed (`avatar`, `brief`, `effort`, `last minute`, `machine`, `wake`), and so are `away none` and `offers none`.
- Nested values show only as counts: `team 1 item`, `wakes 1 item`. So `crew update --team …` doesn't show the team it set.

### D9. [smell] `wisp merge` says only "✓ merged"
- The UI's toast says that local main is up to date and whether the worktree was kept (and why) or removed.
- The CLI says neither, although it removed the worktree.

### D10. [smell] The CLI doesn't show a task's goal
- `wisp show` on a task whose goal was met after 2 rounds says nothing about the goal. Only `runtime.get` → `ext.wisp.goal {state:met, rounds:2}` has it.
- `wisp goal set --id <idle task> --text …` without `--kick` answers `status working`, though nothing will judge it until the task's next turn.

### D11. [smell] The CLI's empty-list words differ
- `wisp ls --crew bob` → "No tasks."; `wisp ls --state running` → "No sessions."

### D12. [smell] `automations.triggers` is declared but not registered
- plugins/automations/api declares `command!(Triggers = "automations.triggers", …)`.
- Calling it answers "unknown command automations.triggers (is its plugin off?)" and logs a WARN. The CLI and UI use `automations.trigger.ls`.

## Count

12 new findings (D1–D12):
- **Major (3):** D1, D2, and D5 (to confirm)
- **Minor (3):** D3, D4, D6
- **Smell (6):** D7–D12
- **Blocker:** none

Audit C's 18 are all fixed. B#4's stdio part, B#6, B#18 and B#19 are fixed.

**Open after this round: 12**, the 12 new. Rows 20 (review) and 44d (Check logs) are still untested.

**NEW findings: 12.**

## Housekeeping
- **Ports and homes.** Only :4961 and /tmp/qaD1 were used. Nothing touched ~/.wisp*, :4477, :4791 or :4800.
- **Stopped.** My server (pid 42397, then its restart) is stopped, and no fake process of mine remains. I stopped my own pid only; no pkill.
- **Gates.** All gate files were released before the stop.
- **Kept.**
  - /tmp/qaD1 (scratch home, projects, records and the copied binaries in opt/) is kept for repro.
  - /tmp/wisp-target-fixer is kept, as the shared build cache.
  - Artifacts are in /tmp/qa-keep/shots-qaD1/.
- **Disk.** 146 GiB free at the end.
