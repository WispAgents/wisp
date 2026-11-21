# Audit C: agent and workspace setup (round C, re-check of audit B plus what it missed)

2026-11-19 · worktree `wisp-8fb64ec939b0` at `55c9d978` · findings only, no repo file edited except this one.

> **Build note:** other sessions were editing this worktree during the round. When the round began only
> `design/v1-reference/` was changed. By the end, about 50 Rust and UI files were changed, among them
> `plugins/cli/core/src/human.rs` (C4), `plugins/worktrees/host/*` (C3) and `plugins/mcp/*`. The binaries and web bundle
> were built at the start of the round. Re-check C3 and C4 against those edits before fixing them.

**Setup** (copied from audit B):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`) and `wisp-grid` were built with
  `CARGO_TARGET_DIR=/tmp/wisp-target-qaC2`, then copied to `/tmp/qaC2/fake`. Copies of the fake agent under each harness
  name went to `/tmp/qaC2/bin`.
  - The half-done earlier attempt had crashed in rustc while building `wisp-channels`. I wiped its home and rebuilt.
  - The web UI was rebuilt at HEAD with `vite build --outDir /tmp/qaC2/web`. The earlier bundle predated `67809c7a`.
- **Server:** `wisp serve --port 4961` with `WISP_HOME=/tmp/qaC2/home` and `HOME=/tmp/qaC2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` pointed at a fake `npm` from the first start.
  - The first start had `WISP_UPDATE_CHECK=0`. Every later start left it unset, to re-check B4 and B5.
- **Catcher on `:4959`:** PostHog, Slack, Telegram, Discord, Linear and the ntfy server all pointed at it, with the same
  answers as in B.
- **Cloud:** `wisp-grid dev --port 4982 --data /tmp/qaC2/grid` (`wisp.localhost`).
  - Only `WISP_CLAUDE_BIN` and `WISP_CODEX_BIN` were set.
  - Spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` were first on its `PATH` and log any call.
    So no real harness CLI could run, whether or not B8 was fixed.
- **Test data:**
  - Audit B's repos `repo`, `repo2` and `repo3`, copied with B's `.wisp`/`AGENTS.md`/`.claude` leftovers removed.
  - B's v1 home, with its machine moved from `qa@127.0.0.1` to `qa@192.0.2.1` (TEST-NET), so no SSH key was ever
    offered to this Mac's sshd.
  - The machine added in the UI used the same TEST-NET address.
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - A persistent headless system Chrome over CDP `:9353` (own profile in `/tmp/qaC2/chrome-prof`), at 1280 and 390.
  - WebKit at 1280 and 390.
  - Every context aborted requests to `trywisp.dev`; none was attempted.
- **Screenshots:** `/tmp/qaC2/shots/` (132).
- **Stopped at the end:** the servers, the grid, the catcher and Chrome. No `/tmp/qaC2` process is left. The 7 GB target
  dir is deleted; `/tmp/wisp-target-fixer` was not touched. `/tmp/qaC2` stays for re-checks.

> **Outbound requests and other things to know.**
> - No `npm view` reached the real registry: the fake `npm` logged all three calls (see B5).
> - No real harness CLI ran: the grid's spy log stayed empty.
> - SSH only went to `192.0.2.1`, which times out.
> - A plugin install from `https://example.invalid/x.git` failed at DNS, by design.
> - `npm run doctor` was not run: it opens TCP connections to 4800 and 4477, which this round must not touch.
> - Nothing reached grid.trywisp.dev, 4477, 4800 or 4791. No real credential was read. No `~/.wisp*` folder was created
>   (`wisp-grid dev --help` ran with `HOME=/tmp/qaC2/realhome`).

## Counts

| | |
|---|---|
| Actions checked | 119 (B's 112 + 7 new) |
| Pass | 103 |
| Fail (a finding) | 10 |
| Not tested or blocked | 6 (32 sandboxes on a dev grid, 41 duplicate-path apply, 63 connector presets, 70 Email card, 72 push card, 96 `npm run doctor`) |
| Audit B findings | 13: 12 pass (fixed), 1 partly fixed (B6 → C6) |
| **New findings** | **9 (C1–C9): 0 high, 2 medium, 7 low** |

## Audit B findings, re-checked

| B# | Result | How checked |
|---|---|---|
| B1 | **P** | All 30 groups (accounts … termctx, crew, settings, plugin, cloud, machines, events, history) answer `✗ wisp <group> has no frobnicate: …` and name `wisp -- "…"`. `changed` and `go` are not groups (`wisp help changed`: "no command or plugin named changed"), so `wisp changed frobnicate` launching a task is correct. B's list was wrong there |
| B2 | **P** | Import: "· project gone not imported: No folder at /tmp/qaC2/v1-gone-folder". `validate -f`: "✗ projects.ghost: No folder at …" (exit 1). `apply --dry-run` says "1 problem: nothing can be applied". A file path is refused: "… is a file, not a folder" |
| B3 | **P** | `v1sess0001 "A v1 task"`: project `repo` |
| B4 | **P** | Update to 9.9.10 runs `claude update` (rec `["update"]`). Toast: "Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/tmp/qaC2/bin/claude update` in a terminal, then re-check." The button shows a pending state at 120 ms |
| B5 | **P** | No `npm view` at start, with or without `WISP_UPDATE_CHECK`. With checks on, the three calls come about a minute after start. The switch is `accounts.updateCheck` (in All settings, deliberately); `WISP_UPDATE_CHECK=0` and `CI` stop it too |
| B6 | **Partly fixed → C6** | Search "auto" and "model" have no Auto hit, and every hit lands on its row. Setup shows no Router, and the formula and `wisp export` have no `router:`. `wisp help` and completion don't name router. But `wisp router` still runs (C6) |
| B7 | **P** | `wisp-grid dev --help` prints its usage (exit 0) and starts nothing; `dev frob` says "✗ wisp-grid dev doesn't take frob" (exit 2) |
| B8 | **P** | Startup says "boxes have no opencode, cursor-agent, copilot, grok, agy: set WISP_<HARNESS>_BIN to a fake, or --real-harnesses …". The woken cloud computer's `runtime.agents` lists claude and codex 9.9.9, the other five unavailable. The spy log stayed empty |
| B9 | **P** | Diagnostics → Computers: "this computer · 2.0.0 · 51 plugins" and "Cloud · 2.0.0 · 48 plugins" |
| B10 | **P** | `--agent <TAB>` gives all 7 harnesses; `settings set worktrees.max <TAB>` gives `off 20 40 80`. `wisp "fix the thing" --agent frob` → "✗ no harness frob: --agent takes claude, codex, …", with no task created |
| B11 | **P** | After the import, a CLI task with no `--agent` runs on codex; New task shows Codex; Usage → Subagents run on shows Haiku |
| B12 | **P** | "Telegram: Unauthorized. Check the token in wisp → Settings → Phone → Telegram." |
| B13 | **P** | Trust: "Trust QA Pink? QA Pink then runs as its own process with your environment: it can read and change your files, use your keys and reach the network …". Remove: "Remove QA Pink? Turns it off and deletes it …" |

## Action table

Key: P = pass, C# = fails with that new finding, — = not tested or blocked. "server" means the effect was read back with
`wisp settings list --json`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded. "@100"/"@150"
means the UI state 100–150 ms after the click; "fb" is ms from the click to the first DOM change.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P | Versions from the fakes; opencode "Free models only · Add key" |
| 2 | Codex Sign in (device code) | P | "Sign in on auth.example.dev/codex/device · Code: ABCD-12345" by 600 ms; "Signed in" at 5 s |
| 3 | Add another Codex account (Work) | P | Ownership terms dialog; `accounts.list` has `Codex 2` (work) |
| 4 | Account menu: Move up / Move down | P | Rows swap @150; server order follows |
| 5 | Rename an account | P | "QA Work" @150 and on the server |
| 6 | Sign out of an account | P (note) | `signedOut`, row shows Sign in; no confirm (as in B) |
| 7 | Remove an account (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. Tasks that ran on it keep their history." |
| 8 | Harness Update button | P | B4 |
| 9 | Model default (launcher pick) | P | Opus @150, kept across reload, `runtime.launch.models.claude: opus`; rec `--model opus` |
| 10 | Thinking default | P | Slider to Max → "Opus · max"; rec `--effort max` |
| 11 | Lean Claude Code runs select | P | `runtime.leanRuns` on → off @150, restored |
| 12 | Ask before running a changed harness | P | Flips @100; kept across a reload (not a settings key); restored |
| 13 | Re-check | P | fb 21 ms; rows don't move |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100, server key changes, restored (same keys as B) |
| 15 | General: 4 selects | P | `settings.editor`, `terminal.terminal`, `review.harness`, `update.updateChannel` @150, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` @100; the menu label flips; kept across reload; put back to System |
| 17 | Notifications: Accept tasks from channels, Default project | P | `channels.acceptChannels`, `channels.project` |
| 18 | Notifications: Desktop notifications | P (note) | Disabled, "Blocked — allow notifications for this site in your browser" (headless) |
| 19 | Memory: Learn from tasks | P | `learn.enabled` |
| 20 | Automations: Webhook triggers | P | `automations.webhookTriggers` |
| 21 | Usage: 5 switches, 4 selects, Automations stop at | P | `budget.*`, `runtime.limitWait`, `runtime.subagentModel`, `machine.*`. Stop line 75 → 90 → Off, fb 21–28 ms, `wisp export budget` `stopAt: "75"`/`"90"`/none |
| 22 | Worktrees: 3 selects | P | `worktrees.cleanup`, `.max`, `.onDone` |
| 23 | Privacy: 3 switches | P | Usage stats, `worktrees.projectAsk`, `git.maskEnvFiles` |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 6 → 6 after a task and `wisp telemetry flush` ("queued false") |
| 25 | Updates: Channel; Check now | P | "A git clone: pull and restart to update"; "Checks are off on this install" |
| 26 | Keyboard shortcuts pane; N, ⌘K, ⌘P | P | N focuses the prompt; ⌘K and ⌘P list; `[` hides the list. Ranking: C7 |
| 27 | Settings search | P | `theme` → General, `ntfy` → Phone, `telemetry` → Privacy, `xyzzy` → "No settings match “xyzzy”"; 33 hits over 6 words each land on a row |
| 28 | All settings | P | No `runtime.auto` row, no `router.*` |
| 29 | Setup: Make the formula | P | Settings and projects; no `router:` |
| 30 | Setup: Import → See what it changes → Apply | P | "Adds 2 · setting settings.editor · project repo3-ui"; toast "Imported". Error paths: C8 |
| 31 | Diagnostics: Copy report | P | 7.8 KB JSON; Computers per B9 |
| 32 | Sandboxes | — | "This grid doesn't make sandboxes" (dev grid) |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus and empty state | P | Focus in `~/code/my-app or a GitHub URL`; "No projects yet. Type a folder’s path above, or paste a repo URL to clone it." |
| 34 | Add a missing folder / a file / a relative name | P | "No folder at …", "… is a file, not a folder: pick the folder it's in", "repo2 isn't a full path: …" |
| 35 | Add a `file://` URL | P | Added as its folder |
| 36 | Add a duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo" |
| 37 | Real folder check via import / apply | P | B2 |
| 38 | Remove a project (0 tasks, then 1 task) | **C2** | One click, fb 20 ms, gone; no confirm, no Undo, even with a task in it |
| 39 | Multi-pick (⌘P projects) | P | All projects ⌘0, repo ⌘1, repo2 ⌘2, repo3-ui ⌘3, Add project…, Manage projects… |
| 40 | CLI task in a project folder | P | `cd repo2 && wisp "qa repo2 task"` → project repo2 |
| 41 | `wisp apply` of a second project for a folder | — | Not re-run (B passed it) |

### Machines, peers, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Machines → Manage → Add `qa@192.0.2.1` | P | fb 42 ms, "Connecting", then "Can't connect … No answer · 192.0.2.1 doesn’t answer. Is it on and awake …" |
| 43 | Edit… (Advanced) | P (note) | Advanced: wisp’s port there, Start command, Connect when wisp starts; no SSH-port field (as in B) |
| 44 | Remove (confirm) | P | "Remove qabox from wisp? Its projects leave the launcher. Nothing on the machine is touched …"; `wisp machines list` → `(none)` |
| 45 | Peers: `wisp machines discover`, `hosts`, `reachable` | **C4** | `(none)`, `(none)`; `reachable` prints `ssh  {3 fields}` and leaves out Tailscale and the target (`--json` has them) |
| 46 | This computer: memory, CPU, disk, what's using it | **C3** | Memory and CPU real; "Disk — free" and "Nothing to free" until a Worktrees scan runs |
| 47 | Pause automations / Resume now | P | "Automations paused until 14:09" (fb 27 ms); "Running as usual" (fb 37 ms) |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in: device code, popup to `/activate?code=…`, dev sign-in, Allow | P | `wisp cloud status`: qa-alice |
| 49 | Don't allow, then sign in again | P | "Sign-in was declined in the browser."; the address stays `http://wisp.localhost:4982` |
| 50 | This computer on the grid → on | P | fb 17 ms, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4982" @150; that address answers 401 without a sign-in |
| 51 | Grid restart: the tunnel comes back | P | `cloud status` online 2 s after the restart |
| 52 | API key: create, use, revoke | P | "Give the key a name." when it's empty; key shown once; `/api/health` 200; "Revoke qa-script? … This can’t be undone." → 401 |
| 53 | Add an account (qa-bob), Use another account | P | Use another account → `/signin?return=/activate?code=…` → qa-bob's Allow page; both rows listed |
| 54 | Switch to qa-bob and back | P | This computer off for qa-bob, back online for qa-alice; the current row's Switch is disabled |
| 55 | Sign out of qa-bob (confirm) | P | "Sign out of qa-bob on wisp.localhost:4982 here? …"; gone @150 |
| 56 | Cloud computer woken by an API call | P | B8 |
| 57 | `wisp computer off`, `wisp cloud signout` | P | "this computer: off", "✓ signed out", "not signed in" |

### Connectors and MCP servers
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector ("QA Echo!" → `qa_echo`) | P | "1 tool"; echo log has initialize, initialized, tools/list |
| 59 | Edit: add env, Save | P | Server `env: [{QA_VAR, set: true}]` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it"; `pasted`, `remote1` added |
| 61 | Connectors reach a run | P | rec `--mcp-config` servers `pasted`, `qa_echo`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, and its saved keys are deleted." |
| 63 | Presets and remote OAuth | — | Presets only show while there are no connectors; OAuth needs a real provider |
| 64 | `wisp mcp` tools, `wisp_launch` → `wisp_list` | P | 11 tools; launched `qa via mcp` (codex), listed by `wisp_list` |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy: on → Test → off | P | "Working" @150; topic `wisp-43799186936303`; the catcher got the test POST, then one reply poll every 3 s; quiet after off |
| 66 | Telegram, bad token | P | B12 |
| 67 | Slack, bad tokens | P | "Slack: invalid_auth. Check the tokens in …"; link code 124143 |
| 68 | Discord, bad token | P | "Discord: 401: Unauthorized. Check the token …" |
| 69 | GitHub (fake gh) | P | "GitHub: gh api user gave no id" |
| 70 | Email | — | Not re-read |
| 71 | Switch off and Remove the tokens | P | All `off`; catcher quiet for 12 s |
| 72 | "push" card | — | Not re-read in the UI (`channels status` still lists `push`, as in B) |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` shows in both lists | P | `user.qa-theme` off in `wisp plugin` and the pane |
| 74 | Install from a folder (UI) | P | "QA Pink 1.0.0 is installed, off · It asks for nothing beyond showing itself · Switch it on · Not now" |
| 75 | Switch it on | P | On @150; styles don't apply while untrusted (`--accent` `#2f5bea`) |
| 76 | Trust / Untrust, partStyles | P | B13; trusted `--accent` `#ff1493`, `launcher.send` pink; untrusted back to `#2f5bea` |
| 77 | Switch off, Remove | P | B13 confirm; gone |
| 78 | Export after Remove | P | `wisp export plugin` lists only `user.qa-theme` |
| 79 | CLI enable/disable reflects live | P | The row follows `wisp plugin enable/disable user.qa-theme` |
| 80 | Built-in off and on (learn) | P | "wisp.learn was switched off" @100; back on; `learn.view` answers |
| 81 | Cast a plugin | P | Task "A top bar button that says hi" in `home/scratch/<id>` |

### Import from v1, `wisp init`, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | P | "10 to add, 3 to change"; "!" lines for machine, ntfy, task; "·" notes |
| 83 | `wisp import --yes` | P | "✓ applied 13 changes", "✓ 1 task imported" |
| 84 | v1 ntfy | P | `routing/topic` → `wisp-old-topic-123456789`; it then polled that topic |
| 85 | v1 defaults | P | editor, maskEnvFiles, worktreeMax, limitHold, notify, leanRuns, retentionDays, subagentModel |
| 86 | v1 project at a missing folder | P | B2 |
| 87 | Imported v1 task's project | P | B3 |
| 88 | v1 default agent and subagent model | P | B11 |
| 89 | `wisp init` (repo, again, non-repo) | P | `.wisp/wisp.yml` + AGENTS.md note; "is ready" again; non-repo refused naming `--here` (exit 2) |
| 90 | `wisp settings get/set` (good, bad value, unknown key) | P | "must be one of off, 20, 40, 80", "no plugin declares the setting nope.key" (exit 1) |
| 91 | `wisp export` yaml/json/toml → validate → diff | P | 28 resources; "0 to add, 0 to change, 0 to remove, 28 unchanged" each. `--format xml`: C5 |
| 92 | `wisp validate`/`apply` errors | P | Missing folder, a file, a missing formula file; each exits 1 with its reason |
| 93 | `wisp completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass |
| 94 | What bash completion offers | P | B10 |
| 95 | `wisp skill install --agent all` | P | Three ✓ skill paths, project files, Codex config; one "·" note for opencode |
| 96 | `npm run doctor` | — | It opens TCP connections to 4800 and 4477; not run. `wisp doctor` is not a command and names `wisp version` and friends (as in B) |
| 97 | `wisp <group> <unknown word>` | P | B1 |
| 98 | `wisp "…" --agent frob` | P | B10 |

### Auto disabled everywhere (`runtime.auto` off)
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | `runtime.auto` default | P | false |
| 100 | Launcher harness picker | P | The 7 harnesses and "Set up another agent…"; no Auto |
| 101 | ⌘K: auto, harness, model | P | No Auto entry |
| 102 | Crew editor harness | P | "As New task" and the 7 harnesses; no Auto |
| 103 | All settings | P | No `runtime.auto`, no `router.*` |
| 104 | Settings search "auto" / "model" | P | B6 |
| 105 | Setup and the formula | P | B6 |
| 106 | CLI | **C6** | Not in `wisp help` or completion, and `--agent auto` falls back to the default. But `wisp router` still lists "preview  What Auto would run for a prompt" |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `plugin` and `cloud status` identical before and after; the machine goes error → connecting |
| 108 | WebKit 1280: home and all 20 panes | P | No overflow, no errors; WebKit's ResizeObserver note from B is gone |
| 109 | Chrome 390: home and all 20 panes | P | No overflow, no errors |
| 110 | WebKit 390: home and all 20 panes | P | No overflow, no errors |
| 111 | Harness update check at start | P | B5 |
| 112 | `wisp-grid dev --help` | P | B7 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects: each project's task count | **C1** | "repo · 0 tasks" and "repo2 · 0 tasks" while each has a task (`wisp ls`: project repo, repo2) |
| 114 | Setup → Import error paths (bad YAML, `wisp: 9`, a bad value, a bare word) | **C8** | Each answers within 41 ms and Apply is disabled for a plan that can't apply. But an earlier text's plan stays under a later error, and the YAML error is a raw parser dump |
| 115 | Plugins → Install error paths (not a plugin, missing folder, bad git URL, bare words) | **C9** | fb 29–40 ms; wording per C9 |
| 116 | CLI error paths (`settings` bare, `settings get` without key, `export --format xml`, `export nosuchkind`, `import`/`apply`/`add` of missing paths, `plugin install`/`enable` of non-plugins) | **C5** | All but two say what's wrong; `export --format xml` and `export nosuchkind` don't |
| 117 | Project colour | P | 9 colours; fb 28 ms; `wisp export project` `color: amber` |
| 118 | CLI text output of nested values (`machines list`, `machines reachable`, `settings get runtime.launch`) | **C4** | `{3 fields}`, `{1 fields}`, raw epoch ms |
| 119 | ⌘K ranking | **C7** | "model" lists "Theme: match the system" and "Dark theme" above Agents |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### C1 · M · Manage projects says "0 tasks" for projects that have tasks
- **Steps:** add `repo` and `repo2` → run a task in each (UI New task in repo, `cd repo2 && wisp "…"`) → top bar
  project menu → Manage projects….
- **Actual:**
  - Both rows read "0 tasks", while `wisp ls --json` gives each task its project (repo, repo2).
  - It's the only number a user sees before pressing Remove (C2). Shot `c-proj-counts.png`.
- **Suspect:** `packages/shell/src/frame/Projects.tsx:88` `counts` maps each task's `summary.cwd` to a project folder.
  A worktree task's cwd is `home/worktrees/<project>-<id>`, so no worktree task ever counts. Count by the task's
  `project` instead.

### C2 · L · Removing a project takes one click, with no confirm and no Undo, even with tasks in it
- **Steps:** Manage projects… → the bin on `repo2` (one task in it).
- **Actual:** gone in 20 ms, with no confirm and no toast. The task keeps `project: repo2`, a project that no longer exists.
- **Expected:** what connectors, machines, accounts, API keys and plugins (B13) now do: a confirm naming what happens to
  its tasks, or an Undo.
- **Suspect:** `packages/shell/src/frame/Projects.tsx:225` runs `dropProject` straight from `onRemove`.

### C3 · M · This computer shows "Disk — free" and "Nothing to free" until the Worktrees pane scans
- **Steps:** a server that's been up 30 minutes, with tasks and worktrees → Settings → This computer.
- **Actual:**
  - `machine.read` answers `disk: {free: null, total: null, worktrees: null}`, and the pane shows "Disk | — free" and
    "Free up space · Nothing to free".
  - Two worktree items sit at `status: checking, size: null`.
  - After one `worktrees.view` call and the next sample, the pane reads "162 GB free · wisp’s folders take 28 KB".
- **Regression:** audit B saw real disk readings.
- **Suspect:** `plugins/machine/host/src/calls.rs:66` now reads only `worktrees.viewLast` ("the last look, never a new
  one"), from the performance round in `928fb5f3`. Until something has asked for a full view, there is none.

### C4 · L · CLI text output hides nested values: `{3 fields}`, `{1 fields}` and epoch ms
- `wisp machines reachable` prints `ssh  {3 fields}` and leaves out `tailscale` and `target`. `--json` has the banner,
  port, Tailscale name and IP, and the `ssh` target.
- `wisp settings get runtime.launch` prints `efforts  {1 fields}`, `labels  {1 fields}`, `models  {1 fields}`
  ("1 fields" is wrong English too).
- `wisp machines list` prints `a030bd  qabox  error  1795089942868  qa@192.0.2.1`: an epoch-ms column with no header.
- **Suspect:** `plugins/cli/core/src/human.rs:63` renders any object as `{N fields}`.

### C5 · L · `wisp export` takes an unknown format or kind without saying so
- `wisp export --format xml` writes YAML and exits 0.
- `wisp export nosuchkind` prints only the usage line (exit 2), with no "✗ no kind nosuchkind: …".
- Every other CLI error path in action 116 names what's wrong.

### C6 · L · `wisp router` still runs and talks about Auto while help says it doesn't exist (B6, partly fixed)
- `wisp help router` → "✗ wisp help router: no command or plugin named router".
- `wisp router` → "wisp router <verb> (6 commands, from wisp.router) · preview  What Auto would run for a prompt …".
- `wisp router frobnicate` lists `preview, record, reset, set, signal, view`.
- **Expected:** while Auto is put away, either hide the group entirely or don't deny it in help.

### C7 · L · ⌘K ranks the theme commands first for "model"
- **Steps:** ⌘K → `model`.
- **Actual:** "Theme: match the system", "Dark theme", Recipes, Agents, Automations, Memory. Settings search for the same
  word puts Agents first, with "New task's last pick" and "The model Claude Code's subagents run on".
- **Suspect:** the new narrowing ranker (`928fb5f3`, ⌘K 940 → 61 ms) matches "model" loosely against "Theme: match the
  system".

### C8 · L · Setup → Import: an earlier text's plan stays under a later error, and YAML errors are raw parser dumps
- **Steps:** paste `wisp: 1\nsettings:\n  worktrees.max: 33` → See what it changes. Then paste `hello` → See what it
  changes.
- **Actual:**
  - The toast says "pasted.yml: a formula is a map (wisp: 1, then its sections)".
  - The pink plan below still shows the previous text's "It can't be applied · settings.worktrees.max … not 33 · Adds 1".
    Shot `c-setup-stale.png`.
  - A YAML syntax error's toast is the parser's whole report: "YAML: error: line 2 column 13: unclosed bracket '{'
    --> <input>:2:13 | 1 | wisp: 1 2 | projects: [ {name: x | ^ …".
- **Expected:** clear the plan when the text changes or fails. Say the line, column and problem in one sentence.

### C9 · L · Plugins → Install: wrong words for a missing folder, a bare word, and where plugins live
- `/tmp/qaC2/nope-dir` (doesn't exist) → "/tmp/qaC2/nope-dir has no wisp-plugin.json". Projects say "No folder at …".
- `not a path` → "not a path has no wisp-plugin.json". Projects say "… isn't a full path".
- The pane's foot reads "Plugins live in ~/.wisp/plugins". This wisp keeps them in `$WISP_HOME/plugins`
  (`/tmp/qaC2/home/plugins`, as `wisp plugin new` printed).
- A bad git URL's toast is git's raw output, including the temp folder `…/plugins/.incoming-01a1…`.
- **Suspect:** `plugins/plugins/host/src/install.rs` (around line 361) and the `plugins.foot.pre` string in
  `plugins/plugins/ui/src/i18n/en.lazy.json:40`.

---

## Smaller notes (not counted)
- **Machines:** the SSH error "ssh: connect to host 192.0.2.1 port 22: Operation timed out" shows twice in one row.
  Esc in the Edit form closes the whole Manage dialog.
- **Projects:**
  - `wisp add` stores `/private/tmp/qaC2/repo2`, while the UI stores the typed `/tmp/qaC2/repo2`. The duplicate check
    still treats them as one folder.
  - The Projects dialog's "+ Machine" opens Settings → Machines and closes Projects.
- **Crew editor:** its harness order (Claude Code, Codex, opencode, Cursor, Antigravity, Grok, Copilot) differs from the
  launcher's (Claude Code, Codex, Cursor, opencode, Copilot, Grok, Antigravity).
- **Setup:** Share setup says "your accounts and screens stay out", yet the formula carries `runtime.launch` (New task's
  last model and thinking pick).
- **Still as in B:**
  - Claude Code's row shows no "Signed in · Sign out" (the fake answers `auth status --text` only).
  - Signing out of a harness account has no confirm.
  - Connectors' `addedAt` is 0.
  - All settings descriptions are developer notes ("{ agent, models: { <harness>: model } … } (v1 defaults.agent …)").
  - Putting a Usage or Worktrees select back stores the default explicitly.
  - `wisp channels status` lists `push`.
- **Learn:** with the fakes, the retro history records "Unreadable reply: All done. ```wisp…" (the fake's answer isn't
  a retro). Expected with fakes.

## Cleanup done
- **Restored:** every setting the round or the v1 import changed is back to its default. `wisp settings list --json`
  shows no value differing from its declared default. Theme System, Usage stats on, ntfy off (the import had turned it
  on), stop line Off, this computer off, grid signed out.
- **Stopped:** `wisp serve` :4961, `wisp-grid dev` :4982 (and its boxes), the catcher :4959 and headless Chrome :9353.
  No process under `/tmp/qaC2` is left.
- **Deleted:** `/tmp/wisp-target-qaC2` (7 GB).
- **Kept for re-checks:** `/tmp/qaC2` (binaries in `fake/` and `bin/`, home, repos, grid data, shots, scripts in `pw/s`
  and `tools/`). The scratch home still holds:
  - the imported v1 machine (`oldbox`, TEST-NET), connector, recipe and task;
  - `repo3-ui` and about 12 fake tasks;
  - the connectors `pasted` and `qa_echo`;
  - `user.qa-theme` (off).
