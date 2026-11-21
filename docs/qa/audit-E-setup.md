# Audit E: agent and workspace setup (round E, re-check of audit D plus what it missed)

2026-11-19 · worktree `wisp-8fb64ec939b0` at `e0cb033e` · findings only, no repo file edited except this one.

> **Build note:** the worktree differed from HEAD only in `design/v1-reference/` (screenshots and one metrics file from
> another session) and an untracked `docs/qa/audit-E-orchestration.md` (another auditor). No code file was changed, so
> the binaries and the web bundle are HEAD's.

**Setup** (copied from audit D, in a fresh `/tmp/qaE2`):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`), `wisp-grid` and `wisp-grid-hostd` were built at HEAD
  with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, then copied to `/tmp/qaE2/fake`. Copies of the fake agent under each
  harness name went to `/tmp/qaE2/bin`. The web UI was built at HEAD with `vite build --outDir /tmp/qaE2/web`.
- **Server:** `wisp serve --port 4951` with `WISP_HOME=/tmp/qaE2/home` and `HOME=/tmp/qaE2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` pointed at a fake `npm` from the first start.
  - The first start had `WISP_UPDATE_CHECK=0`. The second (the restart check, action 107) left it unset, to re-check
    the update check and the harness Update buttons.
- **Catcher on `:4949`:** PostHog, Slack, Telegram, Discord, Linear and the ntfy server all pointed at it, with the same
  answers as in D. The v1 home's ntfy server was moved from D's `:4929` to `:4949`.
- **Cloud:** `wisp-grid dev --port 4952 --data /tmp/qaE2/grid` (`wisp.localhost`).
  - Only `WISP_CLAUDE_BIN` and `WISP_CODEX_BIN` were set.
  - Spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` were first on its `PATH` and log any call.
- **Test data:** fresh clones of D's `repo`, `repo2` and `repo3` (remote removed, no leftovers), plus `repo4` (a clone
  that is not a project) and `repo3-wt` (a linked `git worktree` of `repo3`). D's v1 home, paths moved to `/tmp/qaE2`.
  The machine added in the UI used `qa@192.0.2.1` (TEST-NET).
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - A persistent headless system Chrome over CDP `:9374` (own profile in `/tmp/qaE2/chrome-prof`), at 1280 and 390.
  - For the layout sweep, fresh Chrome and WebKit browsers at 1280 and 390, each in light and dark, closed in
    `finally`.
  - Every context aborted requests to `trywisp.dev`, `example.dev` and `example.invalid`. None was attempted this round.
- **Screenshots:** `/tmp/qaE2/shots/` (148 named `e-*`).
- **Stopped at the end:** the server, the grid (and its boxes), the catcher and Chrome. No `/tmp/qaE2` process is left
  and nothing listens on 4949, 4951, 4952 or 9374. `/tmp/wisp-target-fixer` is shared and was left in place.
  `/tmp/qaE2` stays for re-checks.

> **Outbound requests and other things to know.**
> - No `npm view` reached the real registry: the fake `npm` logged the three calls (action 111).
> - No real harness CLI ran: the grid's spy log stayed empty.
> - SSH only went to `192.0.2.1`, which times out.
> - `npm run doctor` was not run: it opens TCP connections to 4800 and 4477, which this round must not touch.
> - Nothing reached grid.trywisp.dev, 4477, 4800 or 4791. No real credential was read. Every `wisp` call ran with
>   `WISP_HOME=/tmp/qaE2/home`, and `wisp-grid dev` with `HOME=/tmp/qaE2/realhome`.
> - **An auditor slip, and what it showed.** A shell loop passed five CLI checks to `wisp` as one argument each
>   (`wisp "connectors list"`, …). That launched five fake-Claude tasks, with the audit worktree itself as their folder.
>   They ran there in place, not in a worktree of their own. This led to E1. They changed no file (`git status`
>   unchanged, no `wisp/task-01a1f499…` branch), and they were deleted from the scratch home at once.
> - The fake `gh` doesn't answer `gh auth status`. So Settings → Agents → GitHub shows no state and no Sign in (action
>   1, a note). A real `gh` answers it.

## Counts

| | |
|---|---|
| Actions checked | 142 (D's 131 + 11 new) |
| Pass | 122 |
| Fail (a finding) | 10 |
| Not tested or blocked | 10 (32 sandboxes on a dev grid, 37 ghost folder, 72 push card, 96 `npm run doctor`, 115 plugin install error paths, 117 project colour, 122 paste/Continue, 123 a cancelled first sign-in, 124 a sign-in cut by a restart, 125 a sign-in that times out) |
| Audit D findings | 12: 10 pass (fixed), 2 partly fixed (D4 → E5, D11 → E4) |
| **New findings** | **9 (E1–E9): 0 high, 2 medium, 7 low** |

## Audit D findings, re-checked

| D# | Result | How checked |
|---|---|---|
| D1 | **P** | Copy beside the code turns into "Copied" (a check) at 36 ms, and back to "Copy the code" by 1.8 s (`e-14-copy-after.png`) |
| D2 | **P** (wording: E6) | `wisp export router` → "✗ no kind router: accounts, …" (exit 2); the kinds list has no `router`; `validate`/`apply` of a `router:` section refuse it (exit 1). Help, completion (bash/zsh/fish), ⌘K, All settings and Setup have no router either |
| D3 | **P** | `worktrees.max: off`, `worktrees.cleanup: off`, `runtime.leanRuns: off` plan as `"7" → "off"`; Setup → Import shows "all → off", "80 → off" with Apply enabled |
| D4 | **Partly fixed → E5** | `connectors list` names each connector; `settings list` prints `runtime.launch  agent claude, efforts (cl…`; `machine read` and `worktrees view` print their rows. `channels status` still prints seven unnamed rows |
| D5 | **P** | `wisp settings get nope.key` → "✗ no plugin declares the setting nope.key" (exit 1), as `set` does |
| D6 | **P** | `~ setting/runtime.launch` / `agent: "claude" → "codex"`; `~ setting/worktrees.cleanup` / `"7" → "off"`; no bare colon |
| D7 | **P** | The cast task reads `no project` in `wisp ls`; a folder outside every project reads its name (`repo4`) |
| D8 | **P** | `wisp plugin new qa-theme` → "user.qa-theme is installed, not switched on yet"; after a restart, "user.qa-theme is off". Switched on and off with the CLI, it says "was switched off", which is true then |
| D9 | **P** | "✗ No file at /tmp/qaE2/nope.yml" (validate and apply, exit 1); "✗ No folder at /tmp/qaE2/nope-home (wisp import [v1 home folder])"; "✗ /tmp/qaE2/plain has no config.json: is it wisp v1's home (~/.wisp)? …" |
| D10 | **P** (wording: E9) | Grok → Sign out: fb 12 ms, toast at 300 ms "grok is still signed in after `grok logout`: run it in a terminal to sign out there."; the same for Codex's own login |
| D11 | **Partly fixed → E4** | The launcher's Recipes chip reads "Run tests", and so does Automations → Manage. `wisp export tasks` has `title: Run tests`. But Settings → Automations still reads `run-tests` |
| D12 | **P** | A formula adding `repo-again` at `/private/tmp/qaE2/repo` → "✗ projects: project repo and repo-again have the same path (/private/tmp/qaE2/repo): one project per path; name it repo in the formula" (exit 1); `--yes` adds nothing |

## Action table

Key: P = pass, E# = fails with that new finding, — = not tested or blocked. "server" means the effect was read back with
`settings.list`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded. "@100"/"@150" means the UI
state 100–150 ms after the click; "fb" is ms from the click to the first DOM change.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P (note) | Versions from the fakes; opencode "Free models only · Add key". GitHub's row has no state: the fake `gh` can't answer `auth status` |
| 2 | Codex Sign in (device code) | P | fb 60 ms; the box at 600 ms: "1. Open the sign-in page and sign in with your ChatGPT account. · Open the sign-in page · 2. Enter this code there: ABCD-12345 · Waiting for you to finish there…"; "Signed in" at 6 s |
| 3 | Add another Codex account (Work) | P | Ownership terms dialog; `accounts.list` has `Codex 2` (work, ready) |
| 4 | Account menu: Move up / Move down | P | Rows swap @150; server order follows |
| 5 | Rename an account | P | The field is focused with "Codex 2"; "QA Work" @150 and on the server |
| 6 | Sign out of an account | P (note) | "Signed out · Sign in" @150, `signedOut` on the server; no confirm (as in D). Its menu still offers Sign out: E8 |
| 7 | Remove an account (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. Tasks that ran on it keep their history." · Remove account |
| 8 | Harness Update button | P | After the restart with checks on: fb 26 ms; rec `["update"]`; toast "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/tmp/qaE2/bin/claude update` in a terminal, then re-check." |
| 9 | Model default (launcher pick) | P | Opus @150, kept across reload, `runtime.launch.models.claude: opus`; rec `--model opus` |
| 10 | Thinking default | P | Slider to Max → "Opus · max"; rec `--effort max` |
| 11 | Lean Claude Code runs select | P | fb 14 ms; `runtime.leanRuns` on → off, restored |
| 12 | Ask before running a changed harness | P | fb 21 ms, flips @100; restored |
| 13 | Re-check | P | fb 28 ms; toast "Up to date: nothing new"; rows don't move |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100 (fb 13–38 ms), its server key changes, restored |
| 15 | General: 4 selects | P | `settings.editor`, `terminal.terminal`, `review.harness`, `update.updateChannel`, fb 16–39 ms, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` @100; the account menu label flips; kept across reload |
| 17 | Notifications: Accept tasks from channels, Default project | P | `channels.acceptChannels` fb 24 ms; Default project: "Last used (repo)", repo, repo2, repo3-ui |
| 18 | Notifications: Desktop notifications | P (note) | Disabled, "Blocked — allow notifications for this site in your browser" (headless) |
| 19 | Memory: Learn from tasks | P | `learn.enabled`, fb 19 ms |
| 20 | Automations: Webhook triggers | P | `automations.webhookTriggers`, fb 11 ms |
| 21 | Usage: 5 switches, 4 selects | P | `budget.*`, `runtime.limitWait`, `runtime.subagentModel`, `machine.*` (fb 14–40 ms). The segmented stop lines weren't re-run |
| 22 | Worktrees: 3 selects | P | `worktrees.cleanup`, `.max`, `.onDone`, fb 11–27 ms |
| 23 | Privacy: 3 switches | P | Usage stats, `worktrees.projectAsk`, `git.maskEnvFiles`, fb 16–19 ms |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 16 → 16 after a task and `wisp telemetry flush` ("queued false"); switched back on |
| 25 | Updates: Channel; Check now | P | "A git clone: pull and restart to update"; Check now disabled with "Checks are off on this install" |
| 26 | Keyboard shortcuts pane; ⌘K, ⌘P, `[` | P | The pane lists them; ⌘K and ⌘P list; `[` hides the list and brings it back |
| 27 | Settings search | P | `ntfy` → Phone, `telemetry` → Privacy, `worktree` → General, Worktrees and 5 rows; `xyzzy` → "No settings match “xyzzy”"; each hit opens its pane |
| 28 | All settings | P | No `runtime.auto` row, no `router.*` |
| 29 | Setup: Make the formula | P | Settings only; no `router:` |
| 30 | Setup: Import → See what it changes → Apply | P | "Adds 1 · project repo3-ui · Changes 1 · setting settings.editor · cursor → zed"; toast "Imported"; both on the server |
| 31 | Diagnostics: Copy report | P | fb 36 ms, toast "Report copied"; Computers: "this computer · 2.0.0 · 51 plugins · online" |
| 32 | Sandboxes | — | The pane reads ("Sandboxes need your wisp Grid"); a dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus and empty state | P | Focus in `~/code/my-app or a GitHub URL`; "No projects yet. Type a folder’s path above, or paste a repo URL to clone it." |
| 34 | Add a missing folder / a file / a relative name | P | "No folder at …", "… is a file, not a folder: pick the folder it's in", "repo2 isn't a full path: type the folder's full path, like ~/code/app" |
| 35 | Add a `file://` URL | P | `file:///tmp/qaE2/repo2` added as repo2 |
| 36 | Add a duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo"; "That folder is already repo2" |
| 37 | Real folder check via import / apply | — | Not re-run with a ghost folder (the v1 import's missing folder: action 86) |
| 38 | Remove a project (confirm) | P | `plain` (added with `wisp add .`): fb 29 ms, "Remove “plain” from wisp? Its files are kept." · Remove project; gone from `wisp export projects` |
| 39 | Multi-pick (top bar) | P | All projects ⌘0, repo ⌘1, repo2 ⌘2, repo3-ui ⌘3, Add project…, Manage projects… |
| 40 | Tasks in a project folder (UI and CLI) | P | UI task in `repo` (rec cwd `home/worktrees/repo-<id>`), `cd repo2 && wisp "…"` → project repo2 in `repo2-<id>` |
| 41 | `wisp apply` of a second project for a folder | P | D12 |

### Machines, peers, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Machines → Manage → Add `qa@192.0.2.1` | P | fb 33 ms, "Connecting", then "Can't connect · Try again … No answer · 192.0.2.1 doesn’t answer. Is it on and awake …" |
| 43 | Edit… (Advanced) | P (note) | SSH address, Name, wisp’s port there, Start command, Connect when wisp starts |
| 44 | Remove (confirm) | P | fb 22 ms, "Remove qabox from wisp? Its projects leave the launcher. Nothing on the machine is touched …"; `wisp machines list` → `(none)` |
| 45 | Peers: `wisp machines discover`, `hosts`, `reachable` | P | `(none)`, `(none)`; `reachable` shows ssh and tailscale as lines |
| 46 | This computer: memory, CPU, disk, what's using it | P | "61% free", "62% of 8 cores", "132 GB free", "wisp itself 49 MB" |
| 47 | Pause automations / Resume now | P | fb 18 ms, a menu "For 1 hour · Until tomorrow 8:00"; "Automations paused until 18:26" @150; Resume fb 32 ms, "Running as usual" @150 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in: device code, popup to `/activate?code=…`, dev sign-in, Allow | P | The pane shows the code at 150 ms; "Device connected"; `wisp cloud status`: qa-alice |
| 49 | Don't allow | P | "Sign-in was declined in the browser."; the address stays `http://wisp.localhost:4952` |
| 50 | This computer on the grid → on | P | fb 28 ms, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4952" @150 |
| 51 | Grid restart: the tunnel comes back | P | `cloud status` online 2 s after the restart; the address answers 401 without a sign-in |
| 52 | API key: create, use, revoke | P | fb 36 ms, "qa-script is ready · Copy it now. You won’t see it again."; `qa-alice.wisp.localhost:4952/api/health` 200 with it, 401 without; "Revoke qa-script? Anything using this key stops working right away. This can’t be undone." (fb 14 ms) → 401 |
| 53 | Add an account (qa-bob), Use another account | P | Both rows listed |
| 54 | Switch to qa-bob and back | P | fb 30 ms, Current moves @150; This computer off for qa-bob; `cloud status` follows |
| 55 | Sign out of qa-bob (confirm) | P | fb 22 ms, "Sign out of qa-bob on wisp.localhost:4952 here? …"; gone @150 |
| 56 | Cloud computer woken by an API call | P | Its `runtime.agents`: claude and codex 9.9.9, the other five unavailable; spy log empty |
| 57 | `wisp computer off`, `wisp cloud signout` | P | "this computer: off · http://…", "✓ signed out", "not signed in (wisp cloud signin)" |

### Connectors and MCP servers
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector ("QA Echo!" → `qa_echo`) | P | "1 tool" @150; echo log has initialize, initialized, tools/list |
| 59 | Edit: add env, Save | P | Server `env: [{QA_VAR, set: true}]`; the echo started with `QA_VAR=hello-ui` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it"; `pasted`, `remote1` added |
| 61 | Connectors reach a run | P | rec `--mcp-config` servers `pasted`, `qa_echo`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, and its saved keys are deleted." |
| 63 | Presets | P | With no connectors left: Linear (remote, `https://mcp.linear.app/mcp`), Filesystem, Postgres fill their fields (fb 27–29 ms); not added |
| 64 | `wisp mcp` tools, `wisp_launch` → `wisp_list` | P | 250 tools over stdio; launched `qa via mcp` (codex), listed by `wisp_list` |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy: on → Test → off | P | "Working" @150; the catcher got the test POST, then reply polls |
| 66 | Telegram, bad token | P | "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| 67 | Slack, bad tokens | P | "Slack: invalid_auth. Check the tokens in wisp → Settings → Phone → Slack."; link code shown |
| 68 | Discord, bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | "Email works on a cloud computer: set it up there." |
| 71 | Switch off and Remove the tokens | P | All `off`, nothing saved; catcher quiet for 15 s |
| 72 | "push" card | — | No card in the UI; `channels status` still lists `push` (as in D) |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` shows in both lists | P | D8 |
| 74 | Install from a folder (UI) | P | "QA Pink 1.0.0 is installed, off · It asks for nothing beyond showing itself. · Switch it on · Not now" |
| 75 | Switch it on | P | On @150; untrusted styles don't apply (`--accent` `#2f5bea`) |
| 76 | Trust / Untrust, partStyles | P | "Trust QA Pink? QA Pink then runs as its own process with your environment …"; trusted `--accent` `#ff1493`, `launcher.send` pink; untrusted back to `#2f5bea` |
| 77 | Switch off, Remove | P | Off fb 17 ms; Remove fb 26 ms "Remove QA Pink? Turns it off and deletes it …"; gone @150 |
| 78 | Export after Remove | P | `wisp export plugins` lists only `user.qa-theme` |
| 79 | CLI enable/disable reflects live | P | The row follows `wisp plugin enable/disable user.qa-theme` |
| 80 | Built-in off and on (learn) | P | fb 36 ms, "wisp.learn was switched off"; `learn.view` answers `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the new task at once; it runs in `home/scratch/<id>` |

### Import from v1, `wisp init`, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | **E7** | "11 to add, 2 to change"; "!" lines for machine, ntfy, task; "·" notes, one of them about an ntfy token v1 never had |
| 83 | `wisp import --yes` | P | "✓ applied 13 changes", "✓ 1 task imported, 0 unchanged" |
| 84 | v1 ntfy | P | It polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | editor, maskEnvFiles, worktreeMax, limitHold, notify, leanRuns, retentionDays, subagentModel |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at /tmp/qaE2/v1-gone-folder"; "· project old-app: its folder is repo here, kept as repo" |
| 87 | Imported v1 task's project | P | `v1sess0001 "A v1 task"`: project `repo` |
| 88 | v1 default agent and subagent model | P | A CLI task with no `--agent` runs on codex; New task shows Codex; `runtime.subagentModel` haiku |
| 89 | `wisp init` (repo, again, non-repo) | P | `.wisp/wisp.yml` + AGENTS.md note; "is ready" again; non-repo refused naming `--here` (exit 2) |
| 90 | `wisp settings get/set` (good, bad value, unknown key) | P | "worktrees.max must be one of off, 20, 40, 80" (exit 1); D5 |
| 91 | `wisp export` yaml/json/toml → validate → diff | P | 28 resources each; "0 to add, 0 to change, 0 to remove, 28 unchanged"; secrets as `${secret:…}` |
| 92 | `wisp validate`/`apply` errors | P | D9; a folder with no formula: "✗ no *.wisp.yml in /tmp/qaE2/repo" (exit 1) |
| 93 | `wisp completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass; fish 1345 lines |
| 94 | What bash completion offers | P | `--agent <TAB>` gives all 7 harnesses; `settings set worktrees.max <TAB>` gives `off 20 40 80` |
| 95 | `wisp skill install --agent all` | P | Three ✓ skill paths, project files, Codex config; one "·" note for opencode |
| 96 | `npm run doctor` | — | It opens TCP connections to 4800 and 4477; not run |
| 97 | `wisp <group> <unknown word>` | P | `machines`, `plugin`, `cloud`, `settings` each list their verbs |
| 98 | `wisp "…" --agent frob` | P | "✗ no harness frob: --agent takes claude, codex, …" (exit 2); no task created |

### Auto disabled everywhere (`runtime.auto` off)
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `runtime.auto` default | P | Off: `wisp router frob` → "Auto is off (runtime.auto), so the router is put away" |
| 100 | Launcher harness picker | P | The 7 harnesses and "Set up another agent…"; no Auto |
| 101 | ⌘K: auto, harness, model | P | No Auto entry |
| 102 | Crew editor harness | P | "As New task" and the 7 harnesses; no Auto |
| 103 | All settings | P | No `runtime.auto`, no `router.*` |
| 104 | Settings search "auto" / "model" | P | No Auto hit |
| 105 | Setup and the formula | P | No `router:` in Setup's formula or a whole `wisp export` |
| 106 | CLI | **E6** | Hidden from help, completion, kinds and export (D2 fixed); a `router:` section is refused, but its words say no plugin owns it |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `plugin` and `cloud status` identical before and after; the machine goes error → connecting |
| 108 | WebKit 1280: home and all 20 panes | P | Light and dark: no overflow, no console errors, no failed requests |
| 109 | Chrome 390: home and all 20 panes | **E3** | Light and dark: Connectors overflows (a failing connector's pill ends at 407 px); the rest clean, no console errors |
| 110 | WebKit 390: home and all 20 panes | **E3** | Light and dark: the same Connectors overflow. The flagged "1 hour" and "All 7" are one line on screen (`e-wk390-*.png`) |
| 111 | Harness update check at start | P | Restarted 17:33:36 with checks on: no `npm view` at start, the three at 17:34:37 |
| 112 | `wisp-grid dev --help` | P | Usage, exit 0, nothing started; `dev frob` → "✗ wisp-grid dev doesn't take frob" (exit 2) |

### Re-checks of C's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects: each project's task count | P | "repo · /tmp/qaE2/repo · 1 task", "repo2 · /tmp/qaE2/repo2 · 1 task" (`e-22-proj-counts.png`) |
| 114 | Setup → Import error paths | P | A YAML error and `hello` plan nothing and offer no Apply; the error is a toast ("pasted.yml: a formula is a map (wisp: 1, then its sections)") |
| 115 | Plugins → Install error paths | — | Not re-run |
| 116 | CLI error paths | P | `export --format xml` → "✗ no format xml: yaml, json or toml" (exit 2); unknown words list the verbs |
| 117 | Project colour | — | Not re-run |
| 118 | CLI text output of nested values | **E5** | D4, partly fixed |
| 119 | ⌘K ranking | P | "model": Agents first |

### Re-checks of D's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 120 | Sign-in box: numbered steps, Open the sign-in page, the code | P | Steps 1 and 2 with ChatGPT named; the code with an icon button "Copy the code" |
| 121 | Sign-in box: Copy the code | P | D1 |
| 122 | Sign-in box: paste the code, Continue | — | The fake has no Claude or Antigravity sign-in to reach the paste step |
| 123 | A cancelled first sign-in | — | Not re-run (only the Add dialog's Cancel: action 139) |
| 124 | A first sign-in cut by a restart | — | Not re-run |
| 125 | A first sign-in that times out | — | It gives up after 15 minutes; not waited for |
| 126 | Harness Sign out (Grok, no accounts) | P | D10 |
| 127 | A hand-written formula with `off` values | P | D3 |
| 128 | v1 recipe imported | **E4** | D11, partly fixed |
| 129 | `wisp ls` of a task with no project | P | D7 |
| 130 | Dark theme at 1280 and 390, Chrome and WebKit | P | Covered in 108–110; the flagged wraps were two-line list rows, the cast suggestions (prose) and icon-plus-text buttons, each one line on screen |
| 131 | MCP with Auto off | P | No `router_*` tool over stdio `wisp mcp` (250 tools) or `/api/mcp` (246) |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 132 | Agents: Update all (3) | P | fb 34 ms; rec `update` for claude and codex, `upgrade` for opencode; one toast each |
| 133 | Account menu: Make it a personal / work account | P | fb 24 ms, "Personal" @150, `kind: personal` on the server; the item then reads "Make it a work account" |
| 134 | A signed-out account's ⋯ menu | **E8** | It still offers "Sign out" |
| 135 | Add connector errors | P | Empty name: "A name, so agents can tell its tools apart" inline (fb 28 ms); empty command: "The command that starts it"; a name taken: toast "There's a connector called qa_echo already"; a missing command: the row says "It wouldn't start: definitely-not-a-command-qa: not found" |
| 136 | A CLI launch in a git repo that isn't a project | **E1** | `repo4`, `repo3-wt` (a linked worktree) and the audit worktree: rec cwd is the folder itself, twice in a row |
| 137 | Add machine: bad addresses | P | Empty: Add disabled; "has spaces here" and "qa@": toast "SSH host: user@host, a host name, or a Host from ~/.ssh/config." (fb 47–94 ms) |
| 138 | A plan that adds a setting | **E2** | `+ setting/crew.tint (new)`; Setup: "Adds 2 · setting budget.drainWatch · setting crew.tint". Neither says the value |
| 139 | Add another account → Cancel | P | The dialog closes; no account added |
| 140 | Notifications → Default project with projects | P | "Last used (repo)", repo, repo2, repo3-ui |
| 141 | Worktrees: Bring in | P | "1 worktree from git · In repo3-ui · Not now · Bring in" for the linked `repo3-wt` |
| 142 | The harness Sign out toast's words | **E9** | "grok is still signed in after `grok logout` …", "codex is still signed in …" |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### E1 · M · A CLI task in a git repo that isn't a project runs in the user's checkout
- **Steps:** `git clone … /tmp/qaE2/repo4` (not added to wisp) → `cd repo4 && wisp "qa first launch in a new repo"`, then
  a second launch the same way. Worktree per task is on.
- **Actual:**
  - Both runs' recorded cwd is `/private/tmp/qaE2/repo4` itself, not `home/worktrees/repo4-<id>`. The folder isn't
    added as a project either, so every later launch runs there too.
  - The same happens in a linked worktree (`repo3-wt`) and, by accident this round, in the audit worktree
    `~/.wisp/worktrees/wisp-8fb64ec939b0`.
  - A project folder (`repo2`) gets its own worktree, as in D.
  - The fake agent changed no file here. A real one, in Full auto, edits the checkout the user has open.
- **Why:** `crates/wisp/src/scripted.rs` `in_project` only sets `worktree` for a folder inside a known project
  ("Outside every project the arguments stay as they are"). General → Worktree per task says "Each task gets its own
  branch from origin/main, in git repos".
- **Expected:** a git repo outside every project gets a worktree of its own too (as a project does), or the folder is
  added as a project first, or the CLI says it runs in place.

### E2 · M · A plan that adds a setting doesn't say what it sets it to
- **Steps:** `wisp apply -f` (or Setup → Import) a formula with `settings: {budget.drainWatch: true, crew.tint: false}`.
  Neither key has been set yet.
- **Actual:**
  - CLI: `+ setting/budget.drainWatch  (new)`, `+ setting/crew.tint  (new)`. Setup: "Adds 2 · setting
    budget.drainWatch · setting crew.tint".
  - `crew.tint: false` turns a switch that's on by default off. The plan doesn't show that.
  - The v1 import plan is the same: `+ setting/worktrees.max (new)` hides that it becomes 80.
  - A setting that was set before shows both sides (`"7" → "off"`, "cursor → zed").
- **Expected:** a new setting shows its value, or what it changes from the default (`crew.tint: on → off`). Setup
  promises "you see what it would change before anything does".

### E3 · L · A failing connector's error runs off its card on a phone
- **Steps:** add a connector whose command doesn't exist (`definitely-not-a-command-qa`) → Settings → Connectors at
  390 px.
- **Actual:**
  - The pill "It wouldn't start: definitely-not-a-command-qa: not found" isn't cut. It runs under the switch and past
    the card's edge (ends at 407 px; `e-L-chromium-390-light-connectors.png`).
  - Chrome and WebKit, light and dark.
  - At 1280 it stops before the switch.
- **Expected:** the pill truncates (or wraps under the command) inside its row, like the command line above it.

### E4 · L · Settings → Automations still names a v1 recipe by its slug (D11, partly fixed)
- The launcher's Recipes chip and Automations → Manage read "Run tests".
- Settings → Automations' summary row reads `Recipes | run-tests | Manage`.

### E5 · L · `wisp channels status` prints channel rows without their names (D4, partly fixed)
- It prints `channels` and then `state  enabled  installed` and seven rows of `off  false  true`.
- None says which channel it is. `--json` has `transport` (ntfy, telegram, …).
- `connectors list`, `settings list`, `machine read` and `worktrees view` are fixed.

### E6 · L · A `router:` section's refusal says no plugin owns it
- With Auto off, `wisp validate -f` / `wisp apply -f` of `router: {best2: true}` prints "✗ …: no plugin in this wisp
  owns `router:` (sections: …)".
- The router plugin is there, put away. `wisp router frob` says why: "Auto is off (runtime.auto), so the router is put
  away". The D2 fix says a plan naming it says Auto is off.

### E7 · L · The v1 import notes an ntfy token v1 never had
- `wisp import <v1> --dry-run` prints "· channel ntfy: its token stays in secrets (${secret:…} when you declare it)".
- The v1 home's ntfy is `{enabled, topic, server}`, with no token. The note should only show when there is one.

### E8 · L · A signed-out account's menu still offers Sign out
- **Steps:** Settings → Agents → Codex → an added account → ⋯ → Sign out.
- **Actual:** the row reads "Signed out · Sign in". Its ⋯ menu still lists "Rename · Make it a personal account · Move up
  · Move down · Sign out · Remove".
- **Expected:** no Sign out on an account that's signed out (its row offers Sign in).

### E9 · L · The harness Sign out toast opens with the CLI's lowercase name
- "grok is still signed in after `grok logout`: run it in a terminal to sign out there."
- "codex is still signed in after `codex logout`: …"
- Everywhere else the harness is "Grok" or "Codex" ("Claude Code is still 9.9.9 after the update …"). Keep the command
  in backticks, and name the harness: "Grok is still signed in after `grok logout` …".

---

## Smaller notes (not counted)
- **New task (tasks area):** for the first ~150 ms the new row reads "Claude Code · Default model" and the page "Default",
  while the pick is Opus · max (as in D). The run gets `--model opus --effort max`.
- **GitHub row:** when `gh auth status` can't be read (the fake), the row shows neither a state nor Sign in. A real `gh`
  answers it.
- **Connectors after a restart:** each reads "Not checked yet" (before: "1 tool"); a test result isn't kept.
- **`wisp computer off`** prints the address it no longer serves: "◌ this computer: off · http://…".
- **Cloud pane:** right after an API call woke the cloud computer, its row read "Asleep" for a while. It read "Online ·
  Put to sleep" a minute later, as `wisp machines list` already did.
- **Layout probe:** WebKit's per-word rects make "1 hour" (Usage) and "All 7" (Worktrees) look wrapped; on screen each is
  one line.
- **Still as in D:**
  - The SSH error shows twice in one row; Esc in the Edit form closes the whole Manage dialog.
  - The crew editor's harness order differs from the launcher's.
  - Setup's formula carries `runtime.launch` and, after a Usage or Worktrees select is put back, its default
    explicitly (`machine.cpuCeiling: "200"` …).
  - Signing out of a harness account has no confirm.
  - Connectors' `addedAt` is 0.
  - `wisp channels status` lists `push`.
  - Telegram, Slack and Discord retry every ~5 s against a 401 while on.
  - `wisp export <TAB>` offers no kinds.

## Cleanup done
- **Restored:** every setting the round or the v1 import changed is back to its default (`settings.set … null`).
  `settings.list` shows no value differing from its declared default. Also:
  - Usage stats on; ntfy off (the import had turned it on), and its reply polls stopped.
  - This computer off, grid signed out.
  - Connectors `pasted`, `qa_echo`, `qa_bad`, `x` and the imported `old-echo` removed.
  - The second Codex accounts removed.
  - The five tasks launched by mistake in the audit worktree deleted.
- **Stopped:** `wisp serve` :4951, `wisp-grid dev` :4952 (and its boxes), the catcher :4949 and headless Chrome :9374. No
  process under `/tmp/qaE2` is left.
- **Kept for re-checks:** `/tmp/qaE2`: binaries in `fake/` and `bin/`, home, repos (`repo4` and the linked `repo3-wt`
  for E1), grid data, shots, and scripts in `pw/s` (this round's are `e-*`) and `tools/`. The scratch home still holds:
  - the imported v1 machine (`oldbox`, TEST-NET), task kind `run-tests` and its task;
  - projects `repo`, `repo2`, `repo3-ui` and about 14 fake tasks;
  - `user.qa-theme` (off).
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir this round was told to use).
