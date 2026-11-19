# Audit B: orchestration, every action end to end (worktree HEAD c0d655ad + others' uncommitted edits), 2026-11-19

Re-runs audit A's table (docs/qa/audit-A-orchestration.md) after its fixes in c0d655ad, plus what it missed:
every harness, Try, MCP (user-level and per-run), crew org links, trigger edit/pause/delete, phone queue.

## Setup

- **Server.** A scratch `wisp serve` on **:4937**, built from this worktree into /tmp/wisp-target-qaB1. It ran with WISP_HOME=/tmp/qaB1/home, HOME=/tmp/qaB1/userhome and WISP_WEB_DIR=/tmp/qaB1/web (a `vite build` of apps/web).
  - Port 4900 was free when scanned, then taken by another session's server before mine bound. I left that one alone.
  - The web build includes other sessions' **uncommitted** edits (packages/shell, packages/ui, plugins/shell-list, worktrees ui…). UI findings may come from work in progress; each one names its file.
- **Fake harnesses.** Every harness is the wisp-testkit fake, set through WISP_*_BIN: codex, opencode, cursor-agent, copilot, grok and agy, plus `wisp-fake-gh` as WISP_GH_BIN with its state in /tmp/qaB1/state/gh.json.
  - claude goes through a QA wrapper (/tmp/qaB1/claude-qa.mjs) around the fake. The wrapper:
    - answers goal judges: `PASSAFTERn` is met on round n, `NEVERMET` is never met;
    - calls `ask_user` over the run's own MCP (`#ask`);
    - dumps the run's MCP tools and calls `delegate` (`#tools`, `#delegate`);
    - hangs (`#hang`) or hits a usage limit (`#limit`).
  - `FAKE_GATE` holds a run "running" until a gate file appears.
  - Every fake records argv, stdin, cwd and pid (/tmp/qaB1/rec/*.jsonl, /tmp/qaB1/state/calls.jsonl).
- **Project.** /tmp/qaB1/proj, a git repo with math.js, whose bare origin is /tmp/qaB1/remote.git.
- **No outbound traffic.** DO_NOT_TRACK=1 and WISP_TELEMETRY=0. WISP_UPDATE_CHECK=0. WISP_POSTHOG_HOST, WISP_GRID_URL and the Linear/Slack/Discord/Telegram API URLs pointed at 127.0.0.1:9. serve.log names no external host.
- **No real harness.** `ps` was checked during the run: my server's children were only the fakes. Real `claude` processes on the machine belong to other servers, not :4937.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800. Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation (MutationObserver).
- **Artifacts.** Screenshots in /tmp/qa-keep/shots-qaB1/. The server log and the fakes' records are next to them as qaB1-serve.log, qaB1-rec/ and qaB1-calls.jsonl.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses (claude, codex, opencode, cursor, copilot, grok, antigravity) | CLI | pass | Each fake got the prompt (stdin or prompt file) in its own `wisp/task-<id>` worktree, and the project stayed clean. All 7 are detected now; audit A only had 2 |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome, WebKit) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit MultiEdit Write NotebookEdit`; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`. Auto: `bypassPermissions` |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`, `--model sonnet --effort low`. The UI thinking slider gives codex `model_reasoning_effort="high"`, and Compare gives `codex:gpt-5` → `gpt-5` |
| 4 | Worktree vs in place | CLI (`--no-worktree`), UI (Where menu) | pass | In place runs in /tmp/qaB1/proj. Worktree gets its own folder and branch |
| 5 | Compare | CLI (`--compare claude,codex:gpt-5`), UI (Chrome, WebKit) | pass, with findings #13 and #14 | Two tasks, each in its own worktree, sharing one `group` ("vs Compare · Pick one"). Audit A #7 is fixed |
| 6 | Try (subagents=haiku) | CLI | pass | Runs A and B are in separate worktrees; B's fake got `CLAUDE_CODE_SUBAGENT_MODEL=haiku`. The UI path was not tested |
| 7 | Attach files | UI (Chrome, WebKit) | pass | stdin has `<attachments>` with the file's absolute path, and argv has `--add-dir <stuff/files/sha>`. Audit A #1 is fixed |
| 8 | Goal and judges | API, CLI, UI (Chrome, WebKit) | pass | PASSAFTER2 was met on round 2. NEVERMET stalled after 3 rounds, and the question shows in `wisp show`. `goal drop` clears it, `pause` stops judging (0 judge calls on a reply), `resume` + `check` judge again |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | `spec.team` is stored, and the lead's `delegate` tool reads "The team: - codex …" with a `subagent` enum. Audit A #2 is fixed |
| 9b | Delegate a job to the team (subagents) | per-run MCP | pass | `delegate {tasks:[{subagent:"tester"…}]}` ran codex `tester#1` in the lead's folder and returned its report |
| 10 | Follow-up queue: queue, combine, edit, drag reorder, cancel (with confirm) | CLI, API, UI (Chrome, WebKit) | pass | The server queue matches after each step. UI feedback was 1–17 ms |
| 11 | Stop keeps the queue | CLI, UI (Chrome, WebKit) | pass | "Stop Claude Code? Your queued follow-ups stay"; the run is killed and the queue held |
| 12 | Send now (held queue, and while running) | CLI, UI | pass | It runs the picked item, then the rest drain in order. On a running `#hang` run it kills the run and sends the follow-up with `--resume` |
| 13 | Start later | UI (Chrome, WebKit) | **partial** (#6) | The plan keeps `worktree:true` and fired on time in its own worktree. Nothing in the list or on Home shows it |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI | pass | The question and options show in `wisp show`; `wisp reply` reached the agent ("The user answered: SQLite") |
| 15 | Usage limit | CLI | pass | "Claude Code (Main) is at its usage limit. Wait for the reset, or hand the task to another agent." |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass | Own worktree and branch (`wisp/b41c-fork-try-it-with-a-loop-…`), `--resume --fork-session`. Feedback 13 ms. Audit A #4 is fixed |
| 17 | Rerun… | UI (Chrome) | **fail** (#2) | The rerun of a worktree task ran in the main checkout with no branch |
| 17b | `wisp rerun <id>` | CLI | pass | Own worktree; `spec.worktree:true` |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass | A new task with `parentId`, the parent's conversation in its brief, `--permission-mode default`. Audit A #5 is fixed |
| 19 | /btw | UI, CLI, HTTP MCP | pass (claude); **fail** on grok, and side runs on codex/copilot (#1) | The UI card shows text, not raw JSON (audit A #10 fixed). On grok, the question never reaches the agent |
| 20 | Review by another harness | CLI (`review maybe --force`) | **partial** (#1, #12) | codex now runs, read-only, in the PR's worktree (audit A #20 fixed). It gets a literal `{tmp}/codex-base.md`, and the result can't be kept (`runtime.markRead` refused) |
| 21 | Ship: commit & open PR | UI, CLI | pass | Commit with a `Wisp-Task:` trailer, branch pushed to origin, fake PR #1–#3. Feedback 6–19 ms |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass | "…The worktree … was kept: it has 1 uncommitted change"; precious.txt survived; local main fast-forwarded. Audit A #3 is fixed |
| 22b | `wisp merge` (clean worktree) | CLI | pass | Merged, worktree removed |
| 23 | Restore to before latest turn | UI | pass | Confirm dialog; only the task's worktree was reset (main untouched); "Undo" toast; the Changes chip is updated. Audit A #17 is fixed |
| 24 | Bring in | UI | pass | Only the stray worktree is offered ("1 worktree from git"); it became a task on `stray-branch`. Audit A #6 is fixed |
| 25 | Worktree removal | UI (Settings › Worktrees) | pass | The confirm names the uncommitted changes; the folder is gone and the branch stays |
| 26 | Crew: add | CLI, API | pass, with findings #8 and #9 | Ada (cron, `bg: teal`) and Bob (wakes on done) |
| 27 | Crew: give a job | CLI, UI (job box), HTTP MCP | pass, with finding #7 | Jobs run and appear in the thread. Every one ran in /tmp/qaB1/proj (main checkout) |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass | Check-ins counted (3 → 5); the cron fired each minute |
| 29 | Crew: pause / resume | CLI, UI | pass | `paused` toggles on the server; the cron stops |
| 30 | Crew: tint | UI | pass | The sidebar is tinted teal while Ada is open |
| 31 | Crew: wakes on "I mark a task done" | API | pass | Bob's codex job: "task.done: B40 parent side (woken by trigger:crew.bob.1)" |
| 32 | Crew: Org link → the job gets its delegate | UI | pass | "Make Bob Ada's delegate" sets Ada.team=[bob]; Ada's next job's `delegate` lists "bob". Audit A #33b is fixed |
| 33 | Crew: `wisp ls --crew ada` | CLI | **fail** (#10) | "No tasks." |
| 34 | Recipes: New recipe, edit, run with a field from Settings, delete | UI | pass, with finding #11 | The name is made a slug (`Fix-Thing`); Run asks for `{thing}` (audit A #12 fixed); Edit saves; Delete asks first |
| 35 | Triggers: new schedule (time and project), fires, on/off, edit, delete | UI | pass, with findings #7 and #16 | "Every minute in proj" fired. The switch turns it off on the server; Edit saved the prompt; Delete asks. Crew triggers are no longer listed. Audit A #8 and #9 are fixed |
| 36 | Notes (set and read), Thread view, Fresh thread | CLI, UI | pass | `wisp notes --set goal=…`; Thread shows it; `wisp fresh` starts a new conversation (no `--resume`) with the brief and the Notes |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | **fail** (#3) | Refused for every run |
| 37 | User MCP (`wisp mcp`, stdio): launch, reply `queue`, get, stop, wait | MCP | pass | Launch got a worktree (codex, plan); the queue was held through stop; `wait` returns the state |
| 38 | HTTP MCP (`/api/mcp`): fork, crew_assign, btw | MCP | pass | As in the API |
| 38b | HTTP MCP: goal, automations, threads, review, settings, git, history, worktrees tools | MCP | **fail** (#4) | Listed, then refused at call time |
| 39 | A task launched by an agent (`runtime_create` with `worktree:true`, `wisp_launch`) | per-run MCP | **fail** (#5) | Runs in the main checkout |
| 40 | Done on a running task | UI | pass | It stays put, with the toast "Still working — stop it first or let it finish." Audit A #18 is fixed |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | pass | No horizontal overflow on Home, a task or the crew page. Composer controls appear when the input is focused, and a tapped queue row opens Send now / Edit / Combine / Combine all / Move down / Cancel. Audit A #23 is fixed |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured click was 1–28 ms: launch 2–8, queue 1–17, fork 13, rerun 14, restore 3, ship 6–19, merge 2–3, crew 4–28, trigger switch 20, worktree remove 11 |
| 43 | `wisp help runtime "queue order"` | CLI | **fail** (#15) | Launched a task "help runtime queue order" |
| 44 | Issue, held turn's "Start now", Check logs, Try from the UI | — | not tested | No GitHub remote; the hold didn't trigger; the fake gh has no logs; time |

**Counts:** 49 rows (44 actions plus 5 sub-rows).
- **Pass:** 39, of which 8 carry a lesser finding (rows 5, 26, 27, 34, 35 and others).
- **Partial:** 3 (rows 13, 19, 20).
- **Fail:** 6 (rows 17, 33, 36b, 38b, 39, 43).
- **Not tested:** 1 row (4 items).

## Findings

### 1. [major] Side runs on codex, grok and copilot get a literal `{tmp}` in their arguments
- **What runs this way:** /btw, goal judges, review, quick answers and retro all go through `side_run`.
- **Steps:**
  1. `wisp btw <grok task> "B22 btw on grok"`.
  2. Read rec/grok.jsonl.
- **Expected:** the question reaches grok, as in a task turn (`--prompt-file /var/folders/…/wisp-run-<id>/grok-prompt.md`).
- **Actual:**
  - argv is `--prompt-file {tmp}/grok-prompt.md` (a relative path that doesn't exist), and stdin is empty. Grok never gets the question; the fake answers anyway, so wisp shows an "answer".
  - The same `{tmp}` is passed for:
    - codex lean side runs: `-c model_instructions_file="{tmp}/codex-base.md"`, seen on the review run and the retro run;
    - copilot's `{tmp}/copilot-usage.json`.
  - A real CLI would fail or answer blind.
- **Cause, from the code:**
  - `side_run` in **plugins/runtime/host/src/side.rs:78** calls `h.launch(run)` and spawns `launch.args` as they are.
  - `spawn_run` (plugins/runtime/host/src/run.rs:553–555) first writes the launch files (`write_files`) and replaces `{tmp}`. side_run does neither.

### 2. [major] Rerun… from the UI runs a worktree task's copy in the main checkout
- **Steps:**
  1. A task launched in a worktree ("B40 parent rerun"; `runtime.spec` has `worktree:true`).
  2. ⋯ › Rerun… › Rerun. The dialog says "Starts a new task in /tmp/qaB1/proj on this computer".
- **Expected:** a new worktree and branch, as `wisp rerun` does (row 17b) and as the parent had.
- **Actual:**
  - The new task's spec is `{cwd:/tmp/qaB1/proj, lineage:{rerunOf}, …}` with no `worktree`.
  - The fake ran in /private/tmp/qaB1/proj and edited math.js and notes.txt in the main checkout.
- **Suspect:** packages/shell/src/frame/rerun.ts:55, which copies `cwd` from the spec but not `worktree`. The commit for audit A says "Rerun … get their own worktree"; that's true for the CLI only.

### 3. [major] Every agent's `wisp_notes` call is refused, so Notes are never kept by the agent
- **Steps:** as a run (its `x-wisp-session` / `x-wisp-token`), call `wisp_notes {}` or `wisp_notes {set:{decisions:"…"}}`.
- **Expected:** it reads or updates the task's Notes. Every run's prompt says: "keep this task's Notes with `mcp__wisp__wisp_notes`".
- **Actual:** "wisp: threads.notes refused: wisp.mcp doesn't declare threads:read in its manifest". serve.log has 3 such WARNs.
- **Effect:** Fresh thread then starts only from what the user typed into Notes.
- **Suspect:** plugins/mcp/host/wisp-plugin.json, whose permissions lack `threads:*` (see #4).

### 4. [major] `/api/mcp` and each run's MCP list about 250 tools that wisp.mcp can't call
- **Steps:**
  1. `tools/list` on `/api/mcp` lists 446 tools, and a run's MCP lists 279.
  2. Call any of `goal_set`/`drop`/`pause`/`check`, `automations_task_run`, `automations_tasks`, `threads_fresh`, `threads_notes`, `review_view`, `settings_list`, `git_status`, `history_list` or `worktrees_done_list`.
- **Expected:** a listed tool works, or isn't listed. ADR-0040: every feature reachable from the user's harness.
- **Actual:** each one fails with "refused: wisp.mcp doesn't declare <plugin>:<verb> in its manifest".
  - The manifest grants only `connectors crew events:tail/why later machines plugins runtime ship stuff telemetry:track watch`.
  - Outside those groups, 26 plugin groups are listed: accounts, automations, budget, channels, git, goal, grid-client, history, issues, learn, lsp, machine, mcp, push, resources, review, router, sandbox, settings, share, telemetry, termctx, terminal, threads, update, workflows, worktrees.
- **Also:** the user's stdio MCP (`wisp mcp`) has only 11 tools. A user in their harness can't fork, set a goal, give a crew job or ship through it.
- **Suspect:** plugins/mcp/host/wisp-plugin.json, and the tools/list filter that ignores the manifest.

### 5. [major] A task an agent launches runs in the main checkout, even with `worktree:true`
- **Steps:** as a run, MCP `runtime_create {prompt, cwd:/tmp/qaB1/proj, worktree:true}`, or `wisp_launch {prompt, cwd}`.
- **Expected:** a worktree, as the same `runtime.create` over HTTP gives (B5) and as the CLI and user MCP give.
- **Actual:**
  - The task's spec says `worktree:true`, but its cwd is /tmp/qaB1/proj, and the fake ran there.
  - serve.log only warns: "runtime: worktrees.create (it runs in its folder) error=worktrees.create refused: agent:session may not worktrees:create".
  - An agent fanning work out therefore puts every child in the user's checkout.
- **Suspect:** the agent:session capability set (packages/kernel capabilities / plugins/runtime/host/src/plugin/create.rs falling back to in place).

### 6. [minor] Start later: the scheduled task shows nowhere
- **Steps:** Launcher › + › Start later › In 10 min › "Start at 01:43", in Chrome and in WebKit.
- **Expected:** a Scheduled row in the list (sections.ts `plansIn('scheduled')`), as the commit for audit A says.
- **Actual:**
  - `later.plans` holds the plans (`worktree:true`, fixed), and the page subscribes to `plugin:wisp.later:later`.
  - The list has only "Needs you", with no Scheduled section, under All and under proj, even after a reload. Home has nothing either.
  - The plan fired on time, in its own worktree.
- **Suspect:** plugins/shell-list/ui/src/model/sections.ts and wire.ts `listPlan`. shell-list had uncommitted edits in this build.

### 7. [minor, to confirm against v1] Crew jobs, check-ins and triggers run in place in the project's main checkout
- **Where:**
  - Ada's jobs and every-minute check-ins, and Bob's woken job, all have cwd /private/tmp/qaB1/proj. Bob's run left `notes.txt` there.
  - The "uitrig" schedule trigger also runs in place (`proj` in `wisp ls`).
- **Effect:** parallel crew jobs (`2 jobs at once`) and check-ins edit the same folder as the user and each other.
- **Mismatch:** the delegate tool describes Bob as having "own folder and memory".
- **Suspect:** plugins/crew/host (never sets `worktree`), and the automations task launch.

### 8. [minor] `crew.update --team` rejects the shape its own help documents
- **Steps:** `crew.update {id:"ada", team:[{crew:"bob", why:"reviews"}]}`. The help says `--team JSON [{ crew, why? }]`.
- **Actual:** "unknown subagent harness: ". The Org page stores `{agent, crew, name, …}` instead.

### 9. [minor] `wisp crew create --on done` is read as the global `--on <machine>`
- **Actual:** "there's no machine done: this wisp has none yet". The crew verb's own `--on` (v1 trigger kinds) can't be given from the CLI. The API takes `on:["done"]`.

### 10. [minor] `wisp ls --crew ada` says "No tasks."
- Ada has 7 or more jobs and check-ins (`wisp ls` shows "Ada · Job" rows).

### 11. [minor] Recipe Run from Settings defaults to "No folder"
- **Steps:** Recipe with Project "Any project (picked when run)" › Run. The dialog's Project is "No folder".
- **Actual:** the task ran in home/scratch/<id>, with "fix the sign in math.js" asked in an empty folder.
- **Expected:** the launcher's current project (proj).

### 12. [minor] Review: an unreadable answer reads "Codex didn't answer", and the result can't be kept
- With the fake's non-JSON answer, the review is `failed` with "Codex didn't answer". The reviewer did answer; it wasn't a review JSON.
- serve.log: "review: couldn't keep the result error=Forbidden: runtime.markRead refused: plugin:wisp.review may not runtime:markRead". See plugins/review/host/src/flow.rs:211 and wisp.review's manifest.

### 13. [minor] Compare launch throws a Solid error
- **Steps:** Launcher › + › Compare with another agent… › Codex › Launch, in Chrome and in WebKit.
- **Actual:** `pageerror: Stale read from <Show>.` Only the Compare launch does this; goal and later launches are clean.

### 14. [minor] `wisp "x" --agent claude --compare codex` launches only codex
- It prints one id. `--compare` replaces the task list rather than adding to `--agent`. It should add, or refuse the mix.

### 15. [minor] `wisp help runtime "queue order"` launches a task
- `wisp help runtime` lists verbs as "queue order", "queue send-now". Asking help for that spelling launched an agent task titled "help runtime queue order" in the CLI's folder. `wisp help runtime queue.order` works.

### 16. [smell] Trigger row reads "last run now ago"
- Settings › Automations › Triggers, just after a run.

### 17. [smell] `wisp crew assign` prints the raw session JSON
- It's wrapped in a two-column table (`session  {"agent":"claude",…`), instead of a line naming the job.

### 18. [smell] Settings › Worktrees showed 13 of 41 worktrees for minutes
- The header read "13 worktrees · 288 KB" while `worktrees.view` already had 41 items (`looking: true`). There was no "still looking" hint, and "Bring in" was missing until it finished.

### 19. [smell] Small things seen once
- WebKit `pageerror: ResizeObserver loop completed with undelivered notifications.` (launch, queue).
- A 13-byte attachment is listed to the agent as "1 KB".
- The first claude worktree's branch is `wisp/task-01a1f1`, while every other one carries the full id.
- Home's "Needs you" still counts finished tasks ("5 of 22", sidebar "Needs you 20" with 1 waiting).
- The Merge confirm still says it "removes the worktree" even when it will keep it. It tells the truth only after.
- Each run is offered 279 MCP tool schemas.

## Fixed since audit A (re-checked here)
Audit A's findings now confirmed fixed:
- #1 attachments
- #2 team reaches the lead
- #3 merge keeps uncommitted work
- #4 fork's own worktree
- #5 side discussion
- #6 Bring in
- #7 CLI Compare
- #8 and #9 trigger edit, pause, delete and schedule fields
- #10 /btw text in the UI
- #11 question in `wisp show`
- #12 recipe field in Settings
- #17 Restore confirm and chip
- #18 Done on running
- #20 review runs (partly, see #1 and #12)
- #23 phone queue
- #33b org links

Still open: #13 (visibility, now #6 here), and #24 (Needs you counts).

## Count
19 findings:
- **Major (5):** #1–#5
- **Minor (10):** #6–#15
- **Smell (4):** #16–#19
- **Blocker:** none

## Housekeeping
- **Ports and homes.** Only :4937 and /tmp/qaB1 were used. Nothing touched ~/.wisp, ~/.wisp-dev, ~/.wisp-v2-preview, :4477, :4791 or :4800.
- **Stopped.** My server (pid 18774) is stopped, and no fake process of mine remains. I stopped my own pids only; no pkill.
- **Other sessions.** Another session's server started on :4900 mid-setup, and another audit (qaB3) ran its own fakes. I didn't touch either.
- **Kept.** /tmp/qaB1 (scratch home, project and records) is kept for repro. /tmp/wisp-target-qaB1 is deleted.
