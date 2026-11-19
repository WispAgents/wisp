# Audit F: agent and workspace setup (round F, re-check of audit E plus what it missed)

2026-11-19 · worktree `wisp-8fb64ec939b0` at `d4c98dac` · findings only, no repo file edited except this one.

> **Build note:** when the binaries were built (19:52), the worktree differed from HEAD only in `design/v1-reference/`
> (another session's screenshots). Later in the round, other sessions changed docs and the docs site (`README.md`,
> `docs/site/**`, `apps/docs/**`, `scripts/site.mjs`) and added `docs/qa/audit-F-{orchestration,tasks}.md`. None of
> that is in the binaries or the web bundle, which are HEAD's.

**Setup** (copied from audit E, in a fresh `/tmp/qaF2`):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`), `wisp-grid` and `wisp-grid-hostd` were built at HEAD
  with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, then copied to `/tmp/qaF2/fake`. Copies of the fake agent under each
  harness name went to `/tmp/qaF2/bin`. The web UI was built with `vite build --outDir /tmp/qaF2/web`.
- **Server:** `wisp serve --port 4971` with `WISP_HOME=/tmp/qaF2/home` and `HOME=/tmp/qaF2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` pointed at a fake `npm` from the first start.
  - The first start had `WISP_UPDATE_CHECK=0`. The second (the restart check, action 107) left it unset.
- **Catcher on `:4979`:** PostHog, Slack, Telegram, Discord, Linear and the ntfy server, with the same answers as in E.
- **Cloud:** `wisp-grid dev --port 4972 --data /tmp/qaF2/grid` (`wisp.localhost`).
  - Only `WISP_CLAUDE_BIN` and `WISP_CODEX_BIN` were set.
  - Spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` were first on its `PATH`.
- **Test data:** fresh clones of `repo`, `repo2`, `repo3` and `repo4` (remote removed), `repo3-wt` (a linked
  `git worktree` of `repo3`), and E's v1 home with its paths moved to `/tmp/qaF2`. The machine added in the UI used
  `qa@192.0.2.1` (TEST-NET).
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - A persistent headless system Chrome over CDP `:9394` (own profile in `/tmp/qaF2/chrome-prof`), at 1280.
  - For the layout sweep, fresh Chrome and WebKit browsers at 1280 and 390, each in light and dark, closed in
    `finally`.
  - Every context aborted requests to `trywisp.dev`, `example.dev` and `example.invalid`. None was attempted.
- **Ports:** 4971, 4972, 4979 and 9394, none used by audits D or E.
- **Screenshots:** `/tmp/qaF2/shots/` (181, named `f-*`).
- **Stopped at the end:** the server, the grid (its box stopped with it), the catcher and Chrome. No `/tmp/qaF2` process
  is left, and nothing listens on 4971, 4972, 4979 or 9394. `/tmp/wisp-target-fixer` is shared and was left in place.

> **Outbound requests and other things to know.**
> - No `npm view` reached the real registry: the fake `npm` logged the three calls (action 111).
> - No real harness CLI ran: the grid's spy log stayed empty.
> - SSH only went to `192.0.2.1`, which times out.
> - `npm run doctor` was not run: it opens TCP connections to 4800 and 4477, which this round must not touch.
> - Nothing reached grid.trywisp.dev, 4477, 4800, 4791 or 4329. No real credential was read. Every `wisp` call ran with
>   `WISP_HOME=/tmp/qaF2/home` and `HOME=/tmp/qaF2/realhome`.
> - **`wisp update` on a scratch install:** a debug build always reads as a git clone (`install.rs` falls back to the
>   checkout under `debug_assertions`), so `wisp update`, `update apply` and Settings → Updates were checked as a clone.
>   A non-clone install (npm, brew) checks `registry.npmjs.org`, and `WISP_UPDATE_FEED` only redirects the desktop
>   app's feed. So that path can't be tried without reaching a real service, and it wasn't.
> - **An auditor script slip, and what it showed.** The Settings sweep script also flipped every built-in plugin off
>   and on. It stopped with `wisp.settings` off for about 20 minutes; that state led to F1 and F7. Two tasks it led to
>   (`settings get crew.tint`, `learn view`) were fake-Claude runs in `/tmp/qaF2/pw` and `/tmp/qaF2/notrepo`. They
>   changed nothing and were deleted.
> - The fake `gh` doesn't answer `gh auth status`, so Settings → Agents → GitHub shows no state (as in E).

## Counts

| | |
|---|---|
| Actions checked | 154 (E's 142 + 12 new) |
| Pass | 132 |
| Fail (a finding) | 14 |
| Not tested or blocked | 8 (32 sandboxes on a dev grid, 37 ghost folder, 96 `npm run doctor`, 122 paste/Continue, 123 a cancelled first sign-in, 124 a sign-in cut by a restart, 125 a sign-in that times out, 139 Add another account → Cancel) |
| Audit E findings | 9: 8 pass (fixed), 1 still fails (E8) |
| Audit D findings re-checked (D4, D11) | 2 pass (fixed, with E5 and E4) |
| **New findings** | **12 (F1–F12): 1 high, 4 medium, 7 low** |

## Audit E findings, re-checked

| E# | Result | How checked |
|---|---|---|
| E1 | **P** | `cd repo4 && wisp "…"` twice and `cd repo3-wt && wisp "…"`: rec cwd `home/worktrees/repo4-<id>` (both) and `home/worktrees/repo3-<id>` |
| E2 | **P** | `wisp diff -f`: `+ setting/budget.drainWatch (new)` / `false → true`, `+ setting/crew.tint (new)` / `true → false`. Setup: "Adds 2 · setting budget.drainWatch · false → true · setting crew.tint · true → false". The v1 import plan shows `"40" → "80"` for `worktrees.max` |
| E3 | **P** (side effect: F9) | Connectors at 390, Chrome and WebKit, light and dark: no overflow. The failing pill reads "It wouldn't start: defi…" inside its row |
| E4 | **P** | Settings → Automations reads `Recipes | Run tests | Manage`; `wisp export tasks` has `title: Run tests` |
| E5 | **P** | `wisp channels status` prints a `transport` column (ntfy … email), and no `push` row |
| E6 | **P** | `wisp validate -f` / `apply --dry-run` of `router: {best2: true}` → "✗ …: `router:`: Auto is off (runtime.auto), so the router is put away: turn it on with wisp settings set runtime.auto true" (exit 1) |
| E7 | **P** | `wisp import <v1> --dry-run` has no ntfy-token note |
| E8 | **Still fails** | Codex → QA Work → ⋯ → Sign out: the row reads "Signed out · Sign in", and its ⋯ menu still lists "Rename · Make it a personal account · Move up · Move down · Sign out · Remove" |
| E9 | **P** | "Codex is still signed in after `codex logout`: …", "Grok is still signed in after `grok logout`: …" |
| D4 | **P** | Fixed with E5 |
| D11 | **P** | Fixed with E4 |

## Action table

Key: P = pass, F# = fails with that new finding, E8 = still fails, — = not tested or blocked. "server" means the effect
was read back with `settings.list`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded.
"@100"/"@150" means the UI state 100–150 ms after the click; "fb" is ms from the click to the first DOM change.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P (note) | Versions from the fakes; opencode "Free models only · Add key"; GitHub's row has no state (fake `gh`) |
| 2 | Codex Sign in (device code) | P | fb 25 ms; the box at 100 ms: "1. Open the sign-in page and sign in with your ChatGPT account. · Open the sign-in page · 2. Enter this code there: ABCD-12345 · Waiting…"; "Signed in" by 6 s; no popup |
| 3 | Add another Codex account (Work) | P | Ownership terms dialog; `accounts.list` has `Codex 2` (work, ready) |
| 4 | Account menu: Move up / Move down | P | Rows swap @150; server order follows |
| 5 | Rename an account | P | Focused field with "Codex 2"; "QA Work" @150 and on the server |
| 6 | Sign out of an account | P (note) | "Signed out · Sign in" @150, `signedOut` on the server; no confirm (as in E) |
| 7 | Remove an account (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. Tasks that ran on it keep their history." · Remove account |
| 8 | Harness Update button | P | After the restart with checks on: "Update to 9.9.10" ×3; fb 28 ms; toast "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/tmp/qaF2/bin/claude update` in a terminal, then re-check." |
| 9 | Model default (launcher pick) | P | Claude Code → Opus fb 42 ms, "Opus" @150, kept across reload; rec `--model opus` |
| 10 | Thinking default | P | Slider to Max → "Opus · max"; `runtime.launch` `{agent: claude, efforts: {claude: max}, …}`; rec `--effort max` |
| 11 | Lean Claude Code runs select | P | fb 19 ms; Off / wisp’s side runs / All Claude runs; `runtime.leanRuns` all → off, restored |
| 12 | Ask before running a changed harness | P | fb 18 ms, flips @100; restored |
| 13 | Re-check | P | fb 9 ms; toast "Up to date: nothing new" |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100 (fb 17–26 ms), its server key changes, restored |
| 15 | General: 4 selects | P | `settings.editor`, `terminal.terminal`, `review.harness`, `update.updateChannel`, fb 23–33 ms, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` @100; the account menu label flips; kept across reload |
| 17 | Notifications: Accept tasks from channels, Default project | P | `channels.acceptChannels` fb 19 ms; Default project fb 26 ms |
| 18 | Notifications: Desktop notifications | P (note) | Disabled, "Blocked — allow notifications for this site in your browser" (headless) |
| 19 | Memory: Learn from tasks | P | `learn.enabled`, fb 35 ms |
| 20 | Automations: Webhook triggers | P | `automations.webhookTriggers`, fb 16 ms |
| 21 | Usage: 5 switches, 4 selects | P | `budget.*`, `runtime.limitWait`, `runtime.subagentModel`, `machine.*` (fb 9–28 ms); putting a machine select back writes its default explicitly (as in E) |
| 22 | Worktrees: 3 selects | P | `worktrees.cleanup`, `.max`, `.onDone`, fb 9–26 ms |
| 23 | Privacy: 3 switches | P | Usage stats, `worktrees.projectAsk`, `git.maskEnvFiles`, fb 15–36 ms |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 10 → 10 after a task and `wisp telemetry flush` ("queued false"); switched back on |
| 25 | Updates: Channel; Check now | P | Channel select fb 31 ms; "A git clone: pull and restart to update"; Check now disabled, "Checks are off on this install" (with checks on too: a clone never checks) |
| 26 | Keyboard shortcuts pane; ⌘K, ⌘P, `[` | P | ⌘K and ⌘P list; `[` hides the list and brings it back; N focuses the launcher |
| 27 | Settings search | P | `ntfy` → Phone, `telemetry` → Privacy and the Lean row, `worktree` → General, Worktrees and 5 rows; `xyzzy` → "No settings match “xyzzy”"; each hit opens its pane (the second hit opens All settings filtered to the key) |
| 28 | All settings | P | No `runtime.auto` row, no `router.*` |
| 29 | Setup: Make the formula | P | Settings, machines, projects, connectors, channels, notifications, tasks; no `router:` |
| 30 | Setup: Import → See what it changes → Apply | P | "Adds 1 · project repo3-ui · Changes 1 · setting settings.editor · cursor → zed"; toast "Imported"; both on the server |
| 31 | Diagnostics: report | P | The report JSON (7 KB) and "this computer · 2.0.0 · 51 plugins · online" |
| 32 | Sandboxes | — | A dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus | P | Focus in `~/code/my-app or a GitHub URL` |
| 34 | Add a missing folder / a file / a relative name | P | "No folder at …", "… is a file, not a folder: pick the folder it's in", "repo2 isn't a full path: …" |
| 35 | Add a `file://` URL | P | `file:///tmp/qaF2/repo4` added as repo4 |
| 36 | Add a duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo" (both spellings); "That folder is already repo2" |
| 37 | Real folder check via import / apply | — | Not re-run with a ghost folder (the v1 import's missing folder: action 86) |
| 38 | Remove a project (confirm) | P | repo4: fb 27 ms, "Remove “repo4” from wisp? Its 2 tasks and its files are kept." · Remove project; gone from `wisp export projects` |
| 39 | Multi-pick (top bar) | **F3** | With 4 projects the switcher lists All projects, repo … repo4 and Add project…; Manage projects… and Repo history only show once you type |
| 40 | Tasks in a project folder (UI and CLI) | **F2** | UI task in `repo`: rec cwd `home/worktrees/repo-<id>`, `--model opus --effort max`. The first one's page went to "This part of wisp hit an error · Unexpected end of JSON input" |
| 41 | `wisp apply` of a second project for a folder | P | D12 |

### Machines, peers, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Machines → Manage → Add `qa@192.0.2.1` | P | fb 48 ms, "Connecting" @150, then error (`wisp machines list`) |
| 43 | Edit… (Advanced) | P (note) | SSH address, Name, wisp’s port there, Start command, Connect when wisp starts; Esc closes the whole Manage dialog (as in E) |
| 44 | Remove (confirm) | P | The imported `oldbox`: fb 24 ms, "Remove oldbox from wisp? Its projects leave the launcher. Nothing on the machine is touched …"; `(none)` |
| 45 | Peers: `wisp machines discover`, `hosts`, `reachable` | P | `(none)`, `(none)`; `reachable` prints this computer's ssh lines |
| 46 | This computer: memory, CPU, disk, what's using it | P | "45% free", "145% of 8 cores", "111 GB free", "wisp itself 63 MB" |
| 47 | Pause automations / Resume now | P | fb 16 ms, "For 1 hour · Until tomorrow 8:00"; "Automations paused until 21:13" @150; Resume fb 32 ms, "Running as usual" @150 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in: device code, popup, dev sign-in, Allow | P | The code at 150 ms; popup `/activate?code=…`; "Device connected"; `wisp cloud status`: qa-alice |
| 49 | Don't allow | P | "Sign-in was declined in the browser."; the address stays `http://wisp.localhost:4972` |
| 50 | This computer on the grid → on | P | fb 9 ms, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4972" @150 |
| 51 | Grid restart: the tunnel comes back | P | Its box stopped with it; `cloud status` online 2 s after the restart; the address answers 401 without a sign-in |
| 52 | API key: create, use, revoke | P | fb 36 ms, "qa-script is ready · Copy it now. You won’t see it again."; `/api/health` 200 with it, 401 without; "Revoke qa-script? …" (fb 23 ms) → 401 |
| 53 | Add an account (qa-bob), Use another account | **F12** | Both rows listed, qa-alice stays current (`wisp cloud accounts`: `● qa-alice`). ADR-0061 says a new sign-in becomes current |
| 54 | Switch to qa-bob and back | P | fb 29 ms, Current moves @150; This computer off for qa-bob; `cloud status` follows |
| 55 | Sign out of qa-bob (confirm) | P | fb 29 ms, "Sign out of qa-bob on wisp.localhost:4972 here? …"; gone @150 |
| 56 | Cloud computer woken by use | P | `wisp ls --on Cloud`: asleep → online; spy log empty |
| 57 | `wisp computer off`, `wisp cloud signout` | P | "this computer: off · http://…", "✓ signed out", "not signed in (wisp cloud signin)" |

### Connectors and MCP servers
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector ("QA Echo!" → `qa_echo`) | P | "1 tool" by 4 s; echo log: initialize, initialized, tools/list |
| 59 | Edit: add env, Save | P | Server `env: [{QA_VAR, set: true}]`; the echo started with `QA_VAR=hello-ui` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it"; `pasted`, `remote1` added |
| 61 | Connectors reach a run | P | rec `--mcp-config` servers `old-echo`, `pasted`, `qa_echo`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, and its saved keys are deleted." |
| 63 | Presets | P | With no connectors left: Linear (remote, `https://mcp.linear.app/mcp`), Filesystem, Postgres fill their fields (fb 12–30 ms); not added |
| 64 | `wisp mcp` tools, `wisp_launch` → `wisp_list` | P | 250 tools over stdio; launched `qa via mcp` (codex), listed by `wisp_list` |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy: Test | P (note) | fb 65 ms; the catcher got the test POST. Nothing on screen says it was sent (no toast, the button keeps "Test") |
| 66 | Telegram, bad token | P | "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| 67 | Slack, bad tokens | P | "Slack: invalid_auth. Check the tokens …" at the card's foot; link code shown (`f-67-slack.png`) |
| 68 | Discord, bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | "Email works on a cloud computer: set it up there."; its switch disabled |
| 71 | Switch off and Remove the tokens | P | All `off`, nothing saved; no Telegram, Slack or Discord request after that |
| 72 | "push" card | P | No card, and `channels status` no longer lists `push` |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` shows in both lists | P | "user.qa-theme is installed, not switched on yet" |
| 74 | Install from a folder (UI) | P | "QA Pink 1.0.0 is installed, off · It asks for nothing beyond showing itself. · Switch it on · Not now" |
| 75 | Switch it on | P | On @150; untrusted styles don't apply (`--accent` `#2f5bea`) |
| 76 | Trust / Untrust, partStyles | P | "Trust QA Pink? …" · Trust it → `--accent` `#ff1493`, `launcher.send` rgb(255, 20, 147); Untrust → `#2f5bea` |
| 77 | Switch off, Remove | P | Off fb 21 ms; Remove fb 18 ms "Remove QA Pink? Turns it off and deletes it …"; gone @150 |
| 78 | Export after Remove | P | `wisp export plugins` lists only `user.qa-theme` |
| 79 | CLI enable/disable reflects live | P | The row follows `wisp plugin enable/disable user.qa-theme` |
| 80 | Built-in off and on (learn) | P | fb 36 ms, "wisp.learn was switched off"; `learn.view` answers `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the new task at once; it runs in `home/scratch/<id>`; `wisp ls`: `no project` |

### Import from v1, `wisp init`, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | P | "13 to add"; each new setting shows default → value; "!" lines for machine, ntfy, task; "·" notes for the connector's environment and the two projects |
| 83 | `wisp import --yes` | P (see F4) | "✓ applied 13 changes", "✓ 1 task imported, 0 unchanged" |
| 84 | v1 ntfy | P | It polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | editor, maskEnvFiles, worktreeMax, limitHold, notify, leanRuns, retentionDays, subagentModel |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at /tmp/qaF2/v1-gone-folder"; "· project old-app: its folder is repo here, kept as repo" |
| 87 | Imported v1 task's project | P | `v1sess0001 "A v1 task"`: project `repo` |
| 88 | v1 default agent | P | A CLI task with no `--agent` runs on codex |
| 89 | `wisp init` (repo, again, non-repo) | P | `.wisp/wisp.yml` + AGENTS.md note; "is ready" again; non-repo refused naming `--here` (exit 2) |
| 90 | `wisp settings get/set` (good, bad value, unknown key) | P (note) | "worktrees.max must be one of off, 20, 40, 80" (exit 1); "no plugin declares the setting nope.key" (exit 1). A good `set` prints the whole setting record (decl, owner, scope, value) |
| 91 | `wisp export` yaml/json/toml → validate → diff | P | 29 resources each; "0 to add, 0 to change, 0 to remove, 29 unchanged"; secrets as `${secret:…}` |
| 92 | `wisp validate`/`apply` errors | P | D9; a folder with no formula: "✗ no *.wisp.yml in /tmp/qaF2/repo" |
| 93 | `wisp completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass; fish 1450 lines; no `router` in any |
| 94 | What bash completion offers | P | `--agent <TAB>` gives all 7 harnesses; `settings set worktrees.max <TAB>` gives `off 20 40 80`; `export <TAB>` now offers the kinds |
| 95 | `wisp skill install --agent all` | P | Three ✓ skill paths, project files, Codex config; one "·" note for opencode |
| 96 | `npm run doctor` | — | It opens TCP connections to 4800 and 4477; not run |
| 97 | `wisp <group> <unknown word>` | P | `machines`, `plugin`, `cloud`, `settings`, `channels`, `connectors` each list their verbs (exit 2) |
| 98 | `wisp "…" --agent frob` | P | "✗ no harness frob: --agent takes claude, codex, …" (exit 2); no task created |

### Auto disabled everywhere (`runtime.auto` off)
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `runtime.auto` default | P | `false`; `wisp router frob` → "wisp router has no frob: Auto is off (runtime.auto), so the router is put away" (exit 2) |
| 100 | Launcher harness picker | P | The 7 harnesses and "Set up another agent…"; no Auto |
| 101 | ⌘K: auto, harness, model | P | No Auto entry |
| 102 | Crew editor harness | P | "As New task" and the 7 harnesses; no Auto |
| 103 | All settings | P | No `runtime.auto`, no `router.*` |
| 104 | Settings search "auto" / "model" | P | No Auto hit |
| 105 | Setup and the formula | P | No `router:` in Setup's formula or a whole `wisp export` |
| 106 | CLI | P | E6 |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `plugin` and `cloud status` identical before and after; the machine goes error → connecting |
| 108 | WebKit 1280: home and all 20 panes | P | Light and dark: no overflow, no console errors, no failed requests |
| 109 | Chrome 390: home and all 20 panes | **F9** | Light and dark: no overflow, no console errors. Connectors' "1 tool" pills read "1 …" |
| 110 | WebKit 390: home and all 20 panes | **F9** | The same. The probe's flagged "1 hour", "All 14" and "Add another … account" are one line on screen |
| 111 | Harness update check at start | P | Restarted 20:29:19 with checks on: no `npm view` at start, the three at 20:30:19 |
| 112 | `wisp-grid dev --help` | P | Usage, nothing started; `dev frob` → "✗ wisp-grid dev doesn't take frob" |

### Re-checks of C's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects: each project's task count | P | "/tmp/qaF2/repo · 1 task", "/tmp/qaF2/repo2 · 2 tasks", "/tmp/qaF2/repo3 · 0 tasks" |
| 114 | Setup → Import error paths | P | A YAML error and `hello` plan nothing and offer no Apply; a toast "pasted.yml: a formula is a map (wisp: 1, then its sections)" |
| 115 | Plugins → Install error paths | P | "…/notaplugin has no wisp-plugin.json", "No folder at …", "Couldn't reach example.invalid to clone …", "not a path isn't a full path: …", "repo2 isn't a full path: …" (fb 30–37 ms) |
| 116 | CLI error paths | P | `export --format xml` → "✗ no format xml: yaml, json or toml" (exit 2) |
| 117 | Project colour | P | 9 colours, fb 24 ms; `wisp export projects`: `color: amber` |
| 118 | CLI text output of nested values | P | E5 |
| 119 | ⌘K ranking | P | "model": Agents first |

### Re-checks of D's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 120 | Sign-in box: numbered steps, Open the sign-in page, the code | P | Steps 1 and 2 with ChatGPT named; "Copy the code" icon button |
| 121 | Sign-in box: Copy the code | P | "Copied" at 37 ms, back to "Copy the code" by 1.8 s |
| 122 | Sign-in box: paste the code, Continue | — | The fake has no Claude or Antigravity sign-in to reach the paste step |
| 123 | A cancelled first sign-in | — | Not re-run |
| 124 | A first sign-in cut by a restart | — | Not re-run |
| 125 | A first sign-in that times out | — | It gives up after 15 minutes; not waited for |
| 126 | Harness Sign out (Grok, no accounts) | P | E9 |
| 127 | A hand-written formula with `off` values | P | Setup: "Changes 2 · runtime.leanRuns all → off · worktrees.max 80 → off", Apply enabled |
| 128 | v1 recipe imported | P | E4 |
| 129 | `wisp ls` of a task with no project | P | The cast task reads `no project`; a folder outside every project reads its name |
| 130 | Dark theme at 1280 and 390, Chrome and WebKit | P | Covered in 108–110 |
| 131 | MCP with Auto off | P | No `router_*` tool over stdio `wisp mcp` (250 tools) or `/api/mcp` (246) |

### Re-checks of E's new rows
| # | Action | Result | How checked |
|---|---|---|---|
| 132 | Agents: Update all (3) | P | fb 18 ms; rec `update` for claude and codex, `upgrade` for opencode; one toast each |
| 133 | Account menu: Make it a personal / work account | P | fb 26 ms, "Personal" @150, `kind: personal`; the item then reads "Make it a work account" |
| 134 | A signed-out account's ⋯ menu | **E8** | It still offers "Sign out" |
| 135 | Add connector errors | P | Empty name: "A name, so agents can tell its tools apart" (fb 40 ms); empty command: "The command that starts it"; a name taken: toast "There's a connector called qa_echo already"; a missing command: "It wouldn't start: definitely-not-a-command-qa: not found" |
| 136 | A CLI launch in a git repo that isn't a project | P | E1 |
| 137 | Add machine: bad addresses | P | Empty: Add disabled; "has spaces here" and "qa@": toast "SSH host: user@host, a host name, or a Host from ~/.ssh/config." (fb 32–39 ms) |
| 138 | A plan that adds a setting | P | E2 |
| 139 | Add another account → Cancel | — | Not re-run |
| 140 | Notifications → Default project with projects | P | "Last used (repo)", repo, repo2 |
| 141 | Worktrees: Bring in | P (after a restart; see F6) | "1 worktree from git … Bring in" for the linked `repo3-wt` |
| 142 | The harness Sign out toast's words | P | E9 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 143 | An imported v1 connector's environment | **F4** | v1's `Old Echo` had `QA_VAR`; after the import, `old-echo` has `env: []` and started with `QA_VAR=undefined` |
| 144 | Every built-in plugin off and on (Plugins pane) | P (note) | 50 switches: each flips @100 (fb 9–36 ms) and comes back; `wisp.plugins` can't go off; `wisp.issues` reads `resolved` after |
| 145 | A built-in's CLI group while that plugin is off | **F1** | `wisp settings get crew.tint` (wisp.settings off) and `wisp learn view` (wisp.learn off) each launched a task |
| 146 | Settings while `wisp.settings` is off | **F7** | All settings: "No setting matches."; General → Crew tint doesn't move, then a toast "settings.set: wisp.settings is off …" |
| 147 | Worktrees pane after the plugin sweep | **F6** | "4 worktrees" while 14 were on disk; no Bring in |
| 148 | `wisp update` on a git clone | **F5** | "✓ wisp 2.0.0 is the latest"; `--json`: `enabled false`, `checked_at null`, `latest null` |
| 149 | `wisp update view / check / install` | **F10** | Each: "✗ wisp update view needs --peer (a machine's id …)" |
| 150 | A channel switched on and back off | **F8** | After Telegram, Slack, Discord and GitHub were each switched on then off (no token), `wisp export` lists all four (`enabled: false`, `token: ""`) and a `phone-telegram` notification |
| 151 | `wisp connectors remove <id>` | **F11** | "✗ wisp connectors remove doesn't take "pasted" here (see --help)"; `--id pasted` works |
| 152 | `wisp update apply` on a git clone | P | "✗ This copy of wisp is a git clone, so it does not update itself." (exit 1) |
| 153 | Phone: the 7 channel switches | P | Each flips @100 (fb 29–33 ms) and back; Email's is disabled |
| 154 | Grid stopped and started | P | Its box (`wisp serve` under the grid) stopped with it; the tunnel came back in 2 s (51) |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### F1 · H · With a built-in plugin off, its CLI commands start a Full-auto task
- **Steps:** Settings → Plugins → switch `wisp.settings` off (or `wisp plugin disable wisp.learn`) → in a folder, run
  `wisp settings get crew.tint` (or `wisp learn view`).
- **Actual:**
  - `◌ launched: settings get crew.tint` and `◌ launched: learn view`. Each ran Claude Code with
    `--permission-mode bypassPermissions` in the current folder (`/tmp/qaF2/pw`, `/tmp/qaF2/notrepo`), in place.
  - The fake changed nothing. A real agent gets "settings get crew.tint" as its prompt, in Full auto, in whatever
    folder the user was in.
  - With `wisp.settings` off the default agent can't be read either, so it ran Claude, not the user's default (Codex).
- **Compare:** `wisp router frob` with the router put away is refused ("Auto is off …"), and `wisp settings frob` with
  the plugin on lists the verbs. Only an off plugin's group falls through to a prompt.
- **Expected:** a known command group whose plugin is off is refused with the plugin's state ("wisp.settings is off:
  wisp plugin enable wisp.settings"), and never becomes a task.

### F2 · M · A new task's page can crash right after it starts
- **Steps:** home → type a prompt → Enter (project `repo`, worktree per task on).
- **Actual:**
  - The first launch this round showed "This part of wisp hit an error · Unexpected end of JSON input · Try again ·
    Home" (`f-40-uitask.png`). The console had "wisp: the main pane failed SyntaxError: Unexpected end of JSON input"
    from `TaskView`. A reload showed the task.
  - Seen once in 9 UI launches.
- **Why:** `packages/shell/src/frame/TaskView.tsx` ~1019–1035. `changeWhere` returns `''` while the task has no worktree
  yet. Solid's `createResource` skips a fetch only for `null`/`undefined`/`false`, so the fetcher runs
  `JSON.parse('')` and the resource holds that error. When the worktree appears, `diff()` reads `changedNow.latest`
  (its `changeWhere()` guard now passes) and can get the stale error before the new fetch starts.
- **Expected:** return `undefined` (not `''`) when there's no worktree, or guard the parse.

### F3 · M · The project switcher hides Manage projects… (and later projects) once there are four projects
- **Steps:** add four projects → top bar project select (or ⌘P).
- **Actual:**
  - The list shows All projects, the four projects and Add project…. Manage projects… and Repo history only show if
    you type "manage" (`f-39-projsel-manage.png`). With two projects all three tail rows showed.
  - `packages/shell/src/palette/match.ts`: "an empty query shows each group's first `limit` (6) items". The switcher's
    projects and its tail share one group.
  - With six or more projects, Add project… also goes. From the seventh, projects you can reach with ⌘6–⌘9 aren't
    listed either.
- **Expected:** the switcher lists every project with an empty query, and keeps Add project…, Manage projects… and
  Repo history at its foot.

### F4 · M · Importing a v1 connector drops its environment variables
- **Steps:** a v1 home whose connector has `env` (v1 keeps the key names in `config.connectors[].env`, and their values
  in its secrets) → `wisp import <v1> --yes`.
- **Actual:**
  - The plan notes "connector Old Echo: its environment stays in secrets; set it with wisp secret set".
  - The imported `old-echo` has `env: []` and started with `QA_VAR=undefined` (`mcp-echo.log`).
  - The note doesn't hold. The variable's name isn't kept, so a `wisp secret set` doesn't reach the connector, and
    v1's value isn't brought over.
  - `crates/wisp/src/v1_formula.rs` ~130: the connector spec takes only `command`, `args`, `url` and `enabled`.
  - The display name is lost too ("Old Echo" → `old-echo`).
- **Expected:** keep the env names as `${secret:…}` references (and bring over v1's values, or say which to set), so the
  connector starts with them.

### F5 · M · `wisp update` says "latest" when it never checked
- **Steps:** a copy whose checks are off (a git clone, `WISP_UPDATE_CHECK=0`, CI) → `wisp update`.
- **Actual:**
  - "✓ wisp 2.0.0 is the latest". `wisp update --json` has `"enabled": false`, `"checked_at": null`, `"latest": null`.
  - Settings → Updates on the same copy says "Checks are off on this install" and "A git clone: pull and restart to
    update".
- **Expected:** say checks are off (and, for a clone, to pull), as the pane does. Never claim "latest" without a check.

### F6 · L · The Worktrees pane got stuck "looking" after the built-ins were switched off and on
- **Steps:** (seen once) the Settings sweep switched every built-in plugin off and on, `wisp.settings` off for ~20
  minutes. Then tasks kept launching.
- **Actual:**
  - For the next 30+ minutes Settings → Worktrees read "4 worktrees · 40 KB", with 14 on disk.
  - `wisp worktrees view` listed all 14, 10 of them "checking", `looking: true`. Every worktree made after the sweep
    stayed "checking".
  - No Bring in offer for the linked `repo3-wt` (E's action 141).
  - After a restart: 14 listed, sized, and the Bring in offer showed.
  - One `wisp plugin disable/enable wisp.worktrees` then a launch did not reproduce it.
- **A lead:** `look_for_offers` (`plugins/worktrees/host/src/lib.rs` ~155) increments `offering` and decrements it
  after an `await`, with no guard. A look cut by the plugin going off leaves the count up, so the pane says it's
  looking forever.

### F7 · L · Settings with `wisp.settings` off says nothing about why
- With the plugin off (it can be switched off from Plugins):
  - All settings reads "No setting matches.".
  - A switch (General → Crew tint) doesn't move, then shows a toast "settings.set: wisp.settings is off (wisp.settings
    was switched off)".
  - The other panes still show their old values.
- **Expected:** All settings and the panes say the settings plugin is off, with a way to switch it back on. Or the
  plugin can't be switched off, as `wisp.plugins` can't.

### F8 · L · A channel switched on and back off stays in the formula
- **Steps:** Settings → Phone → switch Telegram (Slack, Discord, GitHub) on, then off, without a token.
- **Actual:**
  - `wisp export` (and Setup → "Channels 5") now lists each channel, as `enabled: false`, `token: ""` (GitHub with
    `routing: {publicReports: false}`).
  - It also lists a `phone-telegram` notification.
  - Before, only the imported ntfy was there. Putting a switch back changes what the formula holds.
- **Expected:** a channel switched back off with nothing set leaves no resource, as a setting put back to its default
  leaves none.

### F9 · L · On a phone, a connector's short state pill is cut to "1 …"
- Settings → Connectors at 390 (Chrome and WebKit, light and dark).
- Since E3's fix, every pill truncates: "1 tool" reads "1 …", while the command beside it keeps most of the row
  (`f-L-chromium-390-light-connectors.png`).
- **Expected:** a short pill keeps its words, and the command takes the truncation.

### F10 · L · `wisp update view|check|install` ask for `--peer` on this wisp
- `wisp help update` lists `check · install · view` ("This wisp's version, and the newest one").
- Each prints "✗ wisp update view needs --peer (a machine's id: check, install or view the update there)" (exit 2).
- **Expected:** with no `--peer`, act on this wisp (as `wisp update` and `wisp update apply` do), or help says they're for
  another machine.

### F11 · L · `wisp connectors remove <id>` refuses the id it needs
- `wisp connectors remove pasted` → "✗ wisp connectors remove doesn't take "pasted" here (see --help)".
- `--help` shows only `--args JSON` and `--<field> VALUE`, never which field. `--id pasted` works.
- `wisp plugin rm <id>` and `wisp cloud remove <account>` take it plainly.
- **Expected:** take the id as its word, or name `--id` in the usage line.

### F12 · L · ADR-0061 and the sign-in code disagree on which account is current
- Signing in to a second grid account keeps the first one current. `signin.rs` `done`: "a second account is added
  beside the current one, which keeps this computer on its grid".
- ADR-0061 (accepted, not superseded) says "Signing in adds an account and makes it current", and `done`'s own doc
  comment still says "the account added (or renewed) and made current".
- The behaviour is reasonable. The docs that describe it are not up to date, and no ADR records the change.

---

## Smaller notes (not counted)
- **ntfy Test:** the POST goes, but nothing on screen says it was sent (no toast, the button stays "Test").
- **`wisp settings set`** prints the whole setting record (decl, owner, scope, value), not just the new value.
- **New task (tasks area):** for the first ~150 ms the row reads "Claude Code · Default model" while the pick is
  Opus · max (as in D and E). The run gets `--model opus --effort max`.
- **Cloud pane:** the cloud computer read "Asleep" right after a key call to it had answered 200, until a command woke it
  through wisp (as in E).
- **Slack** shows steps 1–3 ticked and the link code while its tokens are refused (the error is at the card's foot).
- **`wisp worktrees view`** reads `looking true` even after a restart, with every row sized.
- **Still as in E:**
  - The SSH error shows twice in one row; Esc in the Edit form closes the whole Manage dialog.
  - Signing out of a harness account has no confirm.
  - Connectors' `addedAt` is 0.
  - `wisp computer off` prints the address it no longer serves.
  - Putting a Usage select back writes its default explicitly (`machine.cpuCeiling: "200"`).
  - GitHub's Agents row has no state with the fake `gh`.

## Cleanup done
- **Restored:** every setting is back to its default (`settings.set … null`; `settings.list` shows no value differing
  from its declared default). Also:
  - Usage stats on, ntfy off (the import had turned it on), Telegram, Slack and Discord off with no tokens.
  - Every built-in plugin on; `user.qa-theme` off.
  - This computer off, grid signed out.
  - Connectors `old-echo`, `pasted`, `qa_echo` and `qa_bad` removed.
  - The second Codex accounts removed.
  - The two tasks the sweep led to (`settings get crew.tint`, `learn view`) deleted.
- **Stopped:** `wisp serve` :4971, `wisp-grid dev` :4972 (and its box), the catcher :4979 and headless Chrome :9394. No
  process under `/tmp/qaF2` is left.
- **Kept for re-checks:** `/tmp/qaF2`: binaries in `fake/` and `bin/`, home, repos (`repo4` and the linked `repo3-wt`),
  grid data, shots, and scripts in `pw/f` (this round's), `pw/s` and `tools/`. The scratch home still holds:
  - the machine `qabox` (TEST-NET), task kind `run-tests` and its imported task;
  - projects `repo` (colour amber), `repo2`, `repo3-ui` and about 20 fake tasks;
  - the channel and notification rows of F8.
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir this round was told to use).
