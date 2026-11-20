# Audit H: orchestration, every action end to end (worktree HEAD be47769b), 2026-11-20

This round repeats audit G orchestration's scope and method (docs/qa/audit-G-orchestration.md) at HEAD be47769b:
- it re-checks G1–G22, plus E5, E6, E11, E14 and D6;
- it re-runs G's action table;
- it looks for new findings in the same areas and in what landed since G (docs/qa/audit-G-fixes.md,
  audit-G-setup-fixes.md, audit-G-tasks-fixes.md).

Findings only: no repo file was changed except this report.

## Setup

- **Server.** A scratch `wisp serve` on **:4973**.
  - Binaries from /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins` found nothing to rebuild: the build is HEAD's code; the worktree's only edits are design/v1-reference images). `wisp`, `wisp-fake-agent` and `wisp-fake-gh` were copied into /tmp/qaH-orch/opt and /tmp/qaH-orch/bin.
  - WISP_HOME=/tmp/qaH-orch/home, HOME=/tmp/qaH-orch/userhome, WISP_WEB_DIR=/tmp/qaH-orch/web (a fresh `vite build` of apps/web at HEAD).
  - Restarted once, on purpose, for rows 73 and 80.
- **Fake harnesses.** G's setup, moved to /tmp/qaH-orch:
  - codex, opencode, cursor-agent, copilot, grok and agy are the testkit fake.
  - claude is G's QA wrapper, with its cues (`PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`, `#call <tool> <json>`, `#limit`, spec and review stations).
  - `wisp-fake-gh` keeps its state in /tmp/qaH-orch/state/gh.json: G's two issues, no PRs at the start.
  - `FAKE_GATE` is /tmp/qaH-orch/gate.
- **Projects.** All are fresh git repos with G's files.
  - proj, with a bare origin; proj2–proj8 as in G, and proj8 has its own bare origin.
  - proj9start, proj9skip and proj9without are fresh untrusted copies of proj2, never added, for the held turns.
  - proj was added only after row 1; then all eight were added with `wisp add`.
- **Formulas and task files.** G's, in /tmp/qaH-orch/f, plus:
  - `team-c.yml`: team.yml with a claude critic;
  - `team-bad.yml`: an unknown field;
  - `trig-bad.yml`, `trig-words.yml` and `trig-off.yml`;
  - `fac3.yml`: the factory `shipit`, whose check appends a line, so ship has a change;
  - `broken.yml`.
- **Outbound traffic, browsers, tmux.**
  - Outbound traffic and harnesses as in G: DO_NOT_TRACK=1, WISP_TELEMETRY=0, WISP_UPDATE_CHECK=0, every outside URL pointed at 127.0.0.1:9. Only fakes ran.
  - Playwright (/tmp/pw) on system Chrome and on WebKit, at 1280×800 and 390×844. G's scripts were reused with paths and port changed; new ones are `ui/h-*.mjs`. Every script closes its browser in `finally`.
  - tmux on its own socket (`tmux -L qaHo`), zsh with `eval "$(wisp shell-init zsh)"`.
- **Interfaces.** The CLI, `/api/cmd`, `/api/mcp` as the user, a run's own MCP (through the wrapper's `#call`), the stdio `wisp mcp`, and the UI.
- **Artifacts.**
  - Screenshots are in /tmp/qa-keep/shots-qaHo/.
  - Logs are in /tmp/qaH-orch: serve.log, state/calls.jsonl and rec/*.jsonl (every harness argv and stdin).

Every FAIL below was re-checked by hand.
- The `fx-e-stop.mjs` Send-now timeout was a script hover problem: `fx-e-now.mjs` made the same click in 7 ms.
- The `#delegate` cue crashing was the wrapper (it expects a team). H1 was then reproduced with direct `#call delegate` calls.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | From proj, not yet added: each fake ran in its own `proj-<id>` worktree; the checkout stayed clean |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit MultiEdit Write NotebookEdit`, in place; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox` (now also `--dangerously-bypass-hook-trust`). UI Plan + Codex + In place sent in 3 ms |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`; the UI slider reads "High thinking" |
| 4 | Worktree vs in place | CLI, UI | pass | The Where chip reads "This computer · In place"; the codex run was in proj |
| 5 | Compare | CLI | pass | `--compare claude,codex:gpt-5`: two tasks. The UI was not re-run |
| 6 | Try (subagents=haiku) | CLI | pass | Runs A and B, "differs: subagentModel default → haiku"; `--try` without `--agent` is refused with its reason |
| 7 | Attach files | UI (Chrome) | pass | stdin has `<attachments>`; argv has `--add-dir` |
| 8 | Goal and judges | CLI | pass | `wisp goal set <id> never --kick` → "not met after 2 rounds … Keep going?" |
| 9 | Delegate (team) from the launcher | UI | not re-run | |
| 9b | Delegate a job to the team | run MCP | not re-run (team) | The new row 77 (a subagent made on the fly) fails |
| 10 | Follow-up queue: queue, edit, drag reorder, cancel | UI (Chrome) | pass | 3 queued; drag → `runtime.queue.order` ("q3 \| q1 \| q2") |
| 11 | Stop keeps the queue | UI | pass | `held=true` after Stop |
| 12 | Send now | UI | pass | 7 ms; the held item ran |
| 13 | Start later | UI (Chrome) | pass, with finding H7 | Send 6 ms; toast "Starts 16:26"; `wisp later plans` lists it |
| 14 | Ask (ask_user) and answer | run MCP, CLI | pass | `wisp show` lists the numbered options; `wisp reply … SQLite` → "The user answered: SQLite" |
| 15 | Usage limit | CLI | pass | "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent." |
| 16 | Fork… | UI (Chrome) | pass | 10 ms; Fork 15 ms; its own task |
| 17 | Rerun… | UI (Chrome) | pass | "Starts a new task in a new worktree of proj on this computer."; 9 ms; Rerun 18 ms |
| 17b | `wisp rerun <id> -d` | CLI | pass, with finding H10 | "<id> → <new id> (as it ran)"; no `rerun-*` folder |
| 18 | Side discussion (`/side`) | UI (Chrome) | pass | 15 ms; its project reads "proj" |
| 19 | /btw | UI | pass | One Enter, 4 ms; the answer card shows |
| 20 | Review by another harness | CLI, UI | pass (E5 fixed) | `wisp review maybe <id> --force` → status `failed`. The strip reads "Review failed", the reason in its title. A click shows "Reviewing" at 51 ms, then "Review failed" again (the fake answers in ~100 ms) |
| 21 | Ship: commit & open PR | CLI | pass | "✓ shipped: …/pull/2". The UI's Ship menu lists "Ship it… · Open PR · Merge PR #2…" |
| 22 | Merge PR (UI) | UI | not re-run | |
| 22b | `wisp ship` + `wisp merge` | CLI | pass | "! Local main was not updated: your uncommitted changes to notes.txt are in the way…" and "· The worktree and its branch were removed, here and on the remote." Both gone |
| 23 | Restore to before latest turn | UI | not re-run | |
| 24 | Bring in | UI, API | **fail** (E6) | 58 git worktrees in proj, plus a stray with a file. The pane didn't show in 75 s. `worktrees.view` said `looking: true, last: null` on every poll for 4 min, and again after 3 quiet minutes (11+ min after the server started). The server had no `du` or `git` child: nothing was looking |
| 25 | Worktree removal | UI | not re-run | |
| 26 | Crew: add | run MCP card, CLI | pass, with finding H8 | Zed added from a card (its folder is the project, not a worktree: GS3). `crew update` prints a field dump |
| 27 | Crew: give a job | CLI | pass, with finding H8 | `wisp crew assign Zed "…"` → "session … · Zed · Job · running" |
| 28 | Crew: check-in | CLI | pass | `wisp crew checkin Zed` → a Check-in task |
| 29 | Crew: pause / resume | CLI | pass (G12 fixed), with finding H12 | `crew pause Zed`, `crew resume zed`, `--paused false`, `--paused=false`: `crew.list` agrees each time |
| 30 | Crew: tint | UI | not re-run | |
| 31 | Crew wakes on "I mark a task done" | API | not re-run | |
| 32 | Crew: Org link | CLI | not re-run | |
| 33 | `wisp ls --crew zed` | CLI | pass | Lists Zed's Job and Check-in |
| 34 | Recipes | UI | not re-run | |
| 35 | Triggers: new, fires, on/off | formula, UI, API | pass (G3 fixed), with findings H4, H13 | Off 3 ms (`enabled:false`), on sends `enabled:true`, and it stays on. Delete was not re-run |
| 36 | Notes, Fresh thread | CLI | pass (G17 fixed) | `wisp notes <id> --set goal=…`; `wisp fresh <id> -d` starts a thread with no `--resume` and the Notes in its brief |
| 36b | The agent keeps its Notes | run MCP | not re-run | |
| 37 | User MCP (stdio): launch | MCP | pass | 262 tools; `wisp_launch {goal:"never"}` launched |
| 38 | HTTP MCP: launch | MCP | pass (G6 fixed) | `wisp_launch {cwd, agent, goal:"green-build"}` ran in a worktree, and the goal was met after 2 rounds |
| 38b | HTTP MCP: every tool takes its fields | MCP | partial (D6) | 3 tools still take only `{args, peer}`: `factory_set`, `resources_plan`, `resources_read`. Unknown fields are refused ("stuff_get has no field idd: it takes id") |
| 39 | A task launched by an agent | run MCP | pass | `wisp_launch {formula:"ok.yml"}` from a worktree task ran on codex |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 4 ms |
| 41 | Phone 390 (Chrome dark, WebKit light) | UI | pass | Launcher sheet 390 wide, no horizontal overflow; factory band and setup card fit |
| 42 | Immediate feedback | UI | pass | Every measured action took 2–18 ms (see below) |
| 43 | `wisp help runtime answer` | CLI | pass | Each field now has words (G17) |
| 44a | Try from the UI | UI | not re-run | |
| 44b | A held turn's "Trust and start" | CLI | pass | Edits, in a fresh untrusted copy: "◷ Held before it started: This repo asks to run its own programs…"; `wisp held <id> start` → ran. The UI card was not re-run |
| 44c | Start from an issue | UI | not re-run | |
| 44d | Ask to fix (failing check) | UI | not re-run | |
| 44e | A held turn's "Skip" | CLI | pass | "● Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." |
| 44f | A held turn's "Run without them" | CLI | pass | claude got `--setting-sources user --strict-mcp-config` |
| 45 | Long thread | CLI | not re-run | |
| 46 | Schedule triggers over time | formula | pass (G11 fixed) | ~10 min: `tick` (`* * * * *`) ran 8 times, `words` (`cron: "every minute"`) 4 times; both then switched off with `enabled: false` |
| 47 | A project's worktree setup | CLI | pass | `setup-ran` in the worktree; "◷ Held before it started: setup `touch setup-ran; exit 3` failed: exit 3" |
| 48 | Server warnings | — | pass | 1 WARN in the whole round: the failing setup (row 47) |
| 49 | A long follow-up in the reply box | UI | not re-run | |
| 50 | Named goals in a formula | CLI | pass | `apply`, then `diff` "6 unchanged"; `wisp goal declared` lists 33 lines |
| 51 | `wisp run <task>` with `tasks.default` | CLI | pass, with finding H11 | `run fix-build` launched; `run default` → "tasks.default gives every task its defaults: it isn't a task to run (wisp recipes lists them)" |
| 52 | `--goal-text` | CLI | pass | Launched with the words |
| 53 | `wisp goal set <task> <name>` | CLI | pass (G16 fixed), with finding H6 | `nevr` → "no goal named nevr: goals has … ; use --text "…" for words", nothing set; `never` is the declared goal |
| 54 | A goal's round limit | CLI | pass | `never` (`maxRounds: 2`) asked "The goal isn't met after 2 rounds. … Keep going?" |
| 55 | `wisp_launch`'s `goal` | stdio, run, HTTP MCP | pass | HTTP (G6), stdio and run all take it; `goal_*` tools are in all three (G7) |
| 56 | Goal… with 32 goals | UI (Chrome) | pass (G22 fixed) | Pick 2 ms, sent as `{"name":"goal-03"}`. Flyout at y=8, 545 tall; hints 2 lines (35 px) |
| 57 | `/goal` in the reply box | UI | pass (G14 fixed) | `/goal green-build` + one Enter → `goal.set` (8 ms) |
| 58 | `wisp "…" --formula f.yml --agent critic` | CLI | pass (G8 fixed) | codex, `gpt-5`, effort high, "Grace, the critic." in its instructions, in a worktree. team-bad.yml refused before launching: "✗ agents.critic.brief: unknown field (fields: …)" |
| 59 | `wisp run <task> --formula f.yml` | CLI | partial (G2, G9) | In a worktree now (G9 fixed). The judge is the formula's agent (codex), and team-c.yml (claude critic) is "met after 1 round". A judge answering without a verdict sent the work back 2 times, then asked (H3) |
| 60 | Task files: a file, inline, a list, `module.x` | CLI | pass | a, b, c and g started; d, e, f and h refused with G's reasons |
| 61 | `wisp_launch`'s `formula` | run MCP, stdio | pass (G1 fixed) | A worktree's `ok.yml` ran on codex. Refused: `../<other worktree>/ok.yml`, `../../secrets.json` and an absolute path ("…read from inside its task's folder (no absolute paths, no ..)") |
| 62 | New task › Formula or task file… | UI (Chrome) | pass (G10 fixed), with finding H5 | `.qa/a.task.yml` with `formula: ./team.yml` beside it: `runtime.create` carried critic (codex) and the goal; send 3 ms; codex ran |
| 63 | validate names unknown references | CLI | pass (G11 fixed) | "tasks.broken: no goal named green-bild: goals has goal-01 … goal-08 and 24 more"; "factories.broken: no agent named plannr…"; `on: every minute` → "is no source (a schedule is on: schedule with cron: …); sources: … and 18 more" |
| 64 | A child never exceeds its parent | run MCP | pass | Plan parent: `wisp_launch {mode:auto}`, `runtime_create {mode:auto}` and `{factory, mode:auto}` refused; with no mode, plan |
| 65 | Agents' writes | run MCP | pass (G18 fixed) | `connectors_add`, `nosuch_tool` → "Unknown tool: X"; `wisp_crew_remove {"name":"Zed"}` → "…: it takes member" |
| 66 | Setup cards: ask, Allow, Deny | run MCP, CLI, UI | pass (G4 fixed) | `wisp show` names Allow/Deny/words. `wisp reply <id> Allow` → "✓ You allowed: add crew member Zed". Words → "✓ You denied: … (“keep Zed, he runs nightly”)". Typed in the app → `runtime.answer` with the words (5 ms), and the agent got "They said: “no, keep Zed”" |
| 67 | Setup cards on the phone | UI (390 dark, WebKit) | pass | The card fits; Allow 5 ms → `runtime.answer`; the connector was added. The channels' typed answer was not re-run |
| 68 | Factory: the built-in default | CLI | pass | "▦ default: ? spec · …"; words instead of Approve revised the spec |
| 69 | Factory: a declared one, looping | CLI, fake gh | pass (G15, G20 fixed) | `strict`: round 3, review CHANGES; ship "Nothing to ship" (the fake never edits). `shipit`: PR #1 "Made by the wisp factory `shipit` in 1 round."; Merge → "merged PR #1 · Local main was not updated: your uncommitted changes to math.js are in the way. Commit or stash them, then pull." `wisp ls`: "Factory · shipit" |
| 70 | Factory names | CLI | pass | "no factory named nosuch: factories has default, gated, shipit, slowcheck, strict, tight"; `wisp export factories` |
| 71 | Factory: Stop | CLI, UI | pass (G13 fixed), with findings H8 and G15 | UI: ⋯ › Stop the run (2 ms) asks "Stop the factory run? Every station still working stops, and the run ends Stopped. Its stations’ tasks and worktrees are kept."; then "✗ checks · you stopped it". CLI `wisp factory stop <id>` killed the running `sleep 120` |
| 72 | Factory: delete a run | CLI, UI | pass (G13 fixed) | The UI's Delete says "Its stations’ tasks go with it, and their worktrees that have nothing uncommitted." `wisp runtime delete <run>` → gone from `wisp ls --all` |
| 73 | Factory: restart mid-run | CLI | pass | Server killed during `sleep 120`: the check died with it. On start the checks ran again (a new `sleep 120`), "▦ slowcheck: ✓ build · ● checks" |
| 74 | Factory started by an agent | run MCP | pass | Plan parent's `wisp_launch {factory:"gated"}`: the run is `plan`; checks asked "Run `true` on the built code? Run / Skip / Stop"; `factory_stop` isn't an agent's tool (G7) |
| 75 | Factory in the app | UI (Chrome; 1280, 390 dark) | pass | The band lists every station with its output; no overflow at 390. The Factory picker was not re-run |
| 76 | Terminal context in tmux | CLI | pass (G19 fixed) | A closed pane and a new one on the same tty (ttys003): "1 command" listed, only the new pane's `ls nosuchfile` → exit 1 |
| 77 | An agent delegates to a subagent it names (new) | run MCP | **fail** (H1) | See H1 |
| 78 | A misspelled launch flag (new) | CLI | **fail** (H2) | See H2 |
| 79 | Factory with a real change: PR, merge (new) | CLI, fake gh | pass | Row 69 (`shipit`) |
| 80 | Restart with an open setup card (new) | CLI | pass (G21 fixed) | "◐ Stopped · wisp restarted while this was running, so its card asking to add crew member Eve was taken back: nothing changed." |

**Counts:** 90 rows (G's 86, plus 77–80).
- **Pass:** 70, of which 13 carry a lesser finding.
- **Partial:** 2 (rows 38b and 59).
- **Fail:** 3 (rows 24, 77 and 78).
- **Not re-run:** 15 (rows 9, 9b, 22, 23, 25, 30, 31, 32, 34, 36b, 44a, 44c, 44d, 45 and 49). These are UI paths and older rows unchanged since F/G; this round put its time into G's findings and what landed since.

**Feedback (ms, pointerdown or keydown → first DOM change).**
- Launch: 2–3; goal pick 2; goal send 3.
- Formula or task file… 2, its send 3.
- Fork 10 / 15; Rerun 9 / 18; Side 15; btw 4; Done 4.
- Send now 7.
- Trigger switch: off 3.
- Typed answer under a card 5; Allow 5 (WebKit, 390).
- Review click → "Reviewing" 51 (6 to the first DOM change).
- Factory Stop the run 2; Delete 8.

## Audit G's findings, re-checked

| # | Finding | Status at be47769b | Evidence |
|---|---|---|---|
| G1 | An agent's formula in its own worktree is refused as "a secrets file" | **fixed** | `ok.yml` in `home/worktrees/proj-<id>` → "Launched task …"; codex ran. Another worktree, `../../secrets.json` and an absolute path are still refused |
| G2 | A formula's agent can't judge its goal; the goal loops | **partly fixed** | The judge is now the formula's agent: codex judged, and with a claude critic "met after 1 round". But a judge that answers without a verdict is still "not met" and sends the work back ("critic: critic didn't say whether it's met"), 2–3 times until the round limit (H3). The "can't run" path (a judge that never starts) wasn't reproduced live |
| G3 | A trigger switched off in the app can't be switched back on | **fixed** | Off sends `enabled:false`, on sends `enabled:true`; the switch and the server agree. A new round-trip problem: H4 |
| G4 | A setup card can't be answered from the CLI or by typing | **fixed** (CLI, app) | `wisp reply` Allow and words both answer the card; nothing is queued. The app's typed words send `runtime.answer` with `askId`. A chat's typed answer wasn't re-run |
| G5 | "Approve the spec?" doesn't show the spec | **fixed** | `wisp show` prints the spec under the question; the band's Spec row shows it |
| G6 | The HTTP user's `wisp_launch` is v1's | **fixed** | Schema has goal, formula, factory, project and worktree; a run in a worktree, goal met |
| G7 | No goal or factory tools | **fixed** | HTTP: `goal_check/declared/drop/pause/resume/set`, `factory_list/names/remove/set/stop`. A run: the goal tools, `factory_list` and `factory_names` (no `factory_stop`) |
| G8 | An invalid formula launches anyway | **fixed** | Both paths refused with validate's words |
| G9 | `wisp run <task> --formula` runs in place | **fixed** | `home/worktrees/proj-<id>` |
| G10 | The app's task file reads `./x.yml` from the project's root | **fixed** | `.qa/a.task.yml` + `./team.yml` beside it: critic and its goal are carried |
| G11 | Triggers: an `on:` with no source, a cron in words | **fixed** | `on: every minute` refused, naming the sources; the words cron fired 4 times; `diff` right after `apply` says unchanged |
| G12 | `crew update` takes only the hash id; no resume | **fixed** | `crew pause/resume` by name; `--paused false` and `=false` |
| G13 | A factory run's Stop and Delete | **fixed** | Both dialogs (row 71, row 72) |
| G14 | `/goal <declared name>` swallows the first Enter | **fixed** | One Enter → `goal.set` |
| G15 | Factory words | **partly fixed** | Fixed: "1 round"; ship output "merged PR #1"; `wisp ls` "Factory · shipit"; "you stopped it". Left: `wisp show`'s band line still ends with the raw URL ("· https://github.com/acme/repo/pull/1"). A run the user stopped reads **"Blocked"** in the task list ("H factory restart · Factory · slowcheck · Blocked"), where its own band says "stopped" |
| G16 | Goal words | **fixed** | "after 2 rounds"; `goal set <id> nevr` refused. The refusal lists every name (H6) |
| G17 | CLI and formula word slips | **fixed** (the listed ones) | "(a formula starts with `wisp: 1`)", "1 resource", `fresh -d`, `runtime answer --help`, `factory stop <id>`. More of the same kind: H9 |
| G18 | An agent's MCP: wrong words for a missing tool or field | **fixed** | "Unknown tool: connectors_add"; "…: it takes member" |
| G19 | Terminal context lists another pane's commands | **fixed** | Row 76 |
| G20 | A factory's merge doesn't say local main wasn't updated | **fixed** | Row 69 |
| G21 | A setup card outlives its agent | **fixed** | Row 80 |
| G22 | The Goal… flyout at 1280 | **fixed** | y=8, 2-line hints (shot H-goalflyout-light-1280x800.png) |
| E5 | A failed review shows "Not reviewed"; no progress on Review again | **fixed** | "Review failed" with its reason; "Reviewing" 51 ms after the click |
| E6 | With many worktrees, Bring in never shows a stray | **not fixed** | Row 24: `looking: true, last: null` for 11+ minutes with 58 git worktrees, and no look process running |
| E11 | Generated CLI help shows developer notes | **partly fixed** | 14 commands still show them in `--help`. `goal set --name` "(`goals:` in a formula, ADR-0110)"; `machines uncarry` "(here, ADR-0060)"; `runtime create --id` "…ADR-0017…"; `runtime followUp --fresh` "ADR-0096"; `worktrees create --reuse` "ADR-0108"; `later park` "(For later, v1 PATCH /sessions/:id)"; `crew assign --from` "v1 crew.js `#delegate`"; `runtime appState` "v1 /app-state"; `runtime create --cap` "v1 `session.capped`"; `runtime followUp --queue` "v1 `reply`"; `connectors add` "(`POST /connectors`, `PATCH /connectors/:id`)"; `push pair` "(`POST /api/push/register`)". `connectors list --help` says "unknown fields are ignored, as v1 did", but they are refused (`--bogus` and `{"bogus":1}`) |
| E14 | The side panel's Preview tab is icon-only at 1280 | **fixed** | 1280: "Changes 1" 74, "PR #2" 45, "Shell" 39, "Preview" 57, then Thread as a 28 px icon |
| D6 | MCP tools without an argument schema | **partly fixed** (as its fix says) | 3 left: `factory_set`, `resources_plan`, `resources_read` |

**G totals:**
- G1–G22: 20 fixed; 2 partly fixed (G2, G15).
- E5, E6, E11, E14 and D6: 2 fixed (E5, E14), 2 partly fixed (E11, D6), 1 not fixed (E6).

## New findings

### H1. [major] An agent can't make a subagent on the fly: `delegate` refuses the shape its own schema gives
- **Steps.**
  - A task with no team (any default task). Its agent calls `delegate {"tasks":[{"agent":"codex","name":"tester","prompt":"run the tests"}]}`. That is the schema's own way: `items.required: ["prompt"]`, and the description says "A task can … create one on the fly: agent (a harness) … and a name".
  - Also tried without `name`, with `"wait": false`.
- **Actual:**
  - Both calls → `isError`, "wisp: runtime.delegate: missing field `subagent`".
  - Adding `"subagent":"tester"` → "wisp: This session has no subagents yet. Name a harness (agent, and machine to run it elsewhere) in each task to create one". That points back to the first call.
  - The schema has no `subagent` field at all when there is no team (`agent, effort, machine, model, name, prompt, use`).
- **Expected:** `agent` (+ `name`) creates a subagent and runs the task, as the tool says. Only an agent with a team can delegate today.
- **Evidence:** state/calls.jsonl; tasks 01a1f95780587068a84715cf27717ba9 and 01a1f957d0907301a4f70d1d0448edec.

### H2. [major] A misspelled flag is folded into the prompt, and the task launches with the defaults
- **Steps.** From proj, run each of these:
  - `wisp "H typo2" --mdoe plan`
  - `wisp "H typo flag" --agnet codex`
  - `wisp "H flagtest" --bogus 1`
  - `wisp "H later" --at 23:55`
- **Actual:**
  - Each launched at once, the flag in its title and prompt ("H typo2 --mdoe plan"), on Claude Code in **Full auto**.
  - A user who meant Plan got a run that may edit and run anything; one who meant Codex got Claude; one who meant later got now.
  - A verb with a bad flag is refused ("✗ wisp connectors list takes no --bogus (see --help)"), but a prompt with one isn't.
- **Expected:**
  - Refuse an unknown `--word` after a prompt, naming the close flag ("did you mean --mode?"), unless it's after `--` or quoted inside the prompt.
  - At least refuse one that is a close typo of a launch flag.

### H3. [minor] A judge that answers without a verdict sends the work back every round
- **Steps.** `wisp run release --formula team.yml` (judge `critic` on codex), or the app's task file `.qa/a.task.yml`, with the fake codex judge, which never says MET or NOT MET.
- **Actual:**
  - Each round the builder gets "The goal isn't met yet. What the judges said: critic: critic didn't say whether it's met. Keep going until it is."
  - It takes 3–4 paid rounds, then "The goal isn't met after 4 rounds … Keep going?" (`goals.default.maxRounds: 4`).
- **Expected:** this is G2's second half. A judge with no verdict hasn't judged: say so once and ask, as the fix does for a judge that can't run. The builder can't act on "didn't say".

### H4. [minor] The app's trigger switch rewrites `project: proj` as its path, so the formula drifts
- **Steps.** `wisp apply -f trig.yml` (`project: proj`); `wisp diff` → unchanged. Turn the trigger off and on in Settings › Automations.
- **Actual:**
  - The switch sends `resources.apply` with `"project":"/private/tmp/qaH-orch/proj"`.
  - Afterwards `wisp diff -f trig.yml` says "project: "/private/tmp/qaH-orch/proj" → "proj" · 1 to change", and `wisp export triggers` writes the path.
  - G11's round-trip fix keeps the name only when it comes from a file.
- **Expected:** the switch changes `enabled` alone and keeps the stored spec as written.

### H5. [minor] New task with a task file: the bar says Claude Code while the file's agent runs
- **Steps.** New task › + › Formula or task file…, pick `.qa/a.task.yml` (`agent: critic`, codex).
- **Actual:**
  - The chip reads "Formula: a.task.yml", but the harness pill still says "Claude Code" and the model "Default".
  - The send carries `"agent":"critic"` with the formula's codex, and codex runs.
  - What the bar shows isn't what runs.
- **Expected:** the pills show the file's agent (critic · Codex · gpt-5), or say "from the file".

### H6. [minor] `--goal` and `goal set` refusals list every declared goal
- **Steps.** With 32 goals declared, run `wisp "…" --goal nevr` or `wisp goal set <id> nevr`.
- **Actual:**
  - "✗ no goal named nevr: goals has goal-01, goal-02, … goal-30, green-build, never; use --text …": all 32 names on one line.
  - `wisp validate` cuts the same list at 8 ("… goal-08 and 24 more", G10's fix). These CLI paths don't.
- **Expected:** the same 8-name cap, plus the close match ("did you mean never?").

### H7. [minor] `wisp later cancel` says "ok true" for something that isn't there
- **Steps.**
  - `wisp later plans` shows an id cut to 21 characters ("01a1f95a22ac7768acff4…").
  - `wisp later cancel 01a1f95a22ac7768acff4` → "ok  true"; the plan is still listed.
  - `wisp later cancel nosuchid123` → "ok  true".
- **Actual:** success for nothing. The plan needs `later cancel-plan <full id>`, which the list doesn't print.
- **Expected:** "nothing scheduled with that id" (exit 1); the list's id usable as printed, or printed whole.

### H8. [smell] CLI verbs print a raw field dump instead of a sentence
- `wisp factory stop <id>` → "stopping  true".
- `wisp crew checkin Zed` / `crew assign Zed "…"` → "session  01a1f9517ee9… · Zed · Check-in · running".
- `wisp review maybe <id>` → "why  Review by another harness is off."
- `wisp crew update Zed --paused` prints every field ("agent auto · brief … · checkins 0 · color violet …"), or "updated just now".
- `wisp connectors list` → "added 0" for a connector added that minute.

### H9. [smell] Help with no words, or the wire shape instead of words
- `wisp review maybe --help`: "--id TEXT  required." with nothing after it. The same for `later cancel-plan --help`.
- `wisp later plan --help`:
  - it opens with "`later.plan`: `{kind: task, body}` or `{kind: reminder, text, from?}`…";
  - its 8 fields (`--kind`, `--body`, `--at`, …) have no words;
  - nothing says that `wisp "…"` can't take a time (H2).
- `wisp crew assign --help` opens with "`kit: true` with no prompt: "Build your kit"." instead of what the verb does. G17 fixed this kind of opening for `crew update`.
- `wisp factory stop --help` says "`factory.stop`: a run's parent task.".

### H10. [smell] `wisp rerun <id> -d` prints the multi-run hint
"wisp attach <id> to follow one; --wait waits for all and compares" follows a single rerun.

### H11. [smell] `wisp run`'s miss names only recipes
- `wisp run nosuch` → "✗ no recipe or workflow named nosuch: wisp recipes lists them".
- `wisp run default` → "… it isn't a task to run (wisp recipes lists them)".
- `wisp run` also runs declared tasks (`wisp run fix-build`), and neither line points at `wisp automations tasks`.

### H12. [smell] `wisp crew ls` doesn't mark a paused member
After `wisp crew pause Zed`, the list reads "Zed  QA watcher  auto", the same as before; only `crew.list`'s `paused: true` tells.

### H13. [smell] The CLI can resume a trigger but not switch one off or remove it
- `wisp help automations`: `trigger approve / fire / log / ls / push / resume / test`.
- There is no `trigger off`, `pause` or `remove`. Switching one off from the CLI means writing a formula with `enabled: false` and applying it.

## Count

13 new findings (H1–H13):
- **Major (2):** H1, H2
- **Minor (5):** H3–H7
- **Smell (6):** H8–H13
- **Blocker:** none

G's 22: 20 fixed, 2 partly fixed (G2, G15). Of E5, E6, E11, E14 and D6: 2 fixed (E5, E14), 2 partly fixed (E11, D6), 1 not fixed (E6).

**Open after this round: 18**: H1–H13, plus G2, G15, E6, E11 and D6. G2's remainder is H3; G15's are the raw URL in `wisp show` and "Blocked" for a stopped run.

**NEW findings: 13.**

## Cleanup

- **Ports and homes.** Only :4973, /tmp/qaH-orch and the tmux socket `qaHo` were used. Nothing touched ~/.wisp*, :4477, :4800, :4791, :4329 or :5180.
- **Stopped.**
  - The server: `lsof -ti tcp:4973 -sTCP:LISTEN | xargs kill` (pid 58287, after one deliberate restart for rows 73/80). Afterwards :4973 was free.
  - tmux: `tmux -L qaHo kill-server`.
  - No pkill or killall was used.
- **Children.** After the stop, no process from /tmp/qaH-orch was left: no fake, no `sleep 120` check.
  - Every gate file (hq, hdone, gqC, gqW) was released.
  - Both triggers (`tick`, `words`) were left off.
  - The "G13 later ui" plan was cancelled.
  - The factory runs were stopped or ended.
- **Browsers.** Every Playwright browser was closed in its script's `finally`; no Playwright Chrome was left running.
- **Kept for repro.**
  - /tmp/qaH-orch: scratch home, projects, formulas, `ui/` scripts, the QA wrapper and the copied binaries.
  - /tmp/wisp-target-fixer, as the shared build cache.
  - /tmp/qa-keep/shots-qaHo/, the screenshots.
- **Disk.** 95 GiB free at the end.
