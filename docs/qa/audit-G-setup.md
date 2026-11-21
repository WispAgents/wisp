# Audit G: agent and workspace setup (round G, re-check of audit F plus what it missed)

2026-11-20 · worktree `wisp-8fb64ec939b0` at `37b94ef0` · findings only, no repo file edited except this one.

> **Build note:** when the binaries and the web bundle were built (11:32), the worktree differed from HEAD only in
> `design/v1-reference/` (another session's screenshots). Both are HEAD's.

**Setup** (copied from audit F, in a fresh `/tmp/qaG2`):
- **Build:** `wisp`, the fakes (`wisp-fake-agent`, `wisp-fake-gh`), `wisp-grid` and `wisp-grid-hostd` were built at HEAD
  with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`, then copied to `/tmp/qaG2/fake`. Copies of the fake agent under each
  harness name went to `/tmp/qaG2/bin`. The web UI was built with `vite build --outDir /tmp/qaG2/web`.
- **Server:** `wisp serve --port 4981` with `WISP_HOME=/tmp/qaG2/home` and `HOME=/tmp/qaG2/realhome`.
  - Every `WISP_<HARNESS>_BIN` and `WISP_GH_BIN` pointed at the fakes, with `FAKE_RECORD` on and `FAKE_LOGIN_MS=4000`.
  - `WISP_NPM` pointed at a fake `npm` from the first start.
  - The first start had `WISP_UPDATE_CHECK=0`. The later starts (restarts at 12:01 and 12:28) left it unset.
- **Catcher on `:4989`:** PostHog, Slack, Discord, Linear and the ntfy server, with the same answers as in F. New this
  round: a fake Telegram Bot API behind a switch file (`getMe`, `getUpdates` from a queue file, `sendMessage`), used to
  link a group chat as person 1001 and send replies from person 1001 and from a stranger (2002).
- **Cloud:** `wisp-grid dev --port 4982 --data /tmp/qaG2/grid` (`wisp.localhost`).
  - Only `WISP_CLAUDE_BIN` and `WISP_CODEX_BIN` were set.
  - Spy scripts named `cursor-agent`, `grok`, `agy`, `opencode` and `copilot` were first on its `PATH`.
- **Desktop self-update (new):** a scratch packaged-like install, `/tmp/qaG2/pkg/wisp.app/Contents/MacOS/wisp` (a clone of
  the HEAD binary, `Info.plist` 2.0.0), served on `:4986` with its own `WISP_HOME=/tmp/qaG2/home-app`,
  `HOME=/tmp/qaG2/realhome-app` and `WISP_UPDATE_FEED=http://127.0.0.1:4985/releases`.
  - The feed on `:4985` answered `releases/latest` with v2.0.1 and its `wisp-mac-arm64.zip`, plus its digest.
  - The zip held a `wisp.app` (2.0.1) whose executable only logs that it started.
  - No owner app was touched.
- **Test data:**
  - Fresh clones of F's `repo`, `repo2`, `repo3` and `repo4` (remote removed), and `repo3-wt` (a linked `git worktree`
    of `repo3`).
  - F's v1 home, with its paths moved to `/tmp/qaG2`. Its connector is now in v1's real shape: an `id`, env names as a
    list, and the value in v1's `secrets.json`. F's copy had the value inline.
  - The machine added in the UI used `qa@192.0.2.1` (TEST-NET).
- **UI:** Playwright from `/tmp/pw/node_modules/playwright-core`.
  - A persistent headless system Chrome over CDP `:4984` (own profile in `/tmp/qaG2/chrome-prof`), at 1280 and 390.
  - For the layout sweep, fresh Chrome and WebKit browsers at 1280 and 390, each in light and dark, closed in
    `finally`.
  - Every context aborted requests to `trywisp.dev`, `example.dev` and `example.invalid`. None was attempted.
- **Ports:** 4981, 4982, 4984, 4985, 4986 and 4989.
- **Screenshots:** `/tmp/qaG2/shots/` (178, named `g-*`).
- **Stopped at the end:** the server, the scratch app's server, the feed, the grid (its box stopped with it), the
  catcher and Chrome.
  - No `/tmp/qaG2` process is left, and nothing listens on 4980–4990.
  - One fresh-browser script for the scratch app hung before its first step. Its node and Chrome were stopped by pid,
    and the step was re-run through the CDP Chrome.

> **Outbound requests and other things to know.**
> - No `npm view` reached the real registry: the fake `npm` logged each call.
> - No real harness CLI ran: the grid's spy log stayed empty.
> - SSH only went to `192.0.2.1`, which times out.
> - `npm run doctor` was not run: it opens TCP connections to 4800 and 4477.
> - Nothing reached grid.trywisp.dev, github.com, 4477, 4800, 4791 or 4329. No real credential was read. Every `wisp`
>   call ran with a `/tmp/qaG2` `WISP_HOME` and `HOME`.
> - The agent token used for the setup cards is the one the scratch server gave its fake agent (read from the fake's
>   recorded `--mcp-config`).
> - **`wisp update` on a git clone:** checked as in F (a debug build reads as a clone).
>   - The desktop app's path was checked this round on the scratch `.app` above.
>   - `install::detect` takes a `.app/Contents/MacOS` path as the app before it looks for a clone.

## Counts

| | |
|---|---|
| Actions checked | 180 (F's 154 + 26 new) |
| Pass | 155 |
| Fail (a finding) | 15 |
| Not tested or blocked | 10 (32 sandboxes on a dev grid, 37 ghost folder, 63 presets with no connectors, 87 imported task's project, 96 `npm run doctor`, 112 `wisp-grid dev --help`, 122 paste/Continue, 125 a sign-in that times out, 133 personal/work switch, 144 the full built-in sweep: only learn and worktrees) |
| Audit F findings | 12: 11 pass (fixed), 1 still fails (F12, partly fixed) |
| Audit E finding E8 | Still fails |
| **New findings** | **13 (GS1–GS13): 0 high, 5 medium, 8 low** |

(GS: "G, setup". `docs/qa/audit-G-orchestration.md` already uses G1–G22.)

## Audit F findings, re-checked

| F# | Result | How checked |
|---|---|---|
| F1 | **P** | `wisp plugin disable wisp.learn` → `wisp learn view` in `notrepo`: "✗ wisp learn is off: … Switch it on: wisp plugin enable wisp.learn" (exit 2). No task. `wisp plugin disable wisp.settings` is refused ("wisp needs it …") |
| F2 | **P** | 11 UI launches in `repo` (worktree per task): none showed "hit an error". fb 2–3 ms each |
| F3 | **P** | 4 and 7 projects: the switcher lists every project, then Add project…, Manage projects…, Repo history |
| F4 | **P** | v1 connector `{id: c1, env: [QA_VAR]}` + v1 `secrets.json`: plan "· connector old-echo: QA_VAR comes over from v1's secrets (as connector-old-echo-QA_VAR)". Export has `QA_VAR: ${secret:connector-old-echo-QA_VAR}`, and the echo started with `QA_VAR=v1secret`. The display name still goes ("Old Echo" → `old-echo`) |
| F5 | **P** | `wisp update` on the clone: "· wisp 2.0.0 is a git clone: pull and restart to update" |
| F6 | **P** | Worktrees after `wisp.worktrees` off and on: "15 worktrees · 172 KB" with 15 on disk. The pane's Bring in is a new finding (GS1) |
| F7 | **P** | `wisp.settings` and `wisp.plugins` switches are disabled in Plugins; the CLI refuses `disable wisp.settings` |
| F8 | **P** | Telegram, Slack, Discord, GitHub each on then off (fb 31–42 ms): `wisp export channels` still lists only ntfy |
| F9 | **P** | Connectors at 390, Chrome and WebKit, light and dark: the pill keeps its words ("Not checked yet"), the command truncates |
| F10 | **P** | `wisp update view` and `check` print this wisp's view. `install` on the clone: "✗ This copy of wisp is a git clone …" (exit 1) |
| F11 | **P** | `wisp connectors remove qa_tmp2` removes it; `--help` shows `usage: wisp connectors remove <id>` |
| F12 | **Still fails (partly)** | ADR-0109 now records it, and `done`'s comment says it. The module header `plugins/grid-client/host/src/signin.rs:4` still says "Signing in again adds an account … and makes it current (ADR-0061)" |
| E8 | **Still fails** | Codex → QA Work → ⋯ → Sign out → "Signed out · Sign in"; its ⋯ menu still lists "Rename · Make it a personal account · Move up · Move down · Sign out · Remove" |

## Action table

Key: P = pass, GS# = fails with that new finding, E8/F12 = still fails, — = not tested or blocked. "server" means the
effect was read back with `settings.list`, `wisp export …` or a plugin read. "rec" means the argv the fake recorded.
"@100"/"@150" means the UI state 100–150 ms after the click; "fb" is ms from the click to the first DOM change.

### Settings → Agents
| # | Action | Result | How checked |
|---|---|---|---|
| 1 | Pane lists the 7 harnesses and GitHub | P | As F; GitHub's row has no state (fake `gh`) |
| 2 | Codex Sign in (device code) | P | fb 24 ms; "Starting the sign-in…" @100, the box with steps 1–2 and ABCD-12345 by 600 ms; "Signed in" by 6 s; no popup |
| 3 | Add another Codex account (Work) | P | Ownership terms dialog; `accounts.list`: `Codex 2` (work, ready) |
| 4 | Move up / Move down | P | Rows swap @150; server order follows |
| 5 | Rename | P | Focused field "Codex 2" → "QA Work" @150 and on the server |
| 6 | Sign out of an account | P (note) | "Signed out · Sign in" @150, `signedOut`; no confirm (as in F) |
| 7 | Remove (confirm) | P | "Remove QA Work? wisp signs it out and forgets it. …" · Remove account |
| 8 | Harness Update button | P | "Update to 9.9.10" ×3 after the restart with checks on; fb 41 ms; toast "Claude Code is still 9.9.9 after the update …" |
| 9 | Model default | P | Claude Code → Opus fb 33 ms, "Opus" @150, kept across reload; rec `--model opus` |
| 10 | Thinking default | P | Max → "Opus · max"; `runtime.launch` efforts `{claude: max}`; rec `--effort max` |
| 11 | Lean Claude Code runs | P | fb 42 ms; `runtime.leanRuns` all → off, restored |
| 12 | Ask before running a changed harness | P | Flips @100, fb 25 ms; restored |
| 13 | Re-check | P | Codex reads Signed out at once after its auth file was moved aside |

### Every Settings pane and switch (each changed, then put back)
| # | Action | Result | How checked |
|---|---|---|---|
| 14 | General: 10 switches | P | Each flips @100 (fb 9–19 ms), its server key changes, restored |
| 15 | General: 4 selects | P | `settings.editor`, `terminal.terminal`, `review.harness`, `update.updateChannel`, fb 8–27 ms, restored |
| 16 | Theme System / Light / Dark, reload | P | `data-theme` @100; the account menu label flips; kept across reload |
| 17 | Notifications: Accept tasks from channels, Default project | P | fb 41 and 23 ms; Default project lists Last used (repo), repo … repo4 |
| 18 | Desktop notifications | P (note) | Disabled, "Blocked — allow notifications …" (headless) |
| 19 | Memory: Learn from tasks | P | `learn.enabled`, fb 26 ms |
| 20 | Automations: Webhook triggers | P | fb 26 ms |
| 21 | Usage: 5 switches, 4 selects | P (note) | fb 9–26 ms; putting a machine select back writes its default explicitly (as in F; it now shows in Setup's "Settings 13") |
| 22 | Worktrees: 3 selects | P | fb 8–30 ms |
| 23 | Privacy: 3 switches | P | fb 16–21 ms |
| 24 | Usage stats off → no PostHog | P | Catcher `/batch/` 35 → 35 after a task and `wisp telemetry flush` ("queued false"), and after switching back on |
| 25 | Updates (clone): Channel; Check now | P | "A git clone: pull and restart to update"; Check now disabled, "Checks are off on this install" |
| 26 | ⌘K, ⌘P, `[`, N | P | Both list; `[` hides and brings back the list; N focuses the launcher |
| 27 | Settings search | P | `ntfy` → Phone; `telemetry` → Privacy and the Lean row; `worktree` → General, Worktrees and 5 rows; each hit opens its pane |
| 28 | All settings | P | No `runtime.auto` row, no `router.*` |
| 29 | Setup: Make the formula | P | No `router:` |
| 30 | Setup: Import → plan → Apply | P | "Adds 1 · project repo3-wt · Changes 1 · settings.editor cursor → zed"; "Imported"; both on the server. A path already a project under another name is refused (deliberate) |
| 31 | Diagnostics | P | Report JSON 7.3 KB; "this computer · 2.0.0 · 52 plugins · online" |
| 32 | Sandboxes | — | A dev grid makes none |

### Projects
| # | Action | Result | How checked |
|---|---|---|---|
| 33 | Place chip → Other folder…: focus | P | Focus in `~/code/my-app or a GitHub URL` |
| 34 | Missing folder / a file / a relative name | P | Same three messages as F |
| 35 | `file://` URL | P | `file:///tmp/qaG2/repo4` added as repo4 |
| 36 | Duplicate, `/tmp` vs `/private/tmp` | P | "That folder is already repo" (both), "… repo2" |
| 37 | Ghost folder via import | — | Not re-run (the import's missing folder: 86) |
| 38 | Remove a project (confirm) | P | repo5–7, repo3-wt: fb 17–27 ms, "Remove “repo5” from wisp? Its files are kept." · Remove project; gone from the export |
| 39 | Multi-pick (top bar) | P | F3 |
| 40 | Tasks in a project folder | P | F2; rec cwd `home/worktrees/repo-<id>`, `--model opus --effort max` |
| 41 | A second project for one folder | P | Setup refuses "project repo3 and repo3-ui have the same path … one project per path" (deliberate) |

### Machines, This computer
| # | Action | Result | How checked |
|---|---|---|---|
| 42 | Manage → Add `qa@192.0.2.1` | P | fb 33 ms, "Connecting" @150; `wisp machines list` → error |
| 43 | Edit… (Advanced) | P (note) | Every field filled; Esc still closes the whole Manage dialog |
| 44 | Remove (confirm) | P | `oldbox`: fb 7 ms, "Remove oldbox from wisp? Its projects leave the launcher. …" |
| 45 | `machines discover`, `hosts`, `reachable` | P | `(none)`, `(none)`, this computer's lines |
| 46 | This computer: memory, CPU, disk, what's using it | P | "71% free", "37% of 8 cores", "104 GB free", "wisp itself 62 MB" |
| 47 | Pause / Resume | P | fb 21 ms; "Automations paused until 12:45" @150; Resume fb 44 ms, "Running as usual" @150 |

### Cloud (dev grid)
| # | Action | Result | How checked |
|---|---|---|---|
| 48 | Sign in (device code, popup, dev sign-in, Allow) | P | Code at 150 ms; popup `/activate?code=…`; "Device connected"; `cloud status`: qa-alice |
| 49 | Don't allow | P | "Sign-in was declined in the browser." |
| 50 | This computer on the grid | P | fb 26 ms, "Online · mokhless-macbook-pro--qa-alice.wisp.localhost:4982" @150 |
| 51 | Grid restart | P | Box stopped with it; "retrying" then online within ~8 s |
| 52 | API key create, use, revoke | P | fb 36 ms; `/api/health` 200 with it, 401 without; Revoke fb 18 ms → 401 |
| 53 | Add qa-bob | P | qa-alice stays current (ADR-0109) |
| 54 | Switch and back | P | fb 13 ms; Current moves @150; the current row's Switch is disabled |
| 55 | Sign out of qa-bob (confirm) | P | "Sign out of qa-bob on wisp.localhost:4982 here? …"; gone @150 |
| 56 | Cloud computer woken by use | P | `wisp ls --on Cloud`: asleep → online; spy log empty |
| 57 | `wisp computer off`, `cloud signout` | P (note) | "this computer: off · http://…" (still prints the address), "✓ signed out" |

### Connectors and MCP
| # | Action | Result | How checked |
|---|---|---|---|
| 58 | Add a local connector | P | `qa_echo` "1 tool" by 4 s |
| 59 | Edit: add env, Save | P | Server `env: [{QA_VAR, set: true}]`; echo started `QA_VAR=hello-ui` |
| 60 | Paste 2 servers | P | "This config has 2 servers: remote1 will be added with it" |
| 61 | Connectors reach a run | P | rec `--mcp-config` servers `old-echo`, `pasted`, `qa_bad`, `qa_echo`, `wisp` |
| 62 | Remove (confirm) | P | "Remove remote1? Agents lose its tools from their next turn, …" |
| 63 | Presets | — | Not re-run (they show only with no connectors) |
| 64 | `wisp mcp` | P | 262 tools over stdio, 258 at `/api/mcp`, no `router_*`; `wisp_launch` → `wisp_list` |

### Channels
| # | Action | Result | How checked |
|---|---|---|---|
| 65 | ntfy Test | P (note) | fb 41 ms; the catcher got it; still nothing on screen says it was sent |
| 66–69 | Telegram, Slack, Discord, GitHub bad tokens | P | The same four messages as F |
| 70 | Email | P | Switch disabled |
| 71 | Switch off and remove the tokens | P | All off, nothing saved |
| 72 | No "push" card | P | 6 cards |

### Plugins
| # | Action | Result | How checked |
|---|---|---|---|
| 73 | `wisp plugin new` | P | `user.qa-theme` off |
| 74 | Install from a folder (UI) | P | fb 45 ms |
| 75 | Switch on (untrusted) | P | fb 41 ms; `--accent` `#2f5bea` |
| 76 | Trust / Untrust | P | "Trust QA Pink? …" → `#ff1493`, send button rgb(255, 20, 147); Untrust → `#2f5bea` |
| 77 | Off, Remove | P | Off fb 42 ms; Remove fb 34 ms, "Remove QA Pink? …"; row gone @200 |
| 78 | Export after Remove | P | Only `user.qa-theme` |
| 79 | CLI enable/disable | P | `active` / `off` |
| 80 | Built-in off and on (learn) | P | fb 44 ms; `learn.view` → `plugin_off`; back on, it answers |
| 81 | Cast a plugin | P | Opens the task at once; runs in `home/scratch/<id>`; `wisp ls`: `no project` |

### Import, init, settings, export/apply, completion
| # | Action | Result | How checked |
|---|---|---|---|
| 82 | `wisp import <v1> --dry-run` | P | 13 to add; "!" lines for machine, ntfy, task; "·" notes |
| 83 | `wisp import --yes` | P | "✓ applied 13 changes", "✓ 1 task imported" |
| 84 | v1 ntfy | P | Polls `wisp-old-topic-123456789-reply` on the catcher |
| 85 | v1 defaults | P | editor, maskEnvFiles, worktreeMax, limitHold, notify, leanRuns, retentionDays, subagentModel |
| 86 | v1 project at a missing folder | P | "· project gone not imported: No folder at …" |
| 87 | Imported v1 task's project | — | Not read back |
| 88 | v1 default agent | P | The first UI tasks ran on Codex |
| 89 | `wisp init` (repo, again, non-repo) | P | As F; non-repo exit 2 naming `--here` |
| 90 | `settings get/set` | P (note) | "must be one of off, 20, 40, 80" (exit 1); "no plugin declares the setting nope.key" (exit 1) |
| 91 | export yaml/json/toml → validate → diff | P | 34 unchanged each; 6 `${secret:…}` |
| 92 | `validate`/`apply` errors | P | See 172–174 |
| 93 | `completions bash/zsh/fish` | P | `bash -n`, `zsh -n` pass; fish 1578 lines; no `router` |
| 94 | Bash completion | P | `--agent` → 7 harnesses; `worktrees.max` → `off 20 40 80`; `export` → kinds |
| 95 | `wisp skill install --agent all` | P | As F |
| 96 | `npm run doctor` | — | Touches 4800/4477 |
| 97 | `wisp <group> <unknown word>` | P | 10 groups list their verbs |
| 98 | `--agent frob` | P | "✗ no harness frob: …" (exit 2) |

### Auto off everywhere
| # | Action | Result | How checked |
|---|---|---|---|
| 99 | Default, `wisp router frob` | P | `false`; "… Auto is off (runtime.auto), so the router is put away" (exit 2) |
| 100 | Launcher picker | P | 7 harnesses, Factory, "Set up another agent…"; no Auto |
| 101 | ⌘K | P | No Auto entry for "auto", "harness" or "model" |
| 102 | Crew editor harness | P | As New task and the 7 harnesses |
| 103 | All settings | P | 28 |
| 104 | Settings search | P | "auto" and "model": no Auto hit |
| 105 | Setup / formula | P | No `router:` in Setup or `wisp export` |
| 106 | CLI | P | 99. A formula's `agent: auto` is GS6 |

### Cross-cutting
| # | Action | Result | How checked |
|---|---|---|---|
| 107 | Restart | P | `wisp export`, `settings list --json`, `plugin`, `cloud status` identical before and after; the machine goes error → connecting |
| 108 | WebKit 1280, home + 20 panes | P | Light and dark: no overflow, no console errors, no failed requests. The probe's "wraps" are 2-line list rows and one-line 44 px buttons |
| 109 | Chrome 390 | P | F9 fixed |
| 110 | WebKit 390 | P | The same |
| 111 | Harness check at start | P | Restarted 12:01:50; the first `npm view` at 12:02:50, then every ~10 min |
| 112 | `wisp-grid dev --help` | — | Not re-run |

### Re-checks of C's, D's and E's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 113 | Manage projects counts | P | "/tmp/qaG2/repo · 15 tasks", "repo2 · 1 task", "repo3 · 0 tasks" |
| 114 | Setup → Import errors | P | A YAML error and `hello` plan nothing; toast "pasted.yml: a formula is a map …" |
| 115 | Plugins → Install errors | P | Five messages as F, fb 30–35 ms |
| 116 | `export --format xml` | P | "✗ no format xml: yaml, json or toml" (exit 2) |
| 117 | Project colour | P | 9 colours, fb 39 ms; `color: amber` |
| 118 | Nested CLI output | P | `channels status` has `transport` |
| 119 | ⌘K "model" | P | Agents first |
| 120–121 | Sign-in box steps, Copy the code | P | "Copied" at 35 ms, back by 1.8 s |
| 122 | Paste the code, Continue | — | The fake has no paste step |
| 123 | A cancelled first sign-in | P | Codex signed out (auth file moved aside) → Sign in → Cancel at 1 s: fb 43 ms, "Signed out · Sign in" @150 and at 6 s |
| 124 | A first sign-in cut by a restart | P | Sign in → restart → Re-check: "Signed out"; no auth file written |
| 125 | A first sign-in that times out | — | 15 min; not waited |
| 126 | Harness Sign out that stays signed in | P | "Codex is still signed in after `codex logout`: run it in a terminal to sign out there." |
| 127 | Formula with `off` values | P | "Changes 2 · runtime.leanRuns all → off · worktrees.max 80 → off", Apply enabled |
| 128 | v1 recipe | P | Automations: "Recipes · Run tests · Manage" |
| 129 | `wisp ls` with no project | P | Cast task reads `no project` |
| 130 | Dark at 1280 and 390, both engines | P | 108–110 |
| 131 | MCP with Auto off | P | 64 |
| 132 | Update all (3) | P | fb 54 ms; one toast each |
| 133 | Personal / work switch | — | Not clicked this round (the item is in the menu) |
| 134 | A signed-out account's menu | **E8** | Still offers Sign out |
| 135 | Add connector errors | P | "A name, so agents can tell its tools apart" (fb 30 ms); "The command that starts it"; "It wouldn't start: definitely-not-a-command-qa: not found" |
| 136 | A CLI launch in a project | P | rec cwd `home/worktrees/repo2-<id>` |
| 137 | Add machine bad addresses | P | Add disabled when empty; toast for "has spaces here" and "qa@" (fb 33 ms) |
| 138 | A plan that adds a setting | P | 82 |
| 139 | Add another account → Cancel | P | fb 39 ms; dialog closes; no account added |
| 140 | Default project | P | 17 |
| 141 | Worktrees: Bring in | **GS1** | No offer for the linked `repo3-wt` (nor a new `repo3-wt2`), even after a restart |
| 142 | Harness Sign out toast | P | 126 |

### Re-checks of F's rows
| # | Action | Result | How checked |
|---|---|---|---|
| 143 | v1 connector env | P | F4 |
| 144 | Every built-in off and on | — | Only learn and worktrees this round (F's sweep left wisp.settings off for 20 minutes) |
| 145 | An off plugin's CLI | P | F1 |
| 146 | Settings plugin off | P | F7: can't go off |
| 147 | Worktrees count after off/on | P | F6 |
| 148 | `wisp update` on a clone | P | F5 |
| 149 | `update view/check/install` | P | F10 |
| 150 | A channel on and back off | P | F8 |
| 151 | `connectors remove <id>` | P | F11 |
| 152 | `update apply` on a clone | P | exit 1 |
| 153 | Phone switches | P | 6 cards: 5 flip @100 (fb 31–42 ms) and back; Email disabled |
| 154 | Grid stopped and started | P | 51 |

### New this round
| # | Action | Result | How checked |
|---|---|---|---|
| 155 | An agent's connector card on the task page: Show all, Allow (UI) | P (GS13) | "Wants to change your setup · … · needs AGENT_KEY from you · It would run `node …` · Show all (the JSON) · Allow · Deny"; Allow → `agent_echo` added, the agent reads "The user allowed it. Added agent_echo …" |
| 156 | The agent answers its own card | P | 403 "This card is the user's to answer: …" |
| 157 | Crew card, `wisp reply <id> Allow` | **GS3** | "✓ You allowed: add crew member Qa Reviewer"; the member's `cwd` is the asking task's worktree |
| 158 | Project card, typed deny from the CLI | P | `wisp reply <id> "not this one, too risky"` → "✓ You denied: … (“not this one, too risky”)" |
| 159 | Crew card, typed deny in the reply box (1280) | P | The agent reads "The user said no: … They said: “no remote servers please”." |
| 160 | Project card for a project that exists | P (note) | The card asks; Allow says "The project repo4 is /tmp/qaG2/repo4" |
| 161 | A card at 390, light and dark | P | Rendered, no overflow |
| 162 | A card that went away unanswered | **GS5** | Seen once in 4 cards |
| 163 | Telegram: who may answer a setup card | P | Linked group chat (person 1001). The stranger's (2002) "Allow" reply: "That's a change to your setup: answer it in wisp …. Nothing changed."; the linked person's: "Answered.", member added |
| 164 | Question notifications to the phone | **GS2** | Telegram and ntfy carry the question and no buttons |
| 165 | Home and sidebar row of an asking task | **GS12** | "No question asked." beside "Asking" |
| 166 | The card's headline | **GS13** | Backticks shown as typed |
| 167 | Desktop self-update, end to end (scratch `.app`, local feed) | P | `wisp update --json`: `method app`, latest 2.0.1 from the feed. Settings → Updates: "wisp 2.0.1 is out. Installing it restarts wisp · Update now" → "Restarting. This page reconnects once wisp is back" @150. Feed served the zip; the old server exited 0 at 12:19:23; the new bundle started at 12:19:25; the old bundle was removed and the new `Info.plist` reads 2.0.1 |
| 168 | Desktop update with a wrong checksum | **GS9** | Refused ("wisp-mac-arm64.zip didn't match the release's checksum. …"); bundle untouched. `wisp update apply` exits 0 |
| 169 | `wisp shell-init zsh/bash/fish` | P | 15–17 lines; `bash -n`, `zsh -n` pass; records cmd, cwd, exit, ms, at, pid in `term/<tty>.jsonl` (0600) |
| 170 | Terminal context on "check the error above" | P | "Last commands … `npm test -- --plain` → exit 3", git status, the "output couldn't be read" line; a `ghp_…` token reads `<secret>` |
| 171 | A secret assigned inline in a command | **GS4** | `DB_PASSWORD=hunter2qa npm run migrate` goes in as typed |
| 172 | A formula with named goals, `tasks.default`, a factory | P | "✓ 8 resources …" |
| 173 | Unknown names in a formula | P | "goals.default: no agent named reviewr: agents has builder, reviewer …", "no goal named green-buld: goals has green-build", "no factory named lin: factories has line (or default)", "factories.line: no agent named critic …", "tasks.default: no agent named buildr …" (exit 1) |
| 174 | `agent: auto` / `harness: auto` with Auto off | **GS6** | Validates; `wisp run t1 --formula` ran it on Claude |
| 175 | Connectors after a restart | **GS7** | All five "Not checked yet" |
| 176 | `wisp connectors add --args A,B` | **GS8** | Refused |
| 177 | `wisp <group> --help` | **GS10** | Uneven |
| 178 | zsh and fish completion | **GS11** | No setting keys or values |
| 179 | F12's docs | **F12** | Module header still stale |
| 180 | `wisp.settings` and `wisp.plugins` switches | P | Disabled in Plugins |

---

## Findings

Severities: **H** = wrong outcome or data, or reaches a real service on its own; **M** = a feature misbehaves, leaks or
loses something; **L** = polish or wording.

### GS1 · M · Settings → Worktrees never offers Bring in, though the server has the offer
- **Steps:** project `repo3` with a linked worktree (`git worktree add ../repo3-wt`) → Settings → Worktrees, wait,
  restart, wait again (4+ minutes). It was the same for a second one, `repo3-wt2`.
- **Actual:**
  - The pane shows "15 worktrees · 172 KB" and the settings, with no "1 worktree from git … Bring in" (F's action 141
    passed after a restart).
  - `worktrees_offer` in the store stays empty.
  - The offer is in memory, though: `wisp worktrees bring-in repo3 --paths /tmp/qaG2/repo3-wt` created the task. `bring`
    only creates from `st.offers`.
  - `worktrees.view` always answers `looking: true` (by design, `commands.rs` ~165); the rows' `looking` is 0.
- **Where to look:** opening the pane runs `look_for_offers` and then `refresh` (`commands.rs` ~165–172), so the offer
  rows should have been written. Either `pane::publish` failed (its error goes only to `tracing::warn`, and this
  server's log had none) or the rows it writes leave the offer out. No cause was found from outside.
- **Expected:** the offer shows once the look ends, as in F.

### GS2 · M · A question sent to the phone has no answer buttons (ntfy, Telegram)
- **Steps:** Phone → Telegram (or ntfy) on, "Send me: Questions" → an agent asks with options ("Ship to staging or
  prod?", Staging / Prod), or puts up a setup card.
- **Actual:**
  - Telegram got `"? qa G mcp reach\n\nShip to staging or prod?\n\n↩ Reply to this message …"` with no `reply_markup`.
  - ntfy got `{"title":"? qa G mcp reach","message":"Second question: A or B?"}` with no actions.
  - The ntfy card says "Tap a button on the notification to answer".
  - `plugins/channels/host/src/lib.rs:291` sends the `asks` notification with `Vec::new()` actions. `report.rs`
    (`message`, used for a task that came from that channel) turns the options into buttons, "v1 `message`: an ask's
    options are answer buttons".
  - A typed reply still works (163). ADR-0113 says the channel's and the phone's signed taps carry the card's id.
- **Expected:** the ask's options (and a card's Allow / Deny) as signed buttons, as for a channel-started task and v1.

### GS3 · M · A crew member an agent adds gets the asking task's throwaway worktree as its folder
- **Steps:** a task in `repo2` (worktree per task) → its agent calls `runtime.setup.ask` for `crew.agent.add` with no
  `cwd` → Allow.
- **Actual:**
  - The card says "cwd: /tmp/qaG2/home/worktrees/repo2-01a1f89436a675afa49b59da6bb3b9da" and `crew.list` stores it.
  - That folder goes when the task is Done ("Remove clean ones") or idle 7 days (Settings → Worktrees). The member's
    check-ins and jobs then have no folder.
  - `plugins/runtime/core/src/setup.rs:150`: "the task's own folder unless given".
- **Expected:** the task's project folder (or the folder it was started from), never a path under wisp's worktrees.

### GS4 · M · Terminal context sends a secret typed inline in a command
- **Steps:** `eval "$(wisp shell-init zsh)"` → `DB_PASSWORD=hunter2qa npm run migrate` (fails) →
  `wisp "check the error above" --show-context`.
- **Actual:**
  - The context block lists `` `DB_PASSWORD=hunter2qa npm run migrate` → exit 3 ``, and it is in the task's prompt.
  - `curl -H "Authorization: Bearer ghp_…"` reads `Bearer <secret>`.
  - `crates/wisp/src/termctx_cli.rs` `scrub` masks secret shapes and the values of the CLI's own secret-looking env
    vars, not a `NAME=value` assignment whose name is secret-looking.
  - ADR-0023 promises "the value of a secret env var" is redacted.
- **Expected:** `DB_PASSWORD=<secret>` (any `*PASSWORD*`, `*TOKEN*`, `*SECRET*`, `*KEY*` assignment), as the share
  scrubber does for env names.

### GS5 · M · Seen once: a setup card went away unanswered; the agent waits forever and the user's words ran as a follow-up
- **Steps:** (seen once in four cards) an agent asks for `connectors.agent.add` with `url: https://mcp.example.invalid/mcp`
  → `{askId: …b8647a…}` → about a minute later, open the task at 390.
- **Actual:**
  - No card on the page (`g-card-390-light.png`); the task read Finished.
  - Typing "no remote servers please" + Enter started a new Claude turn with that text (rec `stdin`).
  - `runtime.get` showed `ask: null`. `runtime.pollAnswer` for that askId still says `pending`, so the agent waits for
    good.
  - Not reproduced with the same card three times after (each stayed up 2+ minutes, at 390 too).
- **Expected:** a card is only taken back with an answer the agent can read (as G21 does at a restart: "its card … was
  taken back"), and never silently.

### GS6 · L · With Auto off, a formula still takes `auto` as an agent
- `agents: { a1: { harness: auto } }`, `tasks.default: { agent: auto }` and `tasks: { t1: { agent: auto } }` all
  validate.
- The refusal for an unknown agent lists "(or claude, codex, …, copilot, auto)".
- `wisp run t1 --formula auto3.wisp.yml` ran it on Claude without a word.
- E6 refuses `router:` in the same state ("Auto is off (runtime.auto) …").
- **Expected:** the same refusal for `auto`, and no `auto` in the lists, while `runtime.auto` is off.

### GS7 · L · After a restart every connector reads "Not checked yet"
- Before the restart: "1 tool" ×4 and qa_bad's "It wouldn't start: …".
- After the restart, `connectors.list` has `test: null` for all five, and the pane (open for minutes, Chrome and WebKit)
  reads "Not checked yet" for each. Nothing checks them again.
- A failing connector looks fine again, and every run still gets it (61).
- **Expected:** keep the last result, or check each on start or when the pane opens.

### GS8 · L · `wisp connectors add --args A,B` can't set a connector's arguments
- `--help` lists `--args A,B`.
- `wisp connectors add --name qa_tmp --command node --args /tmp/qaG2/tools/mcp-echo.mjs` gives "✗ --args takes a JSON
  object, not …" (exit 2): the connector's `args` field collides with the CLI's `--args JSON` escape.
- `--command "node /path"` works.
- **Expected:** the field gets another flag name, or help doesn't offer `--args A,B`.

### GS9 · L · A failed desktop update exits 0 from the CLI
- On the scratch `.app` with a feed whose digest doesn't match, `wisp update apply` prints the whole view, its `error`
  being "wisp-mac-arm64.zip didn't match the release's checksum. …", and exits 0.
- On a clone the refusal is "✗ …" with exit 1.
- **Expected:** "✗ <the error>" and a non-zero exit.

### GS10 · L · `wisp <group> --help` is uneven
- `connectors`, `plugin`, `channels`, `crew`, `worktrees`, `learn`, `goal`, `factory`, `formula`, `projects`: `--help`
  prints the whole 100-line usage.
- `settings --help` prints "wisp setting needs more: wisp --help".
- `machines`, `cloud` and `update` print their one usage line.
- `wisp help <group>` lists each group's verbs.
- **Expected:** `wisp <group> --help` = `wisp help <group>`.

### GS11 · L · zsh and fish completions are thinner than bash
- bash completes `settings get|set <key>` and the key's values (`off 20 40 80`). zsh and fish complete neither (no
  setting key in either file).
- zsh describes every first word as "a wisp command".
- **Expected:** the same completions on all three. zsh is the macOS default.

### GS12 · L · An asking task's row reads "No question asked."
- Home → Needs you and the sidebar: "qa G mcp reach · No question asked. · Asking" while a setup card or a question is
  open (`g-card-conn.png`).
- The sub-line is the last turn's answer.
- **Expected:** the question (or the card's headline) under an Asking row.

### GS13 · L · A setup card shows Markdown backticks
- Task page: "Claude Code wants to add the connector `agent_echo`: runs `node /tmp/qaG2/tools/mcp-echo.mjs --agent`",
  with the backticks as typed. The same text goes to Telegram.
- The card shows the command in mono on its own line already.
- **Expected:** the headline rendered (mono for code) or written without backticks.

---

## Smaller notes (not counted)
- **Setup cards:**
  - A project card is made for a folder that is already a project.
  - After a card is allowed or denied, the task page keeps no trace of it.
  - The connector an agent adds exports as `AGENT_KEY: ""`, not a `${secret:…}` reference.
- **A second plain question replaced the first open one** without a word ("Ship to staging or prod?" → "Second
  question: A or B?"). The first asker's poll would read cancelled.
- **Plugins lists `router` · On** while Auto is off (deliberately put away; the row is still there).
- **`wisp run <task> --formula F -d`** is refused ("t1 takes no values: "-d" is left over"); `rerun` and `fresh` take `-d`.
- **`wisp connectors test <id>`** prints the connector list, not the tools.
- **v1 import:** the connector's display name is lost ("Old Echo" → `old-echo`, as in F).
- **`learn view` while off** says "wisp.learn is switched off (wisp.learn was switched off)".
- **`accounts.list` (Codex):** Main reads `ready` while the harness row reads Signed out.
- **A task launched on a signed-out Codex** ran with no warning (the fake ran; a real one would fail).
- **Still as in F:**
  - ntfy Test has no on-screen confirmation.
  - `settings set` prints the whole record.
  - Esc in Edit closes the whole Manage dialog.
  - Connectors' `added` is 0.
  - `computer off` prints the address.
  - A Usage select put back writes its default explicitly.
  - GitHub's Agents row has no state with the fake `gh`.

## Cleanup done
- **Settings:** each was put back right after its change, except `settings.editor` (zed, from action 30) and the v1
  import's values.
- **Channels:** Telegram, Slack, Discord and GitHub are switched off and their tokens removed. ntfy is off.
- **Plugins:** `user.qa-theme` is off; QA Pink is removed; every built-in is on.
- **Cloud:** this computer is off, every account is signed out, and every API key is revoked.
- **Setup cards:** every open card was answered.
- **Stopped:** `wisp serve` :4981, the scratch app's `wisp serve` :4986, the feed :4985, `wisp-grid dev` :4982 (and its
  box), the catcher :4989 and headless Chrome :4984. No process under `/tmp/qaG2` is left.
- **Kept for re-checks:** `/tmp/qaG2`.
  - Binaries in `fake/` and `bin/`, both homes, the repos (including `repo3-wt2`), the grid data, `pkg/` and `feed/`,
    the shots.
  - Scripts in `pw/g` (this round's), `pw/f`, `pw/s` and `tools/` (`catch.mjs` with the fake Telegram, `feed.mjs`,
    `serve-app.sh`, `agentcmd.py`, `ctxtest.sh`).
  - The scratch home also holds the crew members Qa Reviewer and Tg Member, the connectors, the machine `qabox`
    (TEST-NET) and about 30 fake tasks.
- **Not deleted:** `/tmp/wisp-target-fixer` (the shared build dir).
