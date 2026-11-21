# Audit I: agent and workspace setup (re-check of audit H setup, plus what landed since)

2026-11-21 · worktree `wisp-8fb64ec939b0` at `54ac84e9` · findings only, no repo file edited except this one.

> **Build note:** the binaries (`wisp`, `wisp-fake-agent`, `wisp-fake-gh`, `wisp-grid`, `wisp-grid-hostd`, dev profile)
> and the web bundle were built from the worktree at 00:28–00:30. The worktree differed from HEAD only in
> `design/v1-reference/` (another session's screenshots), so both are HEAD's.

**Setup** (H's method, in a fresh `/tmp/qaI-setup`; H's scripts copied from `/tmp/qaH-setup` with paths moved, same ports):
- **Build:** `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, binaries in `/tmp/qaI-setup/fake`, the fake agent copied under
  each harness name in `bin/`, fake `gh` and fake `npm`. Web: `vite build --outDir /tmp/qaI-setup/web`.
- **Server:** `wisp serve --port 4991`, `WISP_HOME=/tmp/qaI-setup/home`, `HOME=/tmp/qaI-setup/realhome`, every
  `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` on the fakes, `FAKE_RECORD` on, `FAKE_LOGIN_MS=4000`, `WISP_NPM` fake,
  `DO_NOT_TRACK=1 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`.
  - New this round: `FAKE_GATE=/tmp/qaI-setup/gates`, so a prompt starting `gate <name>` keeps its turn running (a lead
    for `delegate`, a task holding cards) until the file appears. Other prompts run as before.
  - Restarted 8 times: the restart check (107), the stale-proxy cause (IS6) twice, the sign-in cut by a restart (124),
    HS12 three times, and a guard question across a restart (IS2).
- **Catcher on `:4999`:** H's (PostHog, Slack, Discord, Linear, ntfy with the reply queue, the fake Telegram Bot API).
  Telegram taps are fed as `callback_query` updates (`tgtap.py`), messages as `message` updates (`tgmsg.py`).
- **Cloud:** `wisp-grid dev --port 4992 --data /tmp/qaI-setup/grid`, Claude and Codex fakes only, spy scripts first
  on its `PATH`. Stopped and started once for row 51.
- **Desktop self-update:** a fresh scratch `/tmp/qaI-setup/pkg/wisp.app` (HEAD's binary, `Info.plist` 2.0.0) on
  `:4996` with its own home and `WISP_UPDATE_FEED=http://127.0.0.1:4995/releases`. The feed on `:4995` serves v2.0.1, a
  zip of a stub `wisp.app` that only logs that it started.
  - **Deviation (as in H):** that one server ran with `WISP_UPDATE_CHECK` unset for rows 8, 111, 132, 167 and 168
    (with it at `0`, wisp checks nothing, so those can't run). Its feed and fake `npm` are local; HS10 and row 206 ran
    with it at `0`.
- **Test data:** fresh clones of G's `repo`, `repo2`, `repo3`, `repo4` (remote removed), `repo3-wt` (linked worktree
  of `repo3`, made before start) and `repo3-wt2` (made after, for HS4/GS1). H's `v1home`, `v1b` and `myplugin` with
  paths moved.
- **UI:** Playwright from `/tmp/pw` with system Chrome: a persistent headless Chrome on CDP `:4994` (profile
  `/tmp/qaI-setup/chrome-prof`), 1280/1440 and 390; fresh Chrome and WebKit for the layout sweep, first loads and the
  Changes pane, light and dark, each closed in `finally`. Requests to `trywisp.dev`, `example.dev` and
  `example.invalid` were aborted in the browser.
- **Ports:** 4991, 4992, 4994, 4995, 4996, 4999. The dev grid's box listened on an OS-picked loopback port, as in H.
- **Screenshots:** `/tmp/qaI-setup/shots/` (`i-*`, 134 files).

> **Outbound requests and other things to know.**
> - No real harness ran: the grid's spy log is empty. `npm` is the fake (3 `npm view` calls, scratch app only).
> - SSH only went to `192.0.2.1`, `192.0.2.7` and `192.0.2.9` (TEST-NET), which time out.
> - The Plugins row's `https://example.invalid/x.git` made the server try a `git clone` (the `.invalid` name doesn't
>   resolve). Nothing reached grid.trywisp.dev, github.com, 4477, 4800, 4791, 4329 or 5180. `npm run doctor` was not
>   run (it probes 4800 and 4477). The catcher got 0 PostHog calls.
> - Every `wisp` call ran with a `/tmp/qaI-setup` `WISP_HOME` and `HOME`. No real credential was read. The agent token
>   for agent-side calls is the one the scratch server gave its fake agent (read from the fake's recorded
>   `--mcp-config`), as in G and H.
> - **My slips, not counted:** zsh passed a quoted `"machines discover"` (and 25 like it in two sweeps) as one argument,
>   which is a prompt, so 26 fake tasks were launched. Re-run with the words split, each is a command (row 45). The
>   15 sentences of the verb sweep (row 231) launched fake tasks on purpose. All are finished fakes.

## Counts

| | |
|---|---|
| Actions checked | 243 (H's 208 + 35 new) |
| Pass | 223 |
| Fail (a finding) | 16 |
| Not tested | 4: 32 sandboxes (a dev grid makes none), 96 `npm run doctor` (probes 4800/4477), 122 paste a code (the fake has no paste step), 125 a sign-in that times out (15 min) |
| H's not-tested rows run this round | 28 (8, 13, 37, 41, 63, 89, 95, 102, 104, 111, 115, 121, 123, 124, 126, 128, 132, 137, 139, 142, 144, 147, 156, 159, 160, 163, 172, 173) |
| Audit H setup findings | 14 re-checked (HS1–HS14) + GS2, GS6: 13 fixed, 1 partly (HS7 = GS6); GS2 fixed (IS1 beside it) |
| **New findings** | **13 (IS1–IS13): 0 high, 7 medium, 6 low** |

(IS: "I, setup". `docs/qa/audit-I-orchestration.md` already uses I1–I16.)

## Audit H setup findings, re-checked

| # | Result | How checked |
|---|---|---|
| HS1 | **Fixed** | In `/tmp/qaI-setup`, `wisp import v1home --yes` → "✓ applied 14 changes · ✓ 1 task imported, 0 unchanged"; `wisp ls --all`: `v1sess0001 · old-app`. Again: "0 tasks imported, 1 unchanged", exit 0. `import nosuchdir` → "✗ No folder at /private/tmp/qaI-setup/nosuchdir", exit 1. A home with a broken `sessions/s9.json` → "✗ not imported: s9.json: s9.json is not valid JSON …", exit 1. `v1b` (another task) → 1 imported |
| HS2 | **Fixed** | `wisp help trigger` = `wisp trigger --help`: "wisp trigger add\|ls\|list\|rm\|remove\|off\|on\|fire\|test\|log\|resume\|approve\|sources …". `wisp triggers frob` lists `off`, `on`; bash completion's `triggers)` line has them |
| HS3 | **Fixed** | `--modle opus` → "did you mean --mode or --model?"; `--mode opus` → "✗ --mode takes plan, edits or auto, not opus (a model goes after --model: --model opus). Nothing was launched"; `-x` refused; task count unchanged |
| HS4 | **Fixed** | `repo3` and `repo3-wt` both projects, `repo3-wt2` made after start → Worktrees: one offer, "1 worktree from git · In repo3 · 1 with uncommitted changes" at 500 ms; Bring in made task `qa-wt2` |
| HS5 | **Fixed** | Picker → Claude Code · Opus · Max (`runtime.launch` models/efforts). In `repo2`, `wisp "…" -d` → rec `--model opus --effort max`. `--model sonnet` → no `--effort`; `--effort low` → `--model opus --effort low`. The user's `wisp_launch` and `wisp crew wake "Qa Reviewer"` → `--model opus --effort max` too |
| HS6 | **Fixed** | `wisp mcp` (301 tools) and `/api/mcp` (297) list `automations_trigger_switch`/`_remove`; `{name: qa-nightly, on: false}` → `{"ok":true}`, `wisp trigger ls`: off. An agent: not listed, raw `automations.trigger.switch` → 403 "agent:session may not automations:write" |
| HS7 (GS6) | **Partly** (IS8) | Agent's schemas: no `auto` harness in `wisp_crew_add`/`_edit`, `wisp_task`, `delegate`, `wisp_launch`. A crew member added with no harness → `claude`. A formula's crew `agent: auto` refused. Left: see IS8 |
| HS8 | **Fixed** | ntfy on, the agent's `wisp_crew_add` → notification with no actions: "…\n\nThis changes your setup: answer it in wisp (Allow or Deny on the task's card)." |
| HS9 | **Fixed** (IS7 beside it) | All 16 of H's lines (and npm `=`, pnpm, `--password-stdin <<<`, `echo … \| docker login --password-stdin`, `sudo docker login -p`, `NPM_TOKEN=…`, `Authorization: Bearer`, `mysql --password=`, `redis-cli -a`, `wget --password=`, `kubectl --from-literal=password=`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`) masked; `--with-token <<< <secret>` keeps `<<<`; `X-Api-Key: <secret>` keeps the name. Three shapes still leak (IS7) |
| HS10 | **Fixed** | Scratch app, `WISP_UPDATE_CHECK=0`: `wisp update apply` → "✗ Update checks are off here (WISP_UPDATE_CHECK=0 or CI), so wisp didn't look for the update. Unset WISP_UPDATE_CHECK (outside CI) and try again, or download it from …"; the feed got no request |
| HS11 | **Fixed** | Gated lead, `delegate {agent: frob}` → "Nothing was delegated.\n- frob: no harness frob: name one of claude, codex, opencode, cursor, antigravity, grok, copilot"; with a codex task beside it: "Some tasks were not delegated: … Delegated: codex (id …)" |
| HS12 | **Fixed** | Cloud computer under Machines; `wisp serve` restarted and Home opened at once in a fresh Chrome: no 401/404 and no console error, 3 times out of 3 |
| HS13 (GS2) | **Fixed** | Telegram linked (chat 1001): agent `ask_user` "TG I: red or blue?" → inline keyboard; tap Blue → `answerCallbackQuery "Answered."`, the agent read "The user answered: Blue". Done on the finished task's message → "Marked done.", task `done` |
| HS14 | **Fixed** | Phone → Telegram → `123456:badtoken` → toast "That doesn’t look like a bot token: @BotFather gives one like 123456:ABC…" (no `secrets.token:`) |
| GS2 | **Fixed** (IS1 beside it) | ntfy setup cards: no buttons (HS8). Telegram taps answer (HS13); a setup card refuses another person's tap ("That's a change to your setup: answer it in wisp (or from the chat you linked, yourself). Nothing changed."), typed or tapped |
| GS6 | **Partly** | HS7's row |

### H's smaller notes, re-checked
- **Fixed:** the stale "Sign-in was declined in the browser." (shown at 30 s, gone at 75 s); the cloud retry line ("Can’t
  reach the grid (it may be restarting, or this computer is offline): wisp tries again."); ntfy Test now says "Test sent:
  it shows up like this when a task finishes or asks."; the doubled `agents.a1: agents.a1:` path; a trigger's unknown
  `project` refused by validate; unknown short flags.
- **Still as before:** Esc in Edit closes the whole Manage dialog; `connectors add/remove/test` print the list; `computer
  off` prints the address; a Usage or Worktrees select put back writes its default; `learn view` while off says "(wisp.learn
  was switched off)" twice.

## Action table

Key: P = pass, IS# = fails with that new finding, — = not tested (why given). "rec" is the argv/stdin the fake recorded;
"fb" is ms from the click to the first DOM change; "@150" is the UI 150 ms after the click. H's row numbers are kept.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists 7 harnesses and GitHub | P | Claude Code, Codex, opencode, Cursor, Antigravity, Grok, Copilot, GitHub |
| 2 | Codex Sign in (device code) | P | fb 46; steps and ABCD-12345 @100; Signed in after 4.3 s; no popup |
| 3 | Add another Codex account (Work) | P | Work tab → `Codex 2:ready:work` |
| 4 | Move up / down | P | Rows swap @150; server order follows |
| 5 | Rename | P | Focused "Codex 2" → "QA Work" @150 and on the server |
| 6 | Sign out of an account | P | "Signed out · Sign in" @150; `signedOut` |
| 7 | Remove (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. …" · Remove account |
| 8 | Harness Update button | P (new) | Scratch app, checks on: a minute after start 3 `npm view`; "Update to 9.9.10" ×3; Claude's: fb 24, rec `claude update`, toast "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `…/claude update` in a terminal, then re-check." |
| 9 | Model default | P | Claude Code fb 28, Opus fb 40; `models.claude: opus` |
| 10 | Thinking default | P | Slider End fb 5, "Opus · max"; `efforts.claude: max` |
| 11 | Lean runs | P | fb 28, off, back to all |
| 12 | Ask before running a changed harness | P | fb 36, back |
| 13 | Re-check | P (new) | fb 62; toast "Up to date: nothing new" |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | fb 31–43, server key changes each time, left unchanged |
| 15 | General: 4 selects | P | Same |
| 16 | Theme, reload | P | ⌘K Dark theme: fb 24, `data-theme=dark` @100, kept across reload; back to System |
| 17 | Notifications | P | Accept tasks from channels flips; Default project row |
| 18 | Desktop notifications | P (note) | Disabled, "Blocked — allow notifications …" (headless) |
| 19 | Memory | P | 1 switch |
| 20 | Automations: Webhook triggers | P | fb 36 |
| 21 | Usage: 5 switches, 4 selects | P (note) | fb 31–37; machine selects put back write their defaults (as G/H) |
| 22 | Worktrees: 3 selects | P (note) | Clean up put back writes `7` (as G/H) |
| 23 | Privacy: 3 switches | P | 2 flip (fb 37–38); usage stats disabled |
| 24 | Usage stats off → no PostHog | P | "Off (DO_NOT_TRACK, WISP_TELEMETRY=0 or CI)"; catcher: 0 PostHog calls all round |
| 25 | Updates (clone) | P | "A git clone: pull and restart to update"; Check now disabled |
| 26 | ⌘K, ⌘P, `[`, N | P | Lists; `[` hides and brings back; N focuses the textarea |
| 27 | Settings search | P | `telemetry` → Privacy, Lean row; `worktree` → General, Worktrees, rows |
| 28 | All settings | P | 132 rows, no `router`/`runtime.auto` |
| 29 | Setup: Make the formula | P | No `router:` (read through `wisp export`/`resources.write`, the same writer) |
| 30 | Setup: Import → plan → Apply | P | YAML error and `hello` plan nothing; "Changes 2 · leanRuns all → off · worktrees.max 80 → off"; "Adds 1 · project repo3-wt · Changes 1 · settings.editor cursor → zed"; "Imported" |
| 31 | Diagnostics | P | Computers, plugins, loaded rows |
| 32 | Sandboxes | — | A dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Other folder…: focus | P | Focus in `~/code/my-app or a GitHub URL` |
| 34 | Missing / file / relative | P | "No folder at …", "… is a file, not a folder: …", "repo2 isn't a full path: …" |
| 35 | `file://` URL | P | → repo4 |
| 36 | Duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already old-app" (both) |
| 37 | Ghost folder via a formula | P (new) | `projects: [{name: ghost, path: …/ghost-folder}]` → validate and apply "✗ projects.ghost: No folder at …", exit 1 |
| 38 | Remove a project (confirm) | P | repo4: fb 36, "Remove “repo4” from wisp? Its files are kept." |
| 39 | ⌘P switcher | P | All projects, old-app, repo2, repo3-wt, repo4, Add project…, Manage projects…, Repo history |
| 40 | Tasks in a project | P | UI launch: rec cwd `home/worktrees/repo-<id>`, `--model opus --effort max` |
| 41 | A second project for one folder | P (new) | Formula `second-repo2` at repo2's path → "✗ projects: project repo2 and second-repo2 have the same path …: one project per path", exit 1 |

### Machines, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Add `qa@192.0.2.7` | P | fb 47, Connecting @150, then "No answer · 192.0.2.1 doesn’t answer …" |
| 43 | Edit… | P (note) | Every field filled; Esc still closes the whole Manage dialog |
| 44 | Remove (confirm) | P | oldbox: fb 40, "Remove oldbox from wisp? Its projects leave the launcher. …" |
| 45 | `discover`, `hosts`, `reachable` | P | `(none)`, `(none)`, this computer |
| 46 | This computer | P (IS12) | Memory, CPU, Disk, "wisp itself 70 MB" |
| 47 | Pause / Resume | P | "Automations paused until 1:55" @150; Resume @150 "Running as usual" |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in (popup, dev sign-in, Allow) | P | Popup `/activate?code=…`; "Device connected"; qa-alice current |
| 49 | Don't allow | P | Grid "Not allowed"; Add an account shows "Sign-in was declined in the browser."; gone by 75 s |
| 50 | This computer on the grid | P | fb 55, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4992" @150 |
| 51 | Grid restart | P | "retrying · … Can’t reach the grid (it may be restarting, or this computer is offline): wisp tries again."; online 19 s after the grid came back |
| 52 | API key create, use, revoke | P | fb 37; cloud computer `/api/health` 200 with it, 401 without; Revoke (fb 45, "Revoke qa-script? …") → 401 |
| 53 | Add qa-bob | P | qa-alice stays current |
| 54 | Switch and back | P | Current moves; this computer follows the account |
| 55 | Sign out of qa-bob | P | fb 31, "Sign out of qa-bob on wisp.localhost:4992 here? …" |
| 56 | Cloud computer woken by use | P | asleep → online on the API-key request; spy log empty |
| 57 | `computer off`, `cloud signout` | P (note) | "this computer: off · http://…" (still prints the address), "✓ signed out" |

### Connectors and MCP
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector | P | qa_echo "1 tool" by 4 s |
| 59 | Edit: env | P | Server `env: [{QA_VAR, set: true}]` |
| 60 | Paste 2 servers | P | `pasted` and `remote1` added |
| 61 | Connectors reach a run | P (IS3) | rec `--mcp-config` has `old-echo`; `--allowedTools mcp__old-echo mcp__wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, …" |
| 63 | Presets | P (new) | With none: "None yet · … · Linear · Jira · PostHog · GitHub · Filesystem · Postgres" |
| 64 | `wisp mcp` | P | 301 tools on stdio, 297 at `/api/mcp`, no `router_*`, `automations_*` there (HS6) |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy Test | P | fb 53; catcher got it; card "Test sent: it shows up like this when a task finishes or asks." |
| 66 | Telegram bad token | P | Card: "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| 67 | Slack bad tokens | P | "Slack: invalid_auth. Check the tokens …" |
| 68 | Discord bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | Switch disabled |
| 71 | Off and tokens removed | P | All off; export lists only ntfy; no channel key in `secrets.json` |
| 72 | No "push" card | P | 6 cards |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` | P | `user.qa-theme` off |
| 74 | Install from a folder | P | fb 34 |
| 75 | Switch on (untrusted) | P | fb 44; `--accent` stays `#2f5bea` |
| 76 | Trust / Untrust | P | "Trust QA Pink? …" → `#ff1493`, send rgb(255, 20, 147); Untrust → `#2f5bea` |
| 77 | Off, Remove | P | fb 44 / 31; "Remove QA Pink? …"; row gone @200 |
| 78 | Export after Remove | P | Only `user.qa-theme` |
| 79 | CLI enable/disable | P | `on` / `off` |
| 80 | Built-in off and on (learn) | P | `learn.view` → `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the task @150; `wisp ls`: `no project` |

### Import, init, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import v1home --dry-run` | P | 14 to add; "!" lines; "·" notes; "and the history of 1 task" |
| 83 | `wisp import v1home --yes` | P | HS1 |
| 84 | v1 ntfy | P (IS5) | Polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | launch, subagentModel, worktrees.max, maskEnvFiles, leanRuns, editor, notify, limitHold, retentionDays |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at …" |
| 87 | Imported task's project | P | `v1sess0001 · old-app` |
| 88 | v1 default agent | P | `runtime.launch` agent codex |
| 89 | `wisp init` | P (new) | `.wisp/wisp.yml` (`history: { track: ask }`) and AGENTS.md's note; again → "✓ … is ready" |
| 90 | `settings get/set` | P | "must be one of off, 20, 40, 80" (1); "no plugin declares the setting nope.key" (1) |
| 91 | export → validate → diff | P | 16 resources validate; "16 unchanged" |
| 92 | `validate`/`apply` errors | P | 173, 37, 41 |
| 93 | `completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass |
| 94 | Bash completion | P | `trigger`/`triggers` verbs, `worktrees.max` values |
| 95 | `wisp skill install` | P (new) | `realhome/.claude/skills/wisp/SKILL.md`, `.mcp.json` (wisp mcp), `.claude/settings.json` (SessionEnd hook) |
| 96 | `npm run doctor` | — | Probes 4800/4477 |
| 97 | `wisp <group> <unknown word>` | P | crew, trigger, connectors, computer, cloud, sandbox: verbs listed, exit 2 |
| 98 | `--agent frob` | P | "✗ no harness frob: --agent takes claude, …" |

### Auto off everywhere
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `wisp router frob` | P | "… Auto is off (runtime.auto), so the router is put away" (exit 2) |
| 100 | Launcher picker | P | 7 harnesses, Factory, Set up another agent…; no Auto |
| 101 | ⌘K | P | "auto", "harness", "router": tasks only, no Auto entry |
| 102 | Crew editor harness | P (new) | 1440: "As New task · Default" → 7 harnesses, no Auto |
| 103 | All settings | P | 0 `router`/`runtime.auto` rows |
| 104 | Settings search for auto | P (new) | Automations, Webhook triggers, Ask near the limit, guard rows; no Auto |
| 105 | Setup / formula | P | No `router:` |
| 106 | CLI and agent tools | **IS8** | HS7's leftovers |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `wisp plugin` identical before and after |
| 108 | WebKit 1280, home + 20 panes | P | Light and dark: no overflow, no console error |
| 109 | Chrome 390 | P | Same |
| 110 | WebKit 390 | P | Same |
| 111 | Harness check at start | P (new) | Scratch app, checks on: wisp's own `/releases/latest` at start, the 3 `npm view` a minute later |
| 112 | `wisp-grid dev --help` | P | 7 lines |

### Re-checks of C's, D's and E's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects counts | P | "/tmp/qaI-setup/repo · 2 tasks", "repo2 · 10 tasks", … |
| 114 | Setup → Import errors | P | "pasted.yml: a formula is a map …" |
| 115 | Plugins → Install errors | P (new, note) | "… has no wisp-plugin.json", "No folder at …", "relative/dir isn't a full path: …", "Couldn't reach example.invalid to clone …"; a file says "No folder at …/math.js" |
| 116 | `export --format xml` | P | "✗ no format xml: yaml, json or toml", exit 2 |
| 117 | Project colour | P | 9 colours, fb 35; `color: amber` |
| 118 | Nested CLI output | P | `channels status` has `transport` |
| 119 | ⌘K "model" | P | Agents first |
| 120 | Sign-in box steps | P | 2 |
| 121 | Copy the code | P (new) | Copy fb 49; clipboard `ABCD-12345` |
| 122 | Paste the code | — | The fake has no paste step |
| 123 | Cancelled first sign-in | P (new) | Codex 3: Cancel fb 35, "Signed out · Sign in" @150 and at 5 s, `signedOut` |
| 124 | Sign-in cut by a restart | P (new) | Codex 2 signing → restart: no `codex login` left; "Signed out · Sign in", `signedOut` |
| 125 | Sign-in that times out | — | 15 min |
| 126 | Harness Sign out that stays signed in | P (new) | Toast "Codex is still signed in after `codex logout`: run it in a terminal to sign out there." |
| 127 | Formula with `off` values | P | 30 |
| 128 | v1 recipe | P (new) | `wisp recipes`: `run-tests  run the tests and fix failures` |
| 129 | `wisp ls` with no project | P | 81 |
| 130 | Dark at 1280 / 390, both engines | P | 108–110 |
| 131 | MCP with Auto off | P | No `router_*` (but IS8) |
| 132 | Update all | **IS11** (new) | fb 49; ran `claude update`, `codex update` and opencode's; all stayed 9.9.9; no word |
| 133 | Personal / work switch | P | "Make it a personal account" fb 37 → `personal` |
| 134 | A signed-out account's menu | P | E8: Move down (disabled), Sign out (disabled) |
| 135 | Add connector errors | P | "A name, so agents can tell its tools apart", "The command that starts it", "It wouldn't start: …: not found" |
| 136 | A CLI launch in a project | P | HS5 |
| 137 | Add machine bad addresses | P (new) | Empty: Add disabled; "has spaces here" and "qa@": "SSH host: user@host, a host name, or a Host from ~/.ssh/config." |
| 138 | A plan that adds a setting | P | 82 |
| 139 | Add another account → Cancel | P (new) | fb 65; nothing added on the server |
| 140 | Default project | P | 17 |
| 141 | Worktrees: Bring in | P | GS1 / HS4 |
| 142 | Harness Sign out toast | P (new) | 126 |

### Re-checks of F's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 143 | v1 connector env | P | "QA_VAR comes over from v1's secrets (as connector-old-echo-QA_VAR)" |
| 144 | Every built-in off and on | P (new, IS6) | All 50 switchable built-ins off and back on from the CLI: each `off`, then `active` (issues `resolved`), health 200 throughout; settings and plugins refused |
| 145 | An off plugin's CLI | P (note) | "✗ wisp learn is off: … Switch it on: wisp plugin enable wisp.learn" (exit 2) |
| 146 | Settings plugin off | P | "wisp.settings can't be switched off: …"; same for wisp.plugins |
| 147 | Worktrees count after off/on | P (new) | 7 on disk, 7 listed |
| 148 | `wisp update` on a clone | P | `method git` |
| 149 | `update view/check/install` | P | `enabled false`, `method git` |
| 150 | A channel on and back off | P | 71 |
| 151 | `connectors remove <id>` | P (note) | Removed; prints the list |
| 152 | `update apply` on a clone | P | "This copy of wisp is a git clone, so it does not update itself.", exit 1 |
| 153 | Phone switches | P | Telegram/Slack/Discord/GitHub switches fb 35–68 |
| 154 | Grid stopped and started | P | 51 |

### Re-checks of G's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 155 | Connector card on the task page | P | "Wants to change your setup · … · needs AGENT_KEY from you · It would run … · Allow · Deny" |
| 156 | The agent answers its own card | P (new) | `runtime.answer` → 403 "This card is the user's to answer …"; follow-up → 409 "This task waits on its setup card …"; its `later.add` "Allow" never answered it |
| 157 | Crew card, `wisp reply <id> Allow` | P | `Qa Reviewer · claude`, cwd repo2 |
| 158 | Typed deny from the CLI | P | "✓ You denied: add crew member Qa Phone" |
| 159 | Typed deny in the reply box | P (new) | "Deny: not this one" → the agent read "The user said no: nothing was changed. … They said: “Deny: not this one”." |
| 160 | Project card for an existing project | **IS9** (new) | `runtime_projectAdd` for repo2 → card; Allow → "✓ You allowed: add the project …", nothing changed |
| 161 | A card at 390 | P | Card 0–390, no overflow (Chrome) |
| 162 | A card that went away | P | `wisp stop` → the agent read "The card was taken back before the user answered it …" |
| 163 | Telegram: who may answer a card | P (new) | Another person's typed "Allow" in the linked chat: refused; the linked person's typed "Allow" answered |
| 164 | Questions to the phone | P (IS1) | HS8, HS13 |
| 165 | Asking row | P | "Asks: …" in Needs you |
| 166 | Card headline | P | No backticks |
| 167 | Desktop self-update end to end | P | Updates: "wisp 2.0.1 is out. Installing it restarts wisp · Update now" → "Restarting. This page reconnects once wisp is back" @150 (fb 30). Feed served the zip; old server exited 0; the 2.0.1 bundle started; `Info.plist` 2.0.1 |
| 168 | Wrong checksum | P (note) | App started on a feed with a bad digest: `wisp update apply` → "✗ wisp-mac-arm64.zip didn't match the release's checksum. …", exit 1 |
| 169 | `wisp shell-init` | P | `zsh -n` passes |
| 170 | Terminal context | P | "`…` → exit 3" with `--show-context` |
| 171 | Secret inline in a command | P | HS9 |
| 172 | Formula with goals, factory | P (new) | A goal, a factory with a `build` station, a task naming both: "✓ 3 resources"; diff "3 to add" |
| 173 | Unknown names in a formula | P (new) | "no goal named tests-gren", "no project named rep2: did you mean repo2?", "no factory named qa-lin", "no agent named claude-cod" |
| 174 | `auto` in a formula with Auto off | P | "agents.qa-new-auto: harness: auto: Auto is off …" |
| 175 | Connectors after a restart | P | `qa_tmp`'s `test` kept across 6 restarts |
| 176 | `connectors add --args A,B` | P | export `args: [/…/mcp-echo.mjs, "--x"]` |
| 177 | `wisp <group> --help` | P (IS10) | 22 groups: `wisp <g> --help` = `wisp help <g>` |
| 178 | zsh and fish completion | P | `worktrees.max`, no `router` |
| 179 | F12's docs | P | `signin.rs:4–5` |
| 180 | `wisp.settings`, `wisp.plugins` switches | P | 146 |

### New in H, re-checked
| # | Action | Result | How checked |
|---|---|---|---|
| 181 | `wisp import <relative v1 home>` | P | HS1 |
| 182 | `wisp trigger off` / `on` | P | "✓ trigger qa-nightly is off: …"; ls `off`; export `enabled: false`; on |
| 183 | `wisp trigger off nosuch` | P | "✗ no trigger named nosuch (wisp trigger ls lists them)", exit 1 |
| 184 | `wisp trigger rm` | P | "✓ removed trigger qa-nightly" |
| 185 | `wisp triggers off/on` | P | HS2 |
| 186 | `wisp help trigger` | P | HS2 |
| 187 | Misspelled launch flags | P | `--mdoe`, `--efort`: "did you mean …"; nothing launched |
| 188 | `--modle opus` | P | HS3 |
| 189 | Bring in looks on open | P | Offer at 500 ms |
| 190 | One worktree, two projects | P | HS4 |
| 191 | CLI launch's model and effort | P | HS5 |
| 192 | On-the-fly subagent | P | `delegate {agent: codex}` → task on codex |
| 193 | A second unnamed codex task | P | Same member (same id) |
| 194 | `delegate` to an unknown harness | P | HS11 |
| 195 | `delegate {agent: auto}` with Auto off | P (note) | Ran on Claude; the reply says "Delegated: auto" |
| 196 | Agent tools with Auto off | **IS8** | 106 |
| 197 | ntfy: a question's buttons | P | Signed Allow/Deny actions |
| 198 | ntfy: a tap answers | P | The queued action body answered (IS1 is the guard case) |
| 199 | ntfy: a setup card's buttons | P | HS8 |
| 200 | Telegram: a question's buttons | P | `inline_keyboard` Red / Blue |
| 201 | Telegram: a tap | P | HS13 |
| 202 | A card taken back answers its agent | P | 162 |
| 203 | Terminal context: npm, yarn, openssl | P | HS9 |
| 204 | Terminal context: r194's cases | P | HS9's row |
| 205 | `wisp update apply` with checks off | P | HS10 |
| 206 | Desktop update UI with checks off | P | "Update checks are off here (WISP_UPDATE_CHECK=0 or CI). You’re on 2.0.0 … Checks are off on this install" |
| 207 | Trigger switch over MCP | P | HS6 |
| 208 | First page after a restart (cloud machine on) | P | HS12 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 209 | Agent: `runtime_exec` | P | Not in the run's 196 tools; MCP "Unknown tool: runtime_exec"; raw `runtime.exec` → 403 "… an actions block’s command runs when the user clicks it"; no file written |
| 210 | Agent: delete a task | P | Its own and its child's → 403 "it can't delete a task (its work and worktree go with it) …" |
| 211 | Agent: `runtime.projectTest` | P | 403 "it can't set a project's test command …" |
| 212 | Agent: `machines.call` | P | Not listed; raw → 403 "it can't call another computer's wisp directly …" |
| 213 | Agent: route `runtime.exec` to another computer | P | Raw `machines.route` → 403 "runtime.exec isn't something this task may run, here or on another computer"; MCP `machines_route` to the online cloud computer: same; `runtime.list` routes |
| 214 | Harness lowers `worktrees.projectAsk` | P | `settings_set` waits; task "Your harness wants to stop asking before trusting a new project's setup scripts … worktrees.projectAsk: on → off · Allow it only if you asked for this."; setting stays `on` |
| 215 | Harness answers its own guard | P | `wisp_reply`, `runtime_answer`, `runtime_followUp`, `runtime_stop`, `runtime_archive`, `runtime_unask`, `later_add` (in 0 and 1 min) → "That task asks the user, not their harness …" |
| 216 | `mcp_rpc` with a `settings_set` inside | P | Asks (a second guard task) |
| 217 | Deny (`wisp reply <id> Deny`) | P | Harness read "The user said no in wisp, so nothing was changed."; setting kept; task `done` |
| 218 | Allow on the task page | P | fb 39; harness read "The user allowed it in wisp: worktrees.worktree: on → off …"; setting `off`; task `done` |
| 219 | Raise a guard, other settings | P | `worktrees.worktree on`, `crew.tint`, `budget.limitHold 80`, `machine.batteryHold` off→off (default) and 40 apply at once |
| 220 | Values as the kernel reads them | P | `{"value": false}`, `"yes"` ask |
| 221 | A guarded write with `peer` | P | "That changes one of wisp's safety guards, which isn't sent to another computer from here …" |
| 222 | Every other guard | P | `git.maskEnvFiles false`, `channels.acceptChannels true`, `budget.limitHold "95"`/`"off"`, `machine.batteryHold "20"` (from 40), `runtime.setupAnswerSets ["qa-set"]`, `automations.webhookTriggers` each ask, nothing changes |
| 223 | What runs code | P | `runtime_exec`, `worktrees_trust`, `plugins_install`, `plugins_trust`, `plugins_setEnabled` on, `machines_call` each ask; Allow on `runtime_exec` ran exactly `pwd > …/EXEC-USER.txt` (in the task's worktree) |
| 224 | A guard question whose call is gone | **IS2** | 9 hung up, 1 across a restart |
| 225 | A guard question on ntfy | **IS1** | Replayed Allow lowered `worktrees.projectAsk` |
| 226 | A guard question on Telegram | **IS1** | Another person's tap in the linked chat allowed it |
| 227 | A setup card on Telegram | P | Another person's tap: "That's a change to your setup: answer it in wisp (or from the chat you linked, yourself). Nothing changed."; the linked person's Deny tap answered |
| 228 | Saved model/effort beyond the CLI | P | HS5's row (MCP, crew) |
| 229 | An agent profile on `auto` with Auto off | **IS8** | `wisp runtime profile set --harness auto` stored |
| 230 | Misspelled verbs (I2) | P | `crew pasue Zed`, `trigger of tick`, `goal sett abc never`, `later cancle x`, `factory stp x`, `held abc strat`: "did you mean …", exit 2 |
| 231 | Sentences after group words (54ac84e9) | P | `review the auth module`, `plugin for prettier`, `crew a reviewer for the PR`, `update the readme`, `ship the docs`, `run the tests`, `share …`, `budget for march`, `learn from this`, `later today fix it`, `trigger an alert when CI fails` launch; `plugin foo`, `computer start fails`, `cloud setup please now` refused |
| 232 | A git command line after `wisp git` | **IS4** | `wisp git push origin main` launched a Full-auto task |
| 233 | Changes pane on a gone folder | P (note) | "This task’s folder is gone · wisp cleaned it up after the task finished. Its turns still show what it changed." in words, no Retry, no toast (fresh Chrome) |
| 234 | v1 ntfy after the import | **IS5** | No notification rule; "Send me" unticked |
| 235 | `/api/peer` after machines off/on | **IS6** | "No such machine." for a machine added after |
| 236 | Terminal context: more shapes | **IS7** | `az login -p`, `vault login`, `htpasswd -b` |
| 237 | A connector's secret on the command line | **IS3** | `ps` shows `QA_VAR` and the run's token |
| 238 | `wisp help <command>`, `<command> --help` | **IS10** | 30 refusals, 13 `--help` read as an argument |
| 239 | This computer while a run starts | **IS12** | `tasks: []` with a live CLI |
| 240 | A brought-in task's harness words | **IS13** | "· Default model", "Follow up with …" |
| 241 | Cloud: a declined sign-in forgotten | P | Shown at 30 s, gone at 75 s |
| 242 | Cloud: the retry line | P | In words (51) |
| 243 | `wisp import`: unchanged and unreadable | P | HS1's row |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### IS1 · M · A guard question from the user's harness is answered by taps that aren't the user's (ntfy, Telegram)
- **Context:** ADR-0115 §4 says the question "is answered only by the user's own hands (… a channel relaying their
  tap)". HS8's fix took Allow/Deny off setup cards on ntfy for the reason that an ntfy tap can't be the person's
  (`taps_are_personal`: Telegram, Slack, Discord only). The guard question gets neither check.
- **Steps (ntfy):** ntfy on, Send me: Questions → the user's harness calls
  `settings_set {"key":"worktrees.projectAsk","value":"off"}` → the notification arrives → its Allow action's body
  (`{"i":0,"k":"a","r":…,"s":…,"sig":…}`) is posted to the topic's `-reply` stream, as anyone who can read the topic can do.
- **Actual:** `worktrees.projectAsk` is `off` 11 s later; the harness read "The user allowed it in wisp: worktrees.projectAsk:
  on → off". The notification carries `Allow`/`Deny` http actions (`clear: true`).
- **Steps (Telegram):** linked chat 1001 → a guard question → a `callback_query` on its Allow with `from.id` 2002 in chat
  1001 (another member of a linked group chat).
- **Actual:** `answerCallbackQuery "Answered."`, the setting changed. The same tap on a setup card is refused ("That's a
  change to your setup: answer it in wisp (or from the chat you linked, yourself). Nothing changed.").
  `plugins/telegram/host/src/lib.rs` `on_tap` computes `person` (`linked_person`) but the guard question doesn't use it.
- **Also:** the user's harness can read the ntfy topic and server (`resources_export` → `channels[].routing`).
- **Expected:** a guard question is answered as a setup card is: no buttons on ntfy (answer it in wisp), and on
  Telegram/Slack/Discord only by the linked person.

### IS2 · M · A guard task whose harness call is gone stays open with its prompt queued, and a reply runs it in Full auto
- **Steps:** the user's harness calls `settings_set` for a guard (here with a 4 s client timeout, as a harness that gives
  up on a slow tool call), or `wisp serve` restarts while it asks → the user answers Allow or Deny (task page,
  `wisp reply`).
- **Actual:**
  - Nothing changes (safe), but the task stays `review` with its held prompt in the queue: "Waiting to start", "Report ·
    1 queued", in Needs you. 9 such tasks after this round (⌘P counted "21 need you"). The answered ones whose call was
    still waiting went `done` (4).
  - Nothing tells the user the change didn't happen: the CLI says "✓ answered"; the page shows only "Waiting to start".
    ADR-0115 says after a restart "its Allow runs nothing … and says so as any answered question".
  - The task is Full auto on Claude. `wisp reply <it> "hello there"` ran two turns with `--permission-mode
    bypassPermissions` in a scratch folder: "hello there", then the guard's own text ("Your harness wants to let chat
    channels start tasks. channels.acceptChannels: false → true Allow it only if you asked for this.") as a prompt. That
    run's agent then opened a setup card of its own.
  - A guard task nobody answers whose call is gone stays "Asking" in Needs you for good.
- **Expected:** when the call is gone, the guard task settles (done, its held prompt dropped) and says the change wasn't
  made because the harness stopped waiting; a reply never runs the guard text.

### IS3 · M · A connector's secret env and the run's wisp token are on the harness's command line (`ps`)
- **Steps:** a connector with a secret env (the v1 import's `old-echo`, `QA_VAR` from secrets) → any task.
- **Actual:**
  - Claude: `--mcp-config` is inline JSON with `"env":{"QA_VAR":"v1secret"}` and `"x-wisp-token":"…"`.
  - Codex: `-c mcp_servers.old-echo.env={QA_VAR="v1secret"}` and `mcp_servers.wisp.http_headers={…x-wisp-token="…"}`.
  - `ps -axo command` (any local user on macOS) shows both for the running process: `"QA_VAR":"v1secret"`,
    `"x-wisp-token":"27eef54ec3cd…"`. The token is the run's agent token (agent:session).
- **Expected:** secrets and the token go through a 0600 file or the environment, never argv. No ADR covers it.

### IS4 · M · `wisp git push origin main` launches a Full-auto task with that command as its prompt
- **Steps:** in a project, `wisp git push origin main` (also `pull`, `fetch`, `log`, `diff`, `merge`, `rebase` … followed
  by two or more words).
- **Actual:** "◌ launched: git push origin main". `wisp git` has no `push` (`changes, checkout, clone, commit,
  github-repos, origin, restore-turn, revert, stage, status, tools`); `git` is in `PROMPT_GROUPS`, and `push` isn't a
  slip of one of its verbs, so the line is read as a sentence. `wisp git push` alone and `wisp git psuh` are refused.
- **Expected:** a word that is one of git's own commands after `wisp git` is refused ("wisp git has no push: that's
  git's own command; run `git push …`, or quote it to make it a task"), as a slip is. A command line for git should never
  start a Full-auto agent.

### IS5 · M · A v1 ntfy user gets nothing on the phone after `wisp import`
- **Steps:** a v1 home with `remote.ntfy {enabled: true, topic, server}` → `wisp import --yes`.
- **Actual:** the channel `ntfy` is made and polls its reply topic, but no `notifications:` rule is: Phone → ntfy shows
  "Send me: Questions / Finished tasks" both unticked, and questions and finished tasks are never sent (the catcher got
  no POST until I ticked them). v1 sent both (F-chan-1). A channel switched on in v2 gets its rule by itself
  (`phone-telegram: on [asks, finished]`). `crates/wisp/src/v1_formula.rs` makes the channel only.
- **Expected:** the import adds `phone-ntfy` (asks, finished) with the channel, or says it didn't.

### IS6 · M · After `wisp.machines` is switched off and on, a computer added later is "No such machine." to the app until a restart
- **Steps:** `wisp plugin disable wisp.machines`, `wisp plugin enable wisp.machines` (here from the CLI, and in row 144's
  sweep) → add a computer (`wisp machines add --name tnet9 …`, or sign in to a grid, which adds its cloud
  computer) → `GET /api/peer/<its id>/api/events…`.
- **Actual:** `404 {"code":"not_found","error":"No such machine."}`, while `wisp machines list` shows it (the cloud
  computer as online). Machines added before the switch answer ("oldbox is offline …"). The app's stream for that
  computer fails (console: `http 404: GET /api/peer/2428ba/api/events…`). After a restart both resolve.
- **Expected:** the proxy reads the machines as they are now, whatever the plugin went through.

### IS7 · M · Terminal context still sends `az login -p`, `vault login` tokens and `htpasswd -b` passwords (security)
- **Steps:** `eval "$(wisp shell-init zsh)"` → a failing command → `wisp "check the error above" -d --show-context`.
- **Actual:** sent as typed, into the context block and the agent's prompt (rec stdin):
  - `az login -u ada@x.invalid -p hunter2qa` (`sudo docker login -p` is masked).
  - `vault login s.AbCdEfGhIjKlMnOpQrStUvWx` (Vault's token shape).
  - `htpasswd -b .htpasswd ada hunter2qa`.
- **Also (over-masking):** `git remote set-url origin https://x-access-token:ghs_…@github.invalid/o/r.git` shows as
  `…x-access-token:<secret>` with the host and path gone.
- **Expected:** `-p`/`--password` after `az login`, a token after `vault login`, and `htpasswd -b`'s last word masked.

### IS8 · L · With Auto off, `auto` is still offered and stored in places (HS7 / GS6 partly)
- `wisp runtime profile set --name x --harness auto` and the user's `runtime_profile_set {harness: auto}` store an agent
  profile on `auto` (`wisp export`: `agents: [{name: qa-auto-prof, harness: auto}]`). A formula refuses the same for a
  new profile ("harness: auto: Auto is off (runtime.auto) …"). `--agent frob` then offers `qa-auto-prof, qa-cli-auto`.
- Tool texts still offer it: `runtime_create` "`agent: \"auto\"` lets wisp.router pick" (the agent's, `wisp mcp`'s and
  `/api/mcp`'s lists), `runtime_profile_set` "`auto` lets the router pick", and `wisp mcp`'s own `wisp_launch` "auto
  lets wisp pick" (`/api/mcp`'s is fixed).
- An agent's `wisp_crew_add {agent: auto}` makes a card "… add crew member Qa Auto (auto)"; only after Allow:
  "(it didn't go through: Auto is off …)".
- **Expected:** refused up front everywhere `auto` is refused in a formula, and gone from every tool text.

### IS9 · L · A setup card is made for a change that can't or needn't happen; Allow says "✓ You allowed …" and nothing changes
- An agent's `runtime_projectAdd` for `/tmp/qaI-setup/repo2`, already the project repo2 → card "Claude Code wants to
  add the project /tmp/qaI-setup/repo2"; Allow → "✓ You allowed: add the project /tmp/qaI-setup/repo2", 4 projects as
  before, no word that it existed. A missing folder is refused before any card (I6).
- The crew `auto` card of IS8 is the same shape.
- **Expected:** refused before the card ("repo2 is a project already"), as a missing folder is.

### IS10 · L · `wisp help <command>` refuses 30 commands `wisp help` lists, and 13 commands take `--help` as their argument
- `wisp help reply` (and `ls`, `import`, `run`, `show`, `stop`, `wait`, `held`, `diff`, `apply`, `export`, `add`,
  `attach`, `blame`, `btw`, `fmt`, `fresh`, `help`, `init`, `log`, `merge`, `notes`, `open`, `output`, `plan`, `rerun`,
  `resume`, `secret`, `tasks`, `track`, `untrack`, `workspace`) → "✗ wisp help reply: no command or plugin named reply".
- `<command> --help` reads `--help` as its argument: `wisp import --help` → "✗ No folder at /private/tmp/qaI-setup/--help";
  `wisp run --help` → "no declared task, recipe or workflow named --help"; `export --help` → "no kind --help"; `open`,
  `resume`, `track` → "no task --help"; `adopt` → "no codex session --help"; `untrack` → "--help isn't tracked here";
  `blame`, `init`, `secret`, `workspace`, `log` similar. Others print the general usage.
- **Expected:** `wisp help <cmd>` and `wisp <cmd> --help` print that command's usage line(s), as the groups do (GS10).

### IS11 · L · Settings → Agents → Update all says nothing when the updates didn't take
- Scratch app, checks on: "Update all (3)" → fb 49; rec: `claude update`, `codex update`, opencode's updater. All stay
  9.9.9; no toast after 9 s; the rows read "Update to 9.9.10" again.
- One harness's Update says "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run … then re-check."
- **Expected:** Update all says per harness what took and what didn't (with the commands), as one Update does.

### IS12 · L · This computer lists no task while a run's CLI hasn't sent its first event
- A task's agent process alive for 22 min, its task `running`, the list "Starting up" (gated before its first event) →
  This computer → What's using it: "No task is running."; `wisp machine read --json`: `tasks: []`.
- A CLI that hangs on start (a login prompt, a network wait) is the case the user looks there for.
- **Expected:** a running task's process tree is listed from its start.

### IS13 · L · A brought-in task shows no harness: "· Default model" and "Follow up with …"
- Worktrees → Bring in → task `qa-wt2` (repo3): its head reads "· Default model" (a dot before nothing), the reply box
  "Follow up with …", the composer pill "Default" with no harness. `wisp ls` shows its agent column empty.
- **Expected:** the harness a follow-up will run on (the default one) named, or the words without the dangling parts.

---

## Smaller notes (not counted)
- **Guard card:** `runtime.exec`'s card doesn't say which task's folder it runs in (it ran in the task's worktree).
  `wisp ls` prints a guard task over three lines (its question's newlines). The user's `machines_route` with `peer` is
  refused as "That changes one of wisp's safety guards …" for `runtime.list` too. Switching a plugin on that's already on
  (`plugins_setEnabled {enabled: true}` for an active built-in) asks.
- **Agent `later.add`:** an agent's scheduled follow-up is stored with `"by": "user"`.
- **Telegram:** a refused tap (another person's, or an unknown button) removes the message's keyboard, so the linked
  person can only type. Test on a token Telegram refuses says "isn't linked to a chat" and overwrites the 401 in
  `channels view`; the card's own line says "Unauthorized. Check the token …".
- **Changes pane:** "This task’s folder is gone · wisp cleaned it up after the task finished" when the folder was moved by
  hand and the task had no turns. A folder whose `.git` is broken reads `null` ("No changes yet") though it has an
  uncommitted file. In the long-lived CDP profile only (3 of 3 loads), opening Changes on a gone folder also toasted
  "no such folder: /private/tmp/qaI-setup/repo3-wt2"; a fresh Chrome profile never did, with reloads at 0–150 ms. Cause
  not found.
- **Updates:** right after start, before the first check, `wisp update apply` (checks on) says "There's no automatic
  update for this copy of wisp", and `update view` keeps that as `error` after the check found 2.0.1.
- **CLI:** `wisp validate <file>` without `-f` validates the folder's formulas or prints the usage. `wisp trigger off
  qa-nightlx` doesn't name the close trigger. A trigger with an unknown field is refused twice, in two forms. A runtime
  plugin switched off and on leaves its running task saying "wisp restarted while this was running". `delegate {agent:
  auto}` replies "Delegated: auto" for a task that ran on Claude. `wisp help held` says "no command or plugin named held"
  while `wisp held` prints its line (IS10's shape).
- **Plugins:** installing a file path says "No folder at …/math.js" (Projects says "is a file, not a folder").
- **Cloud:** the first "Sign in" button with no account shows no address field until a grid address is typed; a Codex
  sign-in started on the cloud computer by my mis-aimed click was left to time out (the box stopped with the grid).

## Cleanup done
- **Questions and cards:** every open guard question, card and question answered (Deny, or `"Deny: audit cleanup"`); no
  task is running or waiting. The 9 stale guard tasks of IS2 stay in review in the scratch home.
- **Settings:** `worktrees.projectAsk` on, `worktrees.worktree` on, `crew.tint` true; `git.maskEnvFiles` left `true` (the
  v1 import had set it `false`); the CDP profile's theme back to "match the system". The launcher pick (Claude Code ·
  Opus · max) is kept in the scratch home.
- **Channels:** all off; Telegram unlinked, its token and Slack's and Discord's removed; the fake Telegram switch file
  removed. ntfy left with Send me ticked, off.
- **Plugins:** `user.qa-theme` off, QA Pink removed, every built-in on.
- **Automations:** `qa-nightly` removed.
- **Data:** agent profiles `qa-auto-prof`, `qa-cli-auto`, crew `Qa Reviewer`, `Qa Typed`, machines `qabox`, `192.0.2.1`,
  `tnet9`, `oldbox` and connector `qa_tmp` removed; `repo3-wt2` and its `.git` file put back.
- **Cloud:** API key revoked (401), this computer off, qa-bob and qa-alice signed out (Machines empty).
- **Stopped by pid:** `wisp serve` :4991, `wisp-grid dev` :4992 (its box with it), the catcher :4999, the feed :4995,
  headless Chrome :4994. The scratch app's servers on :4996 exited to install, or were stopped. Nothing listens on
  4900–4999 and no process under `/tmp/qaI-setup` is left. Every fresh-browser script closed its browser in `finally`.
- **Kept for re-checks:** `/tmp/qaI-setup` (binaries in `fake/` and `bin/`, both homes, the repos, the grid data, `pkg/`
  (now 2.0.0) and `feed/`, the shots, scripts in `pw/` (`i-*` this round's) and `tools/`, `tgtap.py`, `tgmsg.py`, `um`,
  `a`, `w`, `wa`).
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir).
