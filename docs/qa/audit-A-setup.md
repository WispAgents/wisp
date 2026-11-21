# Audit A: agent and workspace setup (round A, thorough)

2026-11-18 · worktree `wisp-8fb64ec939b0` at `2013facf` (plus other sessions' uncommitted UI edits) · finding only, no repo files edited.

**Setup.**
- Built `target/debug/wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`) and `wisp-grid`, and copied them to `/tmp/qaB/fake`.
- Built the web UI with `vite build --outDir /tmp/qaB/web`.
- `wisp serve --port 4967` ran with `WISP_HOME=/tmp/qaB/home` and `HOME=/tmp/qaB/realhome`. Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, and `FAKE_RECORD` was on after the first restart.
- PostHog, Slack, Telegram, Discord, Linear and the ntfy server all pointed at a local catcher on `:4939`. It answered Telegram and Discord with 401 and Slack with `invalid_auth`.
- Cloud used `wisp-grid dev --port 4971` (`wisp.localhost`).
- Test repos: `/tmp/qaB/repo`, `/tmp/qaB/repo2` and `/tmp/qaB/repo3`.
- UI driven with Playwright: system Chrome headless over CDP, and WebKit 2388. Widths 1280 and 390 (phone).
- Screenshots: `/tmp/qa-keep/shots-A2/`. All servers and the headless Chrome are stopped.

> **One outbound request to the real hosted grid (finding 2).** After a declined sign-in, the address box went back to `grid.trywisp.dev` by itself. My next "Sign in" opened `https://grid.trywisp.dev/signin?return=/activate?code=RSGR-XQNT`, so a device code was requested on the real grid. I closed the popup and pressed Cancel. Nothing was approved and no credential was used. The code expires on its own.

## Counts

| | |
|---|---|
| Actions checked | 104 |
| Pass | 68 |
| Fail (a finding) | 30 |
| Not tested or blocked (credential, dev-grid limit, or fix-r331’s area) | 6 |
| Findings | 33: 8 high, 14 medium, 11 low |

## Action table

Notes on the table:
- P = pass, F# = fail with its finding number, — = not tested or blocked.
- "server" means the effect was read back with `wisp settings list --json`, `wisp export …` or a `wisp <plugin> <verb>` read.
- "rec" means the argv the fake CLI recorded through `FAKE_RECORD`.
- "100ms" means the UI state was read within 100–150 ms of the click.

### First run and projects
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | First run on an empty home | P | Launcher, empty list and a Grid tip. There is no onboarding step; v1 had none to compare. `01-first-run` |
| 2 | Add project: Other folder… → path → Add | P | Typed-path suggestions, a row appears, the launcher chip picks it. `12c-proj-added` |
| 3 | Add a folder that doesn't exist | F1 | Accepted as a project. `13-proj-nonexist` |
| 4 | Add a file path | F1 | Accepted as project "math.js". `13-proj-file` |
| 5 | Add a relative name or a `file://` URL | F3 | Toast shows internal wording |
| 6 | Add a duplicate, same spelling and `/tmp` vs `/private/tmp` | P | Toast "That folder is already repo2"; one row. `14-proj-dup-spelling` |
| 7 | Projects dialog opens | F26 | Input not focused; "pick one below" with nothing below. `11b` |
| 8 | Change colour | P | Menu of 9; `export project` has `color: amber`. `15-proj-colour-menu` |
| 9 | Rename (inline name) | P | Export shows the new name |
| 10 | Remove (trash) | P | Row gone at 150 ms; no confirm and no undo (v1 parity not checked). `15c/15d` |
| 11 | Reorder by drag | P | `runtime.projectOrder` changed; launcher and ⌘P lists follow it. `16-proj-drag`, `17*` |
| 12 | Projects show in pickers (launcher place, ⌘P, Notifications default project) | P | All three lists |
| 13 | CLI `wisp add` (here, missing folder, file) | P | Adds; refuses missing folders and files with a clear message |
| 14 | Task launched from the CLI inside a project folder | F4 | `project: null`, no worktree (rec: cwd is the repo itself) |
| 15 | Task launched from the UI with a project picked | P | Worktree `home/worktrees/repo-…`, branch `wisp/task-…` (rec) |
| 16 | No folder / Cast runs in a scratch folder | P | cwd `home/scratch/<id>` |

### Settings: General, Notifications, Keyboard, Theme
| # | Action | Result | How checked |
|---|---|---|---|
| 17 | The 10 General switches (Babysitting, blocks, worktree, auto-fix conflicts, read times, watch, check compactions, interactive answers, crew tint, full transcript) | P | Each one: UI flips within 80 ms, the server key changes, and switching back restores it |
| 18 | Open files in (editor) | P | `settings.editor` vscode→cursor→vscode |
| 19 | Open tasks in | F5 | No pick sticks; no error. `20-opentasks-*` |
| 20 | Review by another harness | P | `review.harness` off→other→off |
| 21 | Update channel (General and Updates) | P | `update.updateChannel` |
| 22 | Theme System/Light/Dark, then reload | P | `data-theme` at 100 ms; `wisp.theme` kept across reloads. `21-theme-*` |
| 23 | Account menu: theme toggle | F6 | Label wrong after picking Light. `22b` |
| 24 | A CLI `settings set` shows in the open UI | P | Default project select updates within 1.5 s |
| 25 | Desktop notifications switch | P | Blocked browser: "Blocked — allow notifications…". `24-desktop-notif-click` |
| 26 | Accept tasks from channels | P | `channels.acceptChannels` |
| 27 | Default project select | P | `channels.project` (it also lists the bad projects from F1) |
| 28 | Shortcuts N, [, ⌘K (+ typing), ⌘P | P | Focus, list hidden and shown, palette, project switcher |
| 29 | Settings search | F11 | Matches pane titles only; no empty state. `94-search-theme` |
| 30 | All settings pane: labels and descriptions | F12 | `06-allset-*` |

### Phone channels
| # | Action | Result | How checked |
|---|---|---|---|
| 31 | ntfy: server, switch on, Test, switch off | P | Catcher got the test POST and polls every 3 s; polling stops when switched off. `26*` |
| 32 | Telegram: bad token | P | "Not working · Telegram: Unauthorized"; it keeps polling with a 5 s backoff (no guidance text, unlike Slack and Discord) |
| 33 | Slack: bad tokens | P | "Slack: invalid_auth. Check the tokens in …" |
| 34 | Discord: bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 35 | GitHub (fake gh) | P | Error path shown: "gh api user gave no id" |
| 36 | Email | P | Disabled, with "works on a cloud computer" |
| 37 | "push" channel card | F7 | `29c-push-on` |
| 38 | Switch a channel on with no token | P (note) | The switch stays on with "Not set up" and no hint to paste a token first |
| 39 | Remove saved tokens, then off | P | Fields cleared, state Off |

### Agents (fix-r331 owns harness rows and accounts: light pass only)
| # | Action | Result | How checked |
|---|---|---|---|
| 40 | Lean Claude Code runs select | P | `runtime.leanRuns` |
| 41 | Ask before running a changed harness | F8 | Takes effect, but the switch didn't flip within 100 ms |
| 42 | Harness rows, sign-in, accounts, keys | — | fix-r331's area |

### Connectors
| # | Action | Result | How checked |
|---|---|---|---|
| 43 | Add a local connector (a test MCP server I wrote) | P | "1 tool"; the server log shows initialize and tools/list. `31c` |
| 44 | Name normalised ("QA Echo!" → `qa_echo`) | P | |
| 45 | Edit: add an environment variable, Save | F9 | `34b`, `36b` |
| 46 | Check again | P | MCP server restarted |
| 47 | Remove (with confirm) | P | `37-conn-remove` |
| 48 | Paste a config with 2 servers | F10 | `37d/37e` |
| 49 | Connectors reach a run | P | rec: `--mcp-config` has `pasted`, `qa_echo`, `wisp`; `--allowedTools mcp__…` |
| 50 | Presets (Linear, Jira, GitHub, …) and remote OAuth sign-in | — | Needs a real provider sign-in |

### Memory, Automations, Usage/budget, This computer, Worktrees
| # | Action | Result | How checked |
|---|---|---|---|
| 51 | Learn from tasks switch | P | `learn.enabled` |
| 52 | Webhook triggers switch | P | `automations.webhookTriggers` |
| 53 | Recipes: v1 recipe listed, Run now / Remove | P (note) | No Edit on the manage page |
| 54 | Triggers: Manage | — | Not exercised (no trigger source) |
| 55 | Usage switches: live limits, spare, window changes, drain, wait for reset | P | Keys flip and revert |
| 56 | Ask near the limit, Stop a silent run | P | `budget.limitHold`, `budget.silentStop` |
| 57 | Automations stop at (75%), then restart | P | Kept across a restart. `97b` |
| 58 | The stop line in `wisp export` | F13 | `export budget` is empty |
| 59 | This computer: Limits → Change | P | Goes to Usage, filtered to machine |
| 60 | Pause automations → For 1 hour, then Resume now | F14 | `89b`, `89c` |
| 61 | Worktrees: clean up after, keep at most, when done | P | `worktrees.cleanup`, `.max`, `.onDone` |
| 62 | Disk card | P | Showed "—" at first load, then "58 GB free" |

### Machines, Cloud, Sandboxes
| # | Action | Result | How checked |
|---|---|---|---|
| 63 | Add over SSH (`qa@127.0.0.1`) | P (note) | Dialog closes at once; the error and "Let me in" only show on reopening Manage. Advanced's port field didn't take. `51b`, `52` |
| 64 | Edit…, Remove (with confirm) | P | `wisp machines list` empty after |
| 65 | This computer over SSH (sshd, Tailscale, keys) | P | Real readings of this Mac |
| 66 | Cloud: sign in to the dev grid (device code, Allow) | P | `wisp cloud status` shows qa-alice. `63a`, `65` |
| 67 | Cloud: Don't allow | P | "Sign-in was declined in the browser" |
| 68 | Cloud: sign in again after a decline | F2 | Address reset to the hosted grid. `63` |
| 69 | This computer on the grid → on | P | Online; the tunnel serves this wisp (`67-tunnel`); 401 without a sign-in |
| 70 | API key create, Revoke (confirm) | P | Key shown once; the revoked key gets 401 |
| 71 | After revoke | F15 | The revoked key's "ready, copy it" panel stays |
| 72 | Add an account (qa-bob) | F16 | `69d` |
| 73 | Grid page "Use another account" during a device sign-in | F17 | Code lost. `69c` |
| 74 | Switch account | P | Current moves; this computer comes back online for alice |
| 75 | Sign out of an account (confirm) | P | |
| 76 | Wake a cloud computer, shares, publish | — | Not exercised (dev-grid boxes; nothing to share) |
| 77 | Sandboxes pane | — (blocked) | "This grid doesn't make sandboxes" on a dev grid. `72` |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 78 | `wisp plugin new qa-theme` | F18 | Not listed until `enable` by id |
| 79 | Install from a folder (UI) | P | "installed, not switched on yet" |
| 80 | Install errors (no manifest, missing folder, bad git URL, built-in id) | P (note) | Toasts; a missing folder says "has no wisp-plugin.json" |
| 81 | Enable (UI switch) | P | On |
| 82 | Trust / untrust | F19 | No control in the UI; the CLI works and styles apply live (`--accent` #ff1493) |
| 83 | Remove (UI) | P | Gone from the list; the source folder is kept; no confirm |
| 84 | Removed plugin in `wisp export` | F20 | Still `enabled: true` |
| 85 | Plugins list after a CLI enable | F21 | Stays "No plugins of your own yet" until something else refreshes it |
| 86 | Cast a plugin | P | Task in a scratch folder with the right brief. `47e` |

### Privacy, Updates, Setup, Diagnostics
| # | Action | Result | How checked |
|---|---|---|---|
| 87 | Anonymous usage stats off | P | No PostHog `/batch/` after a task and a flush (catcher count 5 → 5); still off after a restart |
| 88 | Ask before a repo's hooks; Hide .env values | — | Not exercised here |
| 89 | Updates: Check now | P (note) | Disabled on a debug build ("a git clone"); CLI `wisp update` checks anyway |
| 90 | Setup: Make the formula | P | YAML shown (it includes the removed plugin, F20) |
| 91 | Setup: Import → See what it changes → Apply | P | "Adds 2"; `settings.editor` = zed, project repo3-ui; toast "Imported" |
| 92 | Diagnostics: Copy report | P | 11.5 KB JSON; no paths or tokens in it |

### CLI setup, formulas, v1 import
| # | Action | Result | How checked |
|---|---|---|---|
| 93 | `wisp export` yaml, json and toml → validate → diff | P | 21 resources; "0 to add … 21 unchanged" each |
| 94 | `wisp validate`/`apply` with `worktrees.max: 20`; `wisp settings set worktrees.max 80` | F22 | |
| 95 | `wisp apply` of a project at an existing project's folder | F23 | Duplicate project |
| 96 | `wisp import <v1 home> --dry-run` / `--yes` | P | Plan with notes; "applied 8 changes, 1 task imported" |
| 97 | v1 ntfy settings | F24 | |
| 98 | v1 defaults (maskEnvFiles, leanRuns, editor, notify, retentionDays, …) | F25 | |
| 99 | `wisp init` (repo, again, non-repo) | P | `.wisp/` and the AGENTS.md note; a second run is quiet; a non-repo is refused with `--here` named |
| 100 | `wisp completions zsh/bash/fish` | P | Valid syntax (`zsh -n`, `bash -n`) |
| 101 | What bash completion offers | F27 | |
| 102 | `wisp mcp` | F28 | 4 tools |
| 103 | `wisp skill install --agent all` / `--project` | P (F29) | Files written; the output contradicts itself |
| 104 | `wisp <group> <unknown word>` from a script or agent (no TTY) | F30 | Launches tasks |

### Cross-cutting
- **Restart:** export, settings, cloud sign-in, this-computer-on, plugins, telemetry and the stop line all survived two restarts. **P**
- **WebKit 1280** (home, settings, Phone, Cloud, Plugins, Connectors, place menu): no page errors, switch flips. **P**
- **Phone 390:** settings list, panes, Cloud and Phone readable, no horizontal overflow, Chrome and WebKit. **P**, with notes F31 and F33.

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service; **M** = a feature misbehaves or loses something; **L** = polish or wording.

### F1 · H · A missing folder or a file can be added as a project from the UI
- **Steps:** Launcher → place chip → Other folder… → type `/tmp/qaB/nope-not-here` (or `/tmp/qaB/repo/math.js`) → Add.
- **Expected:** refused, as the CLI does: `✗ no folder /tmp/qaB/also-missing: wisp add <folder>`.
- **Actual:** both become projects ("nope-not-here", "math.js"). The launcher auto-picks them, and they show in ⌘P, in the Notifications default project and in `wisp export`. `wisp import` does the same for a v1 project whose folder is gone ("gone").
- **Shots:** `13-proj-nonexist.png`, `13-proj-file.png`, `14-proj-dup-spelling.png`

### F2 · H · A declined grid sign-in silently swaps the address back to the hosted grid
- **Steps:**
  1. Settings → Cloud → set the address to `http://wisp.localhost:4971` → Sign in.
  2. On the grid page press "Don't allow".
  3. Press Sign in again.
- **Expected:** the address stays on your grid.
- **Actual:** after "Sign-in was declined in the browser" the field reads `grid.trywisp.dev` again. The retry opened `https://grid.trywisp.dev/signin?…code=…` and asked the real grid for a device code. A self-hosted grid user is quietly sent to the hosted one. "Add an account" also defaults to `grid.trywisp.dev` while the current account is on another grid.
- **Shots:** `63-cloud-signed-in.png` (declined, hosted address back), `64-cloud-pending-real.png`

### F3 · L · Project add errors leak internal names
- **Steps:** Projects → type `repo2` or `file:///tmp/qaB/repo2` → Add.
- **Actual:** toast "path: repo2 isn't a full path to a folder (clone a repo URL first: git.clone)". `git.clone` is a command id. Relative names aren't resolved (home, or the server's cwd).
- **Expected:** plain words, no command ids.
- **Shot:** `13-proj-relative.png`

### F4 · H · Tasks started from the CLI in a project folder get no project and no worktree
- **Steps:** `cd /tmp/qaB/repo2` (a project added with `wisp add`) → `wisp "cli task in repo2" --wait`.
- **Expected:** project = repo2, and a worktree because Worktree per task is on. The help offers `--no-worktree`, which implies a worktree by default.
- **Actual:** `project: null`, `branch: null`, and the agent runs in the repo itself (rec cwd `/private/tmp/qaB/repo2`). The same prompt from the UI gets `project: "repo"` and a `wisp/task-…` worktree. The CLI sends only `cwd`, and nothing maps a cwd to its project. Imported v1 tasks are also left with no project.

### F5 · H · "Open tasks in" can't be changed
- **Steps:** Settings → General → Open tasks in → pick iTerm, Terminal.app, Auto-detect or Custom command.
- **Expected:** the pick sticks and Take over uses that app.
- **Actual:** the select stays "In wisp", with no toast and no error.
- **Cause:** the pane writes the setting `terminal`, which no manifest declares any more (`wisp settings set terminal iterm` → "no plugin declares the setting terminal"). `plugins/terminal/host/src/lib.rs:109` still reads `ctx.setting("terminal")`. The manifest declares only `terminal.command` and `terminal.terminalSessions`. So the terminal app can't be chosen at all.
- **Shots:** `20-opentasks-iTerm.png`

### F6 · M · The account menu's theme item shows the wrong label
- **Steps:** Settings → General → Theme Light (or pick the item in the menu until the theme is light) → open the account menu.
- **Expected:** "Dark theme" while the theme is light.
- **Actual:** "Light theme" while light. It flips the theme correctly; only the label is wrong. With System it says "Dark theme".
- **Shot:** `22b-foot-menu-light-stale.png`

### F7 · M · The "push" channel is an unusable stub
- **Steps:** Settings → Phone → scroll to the last card.
- **Actual:**
  - The card is named with the raw id "push".
  - It has no description and an empty "Set it up".
  - Switching it on gives "transport: Settings → Phone sets up ntfy, telegram, slack, discord, github, email" and it stays Off.
- **Expected:** hidden, or a real setup flow.
- **Shot:** `29c-push-on.png`

### F8 · L · "Ask before running a changed harness" has no immediate feedback (fix-r331's pane)
- **Steps:** Settings → Agents → click the switch.
- **Actual:** the switch hasn't flipped 100 ms after the click (ADR-0017 wants a change within one frame). It does take effect. My revert click landed during the delay and switched it back on.
- **Shot:** `87-ask-changed-harness.png`

### F9 · H · Editing a connector drops the environment variables you add
- **Steps:** Settings → Connectors → ⋯ → Edit → Environment → Add → type `QA_VAR` = `hello` (also tried with real key presses) → Save.
- **Expected:** saved; the MCP server starts with the variable.
- **Actual:** the dialog closes; `connectors list --json` shows `env: []`; Check again starts the server with `QA_VAR=undefined`. The server path works: `wisp connectors update --args '{…"env":[{"key":"QA_VAR","value":"via-update"}]}'` saves it, and the server then gets the value. A connector added with env through the UI paste path also keeps it. So the bug is in the Edit dialog's save.
- **Shots:** `34-conn-edit.png`, `36b-conn-edit-saved.png`

### F10 · M · Pasting a config with several servers adds only the first, silently
- **Steps:** Add connector → paste `{"mcpServers":{"pasted":{…},"remote1":{"url":"…"}}}` → Add connector.
- **Expected:** both are added, or the dialog says only one is taken.
- **Actual:** only `pasted` is added; `remote1` disappears without a word.
- **Shots:** `37d-conn-paste.png`, `37e-conn-paste-added.png`

### F11 · M · Settings search only finds pane names
- **Steps:** Settings → Search settings → type `theme`, `ntfy`, `telemetry` or `xyzzy`.
- **Expected:** finds General → Theme, Phone → ntfy, Privacy → usage stats; shows "No results" when nothing matches.
- **Actual:** the nav goes empty for all four (only "worktree" matched, by pane name) and the right side still shows the previous pane. There is no empty state.
- **Shot:** `94-search-theme.png`

### F12 · M · All settings shows broken labels and one wrong description
- **Splits a sentence:** labels split at the first ":" of the declaration title. `git.cloneDir` reads "Where a pasted repo URL is cloned (''" over "The first of ~/code…)". `runtime.subagentModel` reads "…(task" / "The task's own)". `worktrees.cleanup` and `channels.project` do the same.
- **Key shown as label:** `settings.timeZone` has no title.
- **Whole sentence as label:** "Lean Claude Code runs (…):" and "Hide .env values: what a task shows…" are truncated with "…".
- **Wrong meaning:** `settings.readTimes` is described as "Show how long ago each task changed in the list". The General pane uses the same key for "Read times in prompts" (scheduling "in 5 min …").
- **Shots:** `06-allset-git.cloneDir.png`, `06-allset-settings.timeZone.png`, `06-allset-runtime.launch.png`

### F13 · M · "Automations stop at" isn't part of `wisp export`
- **Steps:** Usage → Automations stop at 75% → `wisp export budget`.
- **Actual:** `wisp: 1` and nothing else, although the line persists across restarts. Setup → Make the formula has no budget either, so a shared or kept setup loses it.

### F14 · M · Pause and Resume automations don't update the pane
- **Steps:** Settings → This computer → Pause automations → For 1 hour.
- **Actual:**
  - It takes effect: `wisp machine load` reports "Automations paused until 22:28".
  - The pane keeps "Running as usual · Pause automations" (checked at 1.6 s and 5 s).
  - After a reload it shows "paused · Resume now". Resume now works on the server, but the pane keeps saying "paused" until the next reload.
- **Expected:** the state changes at once (ADR-0017).
- **Shots:** `89b-paused.png`, `89c-paused-after-reload.png`

### F15 · L · A revoked API key is still shown as ready to copy
- **Steps:** Cloud → API keys → create `qa-script` → Revoke → confirm.
- **Actual:** "No keys yet" shows under a panel that still says "qa-script is ready · Copy it now" with the full key and the curl line. The key correctly gets 401.
- **Shots:** `68c-apikey-revoke.png`, `68d-apikey-after-revoke.png`

### F16 · M · Adding a second grid account switches to it and takes this computer off the grid
- **Steps:** signed in as qa-alice with This computer on the grid on → Add an account → sign in as qa-bob.
- **Actual:**
  - qa-bob becomes Current by itself.
  - "This computer on the grid" turns Off: `wisp computer status` says off, and alice's address answers 401.
  - The phone or browser that used alice's address loses this computer, and nothing says so.
  - Switching back to alice turns it on again.
- **Expected:** keep the current account and the computer's presence, or ask.
- **Shot:** `69d-two-accounts.png`

### F17 · L · Grid: "Not you? Use another account" loses the device code
- **Steps:** a device sign-in page (`/activate?code=…`) while logged in on the grid → "Use another account".
- **Actual:** signs out and lands on the grid's home. The code is gone, so the sign-in in wisp can't finish and must be started again.
- **Expected:** come back to `/activate?code=…` after signing in.
- **Shot:** `69c-use-another-account.png`

### F18 · L · `wisp plugin new` makes a plugin no list shows
- **Steps:** `wisp plugin new qa-theme` → `wisp plugin` and Settings → Plugins.
- **Actual:** neither lists it ("No plugins of your own yet") until `wisp plugin enable user.qa-theme`. A plugin you're writing can't be found from the UI.

### F19 · M · No Trust or Untrust in Settings → Plugins
- **Steps:** install `/tmp/qaB/myplugin` (a theme with partStyles) → switch it on.
- **Actual:** it is on, but its styles don't apply (correct while untrusted), and the row has only the switch and Remove. There is no Trust control and no hint why nothing changed. `wisp plugin trust user.qa-pink` applies it live (send button turns pink), and untrust reverts it. The page says "It comes in off: you see what it asks for first", but the row never shows what it asks for.
- **Shots:** `43-plugin-enabled.png`, `44-plugin-trusted-home.png`

### F20 · M · A removed plugin stays in `wisp export`, enabled
- **Steps:** install user.qa-pink → enable → Remove (UI) → `wisp export plugin`.
- **Actual:** `- name: user.qa-pink · enabled: true`. Setup → Make the formula includes it too, so a shared formula names a plugin that isn't installed.

### F21 · L · Settings → Plugins doesn't follow CLI changes
- **Steps:** Settings → Plugins open → `wisp plugin enable user.qa-theme`.
- **Actual:** still "No plugins of your own yet" until an install in the pane refreshed it.

### F22 · H · Enum settings with number-like values can't be set from the CLI or a formula
- **CLI:**
  - `wisp settings set worktrees.max 80` → "✗ worktrees.max must be one of off, 20, 40, 80".
  - `wisp settings set budget.limitHold 80` → same.
- **Formula:**
  - `settings: { worktrees.max: 20 }` → "✗ settings.worktrees.max: worktrees.max is enum (off, 20, 40, 80), not 20".
  - Only `"20"` (quoted) passes.
- **Import:** a v1 config holding the number fails the whole `wisp import --yes` (exit 1). v1 stored strings, so real homes are probably safe.
- **Expected:** `80` is accepted as the enum value "80".

### F23 · M · `wisp apply` (and v1 import) duplicates a project that already exists for that folder
- **Steps:** project `second-repo` exists at `/private/tmp/qaB/repo2` → apply a formula with `projects: [{name: repo2-formula, path: /tmp/qaB/repo2}]`. Same with `wisp import` of a v1 project at `/tmp/qaB/repo` (already "repo").
- **Actual:**
  - The plan says "+ project/repo2-formula (new)", and apply creates a second project for the same folder.
  - The UI and `wisp add` refuse that ("That folder is already repo2").
  - Tasks in that folder then match neither project (see F4).

### F24 · M · `wisp import` moves the v1 ntfy topic into `allow` and drops its server
- **Steps:** v1 `config.json` with `remote.ntfy = {enabled, topic: "wisp-old-topic-…", server}` → `wisp import --yes` → `wisp export channel`.
- **Actual:** `ntfy: allow: [wisp-old-topic-…], enabled: true, routing.topic: <the v2 topic>`. The phone subscribed to the v1 topic stops hearing anything, and the import turned ntfy on with a topic the user never subscribed to. (`crates/wisp/src/v1_formula.rs:132` maps `chatId|user|topic` → `allow`; v1 `server/channels/ntfy.js` uses `topic` as the publish topic.)

### F25 · M · `wisp import` drops most v1 defaults without a note
- **Mapped:** only six (`timeZone`, `review`, `panes`, `worktreeCleanup`, `worktreeMax`, `worktreeOnDone`; `v1_formula.rs:194`), plus model, effort, mode and subagentModel for the default agent.
- **Dropped, from my v1 home:** `maskEnvFiles: "off"`, `leanRuns: "all"`, `editor`, `notify`, `limitHold`, `retentionDays`. They are not in the plan or its notes.
- **Effect:** a v1 user who turned .env masking off, or set Lean to all, gets v2's defaults silently. `maskEnvFiles` and `leanRuns` are certain v1 keys (`server/env-mask.js` `maskOn`, `server/lean.js`).

### F26 · L · Projects dialog: no focus, and "pick one below" with nothing below
- **Steps:** place chip → Other folder….
- **Actual:**
  - The path field isn't focused (typing goes nowhere).
  - The empty state says "Add a folder above, or pick one below", but nothing is offered below until you type.
- **Shot:** `11b-other-folder-typed.png`

### F27 · M · Bash completion offers the wrong things
- **Steps:** `source <(wisp completions bash)`.
- **Results:**

  | Typed | Offered |
  |---|---|
  | `wisp pl<TAB>` | `plugin plugins plugins` (duplicate) |
  | `wisp plugin <TAB>` | nothing (expected enable/disable/trust/untrust/install/remove/new) |
  | `wisp cloud <TAB>` | nothing |
  | `wisp --agent <TAB>` | nothing |
  | `wisp settings set <TAB>` | `--key --value --json --help` (no setting keys) |

- zsh and fish were only syntax-checked.

### F28 · H · `wisp mcp` lost v1's task tools
- **Steps:** `wisp mcp` → initialize → tools/list.
- **Actual:** 4 tools: `wisp_history`, `wisp_blame`, `wisp_task`, `wisp_resume`.
- **v1:** `server/mcp-wisp.js` gave editors and other agents `wisp_launch`, `wisp_list`, `wisp_get`, `wisp_reply`, `wisp_stop`, `wisp_wait` and `wisp_agents`.
- **Effect:** an editor or harness using `wisp mcp` can no longer start, list, follow up or stop wisp tasks (ADR-0040: meet users where they work). `wisp skill install --project` writes this server into `.mcp.json`.

### F29 · L · `wisp skill install --agent all` contradicts itself
- **Actual:** prints ✓ for claude, codex and opencode, then "✗ wisp skill install --agent claude|codex (not all yet)".
- **Expected:** one clear outcome.

### F30 · H · Outside a terminal, a mistyped verb launches a real Full-auto task
- **Steps:** from a script, an agent or CI (stdin/stdout not a TTY): `wisp budget stop-at`, `wisp crew frobnicate`, `wisp settings frobnicate`, `wisp plugin frobnicate`, `wisp cloud frobnicate`.
- **Expected:** what the help says, and what happens in a TTY: "one word it doesn't have is an error naming its verbs".
- **Actual:** each one launched a task ("◌ launched: crew frobnicate"). They ran in Full auto on the default harness, in the current folder, and changed files there. In a TTY the same command errors with the verb list. Agents are the main non-TTY callers of the CLI, so a typo of theirs starts work.

### F31 · L · Opening the app at `/` reopens the last overlay
- **Steps:** with Settings open, load `http://127.0.0.1:4967/`.
- **Actual:** Settings (the last pane) opens again and the URL is rewritten to `?o=viewer:…`. On a phone width the app opens straight into Settings → General instead of home. A reload of an already-`/` URL does close it, which is inconsistent.
- **Question:** is restoring the overlay on an explicit `/` intended?
- **Shot:** `96-webkitphone-home.png` (asked for home, got General)

### F32 · L · Scrolling to a deep field slides the whole Settings sheet up
- **Actual:** keyboard or scroll-into-view of a lower field (Phone → GitHub card, Telegram card) scrolls the sheet inside its frame. The title row disappears and a blank band shows under it.
- **Shots:** `27b-Telegram-faketoken.png`, `29c-push-on.png`

### F33 · L · Adding a machine closes the dialog before the outcome is known
- **Steps:** Machines → Manage → `qa@127.0.0.1` → Add and connect.
- **Actual:** the dialog closes at once. Settings shows "1 machine · none online", and the failure ("Permission denied … Let me in") is only visible after reopening Manage.
- **Shots:** `51b-mach-200ms.png`, `52-mach-manage-after-add.png`

---

## Smaller notes (not counted)
- **Telegram:** its error has no "Check the token in …" hint, unlike Slack and Discord. With a bad token it keeps polling every ~5 s indefinitely.
- **Connector tools:** the list shows "1 tool" but not the tool names. `addedAt` is `0` for every connector.
- **Plugins:** they show their ids (`user.qa-pink`) rather than their names (`QA Pink`). Built-ins show as `accounts`, `claude`, ….
- **Recipes:** the manage page has no Edit.
- **Remove without confirm:** removing a project or a plugin has no confirm or undo (connectors and machines do confirm).
- **Machines → Advanced:** typing a port didn't change the SSH port (it connected on 22).
- **WebKit phone:** WebKit logs "ResizeObserver loop completed with undelivered notifications" on every pane.
- **`wisp resources kinds`:** its table has no header and the first column is a bare `false`.
- **`wisp init` outside a repo:** the help says it "asks first outside a git repo"; without a TTY it refuses instead (with `--here` named), which is reasonable.

## Cleanup done
- Servers stopped: `wisp serve` :4967, `wisp-grid dev` :4971, the catcher :4939 and the headless Chrome :9333.
- In the scratch home before stopping: this computer off, grid signed out, ntfy off, stop line back to Off.
- `/tmp/qaB` (home, fakes, repos, grid data) is left in place for re-checks.
