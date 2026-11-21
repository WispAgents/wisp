# Audit A: orchestration, every action end to end (worktree HEAD 2013facf), 2026-11-18

**Setup**
- Scratch `wisp serve` on :4796 (built from this worktree into /tmp/wisp-target-qaA), WISP_HOME=/tmp/qaA/home, HOME=/tmp/qaA/userhome.
- Every harness is the wisp-testkit fake: claude through a QA wrapper (/tmp/qaA/bin/claude) that answers goal judges, asks questions, hangs or hits limits on cue. Plus codex, opencode, cursor-agent, copilot, grok, agy, and a fake `gh` with a state file.
- Project: /tmp/qaA/proj, with a bare origin at /tmp/qaA/remote.git.
- `ps` was checked during the run: no real claude or codex ran under the server.

**How**
- **CLI and API:** the CLI (`wisp …` with the scratch home), the HTTP command API (`/api/cmd`) and MCP (`/api/mcp`, with a run's own session token).
- **UI:** Playwright on system Chrome and on WebKit at 1280, plus a phone spot check at 390 (WebKit light, Chrome dark).
- **Checks:** every action was checked end to end. That means the fake's recorded argv, stdin and cwd (rec/*.jsonl and state/calls.jsonl), the task record (`runtime.get` / `runtime.turns`), git state, and the fake gh state.
- **Feedback timing:** measured in the page, from click to the first DOM change (MutationObserver).

Screenshots: /tmp/qa-keep/shots-A/ (uiA*-…png).

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, basic run | CLI, UI | pass | fake argv/stdin; task reaches review |
| 2 | Mode Plan / Edits / Full auto | CLI, UI | pass | Plan = bypass plus `--disallowedTools Edit MultiEdit Write NotebookEdit` plus plan note (by design, v1 #794); codex Plan = `sandbox_mode="read-only"`; spec.mode=plan from the UI |
| 3 | Harness / model / thinking | CLI, UI | pass | codex runs `codex exec`; `--model` and effort flags reach argv |
| 4 | Project picker / worktree vs in place | CLI, UI | pass | UI default "This computer · Worktree" makes `wisp/task-<id>` worktrees; in place edits the project |
| 5 | Compare (vs) | UI | pass | two tasks (claude and codex), each in its own worktree, grouped "vs Compare · Pick one" |
| 5b | Compare (vs) | CLI/API | **fail** (#7) | both twins ran in the project folder |
| 6 | Try (subagent model) | CLI | pass | the second run carries `haiku` |
| 7 | Attach files | UI | **fail** (#1) | chip shows, the turn records it, the agent never gets it |
| 8 | Goal + judges | UI, API | pass | UI launch with Goal: judged until MET (PASSAFTER1 met after 2 rounds); never-met stalls at 3 rounds with "Keep going?"; pause, resume, drop |
| 8b | Set goal… on a task | UI | pass | PASSAFTER2 met after 3 rounds |
| 9 | Delegate: add, copies, name, Use for, Save as preset, Presets, Clear | UI | pass (UI) | preset saved in `runtime.teams` ("Test team", tester ×2), reloads from Presets, Clear empties |
| 9b | Delegate reaches the lead | UI, API, MCP | **fail** (#2) | spec.team stored; MCP `delegate` says "No team yet" |
| 10 | Start later | UI, API | pass/partial (#13) | `later.plans` holds it, it fires on time; nothing on Home shows it |
| 11 | Recipes from the launcher (with a field) | UI | pass | asks for `{thing}`, runs "fix the UIFIELD in math.js", toast "fixmath is ready · Open" |
| 12 | Issue | UI | not testable | proj's remote isn't GitHub ("has no GitHub remote"); Linear and Jira need sign-in |
| 13 | @mention / slash popup | UI | pass | `/` lists /btw, /side, /goal, /draft, /after, /login…; `@` opens the mention list |
| 14 | Follow-up queue: queue, combine, edit, drag reorder, cancel | UI, CLI | pass | server queue order matches after each; Chrome and WebKit |
| 15 | Stop keeps the queue | UI, CLI | pass | "Stop Claude Code? Your queued follow-ups stay"; queue held after stop |
| 16 | Send now (held queue / running, resumable) | UI, CLI | pass | runs the picked item, then the rest drain in order; while running it interrupts (`#hang` run killed, follow-up `--resume`) |
| 17 | Ask (ask_user) and answer | UI, API | pass | waiting → click "Postgres" → running |
| 17b | Ask shown in the CLI | CLI | **fail** (#11) | `wisp show` / `wait` hide the question |
| 18 | Question block + reply option | UI | pass | options rendered, the reply sends |
| 19 | Held turn: harness changed → start | API | pass | held, `runtime.release` starts it and re-pins |
| 19b | Held turn "Start now" | UI | not reproduced | the hold didn't trigger again this round |
| 20 | Usage limit | API | pass | "Wait for the reset, or hand the task to another agent" |
| 21 | Fork… | UI, API | **fail** (#4) | new task copies the turns (`--resume … --fork-session`) but runs in the parent's worktree |
| 22 | Rerun… | UI, CLI | pass | new task, same prompt |
| 23 | Side discussion… (`/side`) | UI | **fail** (#5) | sent as an ordinary follow-up |
| 24 | /btw | UI, CLI | pass/partial (#10) | answered on the side (`--print --fork-session`); answer shows raw wisp JSON |
| 25 | Fresh thread, Notes (Thread) | CLI, UI | pass | `fresh` starts with no resume; Thread shows the Notes sections |
| 26 | Restore to before latest turn | UI | pass/partial (#17) | worktree restored; header count stale |
| 27 | Save as recipe… | UI | pass | `uirecipe` saved |
| 28 | Add to crew… | UI | pass | member "Cy", `*/30 * * * *`, check-in fired at 22:00 |
| 29 | Crew: add from template (CI watcher), Custom… schedule | UI | pass | member saved with role, brief, project, cron |
| 30 | Crew: give a job, Check in now, Pause/Resume, Edit… | UI (Chrome, WebKit) | pass | jobs and check-ins counted; paused toggles; role edited |
| 31 | Crew: wakes on cron / on "I mark a task done" | API | pass | Ada's 1-min cron fired; Bob woke on mark done |
| 32 | Crew: share (Download), Share your crew, import (File…) | UI, CLI | pass | dee.crew.json downloaded and imported as "Dee 2", paused |
| 33 | Crew: org chart link | UI | pass (UI) | "Make Bob Ada's delegate" → Ada.team=[bob]; "1 link" |
| 33b | Crew job gets its delegates | UI, MCP | **fail** (#2) | Ada's job: spec.team null, `delegate` "No team yet" |
| 34 | Crew: Track record… | UI | pass/smell (#22) | per-member cost and job split |
| 35 | Triggers: create (task.done) in UI | UI | pass | fired on Done: "UITRIG fired task.done: Ada · Job" |
| 36 | Triggers: create schedule in UI | UI | **fail** (#9) | no time or project field; Save silently does nothing |
| 37 | Triggers: pause / edit / delete in UI | UI | **fail** (#8) | not offered |
| 38 | Triggers via formula: cron, manual fire, disable | CLI | pass | fires each minute with a full project path; disabled = 0 runs |
| 38b | Trigger with a project name, test, prune | CLI | **fail** (#14, #15) | see findings |
| 39 | Recipes: Run now (Settings) | UI | pass/fail (#12) | no field: runs; with a field: CLI hint toast |
| 40 | Recipes: Remove / edit | UI | Remove pass (no confirm); Edit missing (#8) | |
| 41 | Recipes via CLI: set, run, field | CLI | pass/partial (#16) | substitution works; `--wait` doesn't wait; unknown name launches a task |
| 42 | later.plan / cancelPlan | API | pass | runs on time; cancelled one never ran (empty id says ok: #19) |
| 43 | Worktrees: keep, done.list, done.apply | API | pass | remove refused without force; force removes, branch stays |
| 44 | Worktrees: Bring in | UI | **fail** (#6) | every import fails, silently |
| 45 | Ship: commit & open PR | UI, API | pass | commit with `Wisp-Task` trailer, push, fake PR #3/#4; feedback 95 ms |
| 46 | Ship: Merge PR | UI, API | pass/**fail** (#3) | squash-merges, fast-forwards local main, removes the worktree **with uncommitted work in it** |
| 47 | Fix conflicts | UI, API | pass/smell (#21) | starts a fix-up turn with the PR's conflict brief |
| 48 | Check logs | API | not testable | fake gh has no job logs (empty string) |
| 49 | Review by another harness | API | **fail** (#20) | stuck "running", codex never called |
| 50 | Mark done (Done) | UI | pass/fail (#18) | finished task: done, and fires task.done triggers; running task: navigates away, then errors |
| 51 | Immediate feedback | UI | pass | in-page click→change: queue send 38–104 ms (Chrome), 53–68 ms (WebKit); Send→task page 101–183 ms incl. polling; job post 40–146 ms; answer 267 ms polled (coarse) |
| 52 | Phone spot check (390) | UI | pass/smell (#23) | queue, crew page, Home render without overflow |

Counts: **52 actions, 59 rows with sub-rows**. Pass: 34. Fail: 12. Partial (passes with a finding): 10. Not testable or not reproduced: 3.

## Findings

### 1. [major] Files attached in the browser never reach the agent
- Engine: Chrome and WebKit 1280; codex and claude.
- Steps:
  1. New task, then + › Attach files › attach.txt (6 B). The chip "attach.txt" shows.
  2. Launch.
- Expected: the prompt gets the `<attachments>` section with the file's path, as the protocol promises ("Attached files … are listed in an <attachments> section").
- Actual:
  - The turn records `attachments:[{id,name:"attach.txt",mime,size}]` with **no path**.
  - The agent's stdin is just the notes header plus the prompt, with no attachment section and no file.
  - The file sits in home/stuff/files/<sha>/attach.txt.
- Cause, from reading the code:
  - `upload()` in packages/shell/src/frame/attach.ts keeps only `{id,name,mime,size}` from the upload response.
  - Both launchers then drop path-less attachments: plugins/codex/host/src/launch.rs (`filter(|a| a.path.is_some())`) and plugins/claude/host/src/launch.rs (same filter).
- Shots: uiA6-chromium-ready.png (the chip), uiA6-chromium-sent.png.

### 2. [major] The team (Delegate, presets, crew org links) never reaches the lead
- Engine: UI (Chrome and WebKit), API and MCP.
- Steps:
  1. New task, then + › Delegate… › Presets › "Test team" (tester ×2, claude, "run the tests"), or add a delegate by hand. Launch.
  2. List the run's MCP tools with its own session token, and read `delegate`.
- Expected: the lead knows its team (`tester`) and can hand work to it.
- Actual:
  - The task stores `spec.team=[{agent:"claude",count:2,name:"tester",use:"run the tests"}]`.
  - The run's prompt and system prompt don't mention it, and the `delegate` tool description ends "**No team yet.**".
  - Calling `delegate` as the lead naming the team member fails. Its schema has no field for an existing team member; only `agent`+`name` to make a new one.
- Same for crew: after linking "Make Bob Ada's delegate" in the Org chart (Ada.team=[bob]), Ada's job has `spec.team: null` and `delegate` says "No team yet".
- Shots: uiA6-chromium-preset-loaded.png, uiA26-org-linked.png.

### 3. [major] Merge PR deletes the worktree with uncommitted work in it, without a word
- Engine: Chrome 1280, fake gh.
- Steps:
  1. Task in a worktree, then Ship › Commit & open PR (PR #4).
  2. Then `echo "precious work" > <worktree>/precious.txt` (git status: `?? precious.txt`).
  3. Ship › Merge PR #4… › Merge anyway.
- Expected: Settings says "Remove still asks before one with uncommitted work goes". The merge should keep the folder or ask.
- Actual:
  - The dialog says only "Squash-merge PR #4? Then wisp fast-forwards your local base branch and removes the worktree", with no mention of uncommitted work.
  - After the merge, the worktree folder and its branch are gone. `find /tmp/qaA -name precious.txt` finds nothing, and there's no stash.
  - The same happened with PR #3, whose Fix conflicts turn had just edited files.
- Shot: uiA40-merge2.png.

### 4. [major] Fork runs in the parent's worktree, not "on its own branch"
- Engine: Chrome 1280 (UI dialog) and the API.
- Steps: Task ⋯ › Fork… › "try it with a loop instead" › Fork.
- Expected: the dialog says "A new task with the whole conversation and a copy of its files, on its own branch". The fork should get its own worktree and branch.
- Actual:
  - The fork's cwd is the parent's worktree (`proj-01a1f05e8664…`).
  - It has no `wisp.worktrees` entry of its own, and `git worktree list` shows no new branch.
  - Both tasks now edit the same folder. "Restore to before latest turn" on the parent then wiped the fork's edit too.
  - Navigation was fast (141 ms).
- Shot: uiA13-chromium-Fork_.png.

### 5. [major] `/side` (Side discussion…) is sent as an ordinary follow-up
- Engine: Chrome 1280.
- Steps, three ways:
  - (a) Task ⋯ › Task options › Side discussion…, which pre-fills `/side `, then type and Send.
  - (b) Type `/si`, pick "/side" from the slash popup, type the text, Enter.
  - (c) Type `/side …`, Enter.
- Expected: a side discussion of its own beside the task (the popup text: "Talk it over on a page of its own"; `runtime.create` with `parentId`).
- Actual: each time the task itself got a new turn whose prompt is literally `/side what does math.js export?` (`--resume`, the task's cwd). No side task was made. `/btw` from the same box splits off correctly.
- Shots: uiA13-chromium-Side_discussion_.png, uiA16-chromium-side.png.

### 6. [major] Bring in (Settings › Worktrees) fails for every worktree, silently, and offers wisp's own worktrees
- Engine: Chrome 1280.
- Steps:
  1. Make a stray worktree with `git worktree add /tmp/qaA/stray -b stray-branch`.
  2. Open Settings › Worktrees, where the panel reads "Bring in · 12 worktrees from git · In proj · 6 with uncommitted changes".
  3. Click Bring in.
- Expected:
  - Only the one worktree another tool left is offered (11 of the 12 are wisp's own task worktrees).
  - It becomes a task on its own branch.
- Actual:
  - Nothing happens on screen; the card stays the same.
  - serve.log has 12 × `WARN worktrees: bringing in a worktree error=a shared task to import has its turns (a full share) path=…`, including /tmp/qaA/stray.
  - No task was created.
- Shots: uiA37-worktrees.png, uiA38-bringin.png.

### 7. [major] Compare from the CLI/API runs both agents in the same folder
- Steps: `wisp "compare run" --vs codex` (and `runtime.create` with a twin).
- Expected: each twin in its own worktree, as the UI's Compare does.
- Actual:
  - Both claude and codex ran in /private/tmp/qaA/proj (`proj` in `wisp ls`) and edited the same files.
  - The CLI printed only one task id and showed only codex's answer.

### 8. [minor] Settings › Automations: recipes and triggers can't be edited, and triggers can't be paused or deleted
- Engine: Chrome 1280.
- Recipes:
  - The ⋯ menu offers "Run now" and "Remove" only, with no Edit. Clicking a row does nothing.
  - Remove deletes at once, with no confirm.
- Triggers:
  - The ⋯ menu offers "Test on the last 5 events" and "Run now" only.
  - The on/off is a read-only pill, so there's no pause or resume, and no edit or delete. A trigger made in the UI can't be removed in the UI.
- Each trigger's subtitle is the raw i18n key **`automations.trigger.line`**.
- The crew's internal trigger `crew.bob.1` is listed among the user's triggers.
- Shots: uiA31-minutely-row.png, uiA29-manage0.png.

### 9. [minor] New trigger: "schedule" has no time or project field, and Save silently does nothing
- Steps: Settings › Automations › Triggers › New trigger, name `uitrig`, When "schedule", a prompt, Save.
- Expected: a field for when it runs (and the project). On a bad form, an error.
- Actual:
  - There's no time or cron field and no project field.
  - Save leaves the form open, adds nothing, and shows no message.
  - With When = task.done, Save works, and the trigger fires ("UITRIG fired task.done: Ada · Job", in /private/tmp/qaA/proj).
  - The When list mixes labels ("CI failed", "Files changed") with raw event ids: `chat.message`, `crew.job_filed`, `crew.reported`, `goal.failed`, `later.fired`, `machines.online`, `trigger.fired`, `turn.ended`, `turn.held`…
- Shot: uiA32-newtrigger.png, uiA33-saved.png.

### 10. [minor] /btw answers show the agent's raw wisp JSON
- In the UI: the btw card reads "All done. / {"blocks":[{"status":"done",…}]}".
- In the CLI: `wisp btw <id> "…"` prints the ```` ```wisp ```` fence and JSON instead of the text.

### 11. [minor] A pending question isn't visible in `wisp show` / `wisp wait`
- Steps: a run calls ask_user. Then `wisp show <id>`, `wisp wait <id>`.
- Actual: the task is "waiting", but neither prints the question or its options, so a CLI user can't see what to answer. The UI shows it ("Which database, Postgres or SQLite?").

### 12. [minor] Settings › Recipes › Run now on a recipe with a field shows a CLI hint
- Steps: Settings › Automations › Recipes › fixmath (`fix the {thing} in math.js`) › ⋯ › Run now.
- Actual: the error toast reads "needs {thing}: wisp run <task> thing=…". The launcher's + › Recipes › fixmath asks for `thing` in a dialog and runs, so Settings should do the same.
- Shot: uiA30-fixmath-Runnow.png.

### 13. [minor] Start later: the scheduled task doesn't show anywhere, and loses "Worktree"
- Steps: New task "UI Start later …", + › Start later › In 10 min ("Starts at 22:09"), then click "Start at 22:09".
- Actual:
  - The box clears, but Home and the list show nothing for it; only `later.plans` has it.
  - The plan's body is `{agent, cwd:/private/tmp/qaA/proj, mode, prompt}` with no worktree flag, though the launcher said "This computer · Worktree".
  - It fired on time (22:09:57), **in place in /private/tmp/qaA/proj**, not in a worktree (`wisp ls`: `proj`). It edits the project folder next to whatever else runs there.
- Shot: uiA36-later-sent.png.

### 14. [minor] A trigger given a project name (not a full path) fails silently, and the dry-run says "add" for an invalid formula
- `wisp apply -f wisp.yml` with `project: proj` applies, but the trigger never runs a task; with `/tmp/qaA/proj` it fires each minute.
- The dry-run first prints "+ trigger/minutely (new), 1 to add" and only then lists the formula as invalid (missing `on`).

### 15. [minor] `trigger test` prints "(none)", and `apply --prune` doesn't remove a dropped trigger
- `wisp automations trigger test minutely` → "(none)".
- After emptying wisp.yml, `apply --yes --prune` → "0 to remove… nothing to change". `minutely` is still listed (off).

### 16. [minor] Recipes in the CLI: an unknown name starts a task, and `--wait` doesn't wait
- `wisp run nosuchrecipe` launched an agent task instead of "no recipe named …".
- `wisp run fixmath thing=sign --wait` returned at once.
- A recipe's own `cwd` was ignored: it ran where the CLI was.

### 17. [minor] Restore to before latest turn: no confirm, a success toast, and a stale Changes count
- Steps: Task ⋯ › Task options › Restore to before latest turn.
- Actual:
  - It runs at once (no confirm for a destructive step) and toasts "Worktree restored to before this turn".
  - The worktree is clean afterwards, but the header still reads "Changes +2 −2", even after a reload.
- Shot: uiA13-chromium-Restore_to_before_latest_turn.png.

### 18. [minor] Done on a running task navigates away, then errors about it on another task
- Steps: open a running task (gated rerun) and click Done.
- Actual: the view moves to the next task ("Ada · Job") and then shows the error toast "it is still working: stop it first", which reads as if it were about Ada · Job. Done should be disabled, or stay put and say why.
- Shot: uiA34-done.png.

### 19. [smell] API edge cases
- `later.cancelPlan` with an empty id answers `ok: true`.
- `wisp accounts watch ask --on true` is refused ("doesn't take true here"), and `--on` alone "needs a value": no CLI spelling sets it, while the API takes `{"on":true}`.
- `wisp help runtime followUp` and `wisp help runtime queue.sendNow` (the catalogue's own camelCase ids) launch agent tasks titled "help runtime …".
- `wisp later ls` launches a task "later ls" (one stray word after a command group should be a slip).
- `ship` PR body is a single space (" ") when no description is written.
- `ship.merge` from the API didn't fast-forward local main; the UI's Merge did ("Local main is up to date with origin/main").

### 20. [minor] Review by another harness never runs
- Setup: `review.harness` = codex, on a task with open PR #2.
- Actual: the review state said "running" for more than a minute, but codex was never started (rec/codex.jsonl unchanged, no process).

### 21. [smell] Fix conflicts spends a turn when git sees no conflict
- With GitHub (fake) saying CONFLICTING, but `git merge-tree origin/main HEAD` clean, Fix conflicts still started a fix-up turn.
- `conflicts_with_base` in plugins/ship/host/src/ops.rs is meant to stop that ("Asked before spending an agent run on GitHub's word"). The button may force it on purpose.

### 22. [smell] Crew details
- Crew members carry `agent: "auto"`, shown as "Auto · Full auto" on the crew page and "Every 30 min · auto" in the import review. Auto is gone from every picker.
- The CI watcher template doesn't preselect "CI fails".
- Track record: "Check-ins · 67% · 0 a day" next to "45 tokens · 2 a day".

### 23. [smell] Phone 390: a long queue hides the composer's controls; queue actions are hover-only
- With 9 queued follow-ups, the queue fills the screen. The composer shows "Follow up with Claude Code…" at the bottom edge, with its mode, harness, Queue and Send row off screen.
- Rows show only the grip and ✕. Send now, Combine and Edit appear only on hover, so a phone can't reach them.
- Shot: uiA43-webkit-390-queue.png.

### 24. [smell] Other
- **Success toasts** for actions the user just took: "Opened PR #3", "Worktree restored…".
- **Rerun dialog:** it has only ✕, no Cancel, unlike Fork.
- **Home "Needs you":** it counts finished tasks (51 of 53), and a goal-stalled task shows "Asking".

## Not testable here
- **Other harnesses:** opencode, cursor, copilot, grok and agy weren't detected as installed under the fake PATH, so they weren't driven.
- **Issue:** the project's remote isn't on GitHub.
- **Check logs:** the fake gh has no job logs.
- **Budget hold:** no budget was set up.
- **Held turn's "Start now" in the UI:** not reproduced. Its API path passed.

## Count
24 findings: **7 major** (#1–#7), **12 minor** (#8–#18, #20), **5 smell** (#19, #21–#24). No blocker.

## Housekeeping
- Only the scratch server :4796 and the fake harnesses were used. Nothing touched ~/.wisp, ~/.wisp-dev, :4477, :4791, :4800 or :5180.
- Server :4796 is stopped and no fake processes remain. /tmp/qaA and /tmp/wisp-target-qaA (7.2 GB) are deleted.
- The server log and the fakes' call logs are kept as shots-A/qaA-serve.log, qaA-claude-calls.jsonl and qaA-codex-calls.jsonl (the Bring in warnings and the attachment-less stdin are in them).
