# Audit C: orchestration, every action end to end (worktree HEAD 55c9d978), 2026-11-19

Re-runs audit B's table (docs/qa/audit-B-orchestration.md) and re-checks each of its 19 findings at HEAD 55c9d978.
Then looks for new faults: modes, harnesses, worktree vs in place, compare, try, attachments, goals, the queue, stop,
start later, fork, rerun, side and /btw, delegate, review, ship, restore, bring in, crew, automations, MCP and long
threads.

## Setup

- **Server.** A scratch `wisp serve` on **:4953**, built at HEAD into /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins`). It ran with WISP_HOME=/tmp/qaC1/home, HOME=/tmp/qaC1/userhome and WISP_WEB_DIR=/tmp/qaC1/web (a fresh `vite build` of apps/web).
  - An earlier, half-done attempt had left /tmp/qaC1. I wiped its home, records and state, and kept its project, scripts and fakes.
  - The worktree's only uncommitted edits are design/v1-reference images, so this build is HEAD's code.
- **Fake harnesses.** Every harness is the wisp-testkit fake, set through WISP_*_BIN: codex, opencode, cursor-agent, copilot, grok and agy, plus `wisp-fake-gh` as WISP_GH_BIN with its state in /tmp/qaC1/state/gh.json.
  - claude goes through audit B's QA wrapper (/tmp/qaC1/claude-qa.mjs). It handles judge prompts (`PASSAFTERn`, `NEVERMET`), `#ask`, `#tools`/`#delegate`, `#hang` and `#limit`.
  - `FAKE_GATE` holds a run "running" until a gate file appears.
  - Every fake records argv, stdin, cwd and pid (/tmp/qaC1/rec/*.jsonl, /tmp/qaC1/state/calls.jsonl).
- **Project.** /tmp/qaC1/proj, a git repo with math.js, whose bare origin is /tmp/qaC1/remote.git.
- **No outbound traffic.** DO_NOT_TRACK=1, WISP_TELEMETRY=0 and WISP_UPDATE_CHECK=0. The PostHog, grid, Linear, Slack, Discord and Telegram URLs pointed at 127.0.0.1:9.
- **No real harness.** The server's only children were the fakes and its own helper shell.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800. Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation.
- **Interfaces used.** The CLI, `/api/cmd`, `/api/mcp` as the user and as a run (its `x-wisp-session`/`x-wisp-token`), the stdio `wisp mcp`, and the UI.
- **Artifacts.** In /tmp/qa-keep/shots-qaC1/: screenshots, plus qaC1-serve.log, qaC1-rec/ and qaC1-calls.jsonl.
  - The server ran without RUST_LOG, so serve.log holds only the banner. Server-side warnings were not collected this round.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | Each fake got the prompt in its own `wisp/task-<id>` worktree; the project stayed clean. Branch names are now all the short form |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome, WebKit) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit…`; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`. Auto: `bypassPermissions` |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"` from the CLI and from the UI slider; Compare's `codex:gpt-5` → `gpt-5` |
| 4 | Worktree vs in place | CLI (`--no-worktree`), UI (Where menu) | pass, with finding C4 | In place runs in /tmp/qaC1/proj. The UI's "In place" also turns off the global worktree default |
| 5 | Compare | CLI, UI (Chrome, WebKit) | pass | Two tasks, each in its own worktree. B#13 and B#14 are fixed |
| 6 | Try (subagents=haiku) | CLI | pass | A and B are in separate worktrees; B's fake got `CLAUDE_CODE_SUBAGENT_MODEL=haiku`. `--try` without `--agent` is refused with a clear reason. The UI path was not tested |
| 7 | Attach files | UI (Chrome, WebKit) | pass | stdin has `<attachments>` with the absolute path "(text/plain, 13 B)"; argv has `--add-dir`. B#19's "1 KB" is fixed |
| 8 | Goal and judges | CLI, UI | pass | PASSAFTER2 met on round 2 (`state: met`). NEVERMET asks "The goal isn't met after 3 rounds… Keep going?" in `wisp show` |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | "Add delegate › With the lead" adds Codex. `spec.team=[{agent:codex}]`, and the lead's `delegate` reads "The team: - codex" with a `subagent` enum |
| 9b | Delegate a job to the team | per-run MCP | pass | codex ran in the lead's worktree and returned its report |
| 10 | Follow-up queue: queue, combine, edit, drag reorder, cancel (with confirm) | CLI, API, UI (Chrome, WebKit) | pass, with findings C2, C9 and C10 | The server queue matches after each UI step; feedback was 1–5 ms |
| 11 | Stop keeps the queue | CLI, UI | pass | "Stop Claude Code? Your queued follow-ups stay, held until you send one."; `queueHeld: true` |
| 12 | Send now | CLI, UI | pass | The picked item runs, then the rest drain in order |
| 13 | Start later | UI (Chrome, WebKit) | **partial** (B#6) | Toast "Starts 13:07" and the plan is in `later.plans` (`worktree:true`), but there is no Scheduled section in the list |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI | pass | The question and options show in `wisp show`; `wisp reply` reached the agent ("The user answered: SQLite") |
| 15 | Usage limit | CLI | pass, with finding C13 | "Claude Code (Main) is at its usage limit. Wait for the reset, or hand the task to another agent." |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass | Own worktree; feedback 14 ms |
| 17 | Rerun… | UI (Chrome) | pass, with finding C6 | A worktree task's rerun now gets its own worktree. B#2 is fixed |
| 17b | `wisp rerun <id>` | CLI | pass, with findings C8 and C12 | Own worktree |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass | A new task with `parentId`; feedback 12 ms |
| 19 | /btw | UI, CLI, HTTP MCP | pass | On grok, codex and copilot the side run gets a real temp path, and the question is in its prompt. B#1 is fixed |
| 20 | Review by another harness | CLI (`review maybe --force`) | pass | The result is kept, and an unreadable one reads "Claude Code answered, but not with a review wisp could read". B#12 is fixed |
| 21 | Ship: commit & open PR | UI, CLI | pass | `Wisp-Task:` trailer, branch pushed, fake PR #1 and #2; feedback 7–9 ms. With nothing to commit, the button is disabled |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass, with finding C7 | "…The worktree … was kept: it has 1 uncommitted change"; precious.txt survived; local main fast-forwarded. The confirm wording is still wrong (B#19) |
| 22b | `wisp merge` | CLI | pass | Merged; the worktree was removed |
| 23 | Restore to before latest turn | UI | pass | Confirm dialog, then the toast "The folder is back to before this turn. Undo"; the main checkout was untouched |
| 24 | Bring in | UI | pass (late, see B#18) | "1 worktree from git"; it became a task on `stray-branch` |
| 25 | Worktree removal | UI (Settings › Worktrees) | pass | The folder is gone and the branch stays; feedback 8 ms |
| 26 | Crew: add | CLI, API | pass, with finding C11 | `--on done` and `--team [{crew,why}]` work now (B#8 and B#9 fixed) |
| 27 | Crew: give a job | CLI, UI, HTTP MCP | pass | Jobs run in their own worktree (B#7 fixed); `crew assign` prints a line (B#17 fixed) |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass, with finding C3 | The cron fired every minute; "Check in now" counted 6 → 7 |
| 29 | Crew: pause / resume | CLI, UI | pass | `paused` toggles on the server |
| 30 | Crew: tint | UI | not re-checked | — |
| 31 | Crew: wakes on "I mark a task done" | API | pass | Bob's codex job: "task.done: C1 hello from claude (woken by trigger:crew.bob.1)", in its own worktree |
| 32 | Crew: Org link → the job gets its delegate | CLI | pass | Ada.team=[bob]; Ada's job's `delegate` lists "bob: Bob of the user's crew, Tester… use for: reviews" |
| 33 | `wisp ls --crew ada` | CLI | pass | Lists Ada's job and check-ins. B#10 is fixed |
| 34 | Recipes: new, edit, run, delete | UI | pass | Run defaults Project to proj (B#11 fixed) and ran in a worktree; Delete asks first |
| 35 | Triggers: new schedule, fires, on/off, edit, delete | UI | pass, with finding C3 | Runs in a worktree (B#7 fixed); the row reads "last run just now" (B#16 fixed); the switch took 22 ms |
| 36 | Notes, Thread view, Fresh thread | CLI | pass | `wisp notes --set goal=…`; `wisp fresh` starts without a resume and with the Notes in the brief |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | pass | Read and `set` both work (rev 1). B#3 is fixed |
| 37 | User MCP (`wisp mcp`, stdio): launch, queued reply, get, stop, wait | MCP | pass, with findings C5 and C16 | The queued reply is held through stop; `wait` returns the stopped state |
| 38 | HTTP MCP: fork, crew_assign, btw | MCP | pass | The fork is in its own worktree |
| 38b | HTTP MCP: every listed tool is callable | MCP (user and run) | pass | 240 tools as the user and 235 as a run (audit B: 446 and 279). Every tool called with `{}`: no "doesn't declare" or "may not" refusals (ADR-0107). B#4 is fixed |
| 39 | A task launched by an agent | per-run MCP | **partial** (C1) | `runtime_create {worktree:true}` gets a worktree (B#5 fixed for it). `wisp_launch` ignores `worktree` and `project` |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; feedback 4 ms |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | pass | No horizontal overflow on Home, a task or the crew page; a tapped queue row opens Send now / Edit / Combine / Move down / Cancel |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured click was 1–22 ms: launch 2–9, queue 1–5, fork 14, rerun 12–16, side 12, btw 15, restore 2–3, ship 7–9, merge 2–3, crew 4–20, recipe 1–5, trigger switch 22, worktree remove 8, Done 4, Bring in 2 |
| 43 | `wisp help runtime "queue order"` | CLI | pass | Prints the verb's help. B#15 is fixed |
| 44 | Issue, held turn's "Start now", Check logs, Try from the UI | — | not tested | No GitHub remote; the hold didn't trigger; the fake gh has no logs |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI, UI (Chrome, WebKit) | **fail** (C2) | Shown in 294 ms (Chrome) and 575 ms (WebKit) with no errors, but only 19 turns ran: 7 follow-ups were lost and the order was scrambled |
| 46 | Crew check-ins and schedule triggers over time | CLI, git | **fail** (C3) | 65 worktrees after ~40 min, against `worktrees.max` 40 |

**Counts:** 50 rows (46 actions plus 4 sub-rows).
- **Pass:** 43, of which 14 carry a lesser finding.
- **Partial:** 2 (rows 13 and 39).
- **Fail:** 2 (rows 45 and 46).
- **Not tested or not re-checked:** 2 (rows 30 and 44).
- Row 24 is counted as a pass; it appears late (B#18).

## Audit B's findings, re-checked

| B# | Finding | Status at 55c9d978 | Evidence |
|---|---|---|---|
| 1 | Side runs get a literal `{tmp}` | **fixed** | grok `--prompt-file /var/folders/…/wisp-side-…/grok-prompt.md`; copilot `…/wisp-side-…/copilot-usage.json`; codex lean `model_instructions_file="/var/folders/…/wisp-side-…"`. The question is in each prompt. No `{tmp}` in any record |
| 2 | UI Rerun runs a worktree task in the main checkout | **fixed** | The rerun's cwd is `home/worktrees/proj-01a1f3a8444…`. The dialog text is still wrong (C6) |
| 3 | `wisp_notes` refused | **fixed** | `wisp_notes {set:{decisions}}` → "Updated decisions (rev 1)", and `wisp notes` shows it |
| 4 | MCP lists tools it can't call | **fixed** | 240 tools (user) and 235 (run), no refusals in the sweep. Still open from its "Also": the stdio `wisp mcp` has 11 tools |
| 5 | Agent-launched tasks run in the main checkout | **partly fixed** | `runtime_create {worktree:true}` → worktree. `wisp_launch {worktree:true}` → main checkout (now C1) |
| 6 | Start later shows nowhere | **open** | `later.plans` holds "B26C later ui"; the list shows only "Needs you", even when scrolled |
| 7 | Crew jobs, check-ins and triggers run in place | **fixed** | Ada's job and check-ins, Bob's woken job and the "uitrig" trigger all run in `home/worktrees/proj-<id>`. This caused C3 |
| 8 | `crew.update --team` rejects its documented shape | **fixed** | `--team '[{"crew":"bob","why":"reviews"}]'` → team `[{name:bob, use:reviews, …}]` |
| 9 | `crew create --on done` read as `--on <machine>` | **fixed** | `wakes: [{crew:Bob, on:task.done}]` |
| 10 | `wisp ls --crew ada` says "No tasks." | **fixed** | Lists "Ada · Job" and the check-ins |
| 11 | Recipe Run defaults to "No folder" | **fixed** | The Run dialog's Project is "proj"; the run's cwd is a proj worktree |
| 12 | Review: wrong words, result not kept | **fixed** | `review view` shows `status failed`, "Claude Code answered, but not with a review wisp could read. Review again to ask once more." |
| 13 | Compare launch throws "Stale read from <Show>" | **fixed** | No page error on Compare in Chrome or WebKit |
| 14 | `--agent claude --compare codex` launches only codex | **fixed** | Launches claude and codex |
| 15 | `wisp help runtime "queue order"` launches a task | **fixed** | Prints the verb's help |
| 16 | Trigger row reads "last run now ago" | **fixed** | "last run just now" |
| 17 | `crew assign` prints raw session JSON | **fixed** | `session  01a1f39f… · Ada · Job · running` |
| 18 | Settings › Worktrees count and Bring in late | **partly fixed** | The header now counts every worktree (55, then 65). `worktrees.view` still says `looking: true` 40+ minutes after start, and Bring in appears 4–9 s after opening the page with no hint before it |
| 19 | Small things | **partly fixed** | Fixed: WebKit ResizeObserver error, "1 KB" for 13 bytes, branch naming. Still: the Merge confirm says it "removes the worktree" when it keeps it; each run is offered 235 tool schemas. Home's "Needs you" count is v1's (deliberate) |

**B totals:** 15 fixed, 3 partly fixed (#5, #18, #19), 1 open (#6).

## New findings

### C1. [major] `wisp_launch` from a run drops `worktree`, `project`, `scratch`, `effort` and `subagents`
- **Steps.** As a run, over its own MCP:
  - `wisp_launch {prompt, cwd:/tmp/qaC1/proj, worktree:true}`;
  - `wisp_launch {prompt, project:"proj"}`.
- **Expected:** what the tool's schema says: "worktree: Its own git worktree (default on in git repos, except in plan mode)", and "project: A project by name or id".
- **Actual:**
  - With `worktree:true` (or no worktree), the task ran in /private/tmp/qaC1/proj and edited math.js in the user's checkout.
  - With `project:"proj"`, it ran in an empty folder, home/scratch/01a1f39da117….
  - `runtime_create {worktree:true}` from the same run gets a worktree.
- **Cause.** plugins/mcp/host/src/run.rs:230 (`AgentAction::Launch`) builds `runtime.create` from `prompt`, `cwd`, `agent`, `model` and `mode` only. `project`, `worktree`, `scratch`, `also`, `machine`, `effort`, `subagents` and `preset` are dropped.
- **Note:** this is what remains of B#5. The "project → empty scratch folder" part is new.

### C2. [major] Quick follow-ups race the queue: some are lost with a raw SQL error, the rest can run out of order
- **Steps.** On a task that just finished (claude fake, ~100 ms turns), send `wisp reply <id> "C46 turn N" --queue` for N = 1…10 back to back.
- **Actual:**
  - Three of the ten replies (4, 7, 9) failed with `✗ UNIQUE constraint failed: runtime_turn.session_id, runtime_turn.idx` and never ran. The turns were 1, 2, 3, 5, 6, 8, 10.
  - With 25 replies (row 45), 7 were lost (3, 4, 9, 10, 11, 16, 19). The rest ran as 1, 2, 7, 5, 13, 6, 8, 22, 25, 12, …, not in the order sent. No fake record has the lost texts.
- **Expected:** every follow-up is queued or sent, in order, whatever the timing.
- **Effect:**
  - The user's words are lost, and the error is a raw SQLite message.
  - The UI composer and MCP `wisp_reply` go through the same `runtime.followUp`.
- **Suspect:** `runtime.followUp` picks the next turn index outside the transaction that drains the queue at a turn's end (plugins/runtime/host).

### C3. [major] Every crew check-in and every scheduled trigger run makes a new worktree and branch
- **Steps.**
  1. Ada with cron `* * * * *` (her check-ins), and the "uitrig" trigger set to every minute, as in B.
  2. Leave them on for ~10 minutes.
- **Actual:**
  - Each check-in and each trigger run got its own `home/worktrees/proj-<id>` and branch (`wisp/ada-check-in-<x>`, `wisp/uitrig-<x>`).
  - The project went from 25 to 65 worktrees in ~40 minutes, past `worktrees.max` 40: they hold the fake's uncommitted edit, so cleanup keeps them.
  - Each check-in is also its own row in the list ("Ada · Check-in", Needs you).
- **Expected:**
  - A recurring check-in reuses one folder (the member's), or removes its worktree when it ends with nothing to keep.
  - The fix for B#7 should not turn one member into 60 branches an hour.
- **Suspect:** plugins/crew/host and the automations task launch now set `worktree:true` per run.

### C4. [major, to confirm against v1] Picking "In place" for one launch turns off worktrees for every later task, CLI and MCP included
- **Steps.**
  1. Launcher › Where › In place, then launch.
  2. Later, `wisp "x" --agent claude -d` from the project folder.
- **Actual:**
  - The launcher wrote `settings.set worktrees.worktree = "off"` ("New tasks run in their own worktree by default").
  - Every later CLI launch ran in /tmp/qaC1/proj. Five test parents edited the user's checkout before I noticed.
  - Nothing in the launcher says the pick became the default everywhere.
- **Expected:** "In place" applies to that launch. If it is meant to be sticky, the launcher says so and the CLI stays on its own default.

### C5. [minor] The user's stdio MCP `wisp_launch` with a `cwd` runs in place
- **Steps.** `wisp mcp` › `wisp_launch {prompt, cwd:/tmp/qaC1/proj}`.
- **Actual:**
  - The task ran in /private/tmp/qaC1/proj, the user's checkout.
  - With `project:"proj"`, it got a worktree.
  - The tool has no `worktree` option, and the CLI defaults to a worktree.
- **Effect:** an agent in the user's harness that passes its cwd edits the user's checkout.

### C6. [minor] The Rerun dialog names the main checkout, but the rerun gets a worktree
- "Starts a new task in /private/tmp/qaC1/proj on this computer." The new task's cwd is `home/worktrees/proj-01a1f3a8444…`.

### C7. [minor] Ship › "Merge PR #2…" opens nothing
- **Steps.** Ship menu › "Merge PR #2…".
- **Actual:** no dialog and no command within 500 ms. Only the PR card's Merge button opens "Squash-merge PR #2?".
- **Expected:** the "…" item opens the same confirm.

### C8. [minor] `wisp rerun` leaves a detached `rerun-<sha>-<x>` worktree behind
- **Steps.** `wisp rerun 01a1f39be987…` at 12:48.
- **Actual:**
  - /tmp/qaC1/home/worktrees/rerun-ec2effc96b-104a5da7 (detached at ec2effc) stays.
  - Settings › Worktrees lists it as "No task" (`leftovers: 1`).
  - The rerun itself ran in its own `proj-<id>` worktree.

### C9. [minor] `wisp runtime queue order <id> <order>`, as its usage line shows, says "✓ done" and changes nothing
- Only `--order a,b,c` reorders. A positional order should work, or be refused.

### C10. [minor] `wisp runtime queue edit --items` needs an `at` its help doesn't mention
- **Steps.** `--items '[{"id":…,"text":…}]'`.
- **Actual:** "✗ runtime.queue.edit: missing field `at`". The help says only "the queue in this order, with your edited texts".

### C11. [minor] A crew schedule of "every minute" is refused
- "✗ Couldn't read "every minute" as a schedule. Try "every monday", "8am every weekday" or "every 30 min"."
- `* * * * *` works. A user who reads the hint and types "every minute" gets refused.

### C12. [minor] `wisp rerun <id> -d` prints the usage text
- `-d` ("launch it and return with its id") is documented for launches; rerun refuses it instead of accepting or ignoring it.

### C13. [smell] `wisp show` on a usage-limit task prints raw stderr
- After the right message, it shows an empty `$ ` line and `│ Claude AI usage limit reached|1767225600`, with the epoch.

### C14. [smell] Raw values in CLI output
- `wisp automations trigger ls` prints `uitrig off 0 trigger 1795089973373`: epoch ms and no headers.
- `wisp runtime queue send-now` prints `turnId 01a1f3a1…`.

### C15. [smell] `wisp ls` shows the worktree folder in the project column
- It shows `proj-01a1f39bea4`, `proj-01a1f39be98`… for worktree tasks, `proj` for in-place ones.

### C16. [smell] `wisp_stop` answers "stopped" while the task is still running
- Right after `wisp_stop` → "stopped", `wisp_get` said `[running]`, and so did `runtime.get`.

### C17. [smell] Home sends `runtime.defaultModel` about 20 times before the first launch
- Seen in Chrome's requests on a fresh load of `/` (launch.mjs).

### C18. [minor, to confirm] copilot `/btw` resumes the task id, not the run's copilot session
- The task turn ran `--session-id 01a1f39be8c575d0…` (the run's id).
- The /btw side run passed `--session-id 01a1f39be8c4721…` (the task's id).
- A real copilot would answer without the conversation.

## Count
18 new findings (C1–C18):
- **Major (4):** C1–C4
- **Minor (9):** C5–C12, C18
- **Smell (5):** C13–C17
- **Blocker:** none

Audit B's 19: 15 fixed, 3 partly fixed (#5, #18, #19), 1 still open (#6).

**Open after this round: 22**, the 18 new plus B#6, B#18, B#19 and the stdio part of B#4. B#5's remainder is counted as C1.

**NEW findings: 18.**

## Housekeeping
- **Ports and homes.** Only :4953 and /tmp/qaC1 were used. Nothing touched ~/.wisp*, :4477, :4791 or :4800.
- **Stopped.** My server (pid 6413) is stopped; its helper shell exited with it, and no fake process of mine remains. I stopped my own pid only; no pkill.
- **Gates.** All gate files (g1–g5) were released before the stop.
- **Kept.** /tmp/qaC1 (scratch home, project and records) is kept for repro. /tmp/wisp-target-fixer is kept, as the shared build cache. Artifacts are in /tmp/qa-keep/shots-qaC1/.
- **Disk.** 162 GiB free at the end.
