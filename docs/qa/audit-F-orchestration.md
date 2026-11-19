# Audit F: orchestration, every action end to end (worktree HEAD d4c98dac), 2026-11-19

Re-runs audit E's table (docs/qa/audit-E-orchestration.md) and re-checks each of E1–E15 and D5/D6 at HEAD d4c98dac.
Then looks for new faults in the same areas. Row 36b (the agent keeps its Notes), which E left out, is checked. One row
is new: 49, a long follow-up typed in the reply box.

## Setup

- **Server.** A scratch `wisp serve` on **:4963**, built at HEAD into /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins`).
  - `wisp`, `wisp-fake-agent` and `wisp-fake-gh` were copied into /tmp/qaF1/opt and /tmp/qaF1/bin and run from there.
  - The worktree's only uncommitted edits are design/v1-reference images, so this build is HEAD's code.
  - Environment: WISP_HOME=/tmp/qaF1/home, HOME=/tmp/qaF1/userhome, WISP_WEB_DIR=/tmp/qaF1/web (a fresh `vite build` of apps/web at HEAD).
- **Server warnings.** WISP_LOG defaults to `warn`, so every WARN went to serve.log (RUST_LOG=warn is not read; see audit E).
- **Fake harnesses.** Audit E's setup, moved to /tmp/qaF1:
  - codex, opencode, cursor-agent, copilot, grok and agy are the wisp-testkit fake, set through WISP_*_BIN.
  - claude goes through the QA wrapper (/tmp/qaF1/bin/claude, cues `PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`/`#delegate`, `#limit`, `#hang`).
  - `wisp-fake-gh` is WISP_GH_BIN, with its state in /tmp/qaF1/state/gh.json (E's two issues; no PRs at the start). A failing `test` check was added later for row 44d.
  - `FAKE_GATE` (/tmp/qaF1/gate) holds a run whose prompt starts `gate <name>` until that file exists.
- **Projects.** All are fresh git repos with E's files, made at the start of this audit.
  - **proj**: math.js and its test, whose bare origin is /tmp/qaF1/remote.git. Not added to wisp until row 39, so rows 1–3 also test E1.
  - **proj2, proj5–proj9**: each commits `.claude/settings.json` (a PreToolUse hook `echo hi`) and `.mcp.json` (server `local`), for project trust. One fresh, untrusted copy per hold test.
  - **proj3**: commits `.wisp/worktree.json` `{"setup":"touch setup-ran; exit 3"}`.
  - **proj4**: origin `https://github.com/acme/repo.git`, for issues.
- **No outbound traffic.** DO_NOT_TRACK=1, WISP_TELEMETRY=0 and WISP_UPDATE_CHECK=0. The PostHog, grid, Linear, Slack, Discord and Telegram URLs pointed at 127.0.0.1:9.
- **No real harness.** The server's only children were the fakes.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800.
  - Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation.
  - E's scripts were reused, with paths and port changed. Every new script closes its browser in `finally`.
- **Interfaces used.** The CLI, `/api/cmd`, `/api/mcp` as the user and as a run (its `x-wisp-session`/`x-wisp-token`), the stdio `wisp mcp`, and the UI.
- **Artifacts.** In /tmp/qa-keep/shots-qaF1/: screenshots, qaF1-serve.log and qaF1-calls.jsonl.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | From /tmp/qaF1/proj, not yet added to wisp: each fake ran in its own `proj-<id>` worktree, and the checkout stayed clean (E1 fixed). `wisp ls` lines up its columns with "antigravity" in them |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome, WebKit) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit MultiEdit Write NotebookEdit`, in place; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox` |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`, from the CLI and from the UI slider ("High thinking"); `--compare claude,codex:gpt-5` → `-m gpt-5` |
| 4 | Worktree vs in place | CLI (`--no-worktree`), UI (Where menu) | pass | Where › In place stayed sticky for the next UI launch (ADR-0108, deliberate) |
| 5 | Compare | CLI, UI (Chrome) | pass | CLI: `--compare claude,codex:gpt-5` makes one task per harness, each in its own worktree. UI: send 2 ms, two runs and the "Pick one" row |
| 6 | Try (subagents=haiku) | CLI, UI (Chrome) | pass, with finding F6 | CLI: runs A and B, "differs: subagentModel  → haiku" (two spaces, F6); `--try` without `--agent` is refused with its reason. UI: Haiku pick 2 ms, send 3 ms, `runtime.trial` |
| 7 | Attach files | UI (Chrome, WebKit) | pass | stdin has `<attachments>`; argv has `--add-dir …/stuff/files/<hash>` |
| 8 | Goal and judges | CLI (`wisp goal set --kick`), UI | pass | PASSAFTER2 → `wisp show`: "◎ Goal: PASSAFTER2 sums work · met after 2 rounds". UI goal launch 3 ms |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | "Add delegate › With the lead" adds Codex (4 ms); chip "1 delegate"; team `[{agent: codex}]` |
| 9b | Delegate a job to the team | per-run MCP | pass | The lead's `delegate` call got the codex delegate's report ("done": true); `children: {codex: …}` |
| 10 | Follow-up queue: queue, combine, edit, drag reorder, cancel (with confirm) | CLI, UI (Chrome, WebKit) | pass | Queue 1 ms (Chrome), 5–7 ms (WebKit). Drag q2 to the top → server order on both engines. Combine 4, edit 11 (`runtime.queue.text`), cancel 3–7 (confirm 1). CLI `queue order` and `queue cancel --item-id` change the server order |
| 11 | Stop keeps the queue | CLI, UI | pass | "Stop Claude Code? Your queued follow-ups stay, held until you send one." (1 ms Chrome, 6 ms WebKit); `queueHeld: true` after the stop |
| 12 | Send now | CLI, UI | pass | UI 5–7 ms; the item runs first, then the rest drain in order |
| 13 | Start later | UI (Chrome) | pass | Menu "In 10 min · In 30 min · In 1 hour · In 3 hours · Tomorrow morning"; send 7 ms; toast "Starts 21:22"; the list's "Scheduled 1" shows "F13 later ui · Scheduled task · proj · 21:22" |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI | pass | `wisp show` lists "? Asks: Which database, Postgres or SQLite?" with numbered options and the reply command; `wisp reply … SQLite` → the agent got "The user answered: SQLite" |
| 15 | Usage limit | CLI | pass | `wisp "F15 #limit" --wait` prints "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent." (E8 fixed) |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass, with finding F2 | UI: own worktree, 19 ms. MCP `runtime_fork` makes a fork. Forking a task whose PR was merged gives a task in a folder that is gone (F2) |
| 17 | Rerun… | UI (Chrome) | pass | "Starts a new task in a new worktree of proj on this computer."; own worktree; 16 ms |
| 17b | `wisp rerun <id> -d` | CLI | pass | Own worktree; no `rerun-*` folder left behind |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass, with finding F5 | A new task, 12 ms. It runs in its parent's worktree; `wisp ls` gives its project as "proj-01a1f532312" (F5) |
| 19 | /btw | UI, HTTP MCP | pass | UI 18 ms, the answer card shows on the task. MCP `runtime_btw` on a merged task: see F2 |
| 20 | Review by another harness | Ship, UI, API | **partial** (E5) | With `review.harness: other`, shipping a codex task started a review by Claude. `review.view` says `failed` and why. The button's title now says why and "Click to try again", but its label still reads only "Not reviewed", and Review again (6 ms) shows no "Reviewing". The PR tab has no review section |
| 21 | Ship: commit & open PR | UI, CLI | pass | Ship 7 ms opens "Ship it" at once; Commit & open PR 9 ms, toast "Opened PR #2"; CLI "✓ shipped: …/pull/1" |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass | Confirm "Squash-merge PR #2? … ⚠ 1 check is failing."; go 4 ms. Toast: "Merged PR #2. Local main was not updated: your uncommitted changes to math.js are in the way. Commit or stash them, then pull. The worktree … was kept: it has 1 uncommitted change." (E9 fixed) |
| 22b | `wisp ship` + `wisp merge` | CLI | pass, with finding F1 | "✓ merged PR #1 · ! Local main was not updated: your uncommitted changes to math.js are in the way. Commit or stash them, then pull. · The worktree was removed; its branch stays." The branch was deleted (F1) |
| 23 | Restore to before latest turn | UI | pass | Confirm "Restore to before this turn? …", then the toast "The folder is back to before this turn. Undo"; 3 ms |
| 24 | Bring in | UI | **partial** (E6) | With 56 worktrees, a new stray worktree didn't show in 40 s on the first open. A later open showed it in 774 ms; Bring in 3 ms made the task. Each `worktrees.view` call starts the look again |
| 25 | Worktree removal | UI (Settings › Worktrees) | pass | "Remove this worktree? Its 2 uncommitted changes can’t be brought back."; 10 ms; `worktrees.clean`; the folder is gone |
| 26 | Crew: add | CLI | pass, with findings E11, F6 | `--cron "every minute"` → `* * * * *`, "next in a moment" (E12 fixed); `--on done` → `on task.done`. `crew create --help` still has developer notes (E11). A wrong flag's hint drops the other flags (F6) |
| 27 | Crew: give a job | CLI, UI, HTTP MCP | pass | Each job in its own worktree; UI 6 ms; `crew assign` prints `session <id> · Ada · Job · running`; MCP `crew_assign` too |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass | Cron every minute: every check-in in `proj-ada-check-in`. On a paused member, Check in now (14–18 ms) says "Ada is paused. Resume them to check in." each time, with no WARN. After Resume, a check-in came in 8 s (E7 fixed) |
| 29 | Crew: pause / resume | UI, API | pass | `paused` toggles; 11 ms |
| 30 | Crew: tint | UI | pass | Pick 3 ms; Save 23 ms → `bg: teal`, and the list turns teal |
| 31 | Crew: wakes on "I mark a task done" | API (`runtime.setState done`) | pass | "Bob · Job started by trigger:crew.bob.1", codex, in its own worktree |
| 32 | Crew: Org link → the job gets its delegate | CLI | pass | `crew update ada --team '[{"crew":"bob","why":"reviews"}]'`; the team table says `bob  Bob  codex` (E13 fixed); Ada's job's `delegate` has `subagent` enum `["bob"]` |
| 33 | `wisp ls --crew ada` | CLI | pass | Lists Ada's jobs and check-ins with `proj` in the project column |
| 34 | Recipes: new, edit, run, delete | UI | pass | Create 1 ms; Run 6 ms, the dialog's Project is proj, toast "E34-Fix-Thing is ready"; Edit saves; Delete asks "Delete the recipe “E34-Fix-Thing”? It goes for good…" (1 ms), and the list is empty after |
| 35 | Triggers: new schedule, fires, on/off, edit, delete | UI, API | pass | "uitrig" every minute; it fired 4 times into `proj-trigger-uitrig` (tidied when clean); the switch took 21 ms and turned off; delete asks first (1 ms), and only crew.bob.1 is left. The edit's new prompt wasn't read back |
| 36 | Notes, Fresh thread | CLI | pass | `wisp notes <id> --set goal=…`; `wisp fresh` runs without `--resume`, with the Notes in its brief |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | pass | `wisp_notes {"set":{"decisions":"use SQLite"}}` → "Updated decisions (rev 1)"; `wisp notes <id>` shows it |
| 37 | User MCP (`wisp mcp`, stdio): launch | MCP | pass | 250 tools; `wisp_launch {cwd}` runs in a worktree |
| 38 | HTTP MCP: fork, crew_assign, btw | MCP | pass | The fork answers its id; the crew job runs in its own worktree. `runtime_btw` on a merged task: F2 |
| 38b | HTTP MCP: every listed tool is callable | MCP (user and run) | **partial** (D6) | Missing fields now answer "wisp: crew_get needs `id`." (no serde words). The `args` form works (E10 fixed). Still 55 of 250 tools name no fields, and unknown fields are dropped silently |
| 39 | A task launched by an agent | per-run MCP | pass | `wisp_launch {worktree:true}`, `{project:"proj"}` (after `wisp add`) and `{cwd, agent:codex, effort:"high"}` each run in a worktree; effort reaches codex. Before `wisp add`: "No project “proj”. wisp_available lists them." |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 4 ms; the task stays running |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | pass | No horizontal overflow on Home, a task or the crew page. The trust card fits: 358×263, its buttons on their own rows (E2 fixed) |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured action took 1–23 ms (see below) |
| 43 | `wisp help runtime "queue order"` | CLI | pass | Prints the verb's help |
| 44a | Try from the UI | UI (Chrome) | pass | See row 6 |
| 44b | A held turn's "Trust and start" | UI, CLI | pass | UI 10 ms → the turn ran (`done`); CLI `wisp held <id> start` → "✓ started"; the next task on that repo isn't held. Desktop card: text over a row of buttons, each item with its file and Open file (E3 fixed) |
| 44c | Start from an issue | UI (Chrome) | pass | proj4's picker lists "#7 Sum is wrong for negatives · bug" and "#8" (6 ms); pick 6 ms; Send → "Fix #7: Sum is wrong for negatives" in a proj4 worktree |
| 44d | Check logs / Ask to fix (a failing check) | UI | pass | The strip says "Open · 1 check failing"; the PR tab lists "✕ test" with Ask to fix (3 ms), which fills the reply box with "The `test` check failed on PR #2 (…/runs/42/job/4242). Find out why, fix it and push." (`ship.check_log`; E4 fixed) |
| 44e | A held turn's "Skip" | CLI | pass | "● Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." |
| 44f | A held turn's "Run without them" | CLI | pass | "✓ started without the repo's own programs"; claude got `--setting-sources user` and `--strict-mcp-config` |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI | pass | All 26 turns ran, in the order sent (`f1 … f25`), the queue empty |
| 46 | Crew check-ins and schedule triggers over time | CLI, git | pass | About 25 minutes at every minute: every check-in in `proj-ada-check-in`, every trigger run in `proj-trigger-uitrig`; no growth |
| 47 | A project's worktree setup (`.wisp/worktree.json`) | CLI | pass | `--mode auto` and no mode: the setup runs (`setup-ran`), fails and holds the turn: "◷ Held before it started: setup `touch setup-ran; exit 3` failed: exit 3". `--mode edits` holds it for trust ("Setup step 1 … (.wisp/worktree.json)") (D5 fixed) |
| 48 | Server warnings (serve.log) | — | pass, with finding F2 | 4 WARNs in ~110 tasks: 2 from the failing setup (row 47), 1 from my own bad call, and 1 "a fork's worktree … no such folder" (F2) |
| 49 | A long follow-up in the reply box (new) | UI (Chrome; 1280 and 390) | **fail** (F3) | Typed text stays on one 34 px line and scrolls sideways (`white-space: nowrap`), at 1280 and at 390. Ask to fix's message shows only its tail |

**Counts:** 59 rows (E's 58 plus 49).
- **Pass:** 55, of which 6 carry a lesser finding (rows 6, 16, 18, 22b, 26 and 48).
- **Partial:** 3 (rows 20, 24 and 38b).
- **Fail:** 1 (row 49).

**Feedback (ms, pointerdown → first DOM change).**
- Launch: 2–3 (Chrome), 7–8 (WebKit); compare 2, team 3, goal 3, later 6–7.
- Queue: 1 (Chrome), 5–7 (WebKit).
- Combine 4, edit 11, cancel 3–7 (confirm 1), stop 1–6, send now 5–7.
- Fork 19, rerun 16, side 12, btw 18, restore 3.
- Ship 7, Commit & open PR 9, merge go 4, review again 6, Ask to fix 3.
- Bring in 3, worktree remove 10.
- Crew: open 12, job 6, pause 11, check-in on a paused member 14–18, tint pick 3, tint save 23.
- Recipe: create 1, run 6, delete 1.
- Trigger: create 3, switch 21, delete 1.
- Done 4, Trust and start 10, Open file 12–18, Try pick 2 and send 3, project picker 6, issue picker 6, issue pick 6.

## Audit E's findings, re-checked

| # | Finding | Status at d4c98dac | Evidence |
|---|---|---|---|
| E1 | A CLI task in a repo wisp doesn't know runs in place | **fixed** | All 7 harnesses got a `proj-<id>` worktree before `wisp add`; the checkout stayed clean |
| E2 | The trust card collapses on a phone | **fixed** | 358×263 at 390 on Chrome and WebKit, light and dark; no overflow; no button wraps |
| E3 | The desktop trust card squeezes its text, no files, no Open file | **fixed** | Text across the card, buttons on a row under it; each item shows `.claude/settings.json` / `.mcp.json` and Open file |
| E4 | No Ask to fix for a failing check | **fixed** | PR tab: "✕ test · Ask to fix"; the reply box gets v1's sentence |
| E5 | A failed review shows as "Not reviewed"; Review again shows no progress | **partly fixed** | The button's title now gives the why and "Click to try again", but the title says "Review again to ask once more. Click to try again". The label still says only "Not reviewed", there is no "Reviewing", and the PR tab has no review section |
| E6 | With many worktrees, Bring in shows a stray only after a minute or more | **not fixed** | 56 worktrees: not shown in 40 s on the first open; `worktrees.view` said `looking: true` for minutes while it was asked every 10 s (each call starts the look again). Shown at 774 ms once it was left alone |
| E7 | Check in now on a paused member queues silently | **fixed** | "Ada is paused. Resume them to check in.", no WARN; a check-in 8 s after Resume |
| E8 | The CLI says "finished" for a held turn and a usage limit | **fixed** | `--wait` prints the hold ("◷ Held before it started…") and the limit ("✗ … usage limit"); `-d` prints "launched" |
| E9 | git's cut-off sentence when local main can't be updated | **fixed** | "your uncommitted changes to math.js are in the way. Commit or stash them, then pull.", in the CLI and the toast |
| E10 | The `args` form is ignored; a run must name its session | **fixed** | `crew_agent_list {"args":{"session":"x"}}` works; as a run, `crew_agent_list {}` and `later_agent_schedule {}` need no session |
| E11 | Generated CLI help shows developer notes | **partly fixed** | "`POST /crew`" and the "Additive (date)" notes are gone. `crew create --help` still has "--id … so the row it drew is the one that comes back (never two for a frame)", "--on … v1's trigger kinds" and "--wakes … (ADR-0043)" |
| E12 | "next 1m ago" | **fixed** | "next in a moment" |
| E13 | The team table shows the member's agent as "auto" | **fixed** | `bob  Bob  codex` |
| E14 | The side panel's Preview tab is icon-only at 1280 | **not fixed** | Tabs at 1280: "Changes 2" 91 px, "PR #2 ✕" 75, "Shell" 56, then Preview and Thread as 28 px icons |
| E15 | CLI layout slips | **fixed** | `wisp ls` columns line up with "antigravity"; `wisp show` has one blank line before a hold, a limit or a question |
| D5 | A project's worktree setup never runs from the CLI | **fixed** | Row 47 |
| D6 | MCP tools without an argument schema | **partly fixed** | Missing fields say "needs `id`". 55 of 250 tools still have only `{args, peer}`: all of connectors_*, plugins_*, stuff_*, and others. Unknown fields are still dropped: `wisp_notes {"section":…,"text":…}` read the Notes instead of refusing, and `runtime_fork {"prompt":…,"bogus":1}` forked |

**E/D totals:** 12 fixed (E1–E4, E7–E10, E12, E13, E15, D5), 3 partly fixed (E5, E11, D6) and 2 not fixed (E6, E14).

## New findings

### F1. [minor] `wisp merge` says the branch stays, but it is deleted
- **Steps.** `wisp ship <id>`, then `wisp merge <id>` on a worktree task with no uncommitted work.
- **Actual:**
  - The CLI prints "· The worktree was removed; its branch stays."
  - Afterwards, `git branch` in proj has no `wisp/task-0c2e98`. Ship's `tidy` (plugins/ship/host/src/ops.rs:149–157) deletes the branch here and pushes its deletion to the remote.
  - The sentence is fixed text in `merged_words` (crates/wisp/src/commands.rs:211). It is right for the archive tidy, which keeps the branch, but not after a merge.
- **Expected:** say the branch was deleted (here and on origin), or keep it.

### F2. [minor] Fork, /btw and a follow-up on a merged task end in a folder that is gone
- **Steps.** Ship and merge a task (its worktree and branch go; row 22b). Then `runtime_fork {id}` over MCP, `runtime_btw {id, question}`, and `wisp reply <id> …`.
- **Actual:**
  - The fork answers its id as if it worked. Its cwd is the merged task's removed worktree, and serve.log says "WARN … a fork's worktree (it runs in its parent's folder) error=no such folder". Its first follow-up shows "✗ Not started · This task's folder is gone: … It runs only there, so nothing was started."
  - /btw and a follow-up on the merged task answer in git's words: "This task's worktree couldn't come back on its branch wisp/task-0c2e98 (Conflict: fatal: invalid reference: wisp/task-0c2e98)."
  - The list shows both tasks as "Blocked".
- **Expected:**
  - Refuse the fork up front and say why, or fork onto a fresh branch from the merged commit.
  - Say in plain words that the task was merged and its branch is gone.

### F3. [major] The reply box never wraps or grows: a long follow-up runs off to the side
- **Steps.** Open any task. Type a follow-up longer than the box, or click Ask to fix (row 44d).
- **Actual** (F-fix-after.png, F-comp-1280.png, F-comp-390.png):
  - The reply box stays one line, 34 px tall. The text scrolls sideways (`scrollWidth` 806–904 in a 526 px box at 1280, a 336 px box at 390). The start of what you wrote is out of view.
  - After Ask to fix, the box shows "…ne/repo/actions/runs/42/job/4242). Find out why, fix it and push.", from the middle of a word.
  - Cause: `[data-kit='reply'] { height: 34px; white-space: nowrap; overflow: hidden; }` (packages/ui/src/css/app-task.css:838–844), and the reply textarea (packages/ui/src/app/Task.tsx:829) is never autosized.
  - The new-task box wraps (`pre-wrap`, 64 px).
- **v1** (web/css/composer.css:10; web/js/features/composer.js:157) puts `nowrap` only on the empty box's placeholder (`:placeholder-shown`), and `autosize(ta, 260)` grows the box with its text.

### F4. [smell] A turn's footer says "2 file"
- Every turn that changed 2 files shows "2 file +2 −0", on desktop and phone.
- `shell.task.files` is "{n} file" (packages/shell/src/i18n/en.ts:304), with no `.one`/`.other` forms, and is called with `{ n }` (packages/shell/src/frame/TaskView.tsx:1500).
- The side panel's own keys (`shell-task.changes.file`/`.files`) are right.

### F5. [smell] A side discussion's project shows as its parent's worktree folder
- A `/side` task runs in its parent's worktree, with no `wisp.worktrees` slot of its own.
- `wisp ls` gives its project as "proj-01a1f532312" (the folder name, cut).
- Its page shows no project and no branch.

### F6. [smell] CLI word slips
- A mistyped flag's hint drops the other flags. `wisp crew create --name Bob --agent codex --on done --cwd … --project proj` says "a task that says this: wisp -- crew create --name Bob proj" (no `--agent codex --on done`). For `--cron "every minute"` the quotes are lost: "--cron every minute proj".
- `wisp "…" --agent claude --try subagents=haiku` prints "differs: subagentModel  → haiku", with two spaces where the empty "from" value goes.

### F7. [smell] An agent tool's refusal isn't marked as an error
- As a run, `later_agent_schedule {}` answers `{"error":"Say when: at (a time) or in (minutes)."}` as ordinary content, without `isError`.
- Other refusals set `isError: true` (`wisp_launch`, `runtime_fork`, `crew_get`).

## Count

7 new findings (F1–F7):
- **Major (1):** F3
- **Minor (2):** F1, F2
- **Smell (4):** F4–F7
- **Blocker:** none

Audit E's 15 and D5/D6: 12 fixed, 3 partly fixed (E5, E11, D6), 2 not fixed (E6, E14).

**Open after this round: 12**, the 7 new plus E5, E6, E11, E14 and D6.

**NEW findings: 7.**

## Housekeeping
- **Ports and homes.** Only :4963 and /tmp/qaF1 were used. Nothing touched ~/.wisp*, :4477, :4800, :4791 or :4329.
- **Stopped.** My server is stopped (`lsof -ti tcp:4963 -sTCP:LISTEN | xargs kill`); no pkill. A background poll of `worktrees.view` I started was killed by its own pid.
- **Browsers.** Every Playwright browser of mine was closed. The other headless browsers running at the end belong to another session (/tmp/site-perf) and were left alone.
- **Gates.** Every gate file was released before the stop; no task was left running. Ada was paused, so no check-in was left queued. The "F13 later ui" plan (21:22) was left in the scratch home.
- **Kept.**
  - /tmp/qaF1 (scratch home, projects, records, scripts, and the copied binaries in opt/ and bin/) is kept for repro.
  - /tmp/wisp-target-fixer is kept, as the shared build cache.
  - Artifacts are in /tmp/qa-keep/shots-qaF1/.
- **Disk.** 110 GiB free at the end.
