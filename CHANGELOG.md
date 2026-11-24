# Changelog

All notable changes to `wisp-agents` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0-alpha.11] - 2026-11-24

### Fixed

- Subagents & Chat mode: wired `SUBAGENT_ASK` into delegated subagent system prompts so subagents know they report to their lead and cannot ask the user directly; updated the lead system prompt (`team_note`) and `delegate` MCP tool description so main agents use wisp's `delegate` tool (creating subagents on the fly) instead of built-in subagent tools even when no team is pre-configured; and surfaced delegated agents inline in Chat mode (keeping `agent` steps visible with the `agent` icon and rendering clickable live delegate cards on each turn).
- Updates (relaunch): self-update relaunch (`wisp.update`) now clears `WISP_WEB_DIR` from the spawned child environment so the relaunched launcher serves its own matching `web/` bundle instead of inheriting an older `npx` cache's `WISP_WEB_DIR` (which caused UI/server version mismatches such as `no plugin declares the setting settings.chatWidth`).
- New task (Factory picker): **How factories work** now opens `https://trywisp.dev/docs/concepts/factories/` instead of the nonexistent `docs.trywisp.dev` subdomain.
- Docs (guides): added a dedicated **Workflows** guide (`docs/site/guides/run/workflows.mdx`) and **Workflows** concepts page (`docs/site/concepts/workflows.md`) covering DAG steps, `needs`, `${{ … }}` outputs, `if` conditions, `matrix` fan-out, `inputs`, retries, timeouts, concurrency, budgets, triggers and Desktop app / Phone app / TUI / CLI usage, and added the missing `Run a task through a software factory`, `Workflows` and `Run a task under a formula` cards to `docs/site/guides/index.mdx`.

## [1.0.0-alpha.10] - 2026-11-23

### Added

- Usage & Settings → Usage (every harness): `runtime.usage` now aggregates total, input, cache and output tokens plus a per-harness breakdown (`harnesses`) alongside projects and `harness · model` rows; the **Usage** dialog adds a **By harness** table with harness logos, task counts and token breakdowns, a **Tokens** column across every table, and a daily chart metric switcher (**Spend** / **Tokens** / **Turns**); **Settings → Usage** is organized into **Plan budget**, **Harnesses** (7-day activity and per-harness stop lines for all 7 harnesses: Claude Code, Codex, Antigravity, opencode, Cursor, Copilot, and Grok, plus Claude's subagent model), and **Checks and notifications**; and Copilot (`--usage-output-file` and `result.usage`), opencode, Cursor, and Grok parsers now record cache and token usage fields when reported.

### Fixed

- Checks, budgets & CI stability: sped up `npm run check` by ~2.3× (caching Rust workspace discovery, file reads, stripped code lines, TS comment ranges, and top-level import extraction); coalesced running tasks' live-step row updates (`/summary/live`) onto the shared 100 ms activity coalescer instead of opening separate SQLite transactions per task step (cutting stream WAL write amplification back under the 44× ratchet); deferred `wisp.stuff.homePanes` with `drawnLater` and split `StoryViewer` out of `Stories.tsx` to bring desktop (156.9 / 173.0 KB br) and phone (165.0 / 167.0 KB br) first-screen JS back under the ADR-0105 ratchet; made the first POST on an idle peer synchronous in `@wisp/client` while keeping in-flight POSTs strictly FIFO; preloaded `TaskView` and core task/list extensions in `apps/web/test/home-chunk.ts` to prevent cold dynamic-import timeouts on Windows CI; closed the `runtime.release` / `hold_turn` race in `wisp-runtime` and the two-round detection barrier race in `detect.rs`; added missing `lazy.ts` i18n imports and the root `test:real` script; and bumped `url` 2.5.9 / `form_urlencoded` 1.2.3 across workspace and spike lockfiles (`RUSTSEC-2026-0009`).
- Task view (Chat mode): when **Transcript** view is off, contiguous runs of actions (reads, edits, searches, commands, thoughts) between agent messages now fold inline into a compact pill button on the timeline rail (e.g. `Edited 1 file · Read 3 files · 1 command` with an action count badge and chevron) both while running and when finished, so users see what is happening between messages and can expand any pill in place to inspect the actions taken.
- Task view (Chat mode): **Transcript** view (steps) is now off by default in Chat mode (showing the agent's messages, errors, and final answer, with the activity summary pill in the trail), and its on/off state is persisted per task alongside the task's scroll position in `shell.places` so switching tasks or reloading restores both the same view and scroll position.
- Task view: fixed auto-scroll dropping out of `following` when already at the bottom. Programmatic bottom pins (`pin()`, `toEnd()`, `place()`, and `turns.grew()`) now update `lastTop` and sync the observed `[data-kit="turns"]` height at the moment of the write, so asynchronous `scroll` events and `ResizeObserver` callbacks that fire after subsequent streaming growth or routine-step folding never compute a false negative `moved` delta and pause auto-scroll. Also auto-arms `turnFollower` when content grows while following before the initial `requestAnimationFrame` callback fires.

## [1.0.0-alpha.9] - 2026-11-23

### Fixed

- Phone app + wisp Cloud: a cloud computer no longer flips online/offline several times a second. The box refused every sync write from the phone (its webview's `Origin: https://localhost`, 403 `forbidden origin`), and each refusal reopened the stream at once. The grid now forwards a device token's or API key's request without the caller's Origin (a bearer token can't be sent cross-site), and a peer that keeps refusing writes is reopened on the 2/5/15/30/60 s backoff with its stream left open instead of in a loop.
- wisp Cloud: a new account's box opens again. Since the v2 grid went live (2026-11-21) Caddy's certificate check (`/_grid/tls-ask` on `127.0.0.1`) got a 421, so no account made after it got an HTTPS certificate and its wisp stayed offline. The check is also no longer answered for requests from outside.
- Task view (Chat & Transcript): finished turns now fold runs of 3+ routine exploration steps (reads, searches, commands, thoughts) into a single quiet expandable row (with compact file chips and coalesced duplicate labels) while keeping edits, errors, and mid-turn messages directly visible; Antigravity's `read_file` tool now reports `Reading <path>` instead of generic `"Using read file"`.
- Task view (**Needs your call**): `question` blocks now render in normal chat flow at the end of the answer instead of sticking above the reply box and covering the transcript.
- Task view (Chat mode): audited and fixed auto-scrolling across opening, streaming, turn completion, follow-ups, wheel/trackpad, touch, and scrollbar drag. Tasks in Chat mode now land pinned at the bottom on open (unless restoring a saved mid-thread reading position); follow-ups and reply chips coordinate with the follow state machine instead of writing raw `scrollTop`; finished turns stay pinned to the bottom instead of stopping mid-answer; downward scrolls, horizontal trackpad swipes, plain clicks, and non-scrollable wheels never pause auto-scroll; small upward scrolls/drags are never fought; the bottom re-attach zone widens from 16 px to 48 px; and touch drags and scrollbar thumb drags are tracked explicitly.
- Composer (Home): returning from a task with a right-panel view tab open (Thread, Watch, Terminal, Team, or a pinned My Stuff app) now closes the side panel instead of leaving an empty panel column taking up the right side of the composer page.
- Task view (Chat mode): hardened trailing-message deduplication against `body.text` (not only `turn.answer`), `MAX_TEXT`-clipped (`20,000` UTF-16 unit) final messages, and activity-only fallback turns.

## [1.0.0-alpha.8] - 2026-11-23

### Fixed

- Task view (Chat mode): a finished turn no longer shows its last message twice, once as the last step and again as the answer under it. The check compared the message with only the answer's last paragraph, so it missed every answer carrying a wisp block; it now compares the whole answer.
- Task view (both modes): following the end is now an explicit state, as in t3code and opencode. Only you leave it (a wheel or key up, dragging or touching away from the end, selecting text while output streams), so a streamed chunk never pulls you back while you read. Coming back near the end, sending a follow-up or **Jump to latest** follows again. While following, the end is pinned in the same frame the page grows (no jump-then-catch-up).
- Task view (both modes): **Jump to latest** (↓) shows whenever you've scrolled up from the end (after 150 ms), not only half a screen away, with a dot once new output arrived. It now jumps at once instead of animating, which could read as leaving the end again.
- Task view (Chat mode): **Transcript** (V, and in the ⋯ menu) is back as a switch: on shows the steps as *Tool calls in chat* sets them, off shows only the agent's messages and the answer, with the activity summary under it.

## [1.0.0-alpha.7] - 2026-11-23

### Fixed

- Task view (Chat mode): when a turn finishes while you follow the end, the view no longer jumps back up to the turn's start (it could be screens up). It stays at the end, or pages down one view when the answer is taller than the screen, so it reads from where you were.
- Task view (both modes): scrolling away from the end stops the view following new output, and a round **Jump to latest** button (↓) appears over the reply box, with a dot when new output arrived meanwhile. A follow-up still takes you to the end.
- Task view (Chat mode): the working orb sits on its status line's centre and the step rail runs down into it, instead of floating a few pixels high and apart from the steps.
- Task view (Chat mode): **View transcript** in the ⋯ menu and the **V** key are hidden in Chat mode, where the chat already shows every turn's steps. Before, they stayed ticked and did nothing. Cards mode keeps them as before.

## [1.0.0-alpha.6] - 2026-11-23

### Added

- Usage (Antigravity): added Antigravity plan quota support using `agy -p "/usage" --output-format json` (`Harness::live_limits`, non-blocking post-turn and stale-view refreshes, `autoStopAntigravity` stop line, Home usage bar, Settings → Usage live updates, Spare allowance, and `cache_read_tokens` capture).
- Settings → **Chat**: replaced **Full transcript** in General with a dedicated **Chat** settings pane featuring **Chat mode** (tasks render as a continuous chat with live messages, thinking, and tool calls without an end-of-turn report, while **wisp blocks** stays an independent toggle in General so agents can still embed blocks inline inside chat responses) along with **Tool calls in chat** (`Show all` / `Hide successful` / `Hide all` / `Zen`), **Show thinking**, **Follow-up behavior** (`Interrupt` / `Queue`), **Send shortcut** (`Enter` / `⌘+Enter` for multiline / `⌘+Enter` always), **Chat width** (`Comfortable` / `Wide` / `Full`), and **Time format** (`System default` / `12-hour` / `24-hour`). **Chat mode is on by default**; switch it off in Settings → Chat for the Cards view (end-of-turn report).

### Fixed

- PR card: audited and refined the PR card for merged, closed, and multi-PR tasks. A single merged or closed PR now puts its GitHub link inline in the header row instead of opening a mostly empty 32 px action bar below it; multi-PR chips omit the redundant repo name when all PRs belong to the task's own repo and keep `#number` intact when a long repo name truncates (in both the card and the PR side panel); `+N more` omits its status dot when all hidden PRs are merged/closed and `Show less` renders as a plain ghost toggle instead of a status chip; the card adopts v1's subtle tone-tinted surface; and clicking the row's chevron on desktop hover now opens the PR pane instead of being intercepted by `list-row-actions`.
- wisp Cloud (grid hostd): a cloud box whose process or container exits unexpectedly (crash, OOM, or boxd exit) is now reaped immediately to `stopped` instead of remaining stuck in `running` on a dead port returning `502 Your wisp didn't answer: client error (Connect)` until manually stopped; computer addresses also answer `/_grid/state` like v1 so offline watches reconnect when a computer returns.
- Cloud computers & grid machines: connecting or clicking **Wake** / **Try again** on a cloud computer now sends a non-passive `/api/health` request so sleeping boxes actually wake instead of looping on `503 asleep`, reports the grid's real error or sign-out hint instead of SSH's `"The tunnel opened, but wisp doesn’t answer through it."`, and watches offline `grid-tunnel` computers and sleeping boxes cleanly without hanging.
- Site and docs (`trywisp.dev`, `trywisp.dev/docs`): standardized branding on **Wisp Agents** across page headers, footers, titles, Open Graph/Twitter cards, social preview image (`og.png`), JSON-LD structured data, and `llms.txt`; fixed SEO audit gaps including `noindex` on `/docs/404.html`, `BreadcrumbList`/`TechArticle` JSON-LD on all docs pages, `SoftwareApplication`/`CollectionPage` structured data, SERP title/description lengths, `/cloud.html` heading hierarchy, and IndexNow coverage for `/docs/` URLs.

## [1.0.0-alpha.5] - 2026-11-22

### Added

- Ship it: when the repo has a `.wisp/` folder, the Ship it dialog shows a **Save task to repo (.wisp/)** checkbox; ticking it saves the task's spec, report and scrubbed transcript into the worktree right before committing so it ships inside the PR.

### Fixed

- Crew: fixed `Unknown block · crew` cards showing at the bottom of crew job answers and wired the rest of v1's crew turn-end handling. `{"type":"crew"}` blocks are now stripped before saving (and hidden in the UI for already-saved turns), `crew.topic` names turn 1, `remember`/`notebook`/`wakeIn` update the member's notebook and next wake-up, `forLater: true` persists on the task row (`by: "crew"`), finished jobs append to `member.log`, and `followUp` messages reach linked tasks and join their delegates.
- Sync, live updates & direct follow-ups (desktop, web & mobile): fixed multi-topic duplicate `Patch`/`Append` delivery on `session` rows (`list` + `session:<id>`) that triggered `drifted("list")` and a concurrent `POST unsub`/`sub` race permanently unsubscribing `"list"` until reload; flattened live `ChangeOp::Append` event arrays in `TaskView`, `Compare`, and Team subagents; serialized per-peer `POST /api/events` calls with error recovery; fixed non-atomic multi-page `list` snapshots, stale socket backpressure (`v2_actor`), `SubState::Waiting` 15 s stalls, zombie connection leaks, WebKit HTTP/1.1 SSE abort cleanup, and foreground/online/tunnel-ready reconnection on desktop and mobile; fixed direct (non-queued) follow-ups being blocked by `from_queue` or failing to interrupt once a starting turn becomes resumable; fixed `TurnTranscript`, `Panes.useSession`, `flatSession` column precedence, `error` tasks in `sections`, `ChangesPane`/`PrPane`/`FilePane`/`WatchOpen` live updates, `wisp.later` firing/reminders/plans/`replyPlan`, `wisp.goal` inner turns, and `wisp-store` retention/multi-page delta/read-guard bugs.
- Repo history: fixed **Save to repo** silently writing into `~/.wisp/tasks/` instead of the project's `.wisp/tasks/` when a task's worktree had been cleaned up or its repo had no `.wisp/` yet (`root_of` walked up to `$HOME` and matched wisp's own app data directory). Worktree tasks that are merged, cleaned up, or have no unshipped work now save directly into the main repo checkout so they show up in Repo history immediately.
- Repo history: rebuilt the Repo history page in a scrollable `PageShell` with a Back button on phones, centered reading width on desktop, a Refresh action, and multi-peer routing so remote/mobile projects load from the computer that owns them.
- Repo history (mobile & desktop): **Save to repo** now matches `turn` rows on `session_id` (not only `body.sessionId`), stays in **Keep and copy ›** on phones, and shows a confirmation toast when saving completes even if scrolled away from the bottom of the turn.
- Scheduled tasks: a task whose agent scheduled a follow-up with `resume_later` now moves from Needs you to Scheduled (and shows its scheduled follow-up above the reply box), as in v1. `wisp.later` forwarded the agent's own call to `runtime.setExt`, which is denied to `agent:session`, so the follow-up was saved and still fired on time but never mirrored onto the task row; mirroring now speaks as `wisp.later` (`ctx.root_call()`), and *Send now* (`later.edit` with `now: true`) clears the mirror immediately after firing.
- PR card: fixed multi-PR, empty-toolbar, mobile, and overflow issues on the open task's PR card. A task with several PRs now shows a single summary row (`N PRs` with aggregated status) and every PR as an aligned, truncating chip (collapsing past 5 into a bounded `+N more` toggle), hides its action bar when there are no actions (no empty gap on merged/closed PRs), deduplicates repeated PR entries, keeps the chevron always visible and clickable, preserves red error tint on collapsed review icons, wraps in-flight Ship stage pills, and removes double padding and toolbar clipping on Android and iOS.
- Phone apps (Android): every pushed page (a task, Settings and its panes, a crew member) shows its title right beside the back arrow. The bar was M3's medium one, which put the title on a row under the arrow and hid the inline copy until the page scrolled.
- Phone apps: My Stuff's bar sits right under the status bar like Tasks and Crew. The page padded the status bar's height a second time inside the frame, which already starts under it, leaving a blank band above the header.
- Phone: tapping a suggested follow-up goes to the end of the task. The chip filled the reply box but left the view where the chip was, so the box and what you were about to send sat out of sight; the view now scrolls to the bottom, as sending one with ⌘-click already did.
- Phone app: Tasks waits for a computer. Its tab is dimmed until you open a computer (or wisp Cloud) and it connects, so the app no longer shows an empty task list for no computer; opening one that is still connecting keeps you on Computers and opens its tasks once it is online, and Back on Computers leaves the app instead of landing on Tasks.
- Phone app: every picker in New task opens the same way Delegate does. Mode, Runs on, More (⋯) and Start later were
  anchored dropdowns, and the harness and model picker was a page with a Back row; each now opens as a bottom sheet
  with a handle, its own title and ✕, on Android and iPhone alike. The picker's "Set up another agent…" chip no longer
  wraps one word per line.

## [1.0.0-alpha.4] - 2026-11-21

### Fixed

- Phone app: attaching a file works. The app's connection to a computer had no way to call a plugin's own routes, so every attachment failed on send with "can't be reached that way" (and so did the phone's terminal stream); a file now goes up to the computer like on the desktop.
- Phone browser: a task is one fixed page again. The app was sized to `100vh`, taller than the screen under the browser's URL bar, so the whole page scrolled and took the header and the composer with it; it now fills the visible height, the header and composer stay put and only the conversation scrolls.
- Phone app (Android): a long task title in the task page's header stays on one line and ends in "…" instead of breaking onto a second line.
- wisp Cloud: the phone app signs in again. The v2 grid answered no CORS on `/api/`, so the app (another origin) could not ask for a sign-in code or reach an account it had already added; it now answers v1's open CORS there, on the grid, a box and a computer.

## [1.0.0-alpha.3] - 2026-11-21

### Fixed

- **An earlier turn's "1 file +71 −55" ends at the right edge.** Its action icons are hidden until you hover the
  turn but kept their room after the files, so the counts stopped an icon row short of the edge. The hidden icons
  now sit before the files, and showing them on hover moves nothing.
- **A compare block keeps long text inside its box.** A long URL or path in a Before/After item ran past the card's
  edge; it now breaks within the side.
- **Transcript on a working task reads like an agent's log.** With Transcript on, a running turn showed its steps
  and then the boxed working panel under them, repeating its last five steps. The steps are now the log, closed by one
  live line (the orb, who is working, how long, how many actions, Stop), as Claude Code and Codex show their work.
- **A follow-up sent goes to the end.** Sending a follow-up (typed, or a chip, question option or action reply) while
  scrolled up left the page where it was, so the new turn and its working panel came in out of sight. The task now
  scrolls to its end and shows the new turn whole.
- **Phone app: grid computers connect.** A computer reached through wisp Cloud stayed on its loading skeleton and
  *Offline* in the v2 phone app: the grid answered the app's CORS preflight on the computer's address with 401 and no
  CORS headers, so the WebView refused every request. A computer's and a box's `/api/` now answer it like the grid's
  own device API (open CORS, bearer token, never cookies).
- **Phone app: a grid computer's tasks open.** The task list showed but a task stayed empty: the app's sync posts carry
  `x-wisp-client`, which the grid's CORS didn't allow, so no task was ever subscribed. The grid now allows the headers
  `@wisp/client` sends.
- **A task with several PRs: the PR card holds them.** The other repos' PR chips sat on the card's one row, which never
  wraps, so in the side column they pushed the PR's title out and ran past the card's edge. They now sit on their own
  line under it and wrap.
- **Crew: Share link and Download work.** Share link toasted a bare "not found": wisp Cloud still ran v1's grid, which
  has no place for v2's shares. wisp Cloud now runs the v2 grid, and it takes the crew's `crew:<id>,<id>` share key
  (it refused "Bad share key."). A grid that still can't hold a link says so and points to Download or Copy.
  Download saved the `.crew.json` without a word in the desktop app; it now says where the file went.
- **Share page: Open in wisp on this computer works.** The button was a `wisp://open/…` link that no wisp registers, so
  clicking it did nothing. It now opens your wisp (127.0.0.1:4477) with the link waiting in New task: press Send.
- **Crew files are YAML.** Share → Download saves `<name>.crew.yml` and Copy copies the same YAML (a brief over several
  lines stays lines), like every other file wisp writes. Import takes `.crew.yml`, and a `.crew.json` from before still
  imports.

## [1.0.0-alpha.2] - 2026-11-21

### Added

- **Save to repo in the app.** When a repo on `history: { track: ask }` offers to save a finished task, its last answer
  shows **Save to repo**; any task can be saved from its **⋯** menu (beside Share…). The answer says what was written to
  `.wisp/` and how many secrets were scrubbed.

### Changed

- **A task's PR card stays in sight.** The PR card (its checks, review, GitHub and Merge) moved from the top of the
  turns to the task's details column on the right, so it no longer scrolls away; with the side panel open it sits under
  the details bar.
- **Opening a task no longer animates its answers.** The blocks used to rise in one after another (and stats count up)
  every time a task opened; now they are simply there.
- **The Android app is the new phone app.** The Play build now ships wisp's own phone app (Tasks, Crew, Stuff and
  Computers, every saved computer connected at once) instead of the old computers screen, as version 1.0.0-alpha.1 on
  internal testing.

### Fixed

- **Android: signing in to wisp Cloud is kept again.** The phone app said *This phone can’t keep your sign-in safely,
  so it wasn’t kept* on every phone: the app had no sealed store for the page (`secureGet`/`secureSet`/`secureRemove`
  were missing on the Android side). The device token is now sealed with the Android Keystore key.

- **A running task's row says what the agent is doing.** It read *Starting up* for the whole turn, even after the agent
  had run tools; it now shows the latest step (*Read src/a.ts*, *Run tests*), at most once a second.
- **A flow diagram in an answer no longer grows without end.** Its cards are laid out as in v1 again, and the arrows
  are drawn over them instead of pushing the answer down, so the task page stops growing and the scrollbar stays put.
- **Start now starts a run held near the usage limit.** A crew check-in or automation held at its account's stop
  line ("Held before it started … unattended work waits for it to reset") was held again the moment you pressed
  Start now, so the button seemed to do nothing. Start now is your yes: the run starts.
- **A screenshot you attach shows on the task.** The files sent with a prompt appear under it again, as in v1: a
  picture as a thumbnail that opens full screen, any other file as a pill that opens it.

## [1.0.0-alpha.1] - 2026-11-21

**First public alpha.** wisp manages the code your agents write. It runs the coding agents you already use (Claude
Code, Codex, Cursor CLI, opencode, GitHub Copilot CLI, Grok Build and Antigravity) with your own plans and keys:

- **Declare** your agents, crew, machines, skills, MCP servers, rules and workflows as files in your repo's `.wisp/`
  folder; `wisp plan` shows what changes and `wisp apply` makes it so.
- **Run** any agent, many at once, each task in its own git worktree, on this computer, your other computers or in
  the cloud: follow up, set a goal with judges, compare two agents, ship the pull request.
- **Track** every task next to the code it wrote: `wisp blame` says which task wrote a line and what it was asked.

It works from the desktop app (macOS, Windows, Linux), the terminal UI, the command line and your phone's browser, and
from your own agent through wisp's MCP tools. It's an alpha: expect rough edges, and
[tell us what breaks](https://github.com/WispAgents/wisp/issues).

### Added

- **Software factories.** Run a task through a line of stations instead of one agent: a spec you approve, a build in
  its own worktree, checks that are real commands (`npm test`, not an agent's word), a review by vote, and a PR you
  merge. A failed check or review goes back to the build as round 2, 3… with what failed; past the limit the run asks
  you. `default` needs no setup; declare your own under `factories:` (several builders or reviewers, `back_to: spec`,
  `merge: auto`, a budget). Start one with `wisp "…" --factory [name]`, `wisp run <task> --factory`, a declared task
  or trigger, `wisp_launch`, or **Factory** in New task's harness picker. The run is one task, with a band showing each
  station, the round, what each gave, and Stop, on the desktop, phone, TUI, `wisp show` and chat channels.
- **Goals by name, and defaults for every task.** A formula declares goals once (`goals: { default: { judges, vote,
  maxRounds }, green-build: { text: … } }`) and a task names one (`goal: green-build`); `tasks.default` gives every task
  its agent, mode, worktree and the rest. Set one with `wisp "…" --goal green-build` (or `--goal-text "…"`),
  `wisp goal set <task> green-build`, `wisp_launch`, or **+ → Goal…** in New task and the reply box. Goal… lists your
  most recent goals first, then **All goals…** (searchable) and **New goal…**; `/goal ` suggests goals as you type, and
  `--goal <TAB>` completes them in bash, zsh and fish.
- **Run with a formula from anywhere.** `wisp "…" --formula f.yml[,g.yml]` runs a task under a formula without
  applying it; `wisp run release --formula f.yml version=2.1` runs a task the formula declares; a task file can point
  at its formula (`formula: ./wisp.yml`); `wisp_launch` takes `formula`; New task's **+ → Formula or task file…** picks
  one. A single formula file reads like a `.wisp/` folder (variables, `count`, modules, `${agent.x}`). Every field that
  names something (a trigger's task, a member's agent, a notification's channel, a task's project) is checked by
  `wisp validate` and before a run, with the names there are and the one you probably meant.
- **Agents ask before they change your setup.** When a task's agent wants to add a connector, change your crew or add a
  project, the task shows a card with exactly what would change and every command it would run, and **Allow** or
  **Deny**. Nothing changes until you answer (on the task page, your phone or your chat channels, where only the person
  who linked the chat can answer); the agent can't answer for you, and it asks in Full auto too. `wisp reply <id> Allow`
  answers from the terminal.
- **Your harness asks before it lowers a safety guard.** When Claude Code (or any harness using `wisp mcp`) asks wisp to
  stop asking before trusting a project's setup scripts, run in your checkout instead of a worktree, show `.env` values,
  let webhooks or chat channels start tasks, or install, trust or switch on a plugin, a task asks you first (Allow or
  Deny). The harness waits and can't answer itself; a question it stops waiting for settles by itself.
- **Ask about what just happened in your terminal.** After a command fails, `wisp "check the error above"` hands the
  agent your last commands, their exit codes, the failing one's output (tmux, WezTerm, kitty, iTerm2, Terminal), the
  folder and git status, with secrets masked. `--context`, `--no-context`, `--show-context` and `--lines N` decide it for
  one run, and `wisp shell-init zsh|bash|fish` records each command's exit code.
- **Long threads stay on track.** Each task keeps Notes (its goal, constraints, decisions, what failed, where it stands,
  open questions, key files), kept by the agent with `wisp_notes`, shown in the Thread tab and by `wisp notes <task>`.
  The agent gets them before every follow-up. **Fresh thread** starts a new conversation on the same task from a brief
  you can edit (`wisp fresh <task> [--print]`). With *Compaction audit* on (off by default), wisp checks after a
  compaction what the agent still remembers ("Kept N of M") and puts what was lost in front of the next follow-up. Long
  Bash output from Claude Code and Codex (in Full auto) goes to a file the agent reads from, and the TUI shows when a
  thread is getting long (`t` opens the Thread pane).
- **Theme plugins.** A plugin's styles (by `data-part`, with token overrides like `--accent` and light or dark values)
  restyle the app while it's on and trusted (`wisp plugin trust <id>`), and stop the moment it's switched off,
  untrusted or removed. Only safe CSS is accepted.
- **Menus on a phone are sheets you drill into.** On a touch screen or a narrow window, every menu opens as a bottom
  sheet, and a submenu replaces the list under a back row. A mouse on a desktop keeps the side flyouts.
- **The desktop app updates itself.** It downloads the new release, checks it against the release's checksum, swaps it
  in and reopens (macOS, Windows and Linux). A copy run from its disk image or Downloads asks to be moved to
  Applications first.
- **A new trywisp.dev.** A page per agent, Get the app (leave your email for the phone apps), wisp Cloud, and the docs
  at trywisp.dev/docs.
- **wisp Grid's pages are simpler.** Home is one list of your machines (the cloud wisp first) saying how each is, with
  Open on the row and the rest in its menu; **+ Add** adds a computer of yours; your published apps and shared tasks are
  together under Links; API keys and account deletion live on Account. An offline computer's page waits and opens it as
  soon as it connects. `wisp computer on --grid <grid>` adds this computer to a grid.
- **Signing in from Settings → Agents** shows numbered steps, an *Open the sign-in page* button and the code with Copy.
- **Ask to fix** on a failing PR check puts the check, its link and the end of its log in the reply box.
- **`wisp ls`** shows your newest 50 tasks (`--limit N`, `--all`, `--crew <name>`).
- **Settings → Agents → Check for newer harness versions** (`accounts.updateCheck`): the check runs once a day and on
  Re-check, not at every start.

### Fixed

**Security and privacy**

- A connector's secret environment and a task's wisp token no longer appear on the agent's command line, where other
  users of the computer could read them.
- A task's agent can't run shell commands on your computer through wisp's tools, delete tasks, set a project's test
  command or call your other computers directly; a command in an answer runs only when you click it. An agent inside a
  task can't change your setup or raise a task's mode: your own harness still can, through `wisp mcp`. A task an agent
  starts never gets more than its own task has.
- Terminal context and shares mask secrets typed inline (`DB_PASSWORD=…`, `--password …`, `mysql -p…`, `docker login
  -p`, `curl -u user:…`, `az login -p`, Vault, npm, pnpm and yarn tokens, openssl passwords, `Authorization` headers,
  `.netrc`, URLs with credentials) while keeping ordinary words like `--max-tokens` or `SSH_AUTH_SOCK` as typed.
- A share hides your folders only as whole paths, lists what it hides before you send it, and no longer blanks ordinary
  words that happen to match another task's title.
- Answers to setup and safety questions from chat channels come only from the person who linked the chat; ntfy shows
  such questions without buttons (answer them in wisp).
- An untrusted plugin's styles never apply; trusting or removing a plugin asks first.

**The command line**

- A typo or a mistyped command never starts a Full-auto task. One word that isn't a command (`wisp login`), a verb a
  command doesn't have (`wisp crew pasue`, "did you mean pause?"), an unknown flag (`--mdoe`, "did you mean --mode?") or
  a bad value (`--effort banana`) is refused with what to type instead (exit 2). Two or more words after a word a
  sentence can start with are still a prompt (`wisp review the auth module`), and `wisp -- "…"` is always a task.
- `wisp help <command>` and `wisp <command> --help` print that command's usage for every command, opening with what it
  does, in plain words; `wisp version` prints the version.
- CLI answers read as sentences: `goal set`, `crew create`, `crew update`, `crew assign`, `factory stop`, `later
  cancel`, `review view`, `trigger log` and others say what happened instead of printing fields or JSON; lists and
  nested values print as lines.
- `wisp reply` keeps quick follow-ups in order, each run to the end, and says when a reply interrupts the running turn
  (`--queue` waits behind it).
- `-d` and `--wait` work on every `wisp run` form; `--wait` and `wisp wait` wait through a goal's judging.
- `wisp import <folder>` reads a relative folder, carries connectors' environment and headers, ntfy, and your
  settings, keeps New task's default harness, and says when it couldn't bring something in.
- `wisp update` never says "latest" without checking, says why an install failed, and explains when checks are off or
  wisp is a git clone.
- `wisp "…" --agent claude --compare codex` runs both; `wisp rerun` at an older commit leaves nothing behind.
- `wisp crew pause|resume`, `crew create --on done`, `crew update --team …` and `ls --crew` work as their help says.
- `wisp add` registers a project only for a folder that's there (`wisp add a README` is a task).
- The Claude Code hook `wisp skill install` writes works when wisp's path has a space or a quote (macOS, Linux and
  Windows).
- Bash, zsh and fish completions offer the setting keys and values, the harnesses, the goals and every group's verbs.

**Tasks and worktrees**

- Tasks started from the CLI, your harness (`wisp_launch`, `wisp mcp`), crew jobs, check-ins, triggers, recipes,
  reruns and forks each get their own worktree in a project (unless they plan or Worktree per task is off); crew
  check-ins and triggers reuse one worktree each, and a run that changed nothing leaves none.
- Worktrees keep your work safe: Restore asks first and can be undone; Merge keeps a worktree with uncommitted work or
  unpushed commits and says so; Revert in Changes asks before a whole file goes and has Undo.
- Changes shows only this task's work, and says when a task's folder is gone.
- Settings → Worktrees looks again when it opens, finishes with many worktrees, and Bring in finds stray worktrees on
  Windows and offers each once.
- New task with a task file runs the file's agent on its own model and thinking level, and the pill shows it.
- A task that names no harness runs on New task's saved model and thinking level.
- Start later says where the task went ("Starts 14:32"); Mark unread and Save for later have Undo.
- Ship it is offered only on a worktree task; Compare finds its runs again.
- A trust question for a repo's own programs lists what it would run, and **Trust and start** starts the turn and keeps
  the trust.

**Goals, factories and crew**

- A goal judge that answers without a verdict asks you once instead of sending the work back every round; a judge that
  can't run stops the goal and says why.
- Stopping a factory run takes back its question at once, and a stopped run reads Stopped everywhere.
- A crew member an agent adds works in the project's folder, not the task's own worktree.
- Side runs (`/btw`, goal checks, reviews) reach Codex, Grok and Copilot again.
- wisp's MCP server lists only the tools it can run, every tool takes typed fields, and a missing field is said in
  words.

**Channels and phone**

- Telegram, Slack, Discord and ntfy buttons answer a question with the button's choice; a tap on an old question is
  refused; questions sent by a notification carry their options as buttons.
- A bad channel token says where to fix it, and setting up one channel no longer restarts the others.
- On phones: answer buttons are thumb-sized, long-press menus don't run the item under your finger, a long path or JSON
  wraps inside its card, and text that doesn't fit is cut with "…" rather than mid-word.

**The app**

- wisp opens faster: the first screen loads only what it shows (on a 4× slowed CPU, the first task row in about 650 ms
  instead of 1,050 ms).
- ⌘K and the phone's search open at once with 5,000 tasks; the harness picker, side-panel tabs, Settings, J/K and
  submenus answer within a frame or two; a task you've left frees its memory after 30 seconds.
- The task's side panel always shows Changes, PR, Shell and Preview; the Shell fills its pane and takes the keys when
  you open it; the arrow keys walk the tabs.
- Many windows connecting at once no longer stall live updates; `wisp blame`, history, Usage and the Machine page answer
  several times faster.
- Desktop links open in the click itself, so no webview blocks them.
- Antigravity's sign-in works on macOS again; an unfinished sign-in reads Signed out with Sign in; Update all says, for
  each harness, what it updated to or why it didn't.
- Home's Needs you puts a task that's asking you first.
- A cloud computer's live updates work right after a restart, a computer added after switching machines off and on is
  reachable at once, and Open on a sleeping cloud wisp shows it waking at once.
- With Auto off, nothing offers `auto` or the router.
- Settings → Connectors keeps each connector's last check across a restart.

[Unreleased]: https://github.com/WispAgents/wisp/compare/v1.0.0-alpha.5...main
[1.0.0-alpha.5]: https://github.com/WispAgents/wisp/compare/v1.0.0-alpha.4...v1.0.0-alpha.5
[1.0.0-alpha.4]: https://github.com/WispAgents/wisp/compare/v1.0.0-alpha.3...v1.0.0-alpha.4
[1.0.0-alpha.3]: https://github.com/WispAgents/wisp/compare/v1.0.0-alpha.2...v1.0.0-alpha.3
[1.0.0-alpha.2]: https://github.com/WispAgents/wisp/compare/v1.0.0-alpha.1...v1.0.0-alpha.2
[1.0.0-alpha.1]: https://github.com/WispAgents/wisp/releases/tag/v1.0.0-alpha.1
