# Audit I: orchestration, every action end to end (worktree HEAD 1ea3b84f), 2026-11-20

This round repeats audit H orchestration's scope and method (docs/qa/audit-H-orchestration.md) at HEAD 1ea3b84f:
- it re-checks H1–H13 and the leftovers G2, G15, E6, E11 and D6;
- it re-runs H's whole action table, including the 15 rows H marked "not re-run" (9, 9b, 22, 23, 25, 30, 31, 32,
  34, 36b, 44a, 44c, 44d, 45, 49);
- it hunts for new findings in the same areas and in what landed since H (docs/qa/audit-H-orchestration-fixes.md,
  audit-H-setup-fixes.md, audit-H-tasks-fixes.md): stray-flag refusal (short flags too), `--mode`/`--model` words,
  the on-the-fly delegate and its errors, saved model/effort for CLI/MCP/crew tasks, Auto-off schemas, the
  automations/settings/worktrees/channels tools over MCP and what an agent may call, factory stop taking its question
  back, and `trigger off/on/rm`.

Findings only: no repo file was changed except this report.

## Setup

- **Server.** A scratch `wisp serve` on **:4974**.
  - Binaries from /tmp/wisp-target-fixer (`cargo build -p wisp -p wisp-testkit --bins` at HEAD; the worktree's only
    edits are design/v1-reference images). `wisp`, `wisp-fake-agent` and `wisp-fake-gh` were copied into
    /tmp/qaI-orch/opt and /tmp/qaI-orch/bin.
  - WISP_HOME=/tmp/qaI-orch/home, HOME=/tmp/qaI-orch/userhome, WISP_WEB_DIR=/tmp/qaI-orch/web (a fresh `vite build`
    of apps/web at HEAD).
  - Restarted twice, on purpose, for rows 73 and 80.
- **Fake harnesses.** H's setup, copied to /tmp/qaI-orch with paths and port changed:
  - codex, opencode, cursor-agent, copilot, grok and agy are the testkit fake;
  - claude is H's QA wrapper (cues `PASSAFTERn`, `NEVERMET`, `#ask`, `#tools`, `#delegate`, `#call <tool> <json>`,
    `#limit`, spec and review stations);
  - `wisp-fake-gh` state in /tmp/qaI-orch/state/gh.json: G's two issues, no PRs at the start;
  - `FAKE_GATE` is /tmp/qaI-orch/gate.
- **Projects.** Fresh git repos with H's files: proj (bare origin), proj2–proj8 (proj8 with its own bare origin).
  proj9start/skip/without are fresh untrusted copies of proj2, never added. proj was added only after row 1.
  proj4 got a `https://github.com/acme/repo.git` remote for row 44c (as in H).
- **Formulas.** H's /tmp/qaH-orch/f, copied (goals, many (32 goals), fac, fac2, fac3, trig, trig-words, team, team-c,
  team-bad, broken, nowisp, task files a–h).
- **Outbound traffic, browsers, tmux.** DO_NOT_TRACK=1, WISP_TELEMETRY=0, WISP_UPDATE_CHECK=0, every outside URL at
  127.0.0.1:9; only fakes ran. Playwright from /tmp/pw on system Chrome and WebKit, 1280×800 and 390×844. H's scripts
  were reused with paths and port changed; new ones are `ui/i-*.mjs`. Every script closes its browser in `finally`.
  tmux on its own socket (`tmux -L qaIo`).
- **Interfaces.** The CLI, `/api/cmd`, `/api/mcp` as the user, a run's own MCP (the wrapper's `#call`, and
  `rcall.mjs`, which calls a run's MCP with that run's own session and token from the harness argv), the stdio
  `wisp mcp`, and the UI.
- **Artifacts.** Screenshots in /tmp/qa-keep/shots-qaIo/. Logs in /tmp/qaI-orch: serve.log, state/calls.jsonl,
  rec/*.jsonl (every harness argv and stdin), notes/log.md (this round's running notes).

Every FAIL below was re-checked by hand, each one twice or more. I1 was proved on two harnesses (Claude Plan and
Codex Plan), and I4 from both the request body and the harness argv.

## Action table

| # | Action | Where | Result | How checked |
|---|---|---|---|---|
| 1 | New task, each of the 7 harnesses | CLI | pass | From proj, not yet added: each fake ran in its own `proj-<id>` worktree; the checkout stayed clean |
| 2 | Mode Plan / Edits / Full auto | CLI, UI (Chrome) | pass | Plan: `bypassPermissions` + `--disallowedTools Edit MultiEdit Write NotebookEdit`, in place; codex `sandbox_mode="read-only"`. Edits: `acceptEdits` + `--permission-prompt-tool mcp__wisp__approve`; codex `workspace-write`. Auto: `bypassPermissions`; codex `--dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust`. UI Plan + Codex + In place sent in 2 ms |
| 3 | Model and thinking | CLI, UI | pass | `--model opus --effort high`; codex `model_reasoning_effort="high"`; the UI slider reads "High thinking" |
| 4 | Worktree vs in place | CLI, UI | pass | `--no-worktree` ran in proj; the Where chip reads "This computer · In place" |
| 5 | Compare | CLI | pass | `--compare claude,codex:gpt-5`: two tasks |
| 6 | Try (subagents=haiku) | CLI | pass | Runs A and B, "differs: subagentModel default → haiku"; `--try` without `--agent` refused |
| 7 | Attach files | UI (Chrome) | pass | stdin has `<attachments>`; argv has `--add-dir` |
| 8 | Goal and judges | CLI | pass | `goal set <id> never --kick` → "not met after 2 rounds … Keep going?" (`goal set` prints a field dump: I12) |
| 9 | Delegate (team) from the launcher | UI (Chrome) | pass (was not re-run in H) | + › Delegate… › Add delegate › With the lead → Codex; chip "1 delegate"; send 4 ms |
| 9b | Delegate a job to the team | run MCP | pass (was not re-run in H) | The lead's `delegate` → toast "B delegated: run the tests is ready"; report from codex |
| 10 | Follow-up queue: queue, edit, drag reorder, cancel | UI (Chrome) | pass | 3 queued; drag → `runtime.queue.order` ("q3 \| q1 \| q2"); cancel asks "Cancel “D10C q1”?" |
| 11 | Stop keeps the queue | UI | pass | `held=true` after Stop; the gated turn `stopped` |
| 12 | Send now | UI | pass | 7 ms; q3 ran |
| 13 | Start later | UI (Chrome) | pass (H7 fixed) | Send 8 ms; toast "Starts 22:19"; `wisp later plans` prints the whole id; `later cancel <that id>` takes it back |
| 14 | Ask (ask_user) and answer | run MCP, CLI | pass | `wisp show` lists the options; `wisp reply … SQLite` → "The user answered: SQLite" |
| 15 | Usage limit | CLI | pass | "✗ Claude Code (Main) is at its usage limit · Wait for the reset, or hand the task to another agent." |
| 16 | Fork… | UI (Chrome) | pass | Menu 9 ms; Fork stays disabled until "What should the fork try?" has text; Fork 20 ms → `runtime.fork` + its follow-up |
| 17 | Rerun… | UI (Chrome) | pass | "Starts a new task in a new worktree of proj on this computer."; 9 ms; Rerun 22 ms |
| 17b | `wisp rerun <id> -d` | CLI | pass (H10 fixed) | "→ <new id> (as it ran) · wisp attach <new id> follows it" |
| 18 | Side discussion | UI (Chrome) | pass | 2 ms; the reply box takes the caret; send 15 ms → a task with `sideOf`, in the task's worktree, project proj |
| 19 | /btw | UI | pass | 5 ms; the answer card shows |
| 20 | Review by another harness | CLI, UI | pass | `review maybe <id> --force` → `failed` with its why; the strip: "Review failed" → "Reviewing" at 54 ms → "Review failed" |
| 21 | Ship: commit & open PR | CLI | pass | "✓ shipped: …/pull/2" |
| 22 | Merge PR (UI) | UI (Chrome) | pass (was not re-run in H) | Confirm "Squash-merge PR #2? …" 3 ms; go 3 ms; toast "Merged PR #2. Local main is up to date with origin/main. The worktree … was kept: it has 1 uncommitted change." |
| 22b | `wisp ship` + `wisp merge` | CLI | pass | "✓ merged PR #4 · Local main is up to date with origin/main. · The worktree and its branch were removed, here and on the remote." Both gone |
| 23 | Restore to before latest turn | UI (Chrome) | pass (was not re-run in H) | Confirm 2 ms, go 4 ms, toast "The folder is back to before this turn. Undo"; the turn's file is gone |
| 24 | Bring in | UI (Chrome) | **pass (E6 fixed)** | 170 git worktrees of proj (140 wisp's, 30 extra, one with a file): the pane's Bring in showed "30 worktrees from git · In proj · 1 with uncommitted changes" **624 ms** after opening |
| 25 | Worktree removal | UI (Chrome) | pass (was not re-run in H) | "Remove this worktree? Its 1 uncommitted change can’t be brought back."; 19 ms; `worktrees.clean`; the folder is gone |
| 26 | Crew: add | run MCP card, CLI | pass | Zed added from a card; with Auto off and no harness named it got New task's last pick (codex: HS7) |
| 27 | Crew: give a job | CLI | pass (H8 fixed) | `crew assign Zed "…"` → "✓ Zed · Job, running · wisp attach <id> follows it"; ran codex gpt-5 high (HS5) |
| 28 | Crew: check-in | CLI | pass | "✓ Zed · Check-in, running" |
| 29 | Crew: pause / resume | CLI | pass (H12 fixed) | `crew ls` → "Zed … codex  paused (wisp crew resume to bring them back)"; `--paused false` → "✓ Zed: back: not paused" (I12) |
| 30 | Crew: tint | UI (Chrome) | pass (was not re-run in H) | 10 options; pick 3 ms (no live preview); Save 12 ms → `bg: rose`, the rail turns rose |
| 31 | Crew wakes on "I mark a task done" | API | pass, with finding I7 (was not re-run in H) | `runtime.setState done` → "Bob · Job", codex, own worktree; marking Bob's own job done doesn't wake him again. `--on task.done` didn't take (I7); `--wakes` did |
| 32 | Crew: Org link | CLI | pass (was not re-run in H) | Ada `--team '[{"crew":"bob","why":"reviews"}]'`; Ada's job's `delegate` has `subagent` enum `["bob"]` |
| 33 | `wisp ls --crew zed` | CLI | pass | Zed's Job and Check-in |
| 34 | Recipes | UI (Chrome) | pass (was not re-run in H) | Edit saves "I34 fix the {thing} …" ("Fields when run: {thing}"); Run dialog asks `thing`, go 3 ms, toast "fix-build is ready"; Delete asks "Delete the recipe “fix-build”? It goes for good…" (2 ms) |
| 35 | Triggers: new, fires, on/off, delete | formula, UI, CLI | pass (H4, H13 fixed) | The Settings switch sends `automations.trigger.switch` (3 ms) both ways; `wisp diff` "1 unchanged"; export keeps `project: proj`. `trigger off/on/rm` as in H13; `rm nosuch` exit 1 |
| 36 | Notes, Fresh thread | CLI | pass | `notes --set goal=…`; `fresh -d`: no `--resume`, the Notes in its brief |
| 36b | The agent keeps its Notes | run MCP | pass (was not re-run in H) | `wisp_notes {"set":{"decisions":"use SQLite"}}` → "Updated decisions (rev 1)."; `wisp notes` shows it |
| 37 | User MCP (stdio): launch | MCP | pass | 301 tools; `wisp_launch {goal:"never"}` launched |
| 38 | HTTP MCP: launch | MCP | pass, with finding I10 | `wisp_launch {cwd, agent, goal:"green-build"}` → worktree, "met after 1 round". After the app's In place pick it ran in place (I10) |
| 38b | HTTP MCP: every tool takes its fields | MCP | **partial (D6)** | `factory_set`, `resources_plan`, `resources_read` fixed. 5 tools now exposed by HS6 take only `{args}`: `automations_task_remove/run/set`, `channels_extra`, `channels_update`. `channels_update {"bogus":1}` → "wisp: the  channel isn't on" |
| 39 | A task launched by an agent | run MCP | pass | `wisp_launch {formula:"ok.yml", agent:"a"}` from a worktree task → codex, auto, its own worktree |
| 40 | Done on a running task | UI | pass | Toast "Still working — stop it first or let it finish."; 4 ms |
| 41 | Phone 390 (Chrome dark, WebKit light) | UI | pass | Launcher sheet 390 wide, no horizontal overflow |
| 42 | Immediate feedback | UI | pass | Every measured action 1–22 ms (see below) |
| 43 | `wisp help runtime answer` | CLI | pass | Each field has words |
| 44a | Try from the UI | UI (Chrome) | pass (was not re-run in H) | Compare with another agent… → Claude Code Opus; send 3 ms; "… is ready (2 runs)" |
| 44b | A held turn's "Trust and start" | CLI | pass | "◷ Held before it started: This repo asks to run its own programs…"; `held <id> start` → ran |
| 44c | Start from an issue | UI (Chrome) | pass (was not re-run in H) | Picker 5 ms ("#7 Sum is wrong for negatives · bug", "#8"); pick 5 ms; "Fix #7: Sum is wrong for negatives" in a proj4 worktree |
| 44d | Ask to fix (failing check) | UI (Chrome) | pass (was not re-run in H) | Strip "PR #3 Open · 1 check failing"; PR tab's Ask to fix 1 ms fills the reply box (77 px) with "The `test` check failed on PR #3 (…/runs/42/job/4242). Find out why, fix it and push." |
| 44e | A held turn's "Skip" | CLI | pass | "● Skipped before it started · This repo’s hooks, servers or setup weren’t trusted." |
| 44f | A held turn's "Run without them" | CLI | pass | `--setting-sources user --strict-mcp-config` |
| 45 | Long thread (1 + 25 quick follow-ups) | CLI | **fail (I3, new)** (was not re-run in H) | All 25 answered "✓ queued", yet 3 turns were `stopped` after 51–63 ms and the order was I, 1, 9, 15, 21, 2, 3 … (twice) |
| 46 | Schedule triggers over time | formula | pass | In ~2 min `tick` ran 2 times, `words` 3 times; both then off / removed |
| 47 | A project's worktree setup | CLI | pass | `setup-ran` in the worktree; "◷ Held before it started: setup `touch setup-ran; exit 3` failed: exit 3" |
| 48 | Server warnings | — | pass | 2 WARN in the round: the failing setup (row 47) and my own agent `terminal.open` probe (refused) |
| 49 | A long follow-up in the reply box | UI (Chrome) | pass (was not re-run in H) | 34 → 120 px at 1280, 34 → 186 px at 390, no sideways scroll; empty again: 34 |
| 50 | Named goals in a formula | CLI | pass | apply; `goal declared` 33 lines (the diff shows row 34's edit of fix-build, as expected) |
| 51 | `wisp run <task>` with `tasks.default` | CLI | pass (H11 fixed), with finding I8 | `run fix-build`; `run nosuch`/`run default` name declared tasks, recipes and workflows |
| 52 | `--goal-text` | CLI | pass | Launched with the words |
| 53 | `wisp goal set <task> <name>` | CLI | pass (H6 fixed) | "no goal named nevr: did you mean never? goals has goal-01 … goal-08 and 24 more (wisp goal declared lists them)" |
| 54 | A goal's round limit | CLI | pass | "The goal isn't met after 2 rounds. … Keep going?" |
| 55 | `wisp_launch`'s `goal` | stdio, run, HTTP MCP | pass | All three take it; a run lists 6 `goal_*` tools |
| 56 | Goal… with 32 goals | UI (Chrome) | pass | Flyout y=8, 545 tall; hints 2 lines (35 px) |
| 57 | `/goal` in the reply box | UI | pass | `/goal green-build` + one Enter → `goal.set` (12 ms), set as the declared `green-build` |
| 58 | `wisp "…" --formula f.yml --agent critic` | CLI | pass | codex, gpt-5, high, "Grace, the critic." in its instructions, in a worktree. team-bad.yml refused before launching |
| 59 | `wisp run <task> --formula f.yml` | CLI | pass (H3/G2 fixed), with finding I11 | "The goal can't be judged: critic answered without a verdict (MET or NOT MET) · The work isn't sent back for that." asked once |
| 60 | Task files: a file, inline, a list, `module.x` | CLI | pass | a, b, c and g started; d, e, f and h refused with their reasons |
| 61 | `wisp_launch`'s `formula` | run MCP | pass | Another worktree's file, `../../secrets.json` and an absolute path refused (I14 for the double error) |
| 62 | New task › Formula or task file… | UI (Chrome) | **fail (I4, new)** (H5's pill fixed) | The pill now reads "critic · Codex / gpt-5 · high", but with the bar's Claude model on Opus, `runtime.create` carries `"model":"opus"` and codex runs `-m opus` |
| 63 | validate names unknown references | CLI | pass | `green-bild`, `plannr`, `on: every minute`, and "not a wisp formula (a formula starts with `wisp: 1`)" |
| 64 | A child never exceeds its parent | run MCP | pass (but see I1) | Plan parent: `wisp_launch {mode:auto}`, `runtime_create {mode:auto}` and `{factory, mode:auto}` refused; no mode → plan |
| 65 | Agents' writes | run MCP | pass | `connectors_add`, `nosuch_tool` → "Unknown tool: X"; `wisp_crew_remove {"name":"Zed"}` → "…: it takes member" |
| 66 | Setup cards: ask, Allow, Deny, typed | run MCP, CLI, UI | pass | `wisp reply <id> Allow` → "✓ You allowed: add crew member Zed". Typed in the app → `runtime.answer` with `askId` (7 ms); the agent got "They said: “no, not that one”." |
| 67 | Setup cards on the phone | UI (390 dark, WebKit) | pass | The card fits; Allow → `runtime.answer`; the connector was added (as `qa_phone`: I16) |
| 68 | Factory: the built-in default | CLI | pass | "▦ default: ? spec · …"; words instead of Approve revised the spec |
| 69 | Factory: a declared one, looping | CLI, fake gh | pass | `strict`: round 3, review CHANGES once, ship "Nothing to ship". `shipit`: PR #1, Merge → "✓ ship · PR #1" |
| 70 | Factory names | CLI | pass | "no factory named nosuch: factories has default, gated, shipit, slowcheck, strict, tight"; `wisp export factories` |
| 71 | Factory: Stop | CLI, UI | pass (G15 partly, NEW #1 fixed) | UI: band ⋯ › Stop the run 4 ms, confirm 3 ms; the ask card is gone, and after a reload; band "Checks · Stopped"; CLI killed the running `sleep 120` |
| 72 | Factory: delete a run | CLI | pass | `runtime delete <run>` → gone from `ls --all`, stations too |
| 73 | Factory: restart mid-run | CLI | pass | Server stopped during `sleep 120`: the check died with it once the server exited; on start the checks ran again ("✓ build · ● checks") |
| 74 | Factory started by an agent | run MCP | pass | Plan parent's `wisp_launch {factory:"gated"}`: the run is `plan`; "Run `true` on the built code? Run / Skip / Stop"; `factory_stop` is no agent tool |
| 75 | Factory in the app | UI (Chrome; 1280, 390 dark) | pass | The band lists every station; no overflow at 390 |
| 76 | Terminal context in tmux | CLI | pass | A closed pane's command isn't listed; `GH_TOKEN=…`, npm `:_authToken`, openssl `pass:` all `<secret>` (HS9) |
| 77 | An agent delegates to a subagent it names | run MCP | **pass (H1 fixed)** | `delegate {"tasks":[{"agent":"codex","name":"tester",…}]}` ran; the same name continues the same task; `wait:false` → "Delegated: codex (id …)"; `frob` → "Nothing was delegated. - frob: no harness frob: name one of …" |
| 78 | A misspelled launch flag | CLI | **pass (H2 fixed)** | `--mdoe`, `--agnet`, `--bogus`, `--at`, `-x` refused, exit 1, nothing launched; `--modle` → "did you mean --mode or --model?"; `--mode opus` refused; `-- --mdoe` launches |
| 79 | Factory with a real change: PR, merge | CLI, fake gh | pass | Row 69 (`shipit`) |
| 80 | Restart with an open setup card | CLI | pass | "◐ Stopped · wisp restarted while this was running, so its card asking to add crew member Eve was taken back: nothing changed." |
| 81 | A misspelled verb after a command word (new) | CLI | **fail (I2)** | See I2 |
| 82 | An agent's MCP: `runtime_exec` (new) | run MCP | **fail (I1)** | See I1 |
| 83 | `trigger off / on / rm` (new) | CLI | pass | "✓ trigger tick is off: it fires no more (wisp trigger on tick switches it back on)"; "✓ removed trigger words"; a crew trigger refused (I15) |
| 84 | Factory stop takes its question back; a late answer (new) | API, CLI | pass, with finding I5 | `runtime.answer` with the old askId → conflict "That question isn't open any more …: nothing was sent." |
| 85 | Saved model/effort for CLI/MCP/crew tasks (new) | CLI, MCP, crew | pass | Saved `{codex: gpt-5/high, claude: opus/low}`: `wisp "…"` → codex gpt-5 high; `--agent claude` → opus low; `--model sonnet` → no level; `--effort high` wins; HTTP `wisp_launch` → `-m gpt-5 model_reasoning_effort="high"`; crew job/check-in → codex gpt-5 high |
| 86 | Auto-off schemas (new) | MCP | pass | No tool in the user's or an agent's list offers `auto` as a harness (only mode `auto`) |
| 87 | Automations/settings/worktrees/channels over MCP; what an agent may call (new) | MCP | pass | The user's harness has `automations_trigger_switch/remove`, `settings_*`, `worktrees_*`, `channels_*`. An agent: `automations_trigger_test` answers; `automations_trigger_switch`, `settings_set`, `worktrees_view`, `channels_status` → "Unknown tool" |
| 88 | A subagent can't make its own (new) | run MCP | pass | As the subagent: "Nothing was delegated. - grandkid: A subagent can’t have subagents of its own." |
| 89 | Stray flags with words and quotes (new) | CLI | pass, with finding I9 | `wisp "I quoted: fix the --mode flag parsing"` launches; `-m plan` is `--mode plan` |

**Counts:** 99 rows (H's 90, plus 81–89).
- **Pass:** 94, of which 13 carry a lesser finding (rows 8, 29, 31, 38, 51, 59, 61, 64, 67, 71, 83, 84, 89).
- **Partial:** 1 (row 38b, D6).
- **Fail:** 4 (rows 45, 62, 81, 82).
- **Not tested:** 0. Every row H marked "not re-run" was run this time (all 15 pass, except 45).

**Feedback (ms, pointerdown or keydown → first DOM change).**
- Launch 2; formula send 3; delegate send 4; compare send 3; issue picker 5, pick 5.
- Fork menu 9, Fork 20; Rerun 9 / 22; Side 2, its send 15; btw 5; Done 4.
- Send now 7; Start later send 8; queue cancel 4 / 1.
- Trigger switch 3; typed answer 7; review click → "Reviewing" 54.
- Factory Stop the run 4, confirm 3; Restore 2 / 4; worktree remove 19; Merge 3 / 3.
- Tint pick 3, save 12; recipe run 3, delete 2; Ask to fix 1; `/goal` 12.
- Bring in pane: shown 624 ms after opening Settings › Worktrees.

## Audit H's findings, re-checked

| # | Finding | Status at 1ea3b84f | Evidence |
|---|---|---|---|
| H1 | `delegate` refuses the shape its schema gives | **fixed** | Row 77; also on a task with no team. A subagent can't make its own (row 88) |
| H2 | A misspelled flag is folded into the prompt | **fixed** (flags) | Row 78. The same class is open for verbs: I2 |
| H3 | A judge without a verdict sends the work back every round | **fixed** | Row 59: asks once, sends nothing back |
| H4 | The trigger switch rewrites `project` as its path | **fixed** | CLI and Chrome: `automations.trigger.switch`, `diff` unchanged, export `project: proj` |
| H5 | The bar says Claude Code while the file's agent runs | **fixed** (the pill) | "critic · Codex / gpt-5 · high". But the send still carries the bar's model: I4 |
| H6 | Refusals list every goal | **fixed** | "did you mean never? … goal-08 and 24 more (wisp goal declared lists them)" |
| H7 | `later cancel` says ok for nothing | **fixed** | `later cancel nosuchid123` → "Nothing is scheduled on …", exit 1; `plans` prints whole ids |
| H8 | CLI verbs print a field dump | **fixed** (the listed verbs) | factory stop, crew checkin/assign/update, review maybe, later cancel say sentences. Others still dump: I12 |
| H9 | Help with no words or the wire shape | **partly fixed** | `review maybe`, `later cancel-plan`, `later plan` (description and field words), `crew assign` and `factory stop` openings fixed. Left: `crew assign --id/--prompt/--attachments/--kit/--confirm-limit` have no words; `later plan --kind JSON`, `--at JSON` show the type JSON for a word and a time; `later plan`'s example is a JSON body |
| H10 | `rerun -d` prints the multi-run hint | **fixed** | "wisp attach <id> follows it" |
| H11 | `wisp run`'s miss names only recipes | **fixed** | "no declared task, recipe or workflow named nosuch: `wisp recipes` lists the declared tasks and recipes, `wisp workflows list` the workflows" |
| H12 | `crew ls` doesn't mark a paused member | **fixed** | "paused (wisp crew resume to bring them back)" |
| H13 | The CLI can't switch a trigger off or remove it | **fixed** | Row 83; `wisp help automations` lists `trigger switch` and `trigger remove` |
| G2 | A formula's judge loops | **fixed** (= H3) | Row 59 |
| G15 | Factory words | **partly fixed** | Fixed: `wisp show` "· PR #1"; band "Checks · Stopped"; the app's list "Factory · gated · Stopped". Left: `wisp ls` still prints state **review** for a run you stopped (`01a1faa27cca…  review  Factory · gated`) |
| E6 | Bring in never shows a stray | **fixed** | Row 24: 624 ms with 170 worktrees |
| E11 | Generated help shows developer notes | **partly fixed** | The listed 24 are gone. Left (a sweep of 734 `--help` pages): `machines route` "A v2 command … (plan §7.2.1)"; `runtime setup ask` "`wisp_runtime_core::setup::WRITES`"; `runtime create --subagent-model` "`ext["wisp.runtime"].variant` … `HarnessInfo::`"; `runtime default-model` "(owner's delegate 2026-11-18)"; `crew assign --from` "Set by the caller (`crew:assign`): the runtime passes the lead it runs"; `runtime exec` "no agent capability set names it" (wrong too: I1). 75 pages still open with the wire name (`` `goal.set`: …``, `` `later.edit`: …``, `` `stuff.save`: …``) |
| D6 | MCP tools without an argument schema | **partly fixed** | The 3 are fixed; 5 others took their place (row 38b) |

**H totals:**
- H1–H13: 12 fixed, 1 partly fixed (H9).
- G2, G15, E6, E11, D6: 2 fixed (G2, E6), 3 partly fixed (G15, E11, D6).

## New findings

### I1. [major] An agent runs any shell command on the server through its MCP `runtime_exec`, out of Plan and out of Codex's read-only sandbox
- **Steps.**
  - Launch `wisp "I codex plan" --mode plan --agent codex` in proj (Plan runs in place; codex gets
    `sandbox_mode="read-only"` and the wisp MCP with `default_tools_approval_mode="approve"`).
  - With that run's own MCP session and token (from its argv), call
    `runtime_exec {"id":"<its own task id>","command":"echo from-read-only-codex > ESCAPE.txt; git status --short"}`.
  - The same from a Claude Plan task (`I plan exec probe`): `echo planpwn > PWNED.txt; pwd`.
- **Actual:**
  - `{"code":0,"output":"?? ESCAPE.txt …"}`: the file is in the user's checkout (/tmp/qaI-orch/proj). The command ran
    on the server, as the user, outside the harness's sandbox, and no one was asked.
  - Claude Plan: `{"code":0,"output":"/private/tmp/qaI-orch/proj\n"}` and PWNED.txt in proj.
  - `runtime_exec` is in every run's tool list (200 tools). Its own description says "The user's click only: no agent
    capability set names it." Only another task's id is refused ("an agent's token speaks for its own task only").
  - Row 64's rule (a child never exceeds its parent) is moot: the parent itself exceeds Plan.
- **Expected:** `runtime.exec` is the user's click on an `actions` block. No agent capability names it; it is neither
  listed nor callable from a run's MCP (default deny, ADR-0112), at least never above the task's mode.
- **Evidence:** tasks 01a1fa8ca3ae72eba74ed150674421b5 (codex Plan) and 01a1fa8c740f77faaa686db9ddafa010 (Claude
  Plan); state/run-codexplan.json, state/run-plan.json; `node rcall.mjs …`. Files removed afterwards.
- Also: an agent may `runtime_delete` its own task. It did (01a1fa8bcac6…), and its worktree was left behind.

### I2. [major] A misspelled verb after a command word launches a Full-auto task with those words
- **Steps.** From proj, run each of these:
  - `wisp crew pasue Zed` (and `wisp crew paus Zed`)
  - `wisp trigger of tick`
  - `wisp goal sett abc never`
  - `wisp later cancle x`
  - `wisp factory stp x`
  - `wisp held x strat`
- **Actual:**
  - Each prints "◌ launched: crew pasue Zed (<id>)" and runs an agent in Full auto on the default harness with the
    typo as its prompt.
  - Other words refuse the same slip: `wisp version foo` → "✗ wisp version has no foo … To start a task with these
    words: wisp -- "version foo""; `wisp task spec` → "needs more".
  - H2 (flags) is fixed, but the verb slip is the same harm: a user who meant to pause Zed started an agent.
- **Expected:** a known command word followed by an unknown verb is refused with the close verb ("did you mean
  pause?") and the `wisp -- …` way out, as `version` does.
- **Evidence:** tasks 01a1fa925a7d732493adf46a31a0b1e5 (`crew pasue Zed`), 01a1fa925b4c73f3936d1311089e2ec7,
  01a1fa925c8b7743805449e9c688f24a, 01a1fa925d8a7619a7bd2f151f780f8d, 01a1fa925e54730bac755a34adecac66,
  01a1fa925fc373809f4c08892a8e3442, 01a1fa92608d77f9ba8fcc42a8f0ac83.

### I3. [major] Queued follow-ups are stopped and run out of order
- **Steps.** `wisp "I row45b long thread" --agent claude -d`, then 25 × `wisp reply <id> "g$i"`, one after another.
- **Actual:**
  - Each reply answers "✓ queued".
  - `runtime.turns`: I row45b, g1, g9, g15, g21, g2, g3 … g8, g10 … (startedAt confirms the order).
  - g1, g9 and g15 are **`stopped`** after 51–63 ms; nothing reran them. Three of the user's follow-ups never ran to
    the end, silently, and the rest ran in a different order from the one sent.
  - The same with f1–f25 on a second task (f1, f9, f15 stopped, same order).
- **Expected:** queued follow-ups run once each, in the order sent, none interrupted (G and F: "All 26 turns ran, in
  the order sent").
- **Evidence:** tasks 01a1fa96fd6672d8b2d8cca0677e6892 and the first row-45 task (state/r45.id).

### I4. [major] New task with a task file: the file's agent runs on the bar's model
- **Steps.**
  - In New task, pick Claude Code and Opus once (so the bar remembers Opus for Claude Code).
  - + › Formula or task file…, pick `proj/.qa/a.task.yml` (`agent: critic`, `formula: ./team.yml`; critic is codex,
    gpt-5, high). Send.
- **Actual:**
  - The pill reads "critic · Codex / gpt-5 · high" (H5's fix).
  - `runtime.create` carries `"agent":"critic"`, the formula's critic (`"model":"gpt-5"`), **and** `"model":"opus"`.
  - The task's model is `opus`, and codex runs `-m opus model_reasoning_effort="high"`: a Claude model on Codex,
    which a real Codex refuses.
  - The CLI (`wisp run -f a.task.yml`) runs gpt-5.
- **Expected:** what the pill says runs: the file's model (gpt-5), not the bar's.
- **Evidence:** tasks 01a1faa4aa34799781cfc8da1a3f650c and 01a1fab140… (reproduced); rec/codex.jsonl; the
  `runtime.create` body logged by g-formula.mjs.

### I5. [minor] `wisp reply` to a stopped factory run says "✓ sent", then a turn asks for a question that isn't there
- **Steps.** A `shipit` run waiting on "Approve the spec?"; `wisp factory stop <run>`; then `wisp reply <run> Approve`.
- **Actual:**
  - `runtime.answer` with the old askId is refused, as the fix says ("That question isn't open any more…").
  - But `wisp reply` prints "✓ sent". The run then shows "✗ Not started · This task is a factory run: its stations
    do the work. Answer its question here, or open a station's task…", and there is no question.
- **Expected:** "This factory run is stopped: nothing was sent" (exit 1), as for the askId path.
- **Evidence:** tasks 01a1fa917aa876e88fe2afcda6cc6d75 and 01a1fa91b9d4721580ff826840672bd0.

### I6. [minor] An agent's "add a project" card for a folder that doesn't exist asks first and fails after Allow
- **Steps.** An agent calls `runtime_projectAdd {"path":"/tmp/qaI-orch/proj9start"}` (no such folder).
- **Actual:**
  - The task waits on "Claude Code wants to add the project /tmp/qaI-orch/proj9start · Allow / Deny".
  - Allow → "✓ You allowed: add the project … (it didn't go through: No folder at …)".
- **Expected:** refused to the agent before any card ("No folder at …"); the user is asked only for something that
  can happen.

### I7. [minor] `crew create --on` is ignored, and `crew update --on` says it changed when it didn't
- **Steps.**
  - `wisp crew create --name Bob … --on task.done` → `crew.get` has `"on":[]`.
  - `wisp crew update bob --on task.done` → "✓ Bob: on changed"; `on` is still `[]`.
- **Actual:** Bob never wakes until `--wakes '[{"on":"task.done"}]'` is used. The help says `--on A,B` is "read as
  `wakes`".
- **Expected:** `--on` sets the wakes, or is refused with words; never "changed" for no change.

### I8. [minor] `wisp run <task> --formula F -d` is refused
- **Steps.** `wisp run release --formula ../f/team.yml -d`.
- **Actual:** `✗ release takes no values: "-d" is left over (run a prompt: wisp "…" --formula …)`, while `wisp run
  fix-build -d` works and `-d` is a documented launch flag.
- **Expected:** `-d` (and `--wait`) work the same for every `run` form.

### I9. [minor] `--effort` and `--model` take anything; `--agent auto` with Auto off runs the default silently
- **Steps.**
  - `wisp "I effort" --effort banana`
  - `wisp "I model plan" --model plan`
  - `wisp "I agent auto" --agent auto` (Auto off)
- **Actual:**
  - All three launch in Full auto.
  - Claude gets `--effort banana` and `--model plan`, which a real Claude Code refuses at run time.
  - `--agent auto` silently runs claude. HS7 refuses `agent: auto` for a crew member ("Auto is off…"), and HS3 refuses
    `--mode opus`, but not these.
- **Expected:** refused before launching with words, as `--mode opus` is ("--effort takes low, medium, high…";
  "plan is a mode: --mode plan"; "Auto is off: name a harness or leave it out").

### I10. [minor] After the app's "In place" pick, a user's MCP launch runs in place while the CLI makes a worktree
- **Steps.**
  - New task › Where › In place, and send once. This sets the server's `worktrees.worktree` to `off`.
  - `wisp_launch {cwd:"/tmp/qaI-orch/proj", agent:"claude", …}` over `/api/mcp`.
  - Then `wisp "I where check"` from the CLI.
- **Actual:**
  - MCP: "Launched … in /tmp/qaI-orch/proj" (in place, in the user's checkout).
  - CLI: a `proj-<id>` worktree.
  - One pick in the app silently changes where every MCP-launched task runs.
- **Expected:** CLI and MCP agree. The app's pick is the launcher's default, not a server-wide switch; or, if it is,
  it says so.

### I11. [minor] `wisp run <task> --formula F --wait` returns before the goal is judged
- **Steps.** `wisp run release --formula ../f/team.yml --wait`.
- **Actual:** prints "review <id>" and exits 0. Seconds later the task is `waiting` on "The goal can't be judged…".
- **Expected:** `--wait` waits until the task is done or asks something, judging included (as `wisp wait` says:
  "until it finishes or asks you something").

### I12. [smell] Field dumps and odd words left on CLI verbs
- `wisp goal set <id> green-build` prints "judges / name agent / … / next … / round 0 / set just now / status working
  / text … / vote majority".
- `wisp review view <id>` and `wisp crew create …` print every field ("created in a moment" for a member just made).
- `wisp trigger log tick` prints raw epoch ms ("1795206364108 done → …").
- `wisp crew update Zed --paused false` → "✓ Zed: back: not paused".
- `wisp factory stop <a task that isn't a factory run>` → "This factory run isn't running."

### I13. [smell] Help fields without words
- `crew create`: `--checkin`, `--name`, `--role`, `--avatar`, `--color`, `--brief`, `--cwd`, `--machine`,
  `--sandbox`, `--agent`, `--model`, `--effort`, `--mode`, `--parallel`, `--paused`, `--notebook` have no words.
  It opens with "`checkin: true` starts a first check-in at once."
- `runtime fork`: `--id`, `--title` have no words.
- The rest is E11's remainder above.

### I14. [smell] Refusal words
- `wisp validate`: "no goal named green-bild: goals has goal-01 … and 24 more" has no "did you mean green-build?",
  which the CLI's refusal now has (H6).
- Task-file refusals print unnormalized paths: "/private/tmp/qaI-orch/proj/../f/e.task.yml".
- An agent's formula from another worktree is refused, then a second error is appended: "…(no absolute paths, no
  ..); the task: no agent named a: no agents are declared…".

### I15. [smell] Triggers
- `wisp trigger rm crew.bob.1` / `off` → "trigger crew.bob.1 comes from wisp.crew: switch it there". It doesn't say how
  (`wisp crew update bob --wakes '[]'`).
- `automations_trigger_test {"name":"tick"}` on an off trigger answers "its next run, Fri … 21:01". It doesn't say
  the trigger is off.
- Scheduled runs are titled with the trigger's name ("tick", "words"), not their prompt.

### I16. [smell] Connector names
An agent's `wisp_connector_add {"name":"qa-phone"}` is saved as `qa_phone`, and no one is told. The tool answer says
"fails: Http://127.0.0.1:9/mcp: error sending req…" (capital H, raw error).

### Note (not counted): what the user's harness can switch off
HS6 gives the user's harness (`wisp mcp`, HTTP MCP) `settings_set` and `worktrees_trust`.
`settings_set {"key":"worktrees.projectAsk","value":"off"}` turns off "Ask before trusting a new project's setup
scripts" with no confirmation (checked, then set back on). This is by design (ADR-0040: the user's harness acts for
the user). But any session in the user's Claude Code, a prompt-injected one included, can now switch off wisp's own
safety asks. Worth a decision: are some settings app-only?

## Count

16 new findings (I1–I16):
- **Major (4):** I1, I2, I3, I4
- **Minor (7):** I5–I11
- **Smell (5):** I12–I16
- **Blocker:** none (I1 is close: it breaks Plan's promise and Codex's sandbox)

H's 13: 12 fixed, 1 partly fixed (H9). Of G2, G15, E6, E11 and D6: 2 fixed (G2, E6), 3 partly fixed (G15, E11, D6).

**Open after this round: 20**: I1–I16, plus H9, G15, E11 and D6 (partly fixed). I2 and I4 also carry on from H2 and
H5.

**NEW findings: 16.**

## Cleanup

- **Ports and homes.** Only :4974, /tmp/qaI-orch and the tmux socket `qaIo` were used. Nothing touched ~/.wisp*,
  :4477, :4800, :4791, :4329 or :5180. No real harness and no paid run; only fakes.
- **Stopped.**
  - The server: `kill <pid>` from `lsof -ti tcp:4974 -sTCP:LISTEN` (pid 95000, after two deliberate restarts for
    rows 73/80). Afterwards :4974 was free.
  - tmux: `tmux -L qaIo kill-server`.
  - No pkill or killall was used.
- **Children.** After the stop, no process from /tmp/qaI-orch was left (no fake, no `sleep 120` check).
  - The gate files (idone, isub) were released.
  - Triggers: `tick` off, `words` removed. Bob's `crew.bob.1` wake was left on, in the stopped scratch home.
  - No later plan is left (the "I13 later ui" plan was cancelled).
  - The factory runs were stopped or ended.
  - The probe files written through I1 (PWNED.txt, ESCAPE.txt, pwned-exec) were removed. proj's fake-written
    `notes.txt` was cleaned.
- **Browsers.** Every Playwright browser was closed in its script's `finally`; none is left running.
- **Kept for repro.**
  - /tmp/qaI-orch: scratch home, projects, formulas, `ui/` scripts (H's plus `i-*.mjs`), `rcall.mjs`, the QA wrapper,
    the copied binaries and notes/log.md.
  - /tmp/wisp-target-fixer, as the shared build cache.
  - /tmp/qa-keep/shots-qaIo/, the screenshots.
- **Disk.** 73 GiB free at the end.
