# Audit D: agent and workspace setup (round D, re-check of audit C plus what it missed)

2026-11-19 · worktree `wisp-8fb64ec939b0` at `b7598c9d` · findings only, no repo file edited except this one.

> **Build note:** at the start of the round the worktree differed from HEAD only in `design/v1-reference/` (screenshots
> and one metrics file from another session). No code file was changed, so the binaries and the web bundle are HEAD's.
> By the end, other sessions had changed `desktop/src-tauri/src/{main,qa}.rs`, `packages/shell/src/platform/desktop.ts`
> (and its test), `apps/web/dev/mock-commands.ts`, `docs/owner-list.md` and `docs/v2-plan.md`. None of it is in this
> round's build, and none of it touches the areas of D1–D12.

**Setup** (copied from audit C, in a fresh `/tmp/qaD2`):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`), `wisp-grid` and `wisp-grid-hostd` were built at HEAD
  with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, then copied to `/tmp/qaD2/fake`. Copies of the fake agent under each
  harness name went to `/tmp/qaD2/bin`. The web UI was built at HEAD with `vite build --outDir /tmp/qaD2/web`.
- **Server:** `wisp serve --port 4931` with `WISP_HOME=/tmp/qaD2/home` and `HOME=/tmp/qaD2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` pointed at a fake `npm` from the first start.
  - The first two starts had `WISP_UPDATE_CHECK=0`. The third (the restart check, action 107) left it unset, to
    re-check B4 and B5.
- **Catcher on `:4929`:** PostHog, Slack, Telegram, Discord, Linear and the ntfy server all pointed at it, with the same
  answers as in C.
- **Cloud:** `wisp-grid dev --port 4932 --data /tmp/qaD2/grid` (`wisp.localhost`).
  - Only `WISP_CLAUDE_BIN` and `WISP_CODEX_BIN` were set.
  - Spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` were first on its `PATH` and log any call.
- **Test data:** fresh clones of C's `repo`, `repo2` and `repo3` (no `.wisp`, `AGENTS.md` or `.claude` leftovers). C's v1
  home, paths moved to `/tmp/qaD2`, its machine on TEST-NET (`qa@192.0.2.1`). The machine added in the UI used the
  same address.
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - A persistent headless system Chrome over CDP `:9354` (own profile in `/tmp/qaD2/chrome-prof`), at 1280 and 390.
  - For the layout sweep, fresh Chrome and WebKit contexts at 1280 and 390, each in light and dark.
  - Every context aborted requests to `trywisp.dev`, `example.dev` and `example.invalid`. Two were attempted, both by a
    click: the sign-in box's "Open the sign-in page" (`https://auth.example.dev/codex/device`) and the grid's
    `trywisp.dev` link.
- **Screenshots:** `/tmp/qaD2/shots/` (186; this round's are named `d-*`).
- **Stopped at the end:** the server, the grid (and its boxes), the catcher and Chrome. No `/tmp/qaD2` process is left
  and nothing listens on 4929, 4931, 4932 or 9354. `/tmp/wisp-target-fixer` is shared and was left in place.
  `/tmp/qaD2` stays for re-checks.

> **Outbound requests and other things to know.**
> - No `npm view` reached the real registry: the fake `npm` logged the three calls (B5).
> - No real harness CLI ran: the grid's spy log stayed empty.
> - SSH only went to `192.0.2.1`, which times out.
> - A plugin install from `https://example.invalid/x.git` failed at DNS, by design.
> - `npm run doctor` was not run: it opens TCP connections to 4800 and 4477, which this round must not touch.
> - Nothing reached grid.trywisp.dev, 4477, 4800 or 4791. No real credential was read. No new `~/.wisp*` folder was
>   made: every `wisp` call ran with `WISP_HOME=/tmp/qaD2/home`, and `wisp-grid dev` with `HOME=/tmp/qaD2/realhome`.
> - Another auditor ran in `/tmp/qaD3` (a grid on 4987). This round didn't touch it.
> - The fake agent has no Claude Code sign-in: `claude auth login --claudeai` just hangs. So the paste/Continue step
>   (Claude, Antigravity) couldn't be reached, and the hung sign-in was used for the restart case (action 124).

## Counts

| | |
|---|---|
| Actions checked | 131 (C's 119 + 12 new) |
| Pass | 114 |
| Fail (a finding) | 12 |
| Not tested or blocked | 5 (32 sandboxes on a dev grid, 72 push card, 96 `npm run doctor`, 122 paste/Continue, 125 a sign-in that times out) |
| Audit C findings | 9: 8 pass (fixed), 1 partly fixed (C4 → D4) |
| **New findings** | **12 (D1–D12): 0 high, 2 medium, 10 low** |

## Audit C findings, re-checked

| C# | Result | How checked |
|---|---|---|
| C1 | **P** | Manage projects: "repo · 1 task", "repo2 · 1 task", "repo3-ui · 0 tasks"; both tasks ran in worktrees |
| C2 | **P** | The bin on `repo2` opens "Remove “repo2” from wisp? Its task and its files are kept." (fb 12 ms) |
| C3 | **P** | A fresh server: This computer reads "Disk · 146 GB free" at once; `machine.read` has `free` and `total` |
| C4 | **Partly fixed → D4** | `machines reachable` now shows `ssh`, `tailscale` and their fields as lines; `settings get runtime.launch` shows each map; `machines list` says "just now". But `connectors list`, `channels status`, `settings list`, `machine read` and `worktrees view` still print "N items" or "N fields" |
| C5 | **P** | `export --format xml` → "✗ no format xml: yaml, json or toml" (exit 2). `export nosuchkind` → "✗ no kind nosuchkind: accounts, …" (exit 2) |
| C6 | **P** | `wisp router` → "✗ wisp router isn't a command"; `wisp router frobnicate` → "✗ … Auto is off (runtime.auto), so the router is put away" (exit 2). Deliberate. `wisp export router` remains: D2 |
| C7 | **P** | ⌘K "model": Agents first, no theme command ahead of it |
| C8 | **P** | A YAML error is one sentence ("pasted.yml: YAML: line 2, column 13: unclosed bracket '{'"). After `hello`, no earlier plan stays. Apply is disabled for a plan that can't apply |
| C9 | **P** | "No folder at /tmp/qaD2/nope-dir"; "not a path isn't a full path: type the plugin folder's full path, like ~/code/my-plugin, or a git URL"; "Couldn't reach example.invalid to clone …"; foot "Plugins live in /tmp/qaD2/home/plugins" |

## Action table

Key: P = pass, D# = fails with that new finding, — = not tested or blocked. "server" means the effect was read back with
`wisp settings list --json`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded. "@100"/"@150"
means the UI state 100–150 ms after the click; "fb" is ms from the click to the first DOM change.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P | Versions from the fakes; opencode "Free models only · Add key" |
| 2 | Codex Sign in (device code) | P | fb 50 ms; the box at 100 ms: "1. Open the sign-in page and sign in with your ChatGPT account. · Open the sign-in page · 2. Enter this code there: ABCD-12345 · Waiting for you to finish there…"; "Signed in" at 6 s |
| 3 | Add another Codex account (Work) | P | Ownership terms dialog; `accounts.list` has `Codex 2` (work, ready) |
| 4 | Account menu: Move up / Move down | P | Rows swap @150; server order follows |
| 5 | Rename an account | P | "QA Work" @150 and on the server |
| 6 | Sign out of an account | P (note) | `signedOut`, row shows Sign in; no confirm (as in C) |
| 7 | Remove an account (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. Tasks that ran on it keep their history." |
| 8 | Harness Update button | P | fb 17 ms; rec `["update"]`; toast "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/tmp/qaD2/bin/claude update` in a terminal, then re-check." |
| 9 | Model default (launcher pick) | P | Opus @150, kept across reload, `runtime.launch.models.claude: opus`; rec `--model opus` |
| 10 | Thinking default | P | Slider to Max → "Opus · max"; rec `--effort max` |
| 11 | Lean Claude Code runs select | P | `runtime.leanRuns` on → off @150, restored |
| 12 | Ask before running a changed harness | P | Flips @100; not a settings key (as in C); restored |
| 13 | Re-check | P | fb 27 ms; rows don't move |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100, server key changes, restored |
| 15 | General: 4 selects | P | `settings.editor`, `terminal.terminal`, `review.harness`, `update.updateChannel` @150, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` @100; the menu label flips; kept across reload |
| 17 | Notifications: Accept tasks from channels, Default project | P | `channels.acceptChannels` fb 10 ms; with no projects the select offers only "Last used" |
| 18 | Notifications: Desktop notifications | P (note) | Disabled, "Blocked — allow notifications for this site in your browser" (headless) |
| 19 | Memory: Learn from tasks | P | `learn.enabled` |
| 20 | Automations: Webhook triggers | P | `automations.webhookTriggers` |
| 21 | Usage: 5 switches, 4 selects | P | `budget.*`, `runtime.limitWait`, `runtime.subagentModel`, `machine.*`. The stop line wasn't re-run |
| 22 | Worktrees: 3 selects | P | `worktrees.cleanup`, `.max`, `.onDone` |
| 23 | Privacy: 3 switches | P | Usage stats, `worktrees.projectAsk`, `git.maskEnvFiles` |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 1 → 1 after a task and `wisp telemetry flush` ("queued false") |
| 25 | Updates: Channel; Check now | P | "A git clone: pull and restart to update"; Check now is disabled with "Checks are off on this install" |
| 26 | Keyboard shortcuts pane; N, ⌘K, ⌘P, `[` | P | The pane lists them; N focuses the prompt; ⌘K and ⌘P list; `[` hides the list |
| 27 | Settings search | P | `theme` → General, `ntfy` → Phone, `telemetry` → Privacy, `xyzzy` → "No settings match “xyzzy”"; each hit lands on its row |
| 28 | All settings | P | No `runtime.auto` row, no `router.*` |
| 29 | Setup: Make the formula | P | Settings and projects; no `router:` |
| 30 | Setup: Import → See what it changes → Apply | P | "Adds 2 · setting settings.editor · project repo3-ui"; toast "Imported"; both on the server |
| 31 | Diagnostics: Copy report | P | fb 31 ms, toast "Report copied"; Computers: "this computer · 2.0.0 · 51 plugins", "Cloud · 2.0.0 · 48 plugins" |
| 32 | Sandboxes | — | The pane reads; a dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus and empty state | P | Focus in `~/code/my-app or a GitHub URL` |
| 34 | Add a missing folder / a file / a relative name | P | "No folder at …", "… is a file, not a folder: pick the folder it's in", "repo2 isn't a full path: …" |
| 35 | Add a `file://` URL | P | Added as its folder |
| 36 | Add a duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo" |
| 37 | Real folder check via import / apply | P | "projects.ghost: No folder at /tmp/qaD2/ghost-folder"; "1 problem: nothing can be applied until it's fixed" |
| 38 | Remove a project (confirm) | P | C2 |
| 39 | Multi-pick (top bar) | P | All projects ⌘0, repo ⌘1, repo2 ⌘2, repo3-ui ⌘3, Add project…, Manage projects… |
| 40 | Tasks in a project folder (UI and CLI) | P | UI task in `repo` (worktree `repo-<id>`), `cd repo2 && wisp "…"` → project repo2 |
| 41 | `wisp apply` of a second project for a folder | **D12** | `/tmp/qaD2/repo` is refused ("one project per path"); `/private/tmp/qaD2/repo` is applied as a second project |

### Machines, peers, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Machines → Manage → Add `qa@192.0.2.1` | P | fb 32 ms, "Connecting", then "Can't connect … No answer · 192.0.2.1 doesn’t answer. Is it on and awake …" |
| 43 | Edit… (Advanced) | P (note) | SSH address, Name, wisp’s port there, Start command, Connect when wisp starts |
| 44 | Remove (confirm) | P | fb 27 ms, "Remove qabox from wisp? Its projects leave the launcher. Nothing on the machine is touched …"; `wisp machines list` → `(none)` |
| 45 | Peers: `wisp machines discover`, `hosts`, `reachable` | P | `(none)`, `(none)`; `reachable` shows ssh and tailscale as lines (C4) |
| 46 | This computer: memory, CPU, disk, what's using it | P | C3 |
| 47 | Pause automations / Resume now | P | "Automations paused until 16:10" @150; "Running as usual" @150 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in: device code, popup to `/activate?code=…`, dev sign-in, Allow | P | "Device connected"; `wisp cloud status`: qa-alice |
| 49 | Don't allow | P | "Sign-in was declined in the browser."; the address stays `http://wisp.localhost:4932` |
| 50 | This computer on the grid → on | P | fb 21 ms, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4932" @150 |
| 51 | Grid restart: the tunnel comes back | P | `cloud status` online 1 s after the restart; the address answers 401 without a sign-in |
| 52 | API key: create, use, revoke | P | Key shown once; `/api/health` on the cloud computer 200 with it, 401 without; "Revoke qa-script? … This can’t be undone." → 401 |
| 53 | Add an account (qa-bob), Use another account | P | Both rows listed |
| 54 | Switch to qa-bob and back | P | This computer off for qa-bob; `cloud status` follows |
| 55 | Sign out of qa-bob (confirm) | P | "Sign out of qa-bob on wisp.localhost:4932 here? …"; gone @150 |
| 56 | Cloud computer woken by an API call | P | Its `runtime.agents`: claude and codex 9.9.9, the other five unavailable; spy log empty |
| 57 | `wisp computer off`, `wisp cloud signout` | P | "this computer: off", "✓ signed out", "not signed in" |

### Connectors and MCP servers
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector ("QA Echo!" → `qa_echo`) | P | "1 tool" @150; echo log has initialize, initialized, tools/list |
| 59 | Edit: add env, Save | P | Server `env: [{QA_VAR, set: true}]`; the echo started with `QA_VAR=hello-ui` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it"; `pasted`, `remote1` added |
| 61 | Connectors reach a run | P | rec `--mcp-config` servers `pasted`, `qa_echo`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, and its saved keys are deleted." |
| 63 | Presets | P | With no connectors left: Linear, Jira, PostHog, GitHub, Filesystem, Postgres; Filesystem fills `filesystem` and its command (not added: npx would reach the network). Remote OAuth not tested |
| 64 | `wisp mcp` tools, `wisp_launch` → `wisp_list` | P | 250 tools (b7598c9d lists `/api/mcp`'s); launched `qa via mcp` (codex), listed by `wisp_list` |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy: on → Test → off | P | "Working" @150; the catcher got the test POST, then reply polls |
| 66 | Telegram, bad token | P | "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| 67 | Slack, bad tokens | P | "Slack: invalid_auth. Check the tokens in wisp → Settings → Phone → Slack."; link code shown |
| 68 | Discord, bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | "Email works on a cloud computer: set it up there."; switch and Test disabled |
| 71 | Switch off and Remove the tokens | P | All `off`; catcher quiet for 15 s except one PostHog batch |
| 72 | "push" card | — | No card in the UI; `channels status` still lists `push` (as in C) |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` shows in both lists | **D8** | Listed off in `wisp plugin` and the pane, but the pane says "user.qa-theme was switched off" (it never was on) |
| 74 | Install from a folder (UI) | P | "QA Pink 1.0.0 is installed, off · It asks for nothing beyond showing itself · Switch it on · Not now" |
| 75 | Switch it on | P | On @150; untrusted styles don't apply (`--accent` `#2f5bea`) |
| 76 | Trust / Untrust, partStyles | P | "Trust QA Pink? QA Pink then runs as its own process with your environment …"; trusted `--accent` `#ff1493`, `launcher.send` pink; untrusted back to `#2f5bea` |
| 77 | Switch off, Remove | P | "Remove QA Pink? Turns it off and deletes it …"; gone |
| 78 | Export after Remove | P | `wisp export plugin` lists only `user.qa-theme` |
| 79 | CLI enable/disable reflects live | P | The row follows `wisp plugin enable/disable user.qa-theme` |
| 80 | Built-in off and on (learn) | P | fb 38 ms, "wisp.learn was switched off"; `learn.view` answers `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the new task at once; it runs in `home/scratch/<id>` |

### Import from v1, `wisp init`, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | **D6** | "10 to add, 3 to change"; "!" lines for machine, ntfy, task; "·" notes. A changed setting prints `: "zed" → "cursor"` |
| 83 | `wisp import --yes` | P | "✓ applied 13 changes", "✓ 1 task imported, 0 unchanged" |
| 84 | v1 ntfy | P | It polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | editor, maskEnvFiles, worktreeMax, limitHold, notify, leanRuns, retentionDays, subagentModel |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at …" |
| 87 | Imported v1 task's project | P | `v1sess0001 "A v1 task"`: project `repo` |
| 88 | v1 default agent and subagent model | P | A CLI task with no `--agent` runs on codex; New task shows Codex; `runtime.subagentModel` haiku |
| 89 | `wisp init` (repo, again, non-repo) | P | `.wisp/wisp.yml` + AGENTS.md note; "is ready" again; non-repo refused naming `--here` (exit 2) |
| 90 | `wisp settings get/set` (good, bad value, unknown key) | **D5** | `set` says "must be one of off, 20, 40, 80" and "no plugin declares the setting nope.key" (exit 1). `get nope.key` prints an empty line (exit 0) |
| 91 | `wisp export` yaml/json/toml → validate → diff | P | 28 resources; "0 to add, 0 to change, 0 to remove, 28 unchanged" each |
| 92 | `wisp validate`/`apply` errors | **D9** | Missing folder, a folder with no formula, a missing file each exit 1 with a reason, but a missing file reads "No such file or directory (os error 2)" |
| 93 | `wisp completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass (no fish here) |
| 94 | What bash completion offers | P | `--agent <TAB>` gives all 7 harnesses; `settings set worktrees.max <TAB>` gives `off 20 40 80` |
| 95 | `wisp skill install --agent all` | P | Three ✓ skill paths, project files, Codex config; one "·" note for opencode |
| 96 | `npm run doctor` | — | It opens TCP connections to 4800 and 4477; not run |
| 97 | `wisp <group> <unknown word>` | P | `machines`, `plugin`, `cloud`, `settings` each list their verbs |
| 98 | `wisp "…" --agent frob` | P | "✗ no harness frob: --agent takes claude, codex, …"; no task created |

### Auto disabled everywhere (`runtime.auto` off)
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `runtime.auto` default | P | false |
| 100 | Launcher harness picker | P | The 7 harnesses and "Set up another agent…"; no Auto |
| 101 | ⌘K: auto, harness, model | P | No Auto entry |
| 102 | Crew editor harness | P | "As New task" and the 7 harnesses; no Auto |
| 103 | All settings | P | No `runtime.auto`, no `router.*` |
| 104 | Settings search "auto" / "model" | P | No Auto hit |
| 105 | Setup and the formula | P | No `router:` in Setup's formula or a whole `wisp export` |
| 106 | CLI | **D2** | Not in `wisp help` or completion; `wisp router` refused (deliberate). But `wisp export router` writes the router's settings, the kinds list names `router`, and `validate`/`apply` take and change a `router:` section |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `plugin` and `cloud status` identical before and after; the machine goes error → connecting |
| 108 | WebKit 1280: home and all 20 panes | P | Light and dark: no overflow, no console errors, no failed requests |
| 109 | Chrome 390: home and all 20 panes | P | Light and dark: the same |
| 110 | WebKit 390: home and all 20 panes | P | Light and dark: the same |
| 111 | Harness update check at start | P | Restarted 15:21:39 with checks on: no `npm view` at start, the three at 15:22:40 |
| 112 | `wisp-grid dev --help` | P | Usage, exit 0, nothing started; `dev frob` → "✗ wisp-grid dev doesn't take frob" (exit 2) |

### Re-checks of C's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects: each project's task count | P | C1 |
| 114 | Setup → Import error paths | P | C8 |
| 115 | Plugins → Install error paths | P | C9; fb 31–43 ms |
| 116 | CLI error paths | P | C5; `plugin enable nope.plugin` → "✗ no plugin nope.plugin"; `add` of a missing folder names `wisp -- add …` |
| 117 | Project colour | P | 9 colours; fb 24 ms; `wisp export project` `color: amber` |
| 118 | CLI text output of nested values | **D4** | C4, partly fixed |
| 119 | ⌘K ranking | P | C7 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 120 | Sign-in box: numbered steps, Open the sign-in page, the code | P | Steps 1 and 2 with ChatGPT named; "Open the sign-in page" is a link opening a new tab (blocked); the code in its box with an icon button "Copy the code" |
| 121 | Sign-in box: Copy the code | **D1** | No DOM change within 600 ms and no toast |
| 122 | Sign-in box: paste the code, Continue | — | The fake has no Claude or Antigravity sign-in to reach the paste step |
| 123 | A cancelled first sign-in | P | Codex 3: Cancel → "Signed out · Sign in" @150 (fb 40 ms), `signedOut` on the server |
| 124 | A first sign-in cut by a restart | P | Claude 2's hung sign-in, server restarted: "Signed out · Sign in", `signedOut`; the CLI process was stopped with the server |
| 125 | A first sign-in that times out | — | It gives up after 15 minutes; not waited for |
| 126 | Harness Sign out (Grok, no accounts) | **D10** | fb 28 ms, then the row still reads "Signed in · Sign out" with no message; `accounts.logout` answers `signedIn: false` |
| 127 | A hand-written formula with `off` values | **D3** | `worktrees.max: off` → "worktrees.max is enum (off, 20, 40, 80), not false" |
| 128 | v1 recipe imported | **D11** | v1's "Run tests" is the task kind `run-tests`, and the launcher's Recipes chip reads `run-tests` |
| 129 | `wisp ls` of a task with no project | **D7** | The cast task's project column reads `01a1f42ce94d72ba` (its scratch folder) |
| 130 | Dark theme at 1280 and 390, Chrome and WebKit | P | Covered in 108–110; the flagged wraps were two-line list rows, the cast suggestions (prose) and icon-plus-text buttons, each one line on screen |
| 131 | MCP with Auto off | P | No `router_*` tool over stdio `wisp mcp` (250 tools) or `/api/mcp` (246); `router_preview` → "Unknown tool" |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### D12 · M · A formula adds a second project for a folder spelled another way
- **Steps:** project `repo` at `/tmp/qaD2/repo` → `wisp apply -f` a formula with
  `projects: [{name: repo-again, path: /private/tmp/qaD2/repo}]`.
- **Actual:**
  - The plan adds it ("1 to add") and `--yes` applies it. `wisp export projects` lists both `repo` and `repo-again`.
  - The same formula with `/tmp/qaD2/repo` is refused: "project repo and repo-again have the same path … one project per
    path".
  - The UI's Add and `wisp add` already treat the two spellings as one ("That folder is already repo").
  - The task counts double: removing `repo-again` asks "Its 6 tasks and its files are kept", all of them `repo`'s.
- **Expected:** compare canonical paths, as the UI's Add does.

### D3 · M · A formula written with `off` is refused: "is enum (off, …), not false"
- **Steps:** `wisp apply -f` (or Setup → Import) a formula with `settings: {worktrees.max: off, worktrees.cleanup: off,
  runtime.leanRuns: off}`.
- **Actual:**
  - "3 problems: nothing can be applied until they're fixed".
  - Each reads "✗ settings.worktrees.max: worktrees.max is enum (off, 20, 40, 80), not false".
  - The plan shows `: "7" → false`. Bare `off` is read as a YAML 1.1 boolean.
  - In a `router:` section the same thing turns `scout: off` into `scout: false`.
  - `wisp export` writes `"off"` quoted, so its own output round-trips. A hand-written one, using the word the error
    lists, can't.
- **Expected:** read `on`/`off`/`yes`/`no` as strings where a setting's enum has them, or at least say "write `off` in
  quotes".

### D1 · L · The sign-in box's Copy gives no sign it copied
- **Steps:** Settings → Agents → Codex → Sign in (or an added account's) → the copy button next to the code.
- **Actual:**
  - No DOM change in the 600 ms after the click, and no toast (`d-32-copy-after.png`).
  - The button is an icon only ("Copy the code"), so nothing says the code is on the clipboard.
- **v1:** `agent-login.js:266` toasted "Code copied". The feedback rule (ADR-0017) wants a visible change within 100 ms.
- **Suspect:** `plugins/accounts/ui/src/panes/Steps.tsx` `onCopy` only writes the clipboard;
  `packages/ui/src/settings/SettingsSignIn.tsx:111` shows nothing after.

### D2 · L · With Auto off, the router still shows up in export, validate and apply
- `wisp export nosuchkind` lists `router` among the kinds.
- `wisp export router` writes `router: {best2, blocked, escalate, explore, goal, habits, prefer, scout}`.
- `wisp validate`/`apply` take a `router:` section and change it ("~ router/default · best2: false → true").
- Everything else hides the router while `runtime.auto` is off: help, completion, ⌘K, All settings, Setup's formula and
  a whole `wisp export`. `wisp router` is refused (deliberate).

### D4 · L · CLI text output still hides nested values (C4, partly fixed)
- `wisp connectors list` prints `connectors  3 items`, and `wisp channels status` prints `channels  7 items`: neither
  names a connector or a channel.
- `wisp settings list` prints `runtime.launch  4 fields`, `events.sinks  0 items`.
- `wisp machine read` prints `history  27 items`, and `wisp worktrees view` prints `items  6 items`.
- `machines reachable`, `settings get` and `machines list` are fixed.

### D5 · L · `wisp settings get` of an unknown key answers nothing
- `wisp settings get nope.key` prints an empty line and exits 0.
- `wisp settings set nope.key 1` says "✗ no plugin declares the setting nope.key" (exit 1). `get` should say the same.

### D6 · L · A plan prints a changed setting with a bare colon
- `wisp import <v1> --dry-run` prints `~ setting/settings.editor` then `: "zed" → "cursor"`.
- `apply --dry-run` prints `: "7" → false`.
- A setting's own value has no field name, so the line starts with `:`. Other resources print `agent: "claude" →
  "codex"`.

### D7 · L · `wisp ls` shows a scratch folder's id as a task's project
- A task with no project (Cast a plugin, in `home/scratch/<id>`) reads `01a1f42ce94d72ba` in the project column.
- `wisp ls --json` has `project: null`. The column should be empty or say "no project".

### D8 · L · A plugin that was never on says "was switched off", or "disabled in the plugin group"
- **Steps:** `wisp plugin new qa-theme` → Settings → Plugins.
- **Actual:**
  - Its row says "user.qa-theme was switched off". After a restart: "user.qa-theme is off (disabled in the plugin
    group)".
  - An installed one reads "user.qa-pink is installed, not switched on yet".
- **Expected:** the installed wording, or just "Off".

### D9 · L · A missing file's error is the raw OS message
- `wisp validate -f /tmp/qaD2/nope.yml` and `wisp apply -f …` print "✗ /tmp/qaD2/nope.yml: No such file or directory
  (os error 2)".
- `wisp import /tmp/qaD2/nope-home` prints "✗ /tmp/qaD2/nope-home/config.json: No such file or directory (os error 2)
  (wisp import [v1 home folder])".
- Projects and plugins already say "No folder at …".

### D10 · L · A harness's own Sign out changes nothing on screen
- **Steps:** Settings → Agents → Grok (one login, no accounts) → Sign out.
- **Actual:**
  - Something changes at 28 ms. Through 3 s the row still reads "Grok · 9.9.9 · Signed in · Sign out · Ready", with
    no toast.
  - `accounts.logout` answers `{signedIn: false}` while the row stays signed in.
  - The fake CLI stays signed in after `logout`. b4d56553 says "a CLI that stays signed in says so", but nothing says
    so here.
- **Expected:** "Signed out", or a line saying Grok is still signed in and how to sign out there.

### D11 · L · A v1 recipe's name becomes a slug
- v1's recipe "Run tests" is imported as the task kind `run-tests` (`wisp export tasks`, `wisp recipes`).
- The launcher's Recipes chip reads `run-tests`. Its v1 name, with its capital and space, is gone.

---

## Smaller notes (not counted)
- **New task (tasks area):** for the first ~150 ms the new row reads "Claude Code · Default model" and the page "Default",
  while the pick is Opus · max. The run gets `--model opus --effort max`, and the page then says "Claude Code · Opus".
- **Channels:** while switched on with a bad token, Telegram, Slack and Discord retry every ~5 s against a 401.
- **Completion:** `wisp export <TAB>` offers no kinds.
- **Grid sign-in page:** it links `trywisp.dev` (blocked here).
- **`wisp mcp` (stdio):** it now lists 250 tools, up from 11 in C. b7598c9d made it list `/api/mcp`'s.
- **Still as in C:**
  - The SSH error shows twice in one row; Esc in the Edit form closes the whole Manage dialog.
  - The crew editor's harness order differs from the launcher's.
  - Setup's formula carries `runtime.launch` and, after a Usage or Worktrees select is put back, its default
    explicitly.
  - Signing out of a harness account has no confirm.
  - Connectors' `addedAt` is 0.
  - `wisp channels status` lists `push`.
  - A removed project's tasks keep its name.

## Cleanup done
- **Restored:** every setting the round or the v1 import changed is back to its default (`settings.set … null`).
  `wisp settings list --json` shows no value differing from its declared default. Also:
  - Usage stats on, ntfy off (the import had turned it on).
  - This computer off, grid signed out.
  - Connectors `pasted` and `qa_echo` removed.
  - The duplicate project `repo-again` removed.
- **Stopped:** `wisp serve` :4931, `wisp-grid dev` :4932 (and its boxes), the catcher :4929 and headless Chrome :9354. No
  process under `/tmp/qaD2` is left.
- **Kept for re-checks:** `/tmp/qaD2`: binaries in `fake/` and `bin/`, home, repos, grid data, shots, and scripts in
  `pw/s` (this round's are `d-*`) and `tools/`. The scratch home still holds:
  - the imported v1 machine (`oldbox`, TEST-NET), connector `old-echo`, task kind `run-tests` and task;
  - `repo3-ui` and about 9 fake tasks;
  - accounts Claude 2 and Codex 3 (signed out);
  - `user.qa-theme` (off).
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir this round was told to use).

## Fixes (2026-11-19, uncommitted)

| # | Fix | Test |
|---|---|---|
| D12 | The project kind's one-per-folder check compares folders by their real path (`folder_identity`: links followed, `/private/tmp` = `/tmp`) through a new `StoredKind::unique_by` | `resources-api` `two_spellings_of_one_value_clash_under_the_owners_key`; `runtime` `two_spellings_or_a_link_of_one_folder_are_one_folder_identity`; `serve_setup_d` |
| D3 | `SettingDecl::coerce`: a YAML true/false is the enum's `on`/`yes` or `off`/`no` when it has one; plans normalise a setting through it (`ResourceKind::normalize_named`, additive) so the plan shows `"off"` and an applied one plans unchanged | `kernel_manifest` `an_enum_takes_yaml_on_off_yes_no_read_as_true_or_false`; `serve_setup_d` |
| D2 | `KindInfo::away` (additive): the router kind is put away while `runtime.auto` is off: no kinds list, export (`export router` too), formula, validate or apply; a plan naming it says Auto is off. `apps/docs/data/kinds.schema.json` regenerated (no router while it's off) | `serve_setup_d` |
| D1 | Kit `SettingsSignIn`: Copy turns into a check labelled Copied for 1.5 s (no toast); a failed copy turns back at once | `settings-signin.test.tsx` `copy_the_code_turns_into_a_check_for_a_moment_and_back`; seen in Chrome 1280 light and 390 dark (fb 38 ms) |
| D10 | Only the fake: the stock fake answers `grok models` (wisp's status check) after `logout`; a real grok doesn't (its status reads "not authenticated"). wisp now checks after a harness's own logout (not an added account's) and says "grok is still signed in after `grok logout`: run it in a terminal to sign out there." (an error toast) instead of a silent row | `serve_accounts` `a_cli_still_signed_in_after_its_logout_is_said_audit_d10`; seen in Chrome |
| D4 | CLI human output: a list of rows under a field prints its rows (first 10, then "… N more"); a cell holding a list or map says what's in it | `cli-core` `a_list_of_rows_prints_its_rows_and_a_cell_says_whats_in_it_audit_d4` |
| D5 | `settings.get` of a key no plugin declares and nothing holds is refused, as `set` is (a switched-off plugin's stored key still reads back) | `serve_setup_d` |
| D6 | A whole-value change prints `"zed" → "cursor"` (CLI plan lines and Setup's cards) | `resources-core` `a_whole_value_change_prints_without_a_bare_colon_audit_d6`; `resources/ui` `model.test.ts` |
| D7 | `wisp ls`: a task in `<home>/scratch/<id>` (or with no folder) reads "no project" | `wisp` `a_scratch_task_shows_no_project_and_a_plain_folder_its_name_audit_d7` |
| D8 | Kernel: switching off a plugin that's off keeps its reason ("installed, not switched on yet"); a user's plugin kept off at start reads "is off" (a built-in disabled in the build keeps "disabled in the plugin group") | `kernel_install` `a_plugin_never_switched_on_never_reads_switched_off_audit_d8` |
| D9 | `words::unreadable` / `no_v1_home`: "No file at …", "… is a folder, not a file", "No folder at … (wisp import [v1 home folder])", "… has no config.json" | `wisp` `a_file_that_cant_be_read_is_said_in_words_audit_d9`; `serve_setup_d` |
| D11 | `TaskDecl::title` (additive): a v1 recipe's own name ("Run tests") is its title; New task's chip and Automations show the title, else the name | `wisp` `a_v1_recipe_keeps_its_name_as_its_title_audit_d11`; `shell` `recipes.test.ts`; seen in Chrome |
