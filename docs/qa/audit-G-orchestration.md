# Audit G: orchestration, every action end to end (worktree HEAD a738c770), 2026-11-20

This round re-runs audit F's table (docs/qa/audit-F-orchestration.md) and re-checks F1–F7, plus E5, E6, E11, E14 and D6,
at HEAD a738c770. It then covers what landed since F, end to end on the CLI, the API, MCP and the UI:
- named goals (ADR-0110);
- formulas everywhere (ADR-0111);
- agent limits (ADR-0111, ADR-0112);
- setup cards (ADR-0113);
- software factories (ADR-0114);
- terminal context (ADR-0023).

Rows 50–76 are new.

## Setup

- **Server.** A scratch `wisp serve` on **:4972**, built at HEAD into /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins`).
  - `wisp`, `wisp-fake-agent` and `wisp-fake-gh` were copied into /tmp/qaG1/opt and /tmp/qaG1/bin and run from there.
  - The worktree's only uncommitted edits are design/v1-reference images, so this build is HEAD's code.
  - Environment: WISP_HOME=/tmp/qaG1/home, HOME=/tmp/qaG1/userhome, WISP_WEB_DIR=/tmp/qaG1/web (a fresh `vite build` of apps/web at HEAD).
- **Server warnings.** WISP_LOG defaults to `warn`, so every WARN went to serve.log (RUST_LOG=warn is not read; see audit E).
- **Fake harnesses.** Audit F's setup, moved to /tmp/qaG1:
  - codex, opencode, cursor-agent, copilot, grok and agy are the wisp-testkit fake, set through WISP_*_BIN.
  - claude goes through the QA wrapper (/tmp/qaG1/bin/claude). F's cues (`PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`/`#delegate`, `#limit`, `#hang`) are kept, and this round adds three:
    - `#call <tool> <json>` calls a tool on the run's own MCP and logs the answer.
    - A factory's spec station says `SPEC: …`.
    - A reviewer says `CHANGES: …` once when the task has `#changes`, then `APPROVE`.
  - The wrapper saves each run's MCP address and token (state/mcp-<pid>.json), so `rmcp.mjs` can call as that run.
  - `wisp-fake-gh` is WISP_GH_BIN, with its state in /tmp/qaG1/state/gh.json (F's two issues; no PRs at the start). A failing `test` check was added for row 44d, then removed.
  - `FAKE_GATE` (/tmp/qaG1/gate) holds a run whose prompt starts `gate <name>` (or has a `\n\ngate <name>` paragraph) until that file exists.
- **Projects.** All are fresh git repos made at the start, from F's first commits.
  - **proj**: math.js and its test, with a bare origin (/tmp/qaG1/remote.git). Not added to wisp until after row 1.
  - **proj2, proj5–proj7**: each commits `.claude/settings.json` (a PreToolUse hook) and `.mcp.json` (server `local`), for project trust.
  - **proj3**: `.wisp/worktree.json` `{"setup":"touch setup-ran; exit 3"}`.
  - **proj4**: origin `https://github.com/acme/repo.git`, for issues.
  - **proj8**: a second copy of proj, with its own bare origin (remote8.git), for the factory runs.
  - **Formulas and task files** are in /tmp/qaG1/f, with a `.wisp/` module `team` for `formula: module.team`:
    - `goals.yml`: 2 goals, `goals.default`, `tasks.default` and 2 tasks.
    - `many.yml`: 30 more goals, 32 declared in all.
    - `team.yml`: a variable, an agent `critic` (codex), a goal and a task.
    - `fac.yml` and `fac2.yml`: the factories `strict`, `tight`, `slowcheck` and `gated`.
    - `trig.yml`: a schedule trigger.
- **No outbound traffic.** DO_NOT_TRACK=1, WISP_TELEMETRY=0 and WISP_UPDATE_CHECK=0. The PostHog, grid, Linear, Slack, Discord and Telegram URLs pointed at 127.0.0.1:9.
- **No real harness.** The server's only children were the fakes.
- **UI.** Playwright (/tmp/pw/node_modules/playwright-core) on system Chrome (`channel:'chrome'`) and on WebKit, at 1280×800.
  - Phone checks at 390×844 (touch, light and dark) on both engines.
  - Feedback is timed in the page: from pointerdown or keydown to the first DOM mutation.
  - F's scripts were reused with paths and port changed (`fx-*.mjs`), and new ones written (`g-*.mjs`). Every script closes its browser in `finally`.
- **Terminal context.** A tmux server on its own socket (`tmux -L qaG1`), running zsh with `eval "$(wisp shell-init zsh)"` against the scratch wisp.
- **Interfaces used.** The CLI, `/api/cmd`, `/api/mcp` as the user and as a run (its `x-wisp-session`/`x-wisp-token`), the stdio `wisp mcp`, and the UI.
- **Artifacts.** In /tmp/qa-keep/shots-qaG1/: screenshots, qaG1-serve.log and qaG1-calls.jsonl.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | From /tmp/qaG1/proj, not yet added to wisp: each fake ran in its own `proj-<id>` worktree; the checkout stayed clean |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit MultiEdit Write NotebookEdit`, in place; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox`. UI Plan + Codex + In place sent in 3 ms |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`; the UI slider says "High thinking" |
| 4 | Worktree vs in place | CLI, UI (Where menu) | pass | Where › In place stays sticky for later launches, crew jobs included (ADR-0108, deliberate) |
| 5 | Compare | CLI, UI (Chrome) | pass | CLI `--compare claude,codex:gpt-5`: two tasks. UI: Compare adds Codex as "Claude Code Opus vs Codex Default"; send 3 ms; toast "G compare ui is ready (2 runs)" |
| 6 | Try (subagents=haiku) | CLI, UI (Chrome) | pass | CLI: runs A and B, "differs: subagentModel default → haiku" (F6 fixed); `--try` without `--agent` is refused with its reason. UI: pick 2 ms, send 3 ms, `runtime.trial` |
| 7 | Attach files | UI (Chrome) | pass | stdin has `<attachments>`; argv has `--add-dir …/stuff/files/<hash>` |
| 8 | Goal and judges | CLI (`wisp goal set --kick`), UI | pass | PASSAFTER2 → "◎ Goal: PASSAFTER2 sums work · met after 2 rounds". UI goal launch 2 ms |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass | "1 delegate" chip; send 3 ms |
| 9b | Delegate a job to the team | per-run MCP | pass | The lead's `delegate` call ran: toast "B delegated: run the tests is ready" |
| 10 | Follow-up queue: queue, edit, drag reorder, cancel (with confirm) | UI (Chrome) | pass | Queue 1 ms; drag → `runtime.queue.order`; cancel 3 ms, confirm "Cancel “G stop qb”? · Keep it · Cancel it" 1 ms |
| 11 | Stop keeps the queue | UI | pass | "Stop Claude Code? Your queued follow-up stays, held until you send one." (1 ms); `held=true` after |
| 12 | Send now | UI | pass | 6 ms; the held item ran |
| 13 | Start later | UI (Chrome) | pass | Send 8 ms; toast "Starts 09:55". "Scheduled 1 · G13 later ui · Scheduled task · proj · 09:55" shows once Needs you (65 rows) is collapsed |
| 14 | Ask (ask_user) and answer | per-run MCP, CLI, UI | pass | `wisp show` lists the question with numbered options; `wisp reply … SQLite` → "The user answered: SQLite". Typed in the reply box (9 ms), it is the answer too |
| 15 | Usage limit | CLI | pass | "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent." |
| 16 | Fork… | UI (Chrome), HTTP MCP | pass | UI: 8 ms, Fork 15 ms, its own task. MCP `runtime_fork` on a merged task now makes a fork in a fresh worktree on `wisp/…-fork-…`, and its follow-up runs (F2 fixed) |
| 17 | Rerun… | UI (Chrome) | pass | "Starts a new task in a new worktree of proj on this computer."; 8 ms, Rerun 17 ms |
| 17b | `wisp rerun <id> -d` | CLI | pass | "<id> → <new id> (as it ran)"; no `rerun-*` folder left |
| 18 | Side discussion… (`/side`) | UI (Chrome) | pass | 16 ms; `wisp ls` and the page give its project as "proj" (F5 fixed) |
| 19 | /btw | UI, HTTP MCP | pass | One Enter, 3 ms; the answer card shows. MCP `runtime_btw` on a merged task: "This task's branch … is gone: it was merged (or deleted), so its work is on main now…" (F2 fixed) |
| 20 | Review by another harness | Ship, UI, API | **partial** (E5) | Unchanged: "Not reviewed", and a title that says "Review again to ask once more. Click to try again"; clicking it shows no "Reviewing" |
| 21 | Ship: commit & open PR | UI, CLI | pass | Ship 7 ms; Commit & open PR 9 ms, toast "Opened PR #4"; CLI "✓ shipped: …/pull/1" |
| 22 | Merge PR (worktree with uncommitted work) | UI | pass | Confirm "Squash-merge PR #4? …"; go 3 ms. Toast: "Merged PR #4. Local main was not updated: your uncommitted changes to math.js are in the way… The worktree … was kept: it has 1 uncommitted change." |
| 22b | `wisp ship` + `wisp merge` | CLI | pass | "✓ merged PR #1 · … · The worktree and its branch were removed, here and on the remote." Both are gone (F1 fixed) |
| 23 | Restore to before latest turn | UI | pass | Confirm, then the toast "The folder is back to before this turn. Undo"; 2–4 ms |
| 24 | Bring in | UI | **fail** (E6) | With 54 git worktrees, the stray didn't show in 75 s on two opens. `worktrees.view` said `looking: true, last: null` for 45 minutes |
| 25 | Worktree removal | UI | not re-run | |
| 26 | Crew: add | CLI | pass, with finding E11 | A wrong flag's hint now keeps every flag (F6 fixed). `crew create --help` still has the developer notes (E11) |
| 27 | Crew: give a job | UI | pass | 3 ms |
| 28 | Crew: check-in (cron and "Check in now") | CLI, UI | pass | Check in now 18 ms (0 → 1); the cron check-ins came every minute |
| 29 | Crew: pause / resume | UI, CLI | pass, with finding G12 | UI Resume 2 ms. The CLI can't resume a member (G12) |
| 30 | Crew: tint | UI | not re-run | |
| 31 | Crew: wakes on "I mark a task done" | API | pass | "Bob · Job started by trigger:crew.bob.1", codex |
| 32 | Crew: Org link → the job gets its delegate | CLI | not re-run | |
| 33 | `wisp ls --crew ada` | CLI | pass | Lists Ada's check-ins |
| 34 | Recipes: edit, run, delete | UI | pass | Delete asks "Delete the recipe “fix-build”? It goes for good…" (1 ms) |
| 35 | Triggers: new, fires, on/off, delete | formula, UI, API | **fail** (G3, G11) | Fires every minute with `cron: "* * * * *"`. Switch off: 4 ms. **Switch back on: flips back, stays off** (G3). Delete asks (1 ms). `cron: "every minute"` never fired (G11) |
| 36 | Notes, Fresh thread | CLI | pass | `wisp notes <id> --set goal=…`; `wisp fresh` runs without `--resume`, with the Notes in its brief |
| 36b | The agent keeps its Notes (`wisp_notes`) | per-run MCP | pass | "Updated decisions (rev 1)" |
| 37 | User MCP (`wisp mcp`, stdio): launch | MCP | pass | 251 tools; `wisp_launch {cwd}` runs in a worktree |
| 38 | HTTP MCP: fork, btw | MCP | pass, with finding G6 | Fork and btw as row 16 and row 19. The HTTP user's `wisp_launch` is another tool (G6) |
| 38b | HTTP MCP: every listed tool is callable | MCP | **partial** (D6) | 57 of 251 tools still name no fields. Unknown fields are still dropped: `wisp_notes {"section":…,"text":…}` reads the Notes |
| 39 | A task launched by an agent | per-run MCP | pass | `wisp_launch {worktree:true}` runs in a worktree |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 3 ms |
| 41 | Phone 390 (Chrome, WebKit; light, dark) | UI | pass | No horizontal overflow on Home, a task (a factory run) or the crew page |
| 42 | Immediate feedback (click → first DOM change) | UI | pass | Every measured action took 0–28 ms (see below) |
| 43 | `wisp help runtime answer` | CLI | pass | Prints the verb's help (see G17 for its words) |
| 44a | Try from the UI | UI (Chrome) | pass | Row 6 |
| 44b | A held turn's "Trust and start" | UI, CLI | pass | Card 548×208 (1280) and 358×263 (390, both engines). Open file 10 ms; Trust and start 8 ms → the turn ran |
| 44c | Start from an issue | UI (Chrome) | pass | Picker 6 ms, pick 5 ms; "Fix #7: Sum is wrong for negatives" in proj4 |
| 44d | Ask to fix (a failing check) | UI | pass | "Open · 1 check failing"; Ask to fix 1 ms. The reply box wraps to 77 px with the whole sentence |
| 44e | A held turn's "Skip" | CLI | pass | "● Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." |
| 44f | A held turn's "Run without them" | CLI | pass | claude got `--setting-sources user --strict-mcp-config` |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI | pass | All 26 turns ran, the queue empty. I sent them at once, so their order is arrival order |
| 46 | Crew check-ins and schedule triggers over time | CLI | **partial** (G11) | About 15 minutes: Ada every minute; the trigger every minute once its cron was `* * * * *`. The words form never fired |
| 47 | A project's worktree setup | CLI | pass | `setup-ran` in the worktree; "◷ Held before it started: setup `touch setup-ran; exit 3` failed: exit 3" |
| 48 | Server warnings (serve.log) | — | pass | 4 WARNs in ~130 tasks: 3 from my own unknown commands and 1 from the failing setup (row 47) |
| 49 | A long follow-up in the reply box | UI (Chrome) | pass | `pre-wrap`; it grows from 34 to 142 px; the empty box is one line again (F3 fixed) |
| 50 | Named goals in a formula (new) | CLI | pass, with finding G16 | validate: "tasks.broken: no goal named green-bild: goals has green-build, never". `apply --yes`, then `diff` "6 unchanged". `wisp goal declared` lists them |
| 51 | `wisp run <task>` with `tasks.default` and a goal by name (new) | CLI | pass | fix-build: claude, auto, worktree, "◎ Goal: PASSAFTER2 the build is green · met after 2 rounds". `wisp run default` → "tasks.default gives every task its defaults: it isn't a task to run" |
| 52 | `--goal NAME`, `--goal-text` (new) | CLI | pass | Unknown: "✗ no goal named green-bild: goals has green-build, never". Words: met after 1 round |
| 53 | `wisp goal set <task> <name>` (new) | CLI | pass, with finding G16 | Sets the declared goal with its round limit |
| 54 | A goal's round limit (new) | CLI | pass, with finding G16 | `{ name: never, maxRounds: 1 }` stopped after 1 round and asked "Keep going?" |
| 55 | `wisp_launch`'s `goal` (new) | stdio MCP, run MCP, HTTP MCP | **partial** (G6, G7) | stdio and the run's: a name is the declared goal, words are words. HTTP as the user: no goal, in place (G6). No goal tools at all (G7) |
| 56 | Goal… with 32 goals (new) | UI (New task, reply box; 1280, 390) | pass, with finding G22 | 7 goals, then "All goals (32)…" and "New goal…". Pick 1–2 ms (Chrome), 8 ms (WebKit), sent as `{"name":"goal-03"}`. All goals 2 ms; typing `goal-2` filters. Phone: a sheet, rows ≥ 44 px, no overflow |
| 57 | `/goal` in the reply box (new) | UI | pass, with finding G14 | `/goal PASSAFTER1 tests pass` → `goal.set`. `/goal green-build` → `goal.set {text:"green-build"}` on the second Enter |
| 58 | `wisp "…" --formula f.yml` (new) | CLI | **partial** (G2, G8) | `--agent critic` from the formula: codex, `-m gpt-5`, effort high, the variable in its instructions, in a worktree. An invalid formula launched anyway (G8). The formula's agent can't judge its goal (G2) |
| 59 | `wisp run <task> --formula f.yml` (new) | CLI | **partial** (G2, G9) | Ran the formula's `release` with its goal inlined, but in place (G9), and its goal looped (G2) |
| 60 | Task files: `formula:` a file, inline, a list, `module.x` (new) | CLI | pass | All four ran with their agents. Refused with their reasons: "d.task.yml: no agent named critc: agents has critic (or claude, …)", "… team.yml is named twice", "… is this task file itself", "no module named nope in …/.wisp: it has team" |
| 61 | `wisp_launch`'s `formula` (new) | stdio MCP, run MCP | **partial** (G1) | stdio: runs. As a run in place: runs. Refused with clear reasons: tasks, connectors, `${secret:…}`, absolute paths. **As a run in a worktree, every file is refused as "a secrets file"** (G1) |
| 62 | New task › Formula or task file… (new) | UI (Chrome) | **partial** (G10) | Menu 2 ms; the file chooser opens; "Formula: b.task.yml" chip; send 3 ms with the overlay's agent. A file naming `./team.yml` beside it fails (G10) |
| 63 | validate names unknown references (new) | CLI | **partial** (G11) | Named: goals, agents ("factories.broken: no agent named plannr: no agents are declared (or claude, …)"), modules, a task's agent. A trigger's `on` and its `cron` words aren't checked (G11) |
| 64 | A child never exceeds its parent; agents never raise a mode (new) | run MCP | pass | Plan parent: `wisp_launch {mode:auto}` and raw `runtime_create {mode:auto}` → "Refused: Full auto is above this task's Plan…". With no mode: plan. `runtime_setMode` up → "Refused: an agent can't raise a task's mode (Plan to Full auto)…"; to plan: ok |
| 65 | Agents' writes (new) | run MCP | pass, with finding G18 | `connectors_add`, `crew_create`, `runtime_profile_set` and `resources_apply` aren't offered; the `wisp_crew_*`, `wisp_connector_add` and `runtime_projectAdd` card tools are |
| 66 | Setup cards: ask, Allow, Deny (new) | run MCP, API, CLI, UI | **partial** (G4, G21) | Crew add, crew remove, a connector (`sh -c id`, then `npx … --stdio` + a key name) and a project each made a card. Allow (5 ms) wrote exactly what was shown, and the agent read "The user allowed it. Added lint…". Deny (5 ms): "The user said no…". Words via the API: "They said: “no, not sh”". A stale `askId` → "That question was already answered…"; no `askId` → "A change to the user's setup is answered from its own card…". The agent answering its own card: "an agent's token speaks for its own task only". `wisp reply` and typed words don't answer the card (G4) |
| 67 | Setup cards on the phone and channels (new) | UI (390 dark), API (`channels.act`) | **partial** (G4) | Phone card fits (358 wide), Show all, Allow, Deny. A channel tap without `person`: "That's a change to your setup: answer it in wisp… Nothing changed."; with `person: true`: "Answered." and Zed was added. Typed "Allow" in a chat → "Sent." and nothing was answered (G4) |
| 68 | Factory: the built-in default (new) | CLI | pass, with finding G5 | `--factory` alone: "▦ default: ? spec · ○ build · …". Approve from the CLI → build, checks ("nothing to run"), review, ship. It failed "Nothing to ship: this branch has no changes." (origin already had the fix: right). Words instead of Approve revised the spec ("Change the spec: Also handle floats…") |
| 69 | Factory: a declared one, looping (new) | CLI, fake gh | pass, with findings G15, G20 | `strict` in proj8: the check failed once → round 2; the review said CHANGES → round 3; the limit asked "One more round / Stop". PR #2 opened ("Made by the wisp factory `strict` in 5 round(s).", G15). Merge → MERGED; worktree and branch removed |
| 70 | Factory names (new) | CLI | pass | "no factory named nosuch: factories has default, strict, tight"; `wisp export factories` |
| 71 | Factory: Stop (new) | CLI, UI | pass, with findings G13, G15, G17 | `wisp factory stop --id <run>` killed the running `sleep 120` check: "✗ checks · you stopped it". UI ⋯ › Stop the run, 2 ms, no confirm (G13) |
| 72 | Factory: delete a run (new) | CLI, UI | pass, with finding G13 | `wisp runtime delete <run>`: the run and its stations go. A clean station worktree is removed; one with an untracked file stays |
| 73 | Factory: restart mid-run (new) | CLI | pass | Killed the server while the build ran. On start, the build's turn was "stopped", a new build turn ran, then checks and ship ("✓ build · ✓ checks · – ship") |
| 74 | Factory: started by an agent (new) | run MCP | pass | A Plan parent's `wisp_launch {factory:"gated"}`: the run and its build station are `plan`; `mode:auto` is refused. Checks asked "Run `true` on the built code? Run / Skip / Stop"; the agent answering it: "an agent's token speaks for its own task only" |
| 75 | Factory in the app (new) | UI (Chrome, WebKit; 1280, 390) | pass, with findings G5, G15 | The harness picker's Factory row (4 ms Chrome, 6 ms WebKit) lists the 5 factories with their stations; the pill says "Factory". Send 3 ms (Chrome), 28 ms (WebKit), `"factory":"strict"`. The band, its ⋯ (Open Spec, Stop the run), the question card; Approve 5 ms. The reply box says "its stations do the work". No overflow at 390 |
| 76 | Terminal context: `wisp "check the error above"` in tmux (new) | CLI | pass, with finding G19 | "◌ with terminal context: 5 commands, 27 lines from tmux, git status, 4 secrets masked"; `--show-context` prints it, the key as `<secret>`. "why did that fail" attaches it; "add a readme" doesn't. The fake got the context and the error, never the key |

**Counts:** 86 rows (F's 59 plus 50–76).
- **Pass:** 70, of which 15 carry a lesser finding.
- **Partial:** 11 (rows 20, 38b, 46, 55, 58, 59, 61, 62, 63, 66 and 67).
- **Fail:** 2 (rows 24 and 35).
- **Not re-run:** 3 (rows 25, 30 and 32: worktree removal, tint and the Org link, all unchanged since F).

**Feedback (ms, pointerdown or keydown → first DOM change).**
- Launch: 2–3; goal pick 1–2 (Chrome), 8 (WebKit); goal send 2 / 6; All goals 2.
- Factory: Factory row 4 / 6; factory send 3 / 28 (WebKit); Approve 5; Stop the run 2; Delete 7.
- Launcher extras: compare 3; try 2 and 3; team 3; Formula or task file… 2, its send 3; later 8; issue picker 6, pick 5.
- Reply box: goal pick 1; typed answers 8–10; btw 3; side 16.
- Queue 1; cancel 3 (confirm 1); stop 1; send now 6.
- Task menu: fork 8 / 15; rerun 8 / 17; restore 2 / 4.
- PR: Ship 7, Commit & open PR 9, Merge 3, go 3, review 6, Ask to fix 1.
- Setup card: Allow 5, Deny 5.
- Crew: job 3, Resume 2, check in 18.
- Held turn: Trust and start 8, Open file 10.
- Trigger switch: off 4; on 21–22, then flips back (G3).
- Done 3.

## Audit F's findings, re-checked

| # | Finding | Status at a738c770 | Evidence |
|---|---|---|---|
| F1 | `wisp merge` says the branch stays, but deletes it | **fixed** | "· The worktree and its branch were removed, here and on the remote." Both are gone |
| F2 | Fork, /btw and a follow-up on a merged task end in a folder that is gone | **fixed** | Fork: a fresh worktree on `wisp/…-fork-…`, and its follow-up runs. /btw and a follow-up: "This task's branch wisp/task-eed1db is gone: it was merged (or deleted), so its work is on main now. Start a new task (or fork this one)…" No WARN |
| F3 | The reply box never wraps or grows | **fixed** | `pre-wrap`, 34 → 142 px for a long follow-up; Ask to fix shows the whole sentence (77 px) |
| F4 | "2 file" | **fixed** | "2 files +2 −0" |
| F5 | A side discussion's project shows as its parent's worktree folder | **fixed** | `wisp ls` and the page say "proj" |
| F6 | CLI word slips (a dropped flag; two spaces) | **fixed** | The hint keeps every flag ("… --cron "every minute" --projekt proj"); "differs: subagentModel default → haiku" |
| F7 | An agent tool's refusal isn't marked as an error | **fixed** | `later_agent_schedule {}` → `isError: true`, "wisp: Say when: at (a time) or in (minutes)." |
| E5 | A failed review shows as "Not reviewed"; Review again shows no progress | **partly fixed** (unchanged since F) | Label "Not reviewed"; the title says "…Review again to ask once more. Click to try again"; a click shows no "Reviewing" |
| E6 | With many worktrees, Bring in shows a stray late | **not fixed (worse)** | 54 git worktrees: the stray wasn't shown in 75 s on two opens. Asked 5 times over 45 minutes, `worktrees.view` always said `looking: true, last: null` (F: shown at 774 ms when left alone) |
| E11 | Generated CLI help shows developer notes | **partly fixed** (unchanged) | `crew create --help` still has "… (never two for a frame)", "v1's trigger kinds", "(ADR-0043)". New ones in G17 |
| E14 | The side panel's Preview tab is icon-only at 1280 | **not fixed** | "Changes 1" 91, "PR #2" 62, "Shell" 56, then Preview and Thread as 28 px icons |
| D6 | MCP tools without an argument schema | **partly fixed** | 57 of 251 tools still have only `{args, peer}` (connectors_*, plugins_*, stuff_*, crew_list, …). Unknown fields are still dropped |

**F totals:** all 7 fixed (F1–F7). Of E5, E6, E11, E14 and D6: 3 partly fixed (E5, E11, D6) and 2 not fixed (E6, E14).

## New findings

### G1. [major] An agent's formula file in its own worktree is refused as "a secrets file"
- **Steps.** A task runs in a worktree (every default task in a project). Its agent writes `ok.yml` (one agent, no secret) in its folder, then calls `wisp_launch {"formula":"ok.yml","agent":"a"}` over its own MCP.
- **Actual:**
  - "wisp: formula: ok.yml: a secrets file isn't read for an agent's formula; the task: no agent named a…". The same happens for every file in the worktree.
  - Worktrees live under wisp's home (`~/.wisp/worktrees`, here /tmp/qaG1/home/worktrees). ADR-0111 never reads "wisp's home" for an agent, so every worktree file counts as wisp's own.
  - The same file works from a task running in place in the project.
- **Expected:** a task's own worktree is "its folder" (ADR-0111): read it, keep refusing the rest of wisp's home.

### G2. [major] A formula's agent can't judge its goal, and the goal loops to its limit
- **Steps.** team.yml declares the agent `critic` (codex) and the goal `tiny`, and its task `release` names both. Run `wisp run release --formula team.yml`, or a task file whose `formula: ./team.yml` names `critic` and `tiny`.
- **Actual:**
  - The judge is the task's agent, `critic`, and wisp can't find it: "critic: critic couldn't judge: no harness critic".
  - That counts as "not met". The builder got "The goal isn't met yet. What the judges said: critic: critic couldn't judge: no harness critic. Keep going until it is." three times.
  - Then "not met after 4 rounds" asked "Keep going?". That is 4 rounds spent on a judge that never ran (`goals.default.maxRounds: 4`).
- **Expected:**
  - The judge resolves the formula's agent, as the task did.
  - A judge that can't judge doesn't send the work back: say so once and stop.

### G3. [major] A trigger switched off in the app can't be switched back on
- **Steps.** Settings › Automations › Triggers. Turn a trigger's switch off, then on again.
- **Actual:**
  - Off works (4 ms, `enabled: false`).
  - On sends `resources.apply` with the spec without `enabled`, and the server keeps `false`. The switch flips back to off; there is no toast. It reproduced on three clicks.
- **Expected:** send `enabled: true` (or have apply drop the stored field); on refusal, roll back with a toast.

### G4. [major] A setup card can't be answered from the CLI or by typing, but each says it can
- **CLI.**
  - `wisp show` prints the card as "? Asks: … 1. Allow 2. Deny · Answer: wisp reply <id> "<your answer>"".
  - `wisp reply <id> Allow` prints "✓ queued": a follow-up waits behind the card, and the card stays.
  - After a later Allow in the app, that queued "Allow" ran as a turn: the agent got "Allow" as its prompt.
  - Only `wisp runtime answer <id> Allow --ask-id <id>` works, and `wisp show` never prints the ask id.
- **App.**
  - The card says "Nothing changes unless you allow it. Or type why not below."
  - Typing "keep Eve, she runs the nightly" and Enter sends `runtime.followUp` (8–10 ms). It queues; it is not a Deny with those words (ADR-0113 rule 4).
  - For an `ask_user` question, typed words are the answer.
- **Channels.** Typing "Allow" in a chat answers "Sent.": the same queued follow-up.
- **Expected:**
  - `wisp reply` on a setup card answers it (the user's own CLI is `user:*`), or `wisp show` names the command that does.
  - Typed words under a card are the Deny with those words, as the ADR and the card say.

### G5. [minor] "Approve the spec?" doesn't show the spec
- **Steps.** A factory run waits for spec approval.
- **Actual:**
  - The question's `context` is "". `wisp show`, the phone and channels ask "Approve the spec? Approve to build it, Stop, or say what to change." with no spec.
  - In the app, the band's Spec row says "Waiting on you: …", and the spec is only behind ⋯ › Open Spec.
  - Once approved, the band shows the spec as the station's output.
- **Expected:** the spec (or its start) in the question and on the band while it waits.

### G6. [minor] The HTTP user MCP's `wisp_launch` is v1's: no goal, formula, factory or worktree
- **Steps.** `/api/mcp` with no token (the user): `wisp_launch {"prompt":…,"cwd":"/tmp/qaG1/proj","agent":"claude","goal":"green-build"}`.
- **Actual:**
  - "Launched session `…` on claude in /tmp/qaG1/proj, mode auto." It ran in place in the user's checkout, with no goal (`wisp.goal` empty).
  - Its schema is `prompt, cwd, also, agent, mode, model, effort, subagents, preset, machine, wait, timeout_seconds`.
  - The stdio `wisp mcp` and a run's server take `goal`, `formula`, `factory` and `worktree`, and run in a worktree.
- **Expected:** one `wisp_launch` everywhere, or refuse the fields it drops.

### G7. [minor] No goal or factory tools in either MCP server
- **Actual.**
  - `goal_declared` and `goal_set` answer "Unknown tool" in `wisp mcp` and over HTTP. A run's tool list has no goal_* or factory_* tool either.
  - ADR-0110 says `goal.declared` reaches agents as a generated tool. ADR-0112 lists `goal.set`, `goal.check`, `goal.drop`, `goal.pause` and `goal.resume` among an agent's writes.
  - A harness can name a goal only inside `wisp_launch`. It can't list the declared goals, set one on a running task, or stop a factory run.
- **Cause (likely):** generated tools are only the commands wisp.mcp's own manifest grants (`extras_for`, plugins/mcp/host/src/lib.rs:65), and it grants no `goal:*` or `factory:*`.

### G8. [minor] An invalid formula launches anyway
- **Steps.** team.yml with `agents: { critic: { …, brief: "…" } }` (`brief` isn't a field). Run `wisp "…" --formula team.yml --agent critic`, or `wisp run release --formula team.yml`.
- **Actual:** both launch. The invalid agent is dropped, and the task shows "✗ Not started · no harness critic". `wisp validate -f` says "agents.critic.brief: unknown field (fields: account, …)".
- **Expected:** refuse before launching, with validate's words.

### G9. [minor] `wisp run <task> --formula` runs in place
- **Steps.** From /tmp/qaG1/proj (a wisp project): `wisp run release --formula /tmp/qaG1/f/team.yml`.
- **Actual:** the task's folder is /tmp/qaG1/proj, with no worktree. `wisp "…" --formula` from the same folder, and a plain `wisp run fix-build`, each ran in a worktree.

### G10. [minor] New task's task file reads its `formula: ./x.yml` from the project's root, not beside the file
- **Steps.** New task › + › Formula or task file…, then pick /tmp/qaG1/proj/.qa/a.task.yml, whose `formula: ./team.yml` sits beside it in .qa/.
- **Actual:**
  - The toast: "formula: /tmp/qaG1/proj/./team.yml: No such file or directory (os error 2) a.task.yml: no agent named critic … no goal named tiny: goals has goal-01, goal-02, … goal-30, green-build, never".
  - That is a raw OS error, then all 32 goals, in a toast 13 lines tall. The CLI reads the same file beside it.
- **Expected:**
  - Read it beside the picked file (or ask for its folder), or say plainly that only a file at the project's root can name a sibling.
  - Cut the list of names.

### G11. [minor] Triggers: a source that doesn't exist and a cron in words are accepted, then never fire
- **`on:` that names no source.**
  - `on: "every minute"` (not a source) passed `validate` and `apply`.
  - The trigger was stored "unavailable", with `why` empty, and never fired.
  - ADR-0111 checks the names a formula gives, but not a trigger's `on`.
- **A cron in words.**
  - `on: schedule, cron: "every minute"` (kinds.md: "a cron or words") showed "on" and fired 0 times in 3 minutes.
  - `cron: "* * * * *"` fired every minute.
  - Crew's `--cron "every minute"` does become `* * * * *`.
- **The round trip.** `wisp diff` right after `apply` says "1 to change" for a trigger with `project: proj`: it is stored as its path.

### G12. [minor] `wisp crew update` takes only the hash id, and can't resume a member
- `wisp crew update ada …` and `… Ada …` answer "✗ No such crew member." The id 42c27ec1c563ecea works. `wisp ls --crew ada` and the MCP crew tools take the name.
- `--paused` is a switch: `--paused false` and `--paused=false` answer "✗ No such crew member." ("false" read as the member?).
- There is no `crew resume`, so a paused member can't be resumed from the CLI.

### G13. [minor] A factory run's Stop and Delete
- **Stop.** ⋯ › Stop the run stops every station at once (2 ms), with no confirm and no Undo. The task's own Stop asks "Stop Claude Code? …".
- **Delete.** Delete task on a run asks "Delete this task from wisp? The agent’s own history is kept." It doesn't say that its stations' tasks, and their clean worktrees, go with it.

### G14. [smell] `/goal <declared name>` swallows the first Enter
- `/goal green-build`, then Enter: nothing changes. The second Enter sends `goal.set`.
- `/goal <words>`, `/btw …` and `/side …` go on the first Enter.

### G15. [smell] Factory words
- "round(s)": "Checks still fail after 1 round(s)", "The review still asks for changes after 3 round(s)", and the PR body "Made by the wisp factory `strict` in 5 round(s)."
- "`exit 1` exited 1:." ends on a dangling colon when the check printed nothing.
- A failed ship shows "ship failed: Nothing to ship: this branch has no changes." twice: on the Ship row, and again under the band.
- Ship's output is a raw URL ("merged: https://github.com/acme/repo/pull/2"), not "PR #2".
- `wisp stop <run>` on a run waiting on you ends it as "✗ checks · the question was taken back"; `factory.stop` says "you stopped it".
- A run that ended shows the "Report" pill beside the band's "Done".
- `wisp ls` gives a run's agent as "claude", where ADR-0114 rule 14 says "Factory · <name>, never a harness". The app's row is right.
- With 3 rounds of review, "The review still asks for changes" also covers a reviewer who gave no verdict ("claude gave no verdict (its last line wasn't APPROVE…)").

### G16. [smell] Goal words
- "The goal isn't met after 1 rounds."
- `wisp show` repeats it: "· not met after 1 round: The goal isn't met after 1 rounds."
- `wisp goal set <task> nevr` (a typo of the declared `never`) silently becomes the goal "nevr", while `--goal nevr` is refused. The ADR allows it (a string that isn't a name is words), but the CLI's positional reads as a name.

### G17. [smell] CLI and formula word slips
- "✗ …/goals.yml: not a wisp formula: it starts with `wisp: 1`" reads as if the file starts with it: say "a formula starts with `wisp: 1`".
- "✓ 1 resources in …/trig.yml".
- `wisp fresh <id> -d` prints its usage line and "a task that starts with fresh: wisp -- fresh …", without naming `-d`.
- `wisp runtime answer --help`: "--id TEXT required." with no words, and "(ADR-0113)" (E11's kind).
- `wisp crew update --help` opens with "`queue`: the waiting job ids to keep…".
- `wisp factory stop <id>` refuses the id ("doesn't take … here"). Its help lists no fields (only `--args`, `--<field>`), so `--id` must be guessed.

### G18. [smell] An agent's MCP: the wrong words for a tool it isn't given or a wrong field
- Calling a tool the run isn't given (`connectors_add`, `crew_create`, `nosuch_tool`) answers JSON-RPC `-32601 "Method not found: tools/call"`. The user's server says "Unknown tool: X".
- `wisp_crew_remove {"name":"Eve"}` answers "wisp: name isn't something a crew card can carry". It should say it needs `member`.

### G19. [smell] Terminal context lists another pane's commands
- The hook's log is per tty (`~/.wisp/term/ttys003.jsonl`).
- A new tmux pane on the same tty listed the closed pane's `echo`, `node` and `wisp` commands among "your last commands", before the new pane's own.

### G20. [smell] A factory's merge doesn't say that local main wasn't updated
- After the factory merged PR #2, proj8's local main stayed at `init`: the checkout had an uncommitted math.js.
- The run says only "merged: https://…/pull/2". `wisp merge` and the app's Merge say "Local main was not updated: … Commit or stash them, then pull."

### G21. [smell] A setup card outlives its agent
- After a restart, the asking agent is gone and the turn says "Stopped · wisp restarted…".
- The app still offers that card's Allow and Deny. Allow added the project, and nobody asked is told.
- `wisp show` doesn't show that card at all.

### G22. [smell] The Goal… flyout at 1280
- The flyout is 633 px tall from y=3 (it touches the window's top).
- Each goal's text runs to 3 lines. The phone sheet clamps it at 2 with an ellipsis.

## Count

22 new findings (G1–G22):
- **Major (4):** G1, G2, G3, G4
- **Minor (9):** G5–G13
- **Smell (9):** G14–G22
- **Blocker:** none

F's 7: all fixed. Of E5, E6, E11, E14 and D6: 3 partly fixed (E5, E11, D6), 2 not fixed (E6 worse, E14).

**Open after this round: 27**, the 22 new plus E5, E6, E11, E14 and D6.

**NEW findings: 22.**

## Housekeeping
- **Ports and homes.** Only :4972, /tmp/qaG1 and the tmux socket `qaG1` were used. Nothing touched ~/.wisp*, :4477, :4800, :4791 or :4329.
- **Stopped.** The server is stopped (`lsof -ti tcp:4972 -sTCP:LISTEN | xargs kill`; it was also restarted twice for row 73). The tmux server is stopped (`tmux -L qaG1 kill-server`). No pkill.
- **Children.** No fake harness was left running. Every gate file was released before the stop. Ada was paused again, and the trigger `tick` was left off.
- **Browsers.** Every Playwright browser was closed in its script's `finally`; none of mine was left.
- **Left in the scratch home.** The "G13 later ui" plan (09:55).
- **Kept.**
  - /tmp/qaG1 is kept for repro: scratch home, projects, formulas, the `ui/` scripts, the QA wrapper and the copied binaries.
  - /tmp/wisp-target-fixer is kept, as the shared build cache.
  - Artifacts are in /tmp/qa-keep/shots-qaG1/ (96 files).
- **Disk.** 123 GiB free at the end.
