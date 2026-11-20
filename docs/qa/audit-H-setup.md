# Audit H: agent and workspace setup (re-check of audit G setup, plus what landed since)

2026-11-20 · worktree `wisp-8fb64ec939b0` at `9e15602f` · findings only, no repo file edited except this one.

> **Build note:** the binaries (`wisp`, `wisp-fake-agent`, `wisp-fake-gh`, `wisp-grid`, `wisp-grid-hostd`) and the web
> bundle were built from the worktree at 16:18–16:23. The worktree differed from HEAD only in `design/v1-reference/`
> (another session's screenshots), so both are HEAD's.

**Setup** (G's method, in a fresh `/tmp/qaH-setup`; G's scripts copied from `/tmp/qaG2` with paths and ports moved):
- **Build:** `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, binaries copied to `/tmp/qaH-setup/fake`. The fake agent is
  copied under each harness name in `/tmp/qaH-setup/bin`. The web UI is `vite build --outDir /tmp/qaH-setup/web`.
- **Server:** `wisp serve --port 4991`.
  - `WISP_HOME=/tmp/qaH-setup/home`, `HOME=/tmp/qaH-setup/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` points at the fakes; `FAKE_RECORD` is on; `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` is a fake `npm`; `DO_NOT_TRACK=1`, `WISP_TELEMETRY=0`, `WISP_UPDATE_CHECK=0` on every start.
  - Restarted five times: the GS7 / restart check, two starts with `FAKE_LINGER` / `FAKE_HANG` for the delegate rows,
    the restart back, and the 401 re-check.
- **Catcher on `:4999`:** PostHog, Slack, Discord, Linear, ntfy and G's fake Telegram Bot API.
  - New this round: an ntfy reply queue (`ntfy-queue.jsonl`), so a notification's action body can be fed back as a
    phone tap.
  - Telegram taps are fed as `callback_query` updates.
- **Cloud:** `wisp-grid dev --port 4992 --data /tmp/qaH-setup/grid` (`wisp.localhost`). Only the Claude and Codex fakes
  are set; spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` come first on its `PATH`.
- **Desktop self-update:** a scratch `/tmp/qaH-setup/pkg/wisp.app` (HEAD's binary, `Info.plist` 2.0.0).
  - Served on `:4996` with its own `WISP_HOME`/`HOME` and `WISP_UPDATE_FEED=http://127.0.0.1:4995/releases`.
  - The feed on `:4995` serves v2.0.1, a zip of a stub `wisp.app` that only logs that it started.
  - **Deviation:** this one server had `WISP_UPDATE_CHECK` unset once HS10 was recorded. With it at `0`, wisp refuses
    every update, so the self-update the brief asks for can't run. Its feed is local, and the feed log shows no other
    request.
- **Test data:** fresh clones of G's `repo`, `repo2`, `repo3`, `repo4` (remote removed), `repo3-wt` and `repo3-wt2`
  (linked worktrees of `repo3`), and G's v1 home (`v1home`, plus a copy `v1b` with a second task).
- **UI:** Playwright from `/tmp/pw` with system Chrome.
  - A persistent headless Chrome on CDP `:4994` (profile `/tmp/qaH-setup/chrome-prof`), 1280 and 390.
  - For the layout sweep, fresh Chrome and WebKit at 1280 and 390, light and dark, each closed in `finally`.
  - Requests to `trywisp.dev`, `example.dev`, `example.invalid` and `github.com` were aborted; none was attempted.
- **Ports:** 4991, 4992, 4994, 4995, 4996, 4999.
  - The dev grid's box listened on an OS-picked loopback port (57357), outside 4900–4999. `wisp-grid dev` has no flag
    for it. It stopped with the grid.
- **Screenshots:** `/tmp/qaH-setup/shots/` (`h-*`).

> **Outbound requests and other things to know.**
> - No real harness ran: the grid's spy log is empty. `npm` is the fake.
> - SSH only went to `192.0.2.1` and `192.0.2.7` (TEST-NET), which time out.
> - Nothing reached grid.trywisp.dev, github.com, 4477, 4800, 4791, 4329 or 5180. `npm run doctor` was not run (it
>   probes 4800 and 4477).
> - Every `wisp` call ran with a `/tmp/qaH-setup` `WISP_HOME` and `HOME`. No real credential was read.
> - The agent token for the agent-side calls is the one the scratch server gave its fake agent: read from the fake's
>   recorded `--mcp-config`, as in G.
> - Six of the tasks in the scratch home were launched by mistake. A zsh loop passed `crew frob` and the like as one
>   argument, which made them prompts. They were stopped and are not counted.

## Counts

| | |
|---|---|
| Actions checked | 208 (G's 180 + 28 new) |
| Pass | 159 |
| Fail (a finding) | 17 |
| Not tested or blocked | 32 (each says why in the table: mostly G rows passed before and not re-run this round; also `npm run doctor`, the harness Update and start-up checks under `WISP_UPDATE_CHECK=0`, and sandboxes, which a dev grid doesn't make) |
| Audit G setup findings | 15 re-checked (GS1–GS13, F12, E8): 13 fixed (GS1 and GS4 each with a new finding beside it: HS4, HS9), 2 partly (GS2, GS6) |
| **New findings** | **14 (HS1–HS14): 0 high, 6 medium, 8 low** |

(HS: "H, setup". `docs/qa/audit-H-orchestration.md` already uses H1–H13.)

## Audit G setup findings, re-checked

| G# | Result | How checked |
|---|---|---|
| GS1 | **Fixed** (HS4 beside it) | `repo3-wt2` made after start (a linked worktree with a new file) → Settings → Worktrees: "1 worktree from git · In repo3 · 1 with uncommitted changes · Not now · Bring in" by 500 ms. One Bring in (fb 36 ms) made the task `qa-wt2` and took both offers down. The same folder is also offered under `repo3-wt` (HS4) |
| GS2 | **Partly** | ntfy: a question's options are signed buttons (`Staging`, `Prod`). A tap fed back through the reply topic answered the agent ("The user answered: B"). Telegram: the buttons are sent (`inline_keyboard` Staging / Prod), but every tap is refused (HS13). A setup card's buttons do nothing on ntfy (HS8) |
| GS3 | **Fixed** | Agent in a `repo2` task, `wisp_crew_add` with no `cwd`: the card says `cwd: /private/tmp/qaH-setup/repo2`; `wisp reply <id> Allow` → stored cwd is the project's folder |
| GS4 | **Fixed** (HS9 beside it) | `DB_PASSWORD=<secret> npm run migrate`, `export GITHUB_TOKEN=<secret>`, `curl -u ada:<secret>`, `https://ada:<secret>@…`, `mysql -p<secret>`, `postgresql://ada:<secret>@…`, `STRIPE_SECRET_KEY=<secret>`, `aws configure set aws_secret_access_key <secret>`, `PGPASSWORD=<secret>`, `sshpass -p <secret>`. `TOKENIZERS_PARALLELISM=false` and `docker run -p 8080:80` are kept. npm, yarn and openssl forms leak (HS9) |
| GS5 | **Fixed** | Connector card open → `wisp stop` → the agent's call returns "The card was taken back before the user answered it (the task stopped or its turn ended): nothing changed. Ask again if it's still needed." The silent loss itself wasn't seen in 6 cards (it was seen once in G) |
| GS6 | **Partly** | `agents.a1.harness: auto`, `tasks.default.agent: auto` and `tasks.t1.agent: auto` are refused; `wisp run t1 --formula auto3.wisp.yml` refuses too. A crew member's `agent: auto` is still made, exported and validated, and the agent's tools offer `auto` (HS7) |
| GS7 | **Fixed** | Before and after the restart, `connectors.list` has the same `test` for all four (qa_bad's error kept); the pane reads "1 tool" ×3 and qa_bad's "It wouldn't start: …" |
| GS8 | **Fixed** | `wisp connectors add --name qa_tmp --command node --args /…/mcp-echo.mjs,--x` → export `args: [/…/mcp-echo.mjs, "--x"]` |
| GS9 | **Fixed** | Scratch `.app`, wrong digest: `wisp update apply` → "✗ wisp-mac-arm64.zip didn't match the release's checksum. …", exit 1 |
| GS10 | **Fixed** (HS2 for `trigger`) | 15 groups: `wisp <g> --help` is byte for byte `wisp help <g>`. `formula` and `projects` aren't groups (`wisp help formula`: "no command or plugin named formula"), so they print the usage |
| GS11 | **Fixed** (checked by string) | zsh and fish files have `worktrees.max` and its values, the trigger verbs, no `router` and no "a wisp command"; `zsh -n` and `bash -n` pass |
| GS12 | **Fixed** | Home → Needs you: "qa H card · Asks: Ship to staging or prod? · Asking"; for a card: "Asks: Claude Code wants to add the connector “agent_echo”: runs node …" |
| GS13 | **Fixed** | Card headline: "Claude Code wants to add the connector “agent_echo”: runs node /tmp/qaH-setup/tools/mcp-echo.mjs --agent" with no backticks; the command still shows in mono below |
| F12 | **Fixed** | `plugins/grid-client/host/src/signin.rs:4–5`: "Signing in again adds the account beside the current one, which stays current; … (ADR-0109, superseding ADR-0061 §2)" |
| E8 | **Fixed** | Signed-out Codex 2's ⋯: "Rename · Make it a work account · Move up · Move down (disabled) · Sign out (disabled) · Remove" |

## Action table

Key: P = pass, HS# = fails with that new finding, — = not tested (why given). "server" means read back with
`settings.list`, `wisp export …` or a plugin read; "rec" is the argv/stdin the fake recorded; "fb" is ms from the click
to the first DOM change; "@150" is the UI 150 ms after the click. G's row numbers are kept.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists 7 harnesses and GitHub | P | As G |
| 2 | Codex Sign in (device code) | P | fb 36 ms; the box with steps and ABCD-12345 @100; Signed in after 4.4 s; no popup |
| 3 | Add another Codex account (Work) | P | Ownership dialog; the Work tab picked → `Codex 2:ready:work` |
| 4 | Move up / down | P | Rows swap @150; server order follows |
| 5 | Rename | P | Focused "Codex 2" → "QA Work" @150 and on the server |
| 6 | Sign out of an account | P | "Signed out · Sign in" @150; `signedOut` |
| 7 | Remove (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. …" · Remove account |
| 8 | Harness Update button | — | `WISP_UPDATE_CHECK=0` (the brief) turns the harness check off |
| 9 | Model default | P | Picker → Claude Code (fb 48) → Opus (fb 56); `runtime.launch` `models.claude: opus`; UI launch rec `--model opus` |
| 10 | Thinking default | P | Slider End: fb 9 ms, "Opus · max"; `efforts.claude: max`; UI rec `--effort max` |
| 11 | Lean runs | P | Select all → off (fb 32), back |
| 12 | Ask before running a changed harness | P | Flips @100 (fb 18), back |
| 13 | Re-check | — | Not re-run |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100 (fb 13–34), server key changes, restored |
| 15 | General: 4 selects | P | fb 17–53, restored |
| 16 | Theme, reload | P | `data-theme` @100; the account menu label flips; kept across reload |
| 17 | Notifications | P | Accept tasks from channels fb 16; Default project lists "Last used (old-app)", old-app |
| 18 | Desktop notifications | P (note) | Disabled, "Blocked — allow notifications …" (headless) |
| 19 | Memory | P | `learn.enabled`, fb 37 |
| 20 | Automations: Webhook triggers | P | fb 33 |
| 21 | Usage: 5 switches, 4 selects | P (note) | fb 19–47; a machine select put back writes its default (as G) |
| 22 | Worktrees: 3 selects | P (note) | fb 24–44; Clean up put back writes `7` (as G) |
| 23 | Privacy: 2 switches | P | fb 14–28 |
| 24 | Usage stats off → no PostHog | P | The switch is disabled: "Off (DO_NOT_TRACK, WISP_TELEMETRY=0 or CI)"; catcher got 0 PostHog calls all round |
| 25 | Updates (clone) | P | "A git clone: pull and restart to update"; Check now disabled |
| 26 | ⌘K, ⌘P, `[`, N | P | Both list; `[` hides and brings back; N focuses the textarea |
| 27 | Settings search | P | `ntfy` → Phone; `telemetry` → Privacy, Lean row; `worktree` → General, Worktrees, rows |
| 28 | All settings | P | 132 rows, no `runtime.auto`, no `router` |
| 29 | Setup: Make the formula | P | No `router:` |
| 30 | Setup: Import → plan → Apply | P | YAML error and `hello` plan nothing; "Changes 2 · leanRuns all → off · worktrees.max 80 → off"; "Adds 1 · project repo3-wt · Changes 1 · settings.editor cursor → zed"; "Imported" |
| 31 | Diagnostics | P | Report JSON 8.1 KB |
| 32 | Sandboxes | — | A dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Other folder…: focus | P | Focus in `~/code/my-app or a GitHub URL` |
| 34 | Missing / file / relative | P | "No folder at …", "… is a file, not a folder: …", "repo2 isn't a full path: …" |
| 35 | `file://` URL | P | `file:///tmp/qaH-setup/repo4` → repo4 |
| 36 | Duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already old-app" (both) |
| 37 | Ghost folder via import | — | Not re-run |
| 38 | Remove a project (confirm) | P | repo4: fb 31, "Remove “repo4” from wisp? Its files are kept." |
| 39 | ⌘P switcher | P | All projects, old-app, Add project…, Manage projects…, Repo history |
| 40 | Tasks in a project | P | UI launch rec cwd `home/worktrees/repo-<id>`, `--model opus --effort max` |
| 41 | A second project for one folder | — | Not re-run |

### Machines, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Add `qa@192.0.2.7` | P | fb 35, Connecting @150, then "No answer · 192.0.2.1 doesn't answer …" |
| 43 | Edit… | P (note) | Every field filled; Esc still closes the whole Manage dialog |
| 44 | Remove (confirm) | P | oldbox: fb 24, "Remove oldbox from wisp? …" |
| 45 | `discover`, `hosts`, `reachable` | P | `(none)`, `(none)`, this computer |
| 46 | This computer | P | Memory, CPU, Disk, "wisp itself 57 MB" |
| 47 | Pause / Resume | P | fb 30, "Automations paused until 17:40" @150; Resume fb 44 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in (popup, dev sign-in, Allow) | P | Popup `/activate?code=…`; "Device connected"; `cloud status`: qa-alice |
| 49 | Don't allow | P | Pane back to Add an account with "Sign-in was declined in the browser." (by 5 s) |
| 50 | This computer on the grid | P | fb 57, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4992" @150 |
| 51 | Grid restart | P (note) | Box gone; "retrying · The grid didn’t answer: error sending request for url (…)" (raw error); online after 15 s |
| 52 | API key create, use, revoke | P | fb 61; cloud computer `/api/health` 200 with it, 401 without; Revoke confirm → 401 |
| 53 | Add qa-bob | P | qa-alice stays current |
| 54 | Switch and back | P | fb 79; Current moves; this computer follows the account |
| 55 | Sign out of qa-bob | P | fb 67, "Sign out of qa-bob on wisp.localhost:4992 here? …" |
| 56 | Cloud computer woken by use | P | asleep → online on the API-key request; spy log empty |
| 57 | `computer off`, `cloud signout` | P (note) | "this computer: off · http://…" (still prints the address), "✓ signed out" |

### Connectors and MCP
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector | P | qa_echo "1 tool" by 4 s |
| 59 | Edit: env | P | Server `env: [{QA_VAR, set: true}]` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it" |
| 61 | Connectors reach a run | P | rec `--mcp-config` has `old-echo`; `--allowedTools mcp__old-echo mcp__wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, …" |
| 63 | Presets | — | They show only with no connectors |
| 64 | `wisp mcp` | P | 262 tools on stdio, 258 at `/api/mcp`, no `router_*` (no `automations_*`: HS6) |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy Test | P (note) | fb 52; the catcher got it; nothing on screen says it was sent |
| 66 | Telegram bad token | P (HS14) | A well-shaped token: "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| 67 | Slack bad tokens | P | "Slack: invalid_auth. Check the tokens …" |
| 68 | Discord bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | Switch disabled |
| 71 | Off and tokens removed | P | All off; export lists only ntfy |
| 72 | No "push" card | P | 6 cards |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` | P | `user.qa-theme` off |
| 74 | Install from a folder | P | fb 45 |
| 75 | Switch on (untrusted) | P | fb 60; `--accent` stays `#2f5bea` |
| 76 | Trust / Untrust | P | "Trust QA Pink? …" → `#ff1493`, send rgb(255, 20, 147); Untrust → `#2f5bea` |
| 77 | Off, Remove | P | fb 56 / 58; "Remove QA Pink? …"; row gone @200 |
| 78 | Export after Remove | P | Only `user.qa-theme` |
| 79 | CLI enable/disable | P | `on` / `off` |
| 80 | Built-in off and on (learn) | P | fb 60; `learn.view` → `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the task @150; `wisp ls`: `no project` |

### Import, init, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import v1home --dry-run` | P | 14 to add; "!" lines; "·" notes; "and the history of 1 task" |
| 83 | `wisp import v1home --yes` | **HS1** | "✓ applied 14 changes", then "✓ 0 tasks imported, 0 unchanged" |
| 84 | v1 ntfy | P | Polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | launch, subagentModel, worktrees.max, maskEnvFiles, leanRuns, editor, notify, limitHold, retentionDays |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at …" |
| 87 | Imported task's project | P | From an absolute path: `v1sess0001 · old-app` |
| 88 | v1 default agent | P | `runtime.launch` agent codex |
| 89 | `wisp init` | — | Not re-run |
| 90 | `settings get/set` | P | "must be one of off, 20, 40, 80" (1); "no plugin declares the setting nope.key" (1) |
| 91 | export → validate → diff | P | 27 resources validate; "27 unchanged" |
| 92 | `validate`/`apply` errors | P | 173, 174 |
| 93 | `completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass |
| 94 | Bash completion | P (string) | `trigger` verbs, `worktrees.max` values in the file |
| 95 | `wisp skill install` | — | Not re-run |
| 96 | `npm run doctor` | — | Probes 4800/4477 |
| 97 | `wisp <group> <unknown word>` | P | crew, trigger, connectors, computer: verbs listed, exit 2 |
| 98 | `--agent frob` | P | "✗ no harness frob: --agent takes …" (exit 1) |

### Auto off everywhere
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `wisp router frob` | P | "… Auto is off (runtime.auto), so the router is put away" (exit 2) |
| 100 | Launcher picker | P | 7 harnesses, Factory, Set up another agent…; no Auto |
| 101 | ⌘K | P | No Auto entry for "auto", "harness", "model" |
| 102 | Crew editor harness | — | Not re-run (the crew row didn't show at 1280) |
| 103 | All settings | P | 0 `router`/`runtime.auto` rows |
| 104 | Settings search for auto | — | ⌘K only |
| 105 | Setup / formula | P | No `router:` |
| 106 | CLI and agent tools | **HS7** | The agent's `wisp_crew_add`, `wisp_launch`, `wisp_task`, `delegate` offer `auto` |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `wisp plugin` identical before and after |
| 108 | WebKit 1280, home + 20 panes | P | Light and dark: no overflow, no console error; "wraps" are 2-line list rows and one-line 44 px buttons |
| 109 | Chrome 390 | P | The same (one 401: HS12) |
| 110 | WebKit 390 | P | The same; Usage's "1 hour" buttons are 44 px tap targets |
| 111 | Harness check at start | — | `WISP_UPDATE_CHECK=0` |
| 112 | `wisp-grid dev --help` | P | Its usage, 7 lines |

### Re-checks of C's, D's and E's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects counts | P | "/tmp/qaH-setup/repo · 2 tasks", "repo3-wt · 0 tasks" |
| 114 | Setup → Import errors | P | "pasted.yml: a formula is a map …" |
| 115 | Plugins → Install errors | — | Not re-run |
| 116 | `export --format xml` | P | exit 2 |
| 117 | Project colour | P | 9 colours, fb 27; `color: amber` |
| 118 | Nested CLI output | P | `channels status` has `transport` |
| 119 | ⌘K "model" | P | Agents first |
| 120 | Sign-in box steps | P | 2 |
| 121 | Copy the code | — | Not re-run |
| 122 | Paste the code | — | The fake has no paste step |
| 123 | Cancelled first sign-in | — | Not re-run |
| 124 | Sign-in cut by a restart | — | Not re-run |
| 125 | Sign-in that times out | — | 15 min |
| 126 | Harness Sign out that stays signed in | — | Not re-run |
| 127 | Formula with `off` values | P | 30 |
| 128 | v1 recipe | — | Not read back |
| 129 | `wisp ls` with no project | P | 81 |
| 130 | Dark at 1280 / 390, both engines | P | 108–110 |
| 131 | MCP with Auto off | P | No `router_*` (but HS7) |
| 132 | Update all | — | `WISP_UPDATE_CHECK=0` |
| 133 | Personal / work switch | P | "Make it a personal account" fb 51 → `personal` on the server |
| 134 | A signed-out account's menu | P | E8 |
| 135 | Add connector errors | P | "A name, so agents can tell its tools apart", "The command that starts it", "It wouldn't start: definitely-not-a-command-qa: not found"; duplicate: "There's a connector called qa_echo already" |
| 136 | A CLI launch in a project | **HS5** | rec cwd `home/worktrees/repo2-<id>`, but no `--model opus --effort max` |
| 137 | Add machine bad addresses | — | Not re-run |
| 138 | A plan that adds a setting | P | 82 |
| 139 | Add another account → Cancel | — | Not re-run |
| 140 | Default project | P | 17 |
| 141 | Worktrees: Bring in | P (HS4) | GS1 |
| 142 | Harness Sign out toast | — | Not re-run |

### Re-checks of F's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 143 | v1 connector env | P | `QA_VAR: ${secret:connector-old-echo-QA_VAR}` |
| 144 | Every built-in off and on | — | learn and worktrees only |
| 145 | An off plugin's CLI | P | "✗ wisp learn is off: … Switch it on: wisp plugin enable wisp.learn" (exit 2) |
| 146 | Settings plugin off | P | "wisp.settings can't be switched off: …" (exit 1); same for wisp.plugins |
| 147 | Worktrees count after off/on | — | Not counted |
| 148 | `wisp update` on a clone | P | `method git` |
| 149 | `update view/check/install` | P | Exit 1 on apply |
| 150 | A channel on and back off | P | 71 |
| 151 | `connectors remove <id>` | P | Removed |
| 152 | `update apply` on a clone | P | Exit 1 |
| 153 | Phone switches | P | 5 flip @100 (fb 38–64) and back; Email disabled |
| 154 | Grid stopped and started | P | 51 |

### Re-checks of G's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 155 | Connector card on the task page | P | "Wants to change your setup · … · needs AGENT_KEY from you · It would run … · Allow · Deny" |
| 156 | The agent answers its own card | — | Not re-run |
| 157 | Crew card, `wisp reply <id> Allow` | P | GS3 |
| 158 | Typed deny from the CLI | P | "✓ You denied: add crew member Tg Member (“Deny: audit cleanup”)" |
| 159 | Typed deny in the reply box | — | Not re-run |
| 160 | Project card for an existing project | — | Not re-run |
| 161 | A card at 390 | P | Card 16–374 of 390, no overflow (Chrome) |
| 162 | A card that went away | P | GS5 |
| 163 | Telegram: who may answer a card (typed) | — | Not re-run (buttons: HS13) |
| 164 | Questions to the phone | **HS8 / HS13** | ntfy questions: P. ntfy setup cards: HS8. Telegram taps: HS13 |
| 165 | Asking row | P | GS12 |
| 166 | Card headline | P | GS13 |
| 167 | Desktop self-update end to end | P | Updates: "wisp 2.0.1 is out. Installing it restarts wisp · Update now" → "Restarting. This page reconnects once wisp is back" @150. Feed served the zip; old server exited 0 at 16:56:57; new bundle started 16:56:59; `Info.plist` 2.0.1 |
| 168 | Wrong checksum | P | GS9 |
| 169 | `wisp shell-init zsh/bash/fish` | P | 15–17 lines; `zsh -n`, `bash -n` pass; `term/<tty>.jsonl` 0600 |
| 170 | Terminal context | P | "`… ` → exit 3" lines with `--show-context` |
| 171 | Secret inline in a command | P | GS4 |
| 172 | Formula with goals, factory | — | Not re-run |
| 173 | Unknown names in a formula | — | Not re-run (only `auto`) |
| 174 | `auto` in a formula with Auto off | P | GS6's formula half |
| 175 | Connectors after a restart | P | GS7 |
| 176 | `connectors add --args A,B` | P | GS8 |
| 177 | `wisp <group> --help` | P (HS2) | GS10 |
| 178 | zsh and fish completion | P | GS11 |
| 179 | F12's docs | P | F12 |
| 180 | `wisp.settings`, `wisp.plugins` switches | P | 146 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 181 | `wisp import <relative v1 home>` | **HS1** | `v1b`: 0 imported; `/tmp/qaH-setup/v1b`: 1 imported |
| 182 | `wisp trigger off` / `on` (H13) | P | "✓ trigger qa-nightly is off: it fires no more (wisp trigger on qa-nightly switches it back on)"; ls `off`/`on`; `wisp diff` 1 unchanged both times; export `enabled: false` |
| 183 | `wisp trigger off nosuch` | P | "✗ no trigger named nosuch (wisp trigger ls lists them)" (exit 1) |
| 184 | `wisp trigger rm` | P | "✓ removed trigger qa-nightly" |
| 185 | `wisp triggers off/on` (the alias) | P (HS2) | Works; not offered |
| 186 | `wisp help trigger` / `trigger --help` | **HS2** | "wisp trigger needs more: wisp --help" |
| 187 | Misspelled launch flags (H2) | P | `--mdoe`, `--agnet`, `--dettach`: "✗ wisp takes no --mdoe: did you mean --mode? Nothing was launched …" (exit 1); `--at 23:55`: the start-later hint; task count unchanged |
| 188 | `--modle opus` | **HS3** | "did you mean --mode?" → `--mode opus`: "✗ runtime.create: unknown variant `opus`, expected one of `plan`, `edits`, `auto`" |
| 189 | Bring in looks on open (E6) | P | Offer at 500 ms after opening the pane |
| 190 | One worktree, two projects | **HS4** | Offered under repo3 and repo3-wt |
| 191 | CLI launch's model and effort | **HS5** | 136 |
| 192 | On-the-fly subagent (H1) | P | Lead with no team, `delegate {agent: codex}` → task "check the add function" on codex with the lead as parent |
| 193 | A second unnamed codex task | P | Continued the same member (no `codex-2`) |
| 194 | `delegate` to an unknown harness | **HS11** | `{"reports":[]}` |
| 195 | `delegate {agent: auto}` with Auto off | P | Ran on Claude (the owner's Auto-off rule) |
| 196 | Agent tools with Auto off | **HS7** | 106 |
| 197 | ntfy: a question's buttons | P | GS2 |
| 198 | ntfy: a tap answers | P | "The user answered: B" |
| 199 | ntfy: a setup card's buttons | **HS8** | Crew card and keyed connector card: Allow tapped, nothing happens |
| 200 | Telegram: a question's buttons | P | `inline_keyboard` Staging / Prod |
| 201 | Telegram: a tap | **HS13** | "That button is out of date." (answer and Done) |
| 202 | A card taken back answers its agent | P | GS5 |
| 203 | Terminal context: npm, yarn, openssl | **HS9** | Sent as typed |
| 204 | Terminal context: r194's cases | P | GS4's row |
| 205 | `wisp update apply` with checks off | **HS10** | "There's no automatic update for this copy of wisp. Download it from …" |
| 206 | Desktop update UI with checks off | P | "Update checks are off here (WISP_UPDATE_CHECK=0 or CI)" |
| 207 | Trigger switch over MCP (H13) | **HS6** | No `automations_*` tool for the user or an agent |
| 208 | First page after a restart (cloud machine on) | **HS12** | 401 on `/api/peer/<id>/api/events` |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### HS9 · M · Terminal context still sends npm and yarn tokens and openssl passwords (security)
- **Steps:** `eval "$(wisp shell-init zsh)"` → a failing command → `wisp "check the error above" -d --show-context`,
  once for each line below.
- **Actual:** each goes into the context block and the agent's prompt as typed (rec `claude.jsonl` stdin has the full
  `npm_…` token):
  - `npm config set //registry.npmjs.org/:_authToken npm_Ab3dE5fG7hI9jK1lM3nO5pQ7rS9tU1vW3xY5z`. This is npm's own
    documented form, and the token has npm's real 40-character shape. The `=` form is masked.
  - `yarn config set npmAuthToken s3cr3tvalue123456`.
  - `openssl enc -aes-256-cbc -k hunter2qa -in a.txt` and `openssl rsa -passin pass:hunter2qa -in k.pem`.
- **Also seen** (over-masking, not a leak):
  - `gh auth login --with-token <<< ghp_…` → `--with-token <secret> <secret>` (the `<<<` is eaten).
  - `curl -H "X-Api-Key: abcd…"` → `"X-<secret>"` (the header name is cut).
  - `echo "//…/:_authToken=…" >> ~/.npmrc` loses its closing quote.
- **Expected:** a value after a secret-named key in `npm|pnpm|yarn config set <key> <value>` is masked, as the `=`
  form already is. So are `npm_` tokens by shape, and `-k`, `-pass pass:`, `-passin pass:` for openssl.

### HS13 · M · Every Telegram button on a task says "That button is out of date."
- **Steps:** Telegram linked (fake Bot API, chat 1001) → an agent asks "TG2: red or blue?" with options → tap Blue. The
  `callback_query` has the button's own `callback_data`, the sent message's id and the current date. Then tap Done on
  a finished task's message.
- **Actual:**
  - Both taps get `answerCallbackQuery {"text":"That button is out of date."}`, and the buttons are removed.
  - The question stays open, and the agent waits until it times out.
  - The `callback_data` is `a:1:<sig>:<session>` (59 bytes). The ask id didn't fit in Telegram's 64 bytes, so it isn't
    there.
  - `plugins/telegram/host/src/lib.rs` `on_tap` verifies with `r: t.reference.clone().unwrap_or_default()`, which is
    `""`. `tap.rs`'s header says a missing ref "is read back when the tap comes … from that task's open ask or last
    turn". That doesn't happen, so the signature over the ask id never matches.
  - GS2's fix gives Telegram's buttons that ask id (`answer_buttons`), so every button an ask or a finished task gets
    is refused. A typed reply still works.
- **Expected:** a tap answers, as on ntfy (198); the ref is read back from the open ask or the last turn before
  verifying.

### HS8 · M · A setup card's Allow/Deny buttons on ntfy do nothing
- **Steps:** ntfy on → an agent asks for `wisp_crew_add` (or `wisp_connector_add` with `env: [AGENT_KEY]`) → the
  notification arrives → tap Allow (the action's body fed back on the reply topic, the same way that answered a plain
  question in 198).
- **Actual:**
  - The notification has `Allow` and `Deny` actions with `"clear": true`, so the phone drops the notification.
  - wisp reads the tap (its `since` cursor moves), but nothing changes: the card stays on the task, no member or
    connector is added, the agent's call keeps waiting, and nothing is sent back to the phone.
  - The keyed connector card is one the GS2 fix note says gets no buttons ("an ask only wisp can answer (… a connector's
    keys) gets none"). `needs_wisp()` (`plugins/channels/api/src/views.rs:142`) checks `permission`, `clash` and
    `connector`, not a setup card.
  - On Telegram, G found a setup card can be answered only by the linked person's typed reply (ADR-0113).
- **Expected:** either the tap answers the card (for the linked person, as ADR-0113 allows typed replies), or a setup
  card is sent without buttons and with "answer it in wisp". The user should never see buttons that silently do
  nothing.

### HS1 · M · `wisp import <relative path>` imports the settings but none of the task history, and says "0 tasks imported"
- **Steps:** in `/tmp/qaH-setup`, run `wisp import v1home --yes` (a v1 home with one task in `sessions/`).
- **Actual:**
  - The plan says "and the history of 1 task", then "✓ applied 14 changes · ✓ 0 tasks imported, 0 unchanged", exit 0.
    `wisp ls --all`: "No tasks."
  - Run again with an absolute path, it imports the task: `runtime.import` with the absolute folder gives
    `{"imported":1}`, and `wisp import /tmp/qaH-setup/v1b --yes` gives "✓ 1 task imported".
  - `crates/wisp/src/history_cli.rs` sends `sessions.display()` as typed. The server resolves it from its own folder
    (here `/tmp/qaH-setup/repo`), finds nothing, and reports 0 with no error. `failed` isn't printed either.
- **Expected:** the CLI sends the folder resolved against its own working directory. A folder that the plan counted
  tasks in but the import didn't reach should be an error, not "0 imported".

### HS5 · M · A task launched from the CLI runs on the saved harness but not its saved model and effort
- **Steps:** New task → picker → Claude Code · Opus, Thinking Max (`runtime.launch`
  `{agent: claude, models: {claude: opus}, efforts: {claude: max}}`) → in `repo2`, `wisp "H launch check" -d`.
- **Actual:**
  - The CLI task ran Claude with no `--model` and no `--effort` (rec argv).
  - The same prompt from New task ran with `--model opus --effort max`.
  - `Runtime::last_pick` (`plugins/runtime/host/src/runtime.rs:431`) reads only `agent`.
  - The setting's own title says "a task that names no harness runs on its agent". `v1_formula.rs:360` says v1's
    default is the one "a task that names no harness runs on too (CLI, MCP, crew)".
- **Expected:** a task that names no harness also gets that harness's saved model and effort (CLI, MCP, crew, channels),
  as New task does. The user who picked Opus · max doesn't silently get the CLI's default.

### HS11 · M · `delegate` to an unknown harness drops the task without a word
- **Steps:** a running lead with no team → its agent calls `delegate {tasks: [{agent: "frob", prompt: "x"}]}`.
- **Actual:**
  - The result is `{"reports":[]}`. No error, and no task is made.
  - The H1 fix note says the call "refuses an unknown harness for that task only". A known harness works (192), and
    `{subagent: "nosuch"}` is refused in words ("This session has no subagents yet. Name a harness …").
- **Expected:** a per-task error the lead can read ("no harness frob: claude, codex, …"), as `--agent frob` gives.

### HS7 · L · With Auto off, agents are still offered `auto`, and a crew member an agent adds is stored as `agent: auto` (GS6 partly)
- **Actual:**
  - The task agent's `wisp_crew_add` and `wisp_crew_edit` say "Harness: auto, claude, … Default auto". `wisp_task`'s
    and `delegate`'s `agent` enums start with `auto`; `wisp_launch` lists `auto` first.
  - `wisp_crew_add` with no `agent` stored `Qa Reviewer · auto`. `wisp crew` shows `auto`.
  - `wisp export` writes `crew: [{name: Qa Reviewer, agent: auto, …}]`, and `wisp validate` takes it (27 resources).
    `agents.a1.harness: auto` and `tasks.*.agent: auto` are refused in the same state (GS6's fix).
  - A run on `auto` goes to the default harness (195), so nothing breaks, but the setup now holds a value Auto-off says
    is put away.
- **Expected:** with `runtime.auto` off, no `auto` in the tool schemas, a crew member defaults to the default harness,
  and a formula's crew `agent: auto` is refused like the others.

### HS6 · L · The new trigger switch (and every automations command) isn't reachable over MCP
- **Actual:**
  - `tools/list` over stdio (262 tools), at `/api/mcp` (258), and for a task's agent has no `automations_*` tool.
    There is none for `channels`, `worktrees` or `settings` either.
  - The H13 fix note says `automations.trigger.switch` and `automations.trigger.remove` are "exposed over MCP like every
    catalogue command".
  - The automations manifest doesn't say `uiOnly`. `tools/check`'s `rust-mcp` rule only checks registration.
- **Expected:** per ADR-0040, `trigger switch`/`remove` (at least) reachable from a harness, or the commands marked
  `uiOnly` with a reason and the fix note corrected.

### HS2 · L · `wisp help trigger` says "needs more"; the `triggers` alias doesn't offer `off`/`on`
- `wisp help trigger` and `wisp trigger --help` (identical, so GS10's rule holds) print only "wisp trigger needs more:
  wisp --help". Other groups list their verbs.
- `wisp triggers off qa-nightly` works, but `wisp triggers frob` lists "add, ls, list, rm, remove, fire, test, log,
  resume, approve, sources" with no `off`/`on`. The bash completion's `triggers)` line lacks them too; `trigger)` has
  them.
- **Expected:** `wisp help trigger` lists its 13 verbs; `triggers` names the same verbs as `trigger`.

### HS3 · L · `--modle opus` is steered to `--mode`, whose error is a raw parser message
- `wisp "H typo" --modle opus` → "✗ wisp takes no --modle: did you mean --mode?". The most common slip of `--model` (a
  swap) is one deletion from `--mode`.
- Following it, `wisp "H typo" --mode opus` → "✗ runtime.create: unknown variant `opus`, expected one of `plan`,
  `edits`, `auto`" (exit 1). That is a serde message with backticks.
- **Expected:** suggest both close flags (or prefer `--model` when the value is a model), and word the mode refusal:
  "--mode takes plan, edits or auto (opus is a model: --model opus)".

### HS4 · L · Bring in offers one worktree twice when a project is a linked worktree of another
- `repo3` and `repo3-wt` (a linked worktree of repo3, added as a project by action 30) both get the offer for
  `repo3-wt2`: "1 worktree from git · In repo3 …" and "1 worktree from git · In repo3-wt …".
- `worktrees_offer` holds both rows.
- One Bring in made one task and cleared both, so nothing doubles. The pane still shows the same folder as two offers.
- **Expected:** one offer per folder, under the project whose repository owns it.

### HS10 · L · `wisp update apply` with checks off blames the copy, not the setting
- On the scratch `.app` with `WISP_UPDATE_CHECK=0`: `wisp update apply` → "✗ There's no automatic update for this copy
  of wisp. Download it from https://github.com/WispAgents/wisp/releases/latest" (exit 1).
- The same copy updates fine with checks on (167).
- Settings → Updates says the right thing: "Update checks are off here (WISP_UPDATE_CHECK=0 or CI)".
- **Expected:** the CLI says checks are off and how to turn them on, as the pane does.

### HS12 · L · Right after a restart, the first page load gets a 401 from the cloud computer's event stream
- With a cloud computer under Machines, restart `wisp serve`, then open Home within ~5 s.
- The console shows `http 401: GET /api/peer/234628/api/events?…&passive=1` and "Failed to load resource: … 401
  (Unauthorized)".
- A load 20 s later is clean. Seen in the first browser of the layout sweep and reproduced once by hand.
- **Expected:** the peer stream waits for the grid link (or retries quietly) instead of failing with a 401.

### HS14 · L · A malformed Telegram token's toast starts with an internal field name
- Phone → Telegram → paste `123456:badtoken` → toast "secrets.token: That doesn’t look like a bot token: @BotFather
  gives one like 123456:ABC…".
- **Expected:** the sentence without `secrets.token:`.

---

## Smaller notes (not counted)
- **Trigger files:**
  - A trigger whose `project` names no project plans as "1 to add". Only `apply` refuses it ("project: no project is
    named "repo" … (nothing was changed (0 undone))"). `wisp validate` didn't catch it either.
  - `wisp trigger offf qa-nightly` (a typo plus a name) launched a task "trigger offf qa-nightly". That's the QA-round-14
    rule (two words after a group are a prompt). A one-word slip is refused.
- **Launch flags:** an unknown short flag (`wisp "H typo" -x`) is still read as part of the prompt and launched.
- **Formula wording:** the `auto` refusal repeats its path: "agents.a1: agents.a1: harness: auto: …".
- **Cloud:**
  - The Add an account section still shows "Sign-in was declined in the browser." when reopened minutes after a
    decline.
  - The retry line shows a raw request error ("error sending request for url (http://…/tunnel)").
- **Setup cards:** a second question or card replaces the first open question, whose agent reads "cancelled" (as in
  G).
- **Still as in G:**
  - ntfy Test has no on-screen confirmation.
  - Esc in Edit closes the whole Manage dialog.
  - `connectors.list` still stores `addedAt: 0` (the printer hides it now).
  - `connectors test <id>` prints the list.
  - `computer off` prints the address.
  - A Usage or Worktrees select put back writes its default explicitly.
  - `learn view` while off says "(wisp.learn was switched off)" twice over.
  - Plugins lists `router · On` while Auto is off.

## Cleanup done
- **Setup cards:** every open card and question was answered (`wisp reply … "Deny: audit cleanup"`). No task is running
  or waiting.
- **Settings:** `settings.editor` is back to `cursor`. The v1 import's values and the launcher pick (Claude Code · Opus ·
  max) are kept in the scratch home. The CDP profile's theme is back to System.
- **Channels:** Telegram, Slack, Discord and GitHub are off and their tokens removed; ntfy is off; the fake Telegram
  switch file is removed.
- **Plugins:** `user.qa-theme` is off, QA Pink is removed, and every built-in is on (learn and worktrees switched back).
- **Automations:** the trigger `qa-nightly` was removed with `wisp trigger rm`.
- **Cloud:** this computer is off, qa-bob and qa-alice are signed out, and the API key is revoked.
- **Stopped by pid:** `wisp serve` :4991, `wisp-grid dev` :4992 (its box went with it), the catcher :4999, the feed
  :4995, headless Chrome :4994. The scratch app's server on :4996 exited itself to install the update; the 2.0.1 stub
  only logged and exited.
  - No process under `/tmp/qaH-setup` is left, and nothing listens on 4990–4999.
  - Every fresh-browser script closed its browser in `finally`.
- **Kept for re-checks:** `/tmp/qaH-setup`.
  - Binaries in `fake/` and `bin/`; both homes; the repos (including `repo3-wt2`); the grid data; `pkg/` (now 2.0.1)
    and `feed/`; the shots.
  - Scripts in `pw/h` (this round's), plus `pw/g`, `pw/f`, `pw/s` and `tools/`. `catch.mjs` now has the ntfy reply
    queue. Also `ctxtest.sh`, `ctx-cmds.txt`, `wa`, `w`.
  - The scratch home also holds the crew member Qa Reviewer (`agent: auto`, HS7), the machine `qabox` (TEST-NET) and
    about 50 fake tasks.
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir).
