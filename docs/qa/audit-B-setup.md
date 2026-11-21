# Audit B: agent and workspace setup (round B, re-check of audit A plus what it missed)

2026-11-19 · worktree `wisp-8fb64ec939b0` at `c0d655ad` (plus other sessions' uncommitted edits) · finding only, no repo file edited
except this one.

**Setup** (copied from audit A):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`) and `wisp-grid` were built with
  `CARGO_TARGET_DIR=/tmp/wisp-target-qaB2`, then copied to `/tmp/qaB2/fake`. The web UI was built with
  `vite build --outDir /tmp/qaB2/web`.
- **Server:** `wisp serve --port 4951` with `WISP_HOME=/tmp/qaB2/home` and `HOME=/tmp/qaB2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_UPDATE_CHECK=0`.
  - From the first restart on, `WISP_NPM` pointed at a fake `npm` (see B5).
- **Catcher on `:4949`:** PostHog, Slack, Telegram, Discord, Linear and the ntfy server all pointed at it. It answers
  Telegram and Discord with 401 and Slack with `invalid_auth`.
- **Cloud:** `wisp-grid dev --port 4972 --data /tmp/qaB2/grid` (`wisp.localhost`).
- **Test data:** repos `/tmp/qaB2/repo`, `repo2` and `repo3`. The v1 home is audit A's, copied to `/tmp/qaB2/v1home`.
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - System Chrome headless over CDP at 1280 and 390, and WebKit at 1280 and 390.
  - Every browser context aborted any request to `trywisp.dev`; none was attempted.
- **Screenshots:** `/tmp/qaB2/shots/` (134).
- **Stopped at the end:** the servers, the catcher and Chrome. The 8.6 GB target dir is deleted. `/tmp/qaB2` stays for
  re-checks.

> **Outbound requests and other things to know.**
> - **npm registry.** The first `wisp serve` start ran `npm view <package> version` for Claude Code, Codex and
>   opencode against the real npm registry (public package metadata, no credentials). This is the harness update
>   check, finding B5. Every later start used a fake `npm`.
> - **Real harness CLIs.** The dev grid's cloud computer ran version and sign-in probes of this Mac's real `cursor-agent`,
>   `grok` and `agy` binaries (finding B8). It had its own HOME (`/tmp/qaB2/grid/users/…`), so they found no
>   credentials and no task ran on them. I restarted the grid with the fakes first on PATH.
> - **Stray dev grid.** I ran `wisp-grid dev --help` by mistake. It started a dev grid on `:8080` with its data in
>   `~/.wisp-grid-dev` (finding B7). I stopped it within a minute and deleted that folder, which held only the empty
>   database the run had just created.
> - Nothing reached grid.trywisp.dev, 4477, 4800 or 4791. No real credential was read.

## Counts

| | |
|---|---|
| Actions checked | 112 |
| Pass | 94 |
| Fail (a finding) | 14 |
| Not tested or blocked | 4 (`npm run doctor`, removing a project, connector presets and remote OAuth, Sandboxes on a dev grid) |
| Audit A findings | 33: 29 pass, 4 partly fixed (F1, F4, F27, F30), 0 still failing as before |
| New findings | 13 (B1–B13): 0 high, 6 medium, 7 low. B1, B2, B3 and B10 are what remains of F30, F1, F4 and F27 |

## Audit A findings, re-checked

| A# | Result | How checked |
|---|---|---|
| F1 | **Partly fixed → B2** | UI: "No folder at …" and "… is a file, not a folder: pick the folder it's in". `wisp import` still adds v1's `gone` (folder missing); `wisp validate`/`apply` accept a project at a missing folder |
| F2 | P | After "Don't allow" the address stays `http://wisp.localhost:4972`; Add an account defaults to the current grid |
| F3 | P | "repo2 isn't a full path: type the folder's full path, like ~/code/app"; a `file://` URL is accepted as its folder |
| F4 | **Partly fixed → B3** | `cd repo2 && wisp "…"`: `project: repo2`, worktree `home/worktrees/repo2-…`. The imported v1 task still has no project |
| F5 | P | "Open tasks in": iTerm, Terminal.app, Auto-detect, Custom command (shows Terminal command) and In wisp all stick (`terminal.terminal`) |
| F6 | P | Light → menu says "Dark theme", Dark → "Light theme"; the menu item flips the theme and its label |
| F7 | P | Phone shows ntfy, Telegram, Slack, Discord, GitHub, Email; no "push" card (`channels status` still lists it, not shown) |
| F8 | P | "Ask before running a changed harness" flips at 80 ms |
| F9 | P | Edit → Environment `QA_VAR=hello-ui` → Save: server `env: [{QA_VAR, set: true}]`; the MCP server starts with `QA_VAR=hello-ui` |
| F10 | P | Pasting 2 servers says "This config has 2 servers: remote1 will be added with it"; both added |
| F11 | P, with **B6** | `theme` → General, `ntfy` → Phone, `telemetry` / `usage stats` → Privacy, `xyzzy` → "No settings match “xyzzy”". The "Auto" hit leads nowhere (B6) |
| F12 | P | Labels and descriptions read right (`git.cloneDir`, `runtime.subagentModel`, `settings.timeZone`, `settings.readTimes`) |
| F13 | P | Usage → Automations stop at 75% → `wisp export budget` has `stopAt: "75"`; Setup's formula has it too |
| F14 | P | Pause → For 1 hour: "Automations paused until 2:51" at 150 ms; Resume now: "Running as usual" at 150 ms |
| F15 | P | Revoke → "No keys yet" and the key panel is gone; the key gets 401 |
| F16 | P | Adding qa-bob keeps qa-alice Current and this computer online (`wisp computer status`) |
| F17 | P | "Use another account" → `/signin?return=/activate?code=…` → after qa-bob signs in, the same code's Allow page |
| F18 | P | `wisp plugin new qa-theme`: listed by `wisp plugin` and in Settings → Plugins (off) |
| F19 | P, with **B13** | Trust and Untrust are on the row. Trusted, `--accent` is `#ff1493` and `launcher.send` is pink; untrusted, back to `#2f5bea` |
| F20 | P | After Remove, `wisp export plugin` lists only `user.qa-theme` |
| F21 | P | With Plugins open, `wisp plugin enable/disable user.qa-theme` shows in the row within 2.5 s |
| F22 | P | `wisp settings set worktrees.max 80`, `budget.limitHold 80` and a formula `worktrees.max: 20` (unquoted) are accepted |
| F23 | P | `wisp apply` of a second project at repo2's folder: "one project per path; name it repo2 in the formula". The import maps "Old App" to the existing `repo` |
| F24 | P | Import: ntfy `routing.topic: wisp-old-topic-123456789`, server kept, `allow: []` |
| F25 | P | Import carries `maskEnvFiles`, `leanRuns`, `editor`, `notify`, `limitHold` and `retentionDays`; `subagentModel` goes to `agents.default` (B11) |
| F26 | P | Other folder… focuses the path field; the empty state reads "No projects yet. Type a folder’s path above, or paste a repo URL" |
| F27 | **Partly fixed → B10** | `wisp pl<TAB>` → `plugin plugins`; `plugin`, `cloud` and `settings set` complete. `--agent` offers only claude and codex |
| F28 | P | `wisp mcp`: 11 tools (`wisp_launch`, `wisp_list`, `wisp_get`, `wisp_reply`, `wisp_stop`, `wisp_wait`, `wisp_agents`, …); `wisp_launch` → `wisp_list` end to end |
| F29 | P | `--agent all`: three ✓ lines, project files, and one "·" note for opencode's MCP setup by hand; nothing contradicts |
| F30 | **Partly fixed → B1** | `crew`, `settings`, `plugin`, `cloud`, `machines`, `budget stop-at`: "✗ wisp … has no …" (exit 2, TTY or not). 21 plugin groups still launch on an unknown word |
| F31 | P | A fresh `/` after Settings, or after a task, stays on home at 1280 and 390 |
| F32 | P | Focusing Phone → Discord's token scrolls the content; the title row stays |
| F33 | P | Add and connect keeps Manage open; the new row shows "Can't connect" with the SSH error and "Let me in" |

## Action table

Key: P = pass, B# = fails with that new finding, — = not tested or blocked. "server" means the effect was read back with
`wisp settings list --json`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded. "@100" means the
UI state was read 100–150 ms after the click.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P | Versions from the fakes; opencode "Free models only · Add key" |
| 2 | Codex Sign in (device code) | P | "Sign in on auth.example.dev/codex/device · Code: ABCD-12345" at 100 ms; "Signed in" when the fake finishes (4 s); `codex-home/auth.json` written |
| 3 | Add another Codex account (Work) | P | Dialog with the ownership terms; sign-in progress on the new row; `accounts.list` has `Codex 2` (work) |
| 4 | Account menu: Move up / Move down | P | Rows swap at 150 ms; server order follows. The run's account is the launcher's last pick, else the first in order |
| 5 | Rename an account | P | "QA Work" at 150 ms and on the server |
| 6 | Sign out of an account | P (note) | State `signedOut`, row shows Sign in; no confirm |
| 7 | Remove an account (confirm) | P | "Remove QA Work? … Tasks that ran on it keep their history" |
| 8 | Harness Update button | **B4** | Toast "Claude Code updated to 9.9.10" while it is still 9.9.9 and still offers the update |
| 9 | Model default (launcher pick) | P | Opus kept in `runtime.launch` and across reload; rec `--model opus` |
| 10 | Thinking default | P | Track to X-High → `efforts.claude: xhigh`, "Opus · xhigh" after reload; rec `--effort xhigh` |
| 11 | Lean Claude Code runs select | P | `runtime.leanRuns` on → off → on |
| 12 | Ask before running a changed harness | P | See F8 |
| 13 | Re-check, Installed something new? | P | Rows refresh without moving |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each one flips at 100 ms, its key changes, and switching back restores it (`terminal.terminalSessions`, `runtime.blocks`, `worktrees.worktree`, `ship.fixConflicts`, `settings.readTimes`, `runtime.watch`, `threads.compactAudit`, `runtime.apps`, `crew.tint`, `settings.transcript`) |
| 15 | General: 4 selects (Open files in, Open tasks in, Review, Updates) | P | Change at 150 ms, server key, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` at 100 ms; kept across reload |
| 17 | Notifications: Accept tasks from channels, Default project | P | `channels.acceptChannels`, `channels.project` |
| 18 | Notifications: Desktop notifications | P (note) | Disabled with "Blocked — allow notifications…" (headless Chrome) |
| 19 | Memory: Learn from tasks | P | `learn.enabled` |
| 20 | Automations: Webhook triggers | P | `automations.webhookTriggers` |
| 21 | Usage: 5 switches, 4 selects, Automations stop at | P | `budget.*`, `runtime.limitWait`, `runtime.subagentModel`, `machine.*`; stop line 75 → 90 → Off |
| 22 | Worktrees: 3 selects | P | `worktrees.cleanup`, `.max`, `.onDone` |
| 23 | Privacy: 3 switches | P | Usage stats (telemetry plugin), `worktrees.projectAsk`, `git.maskEnvFiles` |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 13 → 13 after a task and `wisp telemetry flush`, and again after a restart |
| 25 | Updates: Channel; Check now | P | `update.updateChannel`; "A git clone: pull and restart to update", Check now disabled |
| 26 | Keyboard shortcuts pane; N, ⌘K, ⌘P | P | N focuses the prompt; ⌘K and ⌘P lists |
| 27 | Settings search | P, with **B6** | See F11 |
| 28 | All settings | P | Labels read right; `runtime.auto` has no row |
| 29 | Setup: Make the formula | P, with **B6** | YAML with plugins, settings, projects, connectors (env as `${secret:…}`), budget and channels. It also has `router:` |
| 30 | Setup: Import → See what it changes → Apply | P | "Adds 2 · setting settings.editor · project repo3-ui"; toast "Imported"; `settings.editor` = zed |
| 31 | Diagnostics: Copy report | P, with **B9** | 10 KB JSON, "Report copied"; Computers lists "this computer" twice |
| 32 | Sandboxes | — | "This grid doesn't make sandboxes" (dev grid) |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus and empty state | P | F26 |
| 34 | Add a missing folder / a file / a relative name | P | F1, F3 |
| 35 | Add a `file://` URL | P | Added as its folder |
| 36 | Add a duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo" |
| 37 | Real folder check via import / apply | **B2** | `gone` imported; `apply` plans `+ project/ghost` |
| 38 | Remove a project | — | Not re-checked (audit A passed it) |
| 39 | Multi-pick (⌘P projects, launcher) | P | ⌘P lists All projects, repo ⌘1, repo2 ⌘2, Add project…, Manage projects… |
| 40 | CLI task in a project folder | P | F4 |
| 41 | `wisp apply` of a second project for a folder | P | F23 |

### Machines, peers, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Machines → Manage → Add `qa@127.0.0.1` | P | F33 |
| 43 | Edit… (Advanced: remote wisp port, wisp path) | P (note) | Advanced holds the remote wisp port (4477 there) and binary; an SSH port comes from `~/.ssh/config` |
| 44 | Remove (confirm) | P | "Remove 127.0.0.1 from wisp? … Nothing on the machine is touched"; `wisp machines list` drops it |
| 45 | Peers: `wisp machines discover`, `hosts`, `reachable` | P | `(none)`, `(none)`, this Mac's SSH and Tailscale readings |
| 46 | This computer: memory, CPU, disk, what's using it | P | Real readings |
| 47 | Pause automations / Resume now | P | F14 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in: device code, popup to `/activate?code=…`, dev sign-in, Allow | P | `wisp cloud status`: qa-alice |
| 49 | Don't allow, then sign in again | P | F2 |
| 50 | This computer on the grid → on | P | "Online · …--qa-alice.wisp.localhost:4972" at 5 s; that address answers 401 without a sign-in |
| 51 | Grid restart: the tunnel comes back | P | "retrying … The grid didn’t answer", then online within 10 s |
| 52 | API key: create, use, revoke | P | Key `wgk_…` shown once; `curl …/api/health` 200; revoked 401; F15 |
| 53 | Add an account (qa-bob), Use another account | P | F16, F17 |
| 54 | Switch to qa-bob and back | P (note) | This computer goes off for qa-bob (presence is per account) and comes back for qa-alice. The current row's Switch is disabled |
| 55 | Sign out of qa-bob (confirm) | P | "Sign out of qa-bob on wisp.localhost:4972 here?"; gone at 150 ms |
| 56 | Cloud computer woken by an API call | **B8** | It ran this Mac's real Cursor, Grok and Antigravity CLIs |
| 57 | `wisp computer off`, `wisp cloud signout` | P | Status off, "not signed in" |

### Connectors and MCP servers
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector ("QA Echo!" → `qa_echo`) | P | "1 tool"; echo server log has initialize and tools/list |
| 59 | Edit: add env, Save | P | F9 |
| 60 | Paste 2 servers | P | F10 |
| 61 | Connectors reach a run | P | rec `--mcp-config` has `pasted`, `qa_echo`, `remote1`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? … its saved keys are deleted" |
| 63 | Presets (Linear, GitHub, …) and remote OAuth | — | Need a real provider sign-in |
| 64 | `wisp mcp` tools, `wisp_launch` → `wisp_list` | P | F28 |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy: server → on → Test → off | P | Topic picked; catcher got the test POST and the reply poll every 3 s; polling stops when off. "Test sent: it shows up like this…" inline |
| 66 | Telegram, bad token | P (note) | "Not working · Telegram: Unauthorized"; no "Check the token in…" hint, unlike Slack and Discord |
| 67 | Slack, bad tokens | P | "Slack: invalid_auth. Check the tokens in wisp → Settings → Phone → Slack." Link code shown |
| 68 | Discord, bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | P | "Email works on a cloud computer: set it up there." |
| 71 | Switch off and Remove the tokens | P | All `off`, `saved: []`; catcher quiet for 12 s |
| 72 | "push" card | P | F7 |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` shows in both lists | P | F18 |
| 74 | Install from a folder (UI) | P | "QA Pink 1.0.0 is installed, off · It asks for nothing beyond showing itself · Switch it on · Not now" |
| 75 | Switch it on | P | On at 150 ms; styles don't apply while untrusted |
| 76 | Trust / Untrust, partStyles | P, with **B13** | F19; no confirm on Trust |
| 77 | Switch off, Remove | P (note) | Gone; the source folder is kept; no confirm on Remove |
| 78 | Export after Remove | P | F20 |
| 79 | CLI enable/disable reflects live | P | F21 |
| 80 | Built-in off and on (learn) | P | "wisp.learn was switched off", back on; `wisp plugin` active |
| 81 | Cast a plugin | P | Task "A top bar button that says hi" in `home/scratch/<id>` |

### Import from v1, `wisp init`, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | P | Plan with drift marks, "!" warnings for the machine, ntfy and recipe, and "·" notes |
| 83 | `wisp import --yes` | P | "applied 13 changes, 1 task imported" |
| 84 | v1 ntfy | P | F24 |
| 85 | v1 defaults | P | F25 |
| 86 | v1 project at a missing folder | **B2** | `gone` imported |
| 87 | Imported v1 task's project | **B3** | `project: None` with `cwd` the `repo` project's folder |
| 88 | v1 default agent and subagent model | **B11** | The CLI now defaults to codex while New task stays Claude Code; Usage still says "Same as the task" |
| 89 | `wisp init` (repo, again, non-repo) | P | `.wisp/wisp.yml` + AGENTS.md note; a second run says "ready"; a non-repo is refused with `--here` named |
| 90 | `wisp settings list/get/set` (good, bad value, unknown key) | P | "must be one of off, 20, 40, 80"; "no plugin declares the setting nope.key" |
| 91 | `wisp export` yaml/json/toml → validate → diff | P | 32 resources; "0 to add, 0 to change, 0 to remove, 32 unchanged" each |
| 92 | `wisp validate`/`apply` errors (duplicate path) | P | F23 |
| 93 | `wisp completions bash/zsh/fish` | P | `bash -n` and `zsh -n` pass (fish not installed) |
| 94 | What bash completion offers | **B10** | F27 |
| 95 | `wisp skill install --agent all` | P | F29 |
| 96 | `wisp doctor` | — | Not a CLI command (it names `wisp version` and friends; v1 had none). `npm run doctor` not run: it connects to 4800 and 4477 |
| 97 | `wisp <group> <unknown word>`, no TTY and TTY | **B1** | F30 |
| 98 | `wisp "…" --agent frob` | **B10** | Task created and Blocked "no harness frob", not refused |

### Auto disabled everywhere (`runtime.auto` off)
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `runtime.auto` default | P | `wisp settings get runtime.auto` → false |
| 100 | Launcher harness picker | P | Claude Code, Codex, Cursor, opencode, Copilot, Grok, Antigravity, "Set up another agent…"; no Auto |
| 101 | ⌘K: auto, harness, model | P | No Auto entry |
| 102 | Crew editor harness | P | "As New task" and the 7 harnesses; no Auto |
| 103 | All settings | P | No `runtime.auto` row |
| 104 | Settings search "auto" / "model" | **B6** | Lists "Auto"; clicking it shows "No setting matches." |
| 105 | Setup and the formula | **B6** | "Router 1" in What's set up here; `router:` block (best2, escalate, scout…) in every export |
| 106 | CLI | **B6** | `wisp router` (preview, set "Change Settings → Auto", …) in `wisp help` and completion. `--agent auto` falls back correctly (no router pick) |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `cloud status` and `plugin` identical before and after; telemetry still off |
| 108 | WebKit 1280: home and all 20 panes | P (note) | No overflow; WebKit logs "ResizeObserver loop completed with undelivered notifications" |
| 109 | Chrome 390: home and all 20 panes | P | No overflow, no errors |
| 110 | WebKit 390: home and all 20 panes | P (note) | No overflow; ResizeObserver note |
| 111 | Harness update check at start | **B5** | `npm view` × 3 at every start, against the real registry |
| 112 | `wisp-grid dev --help` | **B7** | Starts a grid |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### B1 · M · Unknown words after 21 plugin command groups still launch a Full-auto task (F30, partly fixed)
- **Steps:** in a repo, without a TTY or with one, run `wisp budget frobnicate`, `wisp git frobnicate`, `wisp connectors
  frobnicate`, `wisp channels frobnicate` or `wisp runtime frobnicate`.
- **Expected:** what the core groups do now: `✗ wisp budget has no frobnicate: ask, compare, …` (exit 2).
- **Actual:** "◌ launched: budget frobnicate". The task runs in Full auto in the current folder.
  - In a TTY it attaches and waits.
  - Affected groups: changed, go, accounts, automations, budget, channels, cli, connectors, git, goal, grid-client,
    issues, later, learn, lsp, push, resources, review, router, runtime, stuff, telemetry, termctx.
  - Only a near-miss or a hyphenated word errors (`budget stop-at`).
- **Suspect:** `crates/wisp/src/words.rs` `plugin_slip` (line 503) treats any other word as a prompt. An agent's typo
  after a group name starts work.

### B2 · M · A project at a missing folder still gets in through `wisp import` and `wisp apply` (F1, partly fixed)
- **Steps:**
  1. `wisp import /tmp/qaB2/v1home --yes`. Its v1 project `gone` points at `/tmp/qaB2/v1-gone-folder`, which doesn't exist.
  2. A formula `projects: [{name: ghost, path: /tmp/qaB2/ghost-folder}]` → `wisp validate`, then `wisp apply --dry-run`.
- **Expected:** what the UI and `wisp add` do: refused, or skipped with a note ("No folder at …").
- **Actual:**
  - The plan says `+ project/gone (new)` with no warning, and `wisp export project` then lists `gone`.
  - `validate` says ✓ and `apply` plans `+ project/ghost`.
  - Such a project shows in pickers and Default project like a real one.
- **Suspect:** the project kind's apply path (`plugins/runtime` project resource) and `crates/wisp/src/v1_formula.rs`
  don't share the UI's folder check.

### B3 · L · An imported v1 task has no project (F4, partly fixed)
- **Steps:** `wisp import … --yes` → `wisp ls --all --json`.
- **Actual:** `v1sess0001 "A v1 task" project: None`, with `cwd: /tmp/qaB2/repo`, which is the `repo` project's folder.
  New CLI tasks in that folder now get the project; imported ones don't.
- **Suspect:** the task importer in `crates/wisp/src/v1_formula.rs` never maps a cwd to its project.

### B4 · M · A harness "Update" reports success when nothing changed
- **Steps:** Settings → Agents, with `npm view` answering 9.9.10 for Claude Code (installed 9.9.9) → Update to 9.9.10.
- **Actual:**
  - wisp ran `claude update` (rec: argv `["update"]`). It exited 0 and changed nothing.
  - The toast says "Claude Code updated to 9.9.10".
  - The re-detected row still reads 9.9.9 and still offers "Update to 9.9.10".
- **Expected:** compare the re-detected version with the target. When it didn't move, say so ("still 9.9.9: run …").
- **Suspect:**
  - `plugins/accounts/host/src/upgrade.rs` `upgrade()` treats exit 0 as done.
  - `plugins/accounts/ui/src/panes/Agents.tsx:169` builds the toast from `h.latest`, not the new version.

### B5 · M · Every `wisp serve` start asks the real npm registry for harness versions, with no switch to stop it
- **What happens:** at start, and every 12 hours, `plugins/accounts/host/src/upgrade.rs` `follow()` runs
  `npm view @anthropic-ai/claude-code version`, `npm view @openai/codex version` and `npm view opencode-ai version`.
- **Effect:** this scratch QA server reached registry.npmjs.org on its first start.
  - None of these turns it off: `WISP_UPDATE_CHECK=0`, `CI`, "Checks are off on this install" on Updates, or Privacy.
  - Only the undocumented `WISP_NPM` env redirects it.
- **Expected:** honour the same switches as wisp's own update check. At least skip it when update checks are off. Name it
  under Privacy, since it reveals which harnesses are installed.
- **Evidence:** `/tmp/qaB2/npm.log` (the fake `npm`, used after the first start).

### B6 · M · Auto still shows outside its switch: Settings search, Setup, every formula and the CLI
- **Settings search:** with `runtime.auto` false, typing `auto` or `model` lists "Auto"
  (`data-pane="setting:runtime.auto"`). Clicking it opens All settings with "No setting matches.", because All
  settings hides that row (`plugins/settings/ui/src/panes/All.tsx:59`) while the search index doesn't.
  Shot `55-search-auto.png`.
- **Setup and formulas:**
  - Settings → Setup lists "Router 1" under What's set up here.
  - Make the formula and `wisp export` always include a `router:` block (best2, escalate, explore, goal, habits, scout),
    so every shared setup carries Auto's config.
- **CLI:** `wisp router` (preview, record, reset, set "Change Settings → Auto", signal, view) is in `wisp help` and
  completion.
- **Expected:** while Auto is put away (owner, 2026-11-17), it appears nowhere a user looks. The checked places pass:
  picker, ⌘K, crew editor and All settings.

### B7 · L · `wisp-grid dev --help` starts a dev grid instead of printing help
- **Steps:** `wisp-grid dev --help`.
- **Actual:**
  - A grid starts on `:8080` with its data in `~/.wisp-grid-dev`, which it creates.
  - It runs until Ctrl-C, with no usage text.
  - With the default port and the real home, it can collide with a developer's own dev grid.
- **Suspect:** `apps/grid/src/cli.rs:75` `dev()` reads only the flags it knows.

### B8 · M · `wisp-grid dev` boxes run the developer's real harness CLIs
- **Steps:** `wisp-grid dev` with fake `WISP_CLAUDE_BIN`/`WISP_CODEX_BIN` → sign in → wake the cloud computer (an API
  call).
- **Actual:** the cloud computer's Agents list shows Cursor `2026.11.13-…`, Grok `1.0.70` and Antigravity `1.5.1`. These
  are this Mac's real CLIs.
  - The box inherits the grid's `PATH`.
  - The grid forwards only `WISP_CLAUDE_BIN`, `WISP_CODEX_BIN` and `WISP_WEB_DIR`.
- **Effect:** the box ran their version and sign-in probes (with the box's own HOME, so no credentials). A task sent to
  one would run the real CLI. A dev or CI grid can't be fully faked this way.
- **Suspect:** `apps/grid/src/cli.rs:86`, the `pass` list. Forward every `WISP_*_BIN`, or document putting the fakes
  first on `PATH`.

### B9 · L · Diagnostics names the cloud computer "this computer"
- **Steps:** signed in, with the cloud computer awake → Settings → Diagnostics.
- **Actual:** Computers lists "this computer · 2.0.0 · 51 plugins · online" and a second "this computer · 2.0.0 · 48
  plugins · online".
- **Expected:** the second is the cloud computer, under its own name.
- **Shot:** `106-diag.png`
- **Suspect:** `packages/shell/src/devtools/Inspector.tsx:20` (`peers.name`).

### B10 · L · Harness names in the CLI: completion offers two, and an unknown one is taken (F27, partly fixed)
- **Completion:**
  - `wisp --agent <TAB>` offers only `claude codex`, while help lists 7 harnesses.
  - `wisp settings set worktrees.max <TAB>` offers `--key --value --json --help`, not `off 20 40 80`.
- **Unknown harness:** `wisp "x" --agent frob` creates a task that ends Blocked "no harness frob" (it's in the list).
  The CLI could refuse before launching, as it does for an unknown verb.

### B11 · L · After a v1 import, the CLI and New task disagree on the default harness
- **Steps:** import a v1 home whose `defaults.agent` is `codex` and `subagentModel` is `haiku` → `wisp "no agent named"`;
  then New task in the app.
- **Actual:**
  - The CLI task runs on Codex (from `agents.default.harness: codex`).
  - New task still picks Claude Code (from `runtime.launch`).
  - `subagentModel: haiku` is stored only on `agents.default`, while Usage → Subagents run on still says "Same as the task".
- **Expected:** one default harness and subagent model, shown where the user sets them.

### B12 · L · Telegram's error has no "Check the token" hint (audit A note, still open)
- **Actual:** "Telegram: Unauthorized". Slack and Discord say "… Check the token(s) in wisp → Settings → Phone → …".
  With a bad token it keeps polling every ~5 s.

### B13 · L · Trust and Remove a plugin take one click, with no confirm
- **Steps:** Settings → Plugins → a user plugin → Trust; then Remove.
- **Actual:**
  - Trust applies at once. The row then says "Trusted: it runs as its own process with your environment".
  - Remove takes it out with no confirm and no Undo.
  - Connectors, machines, accounts and API keys all confirm or offer Undo.
- **Expected:** a confirm for Trust that names what the plugin gets, and a confirm or Undo for Remove.

---

## Smaller notes (not counted)
- **Agents:**
  - Claude Code's row shows no "Signed in · Sign out" while Codex, Grok and Antigravity do. It may be the fake: it
    answers `auth status --text` but not `--json`.
  - Signing out of a harness account has no confirm.
- **Agents (my script's fault):** a misdirected click pressed Antigravity's Sign out. The toast "agy is still signed in:
  run agy in a terminal and type /logout" is correct for the fake.
- **Cloud:** after an API call woke the cloud computer, the Cloud pane kept "Asleep" until it was reopened.
- **Channels:** `wisp channels status` still lists a `push` entry, which is hidden in the UI.
- **Connectors:** `addedAt` is `0` for every connector.
- **All settings:** some descriptions are developer notes ("{ agent, models: … } (v1 defaults.agent / models /
  efforts)") and raw JSON fields. `settings.editor` reads "Vscode, cursor, windsurf, …".
- **Restoring defaults:** putting a Usage or Worktrees select back stores the default explicitly (`machine.cpuCeiling
  "200"`, `worktrees.cleanup "7"`) where it was unset. The value is the same.
- **WebKit:** it logs "ResizeObserver loop completed with undelivered notifications" on panes.

## Cleanup done
- **Restored:** every setting changed here is back to its value or its default:
  - `budget.limitHold` 90, `git.maskEnvFiles` true, `runtime.leanRuns` on, `runtime.retentionDays` 0,
    `settings.editor` vscode, `settings.notify` off, `worktrees.max` 40, `runtime.launch` {}.
  - Theme System, telemetry on, ntfy off, stop line Off.
  - This computer off and the grid signed out.
- **Stopped:** `wisp serve` :4951, `wisp-grid dev` :4972, the catcher :4949 and the headless Chrome :9343.
- **Deleted:** `/tmp/wisp-target-qaB2` (8.6 GB) and the stray `~/.wisp-grid-dev`.
- **Kept for re-checks:** `/tmp/qaB2` (binaries in `fake/`, home, repos, grid data, shots). The scratch home still holds
  the imported v1 machine, connector, recipe and task, and about 30 fake tasks from the CLI checks.
