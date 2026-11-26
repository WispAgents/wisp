# Changelog

All notable changes to `wisp-agents` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **A browser for every agent**: Settings → Agents → **Browser** picks what agents drive: **Default** (Claude Code follows its own `/chrome` choice, as before), **Off**, **On (Chrome)** (Claude in Chrome on Claude Code, Playwright on your Chrome for Codex, opencode, Copilot and Grok) or **Headless** (Playwright's own hidden browser, a fresh profile each run). Each harness can have its own, from its page in Settings → Agents or the **Browser** row in the harness picker. Cursor and Antigravity can't take one and the picker says why. In a sandbox, any choice but Off uses the sandbox's own browser, the one you can watch; On (Chrome) is unavailable there because Claude in Chrome needs your own Chrome. In Edits mode, Claude in Chrome's clicks and screenshots no longer each ask for approval, and a browser step that does ask names the page ("open https://example.com").
- Workflow board: each step card has a **⋯** menu with **Skip** (for a waiting or running step; the steps after it still run) and **Retry** (for a failed or skipped one; the steps its failure blocked run again too). A fan-out card can retry all its failed runs or skip the rest at once. **Save as workflow** keeps a run's workflow under its name, so `/workflow <name>` runs it again and `wisp export workflows` exports it.
- Agents can change a workflow while it runs with the `workflows_edit` tool: skip or retry steps, add steps, change a step that hasn't started, or change the run's concurrency and budget. Retrying or adding a step after the run ended starts it again. When a step fails, the agent hears at once, so it can retry or skip it without waiting for the run to end.
- Prebuilt workflows: **audit**, **fix-failing-tests**, **two-ways**, **explore-plan-build** and **migrate**. Run one by name with `/workflow audit` or `wisp run audit`. A prebuilt workflow with a single input takes the words after its name as that input, as in `/workflow two-ways make the parser faster`. The `/` menu lists them next to your own. `wisp workflows starters` prints them so you can copy one into your `.wisp/` formula as a starting point; `wisp apply` imports it.
- Workflows on the task page: a running workflow now shows as a compact row at the top of the task's details on the right, above the PR card. It has the run's name, its progress (`4/9`) and a bar with a segment per step that fills as steps finish. Click it to open the run's board in place of the chat. From the board, click a step to open its task, and use the breadcrumb or Back to return to the task. A step's own task shows the same row, which opens its run's board again.
- A task whose agent started a workflow now stays **Working** until the workflow ends, instead of showing as waiting on you while its steps run. When the run ends, the agent gets the results and answers. Stopping the task also stops its workflow.
- **Workflows for every agent**: a task's agent can now run a workflow, on any harness (Claude Code, Codex, opencode, Cursor, …), as Claude Code's own workflows do. Type `/workflow` and what you want done (`/workflow audit every route for missing auth checks`) in New task or a reply: the agent plans the steps and starts them as subtasks, each its own agent. Steps can wait on each other, fan out over a list an earlier step found (one subtask per file, route or failing test), or try the same thing on several agents. `/workflow <name>` runs a saved workflow, and `/` suggests the saved ones. When the run ends, the agent gets every step's result and answers you. Agents can also start runs themselves with the `workflows_run` tool.
- Task page: a live board for workflow runs. Each phase is a lane, each step a card showing its agent, how long it took, its cost and a line of what it found. A fan-out is one card with a dot per item. Click a card or a dot to open that step's task. The header shows the phase the run is in, its progress, its time and its cost against the budget. Its menu has **Stop the run** and **Copy as workflow**, which copies the run's workflow so you can save it and run it again. Earlier runs fold into one line each.
- Prompt box: **/plugin** makes a wisp plugin from New task or a task's reply box. Type `/plugin` and what it should do: an agent writes it as a plugin of yours and installs it off, as **Cast a plugin** in Settings → Plugins does, and the new task opens. New task's agent pick goes with it. `/plugin` alone opens Settings → Plugins.
- Mark done → *Remove its worktree?*: a folder holding uncommitted work lists its files. Click **3 uncommitted files** on the row to see each file with its change (A, M, D, R, or U for untracked) and its path. When only one folder holds work, its files show when the dialog opens. Past 100 files, the last line says how many more.
- Task details: click the branch chip to copy the task's worktree path. A tooltip says **Worktree path copied**. Hover the chip to see the branch and the path.
- Usage page: the chart stacks each harness in its own colour. Click a harness in the legend to hide or show it. **Day** and **Week** group the bars by day or by ISO week. A card per harness shows its tokens, spend, cache hit %, turns, how many worked, and how much of its fullest plan window is left with when it resets. **Compare** puts two or three harnesses side by side. All the tables are still there.
- Harness picker: a harness's line says when its window resets ("Weekly 64% used · resets in 2h 10m"). A harness at its limit is greyed but still listed, with its reset time. A harness whose CLI reports no limits says **Limits not reported by Cursor**. Its usage is read again each time the picker opens.
- Harness picker: one search box across the top finds any harness or model as you type, grouped by harness (↑↓ and Enter pick, Tab jumps to the next harness). **Recent & favourites** lists your last 5 picks and the ones you starred (click the star on a model, or press f); they're kept on the computer, not in the browser. A signed-out harness has **Sign in** right under its name.
- **Continue on…**: carry a task over to another harness from the task's menu, ⌘K, ⇧C or `/continue …` in the reply box. Pick the harness and model, see what carries over (the last summary, the files, the turns it missed, your rules, and about how many tokens), choose **Summary only** or **Include full transcript**, then Continue. The Timeline marks the switch with a line: "Switched from Claude Code (Opus 5.5) to Codex · context: summary". The refused and usage-limit cards' harness buttons open the same dialog.
- Settings → Agents is a list and a detail: **General** first, then each harness with a dot for ready, signed out, not installed or at its limit, and a search by harness or setting ("cursor model"). A harness's page has its sign-in and accounts, its program path and version with Re-check, a switch to turn it off, its default model, thinking level and mode for new tasks (the same ones New task uses), and where automations stop on it. A model it no longer offers, a missing program and a key that failed its test each say so on their row. The settings search opens straight on the harness.
- Usage: a turn its harness gives no cost for is priced from its tokens at API list price and shown as **~$1.20**, in the tiles, the bars, the stacked chart's tooltips and the week bars. A cost with some turns unpriced ends in **+**. Every price comes from the vendor's own pricing page (Claude, OpenAI, Gemini, xAI), with its link and date in the table; a model those pages don't list (Cursor's Composer, the Grok CLI's own model ids, gpt-5-codex) stays tokens-only and reads **not priced**.
- Settings → Usage: the Spend tile marks an estimate with **~** and a partial sum with **+**, and says **Tokens** when nothing in the range has a price, instead of **Spend $0.00**.
- Usage: Cursor shows its plan ("Cursor plan: Pro", from `cursor-agent about`), and opencode and Grok show what their own CLI totals for the last 7 days ("opencode says $1.25 · 287M tokens in 7 days", from `opencode stats` and `grok usage`), on their card and their Settings row. wisp reads these in the background when Usage opens; nothing is spent.

### Changed

- The npm package, the README and the site's structured data now say **Wisp Agents** and point to trywisp.dev, so searches for "Wisp Agents" find the project.

### Fixed

- wisp Cloud: opening your cloud wisp while it sleeps shows its waiting page at once instead of a blank tab for up to 45 seconds. The page says *Waking your wisp*, or *Waiting for room* with your place in line when the grid is full, and opens your wisp as soon as it runs. If it can't start, the page says why, with **Try again** and **Home**, instead of a dead-end error page. A full grid now puts the start in line and starts it as soon as another box stops (people ahead of scheduled tasks), instead of refusing it. Scripts and the API still get 503, now with `retry-after`.
- wisp Cloud: a scheduled task's wake that finds the grid full waits in line and runs once there's room. One that fails to start keeps its schedule and tries again (30 s, then 1, 2, 4… up to 10 minutes apart) for up to an hour, instead of being dropped.
- wisp Cloud: sleeping and opening a cloud wisp at the same moment no longer leaves it marked asleep with its box still running.
- wisp Cloud: two boxes starting at once can no longer both take the grid's last place. A new `hostd.maxRunningPerUser` setting in `grid.json` (default 4) limits how many of one person's boxes run at once, counting their own box and their sandboxes. Past it, a sandbox waits in line and doesn't take anyone else's place. Your own box always starts.
- wisp Cloud: deleting your account now also deletes your sandboxes and their files, and every secret the grid kept for you (sandbox sign-ins and env vars, your channel bot, your data key).
- Changes in a monorepo folder (a task opened at `packages/web`): every file is listed relative to that folder, and Stage, Revert and Undo act on the file listed. Revert no longer writes a stray `packages/web/packages/web/…` copy, and hunk staging works there. The scope is that folder everywhere the branch's changes are read: the PR text Ship drafts and the files Review reads now cover only that folder's changes, not the branch's edits in other packages.
- A scheduled task (Start later) whose start fails, because wisp's runtime was away or refused it, is no longer deleted unseen. It tries again after a minute, then five, and after three failures it waits in **Drafts** with *Couldn’t start at its time*. **Start now**, an edit or Cancel on a scheduled task that is already starting at its time is refused (it says the task is starting) instead of starting it twice or losing the edit.
- A follow-up's turn is always counted as the follow-up's, even when it starts before the send returns. The user's next turn then starts the agent's wake-up chain again, and a reminder that turn cleared stays cleared.
- Crew check-ins and declared task schedules run in your time zone, not UTC: a `0 8 * * 1-5` check-in is at 08:00 where you are. A declared schedule missed while the computer slept runs once when it wakes, marked late, never twice.
- wisp now prunes its change feed and frees unreferenced blobs every hour (the first a few minutes after start), so the database and `blobs/` stop growing without bound. A blob saved while the cleanup frees the same content keeps its file.
- A task list that changed while its first look was read in pages, with the change feed pruned meanwhile, no longer drops the changes in between: the snapshot closes at the seq its pages cover.
- Your harness can no longer answer its own **Your harness wants to…** card by naming the card's task inside `args` (`{"args": {"id": …}}`), nor by `sessionId` or `session`: wisp refuses it as it does a top-level `id`.
- Your harness adding or changing a connector's command or URL, testing or importing one, or setting a project's test command now asks you first on a **Your harness wants to…** card, as switching off a safety guard does. Nothing is stored or started until you allow it. Switching a connector on or off still applies at once.
- Connectors: a `${secret:…}` in a connector's headers or environment is read once and only from that connector's own secrets or a formula's (`wisp secret set`): another connector's token or a plugin's secret stays as written, and a secret whose value names itself no longer hangs wisp. A sign-in token is sent as it is.
- Connectors: changing a signed-in connector's URL to another server signs it out (its row shows **Sign in**), so the first server's token never goes to the new one. A sign-in token is never sent over plain `http://` to another computer.
- Slack, Discord and Telegram: a link code links one person and is then used up (**Relink** shows a new one), and five wrong codes from anyone change it, as Settings then shows. `/wisp link` on Discord and codes sent to an unlinked Telegram bot are limited to three tries a minute per person, as Slack's and Discord's `link` already were.
- Triggers: a trigger whose run was stopped, whose task was deleted, or whose target is a workflow or a crew member now runs again on its next event. A follow-up that interrupts the run's turn no longer counts as its end, so a second run no longer starts beside it. Before, it said "skipped: already running" for good. A run whose end wisp never heard is let go at the next check, about 15 seconds later.
- Triggers: two triggers that fire on each other's runs now stop after three rounds, as a trigger on its own tasks does. A trigger whose target is a workflow no longer fires on the workflow's own steps.
- Triggers: jobs queued outside a trigger's hours now wait for the hours. A trigger set to run one job at a time (or N at a time) now starts its queued jobs one (or N) at a time. A paused or switched-off trigger keeps its queue.
- Workflows: a step with a goal now counts as done once its judges agree the goal is met. If the goal stalls, the step fails and says why, even while the goal's question waits for your answer. A paused goal shows the step as waiting, with why. With the goal plugin off, the step's own answer counts.
- Workflows and factories: a step whose turn waits for a usage limit, a trust check or the plan now shows as waiting, with why, and no longer fails with "it ended held". A step whose task you delete now fails with "its task was deleted". Before, the run waited for it forever.
- Goals: a goal that wisp stopped in the middle of its check now gets checked again when wisp starts. Before, it could say "judging" forever.
- Chat mode: a turn whose answer leaves a dev server or another background task running now ends, and the follow-ups queued behind it start. Before, the turn stayed in Working for as long as the background task ran. With wisp blocks on and Chat mode off, a turn that says it will report back still waits for its background tasks, but for at most 30 minutes without new output.
- Two permission prompts asked at once (parallel tool calls) now both wait for you: the card shows the first, and the next one comes up once you answer it. Before, the second replaced the first, which was denied with "No answer from the user in time". A question the agent replaces with a newer one now says so in the transcript, and the agent is told it was replaced instead of "the user did not answer".
- A permission prompt nobody answers for 30 minutes is denied and taken off the task, and the task goes back to Working. Before, its card stayed up and the task waited on it, and a late **Allow** said "You allowed" for a call that never ran.
- New task: **Stop** while its first turn is still starting (its worktree coming back, a check before it runs) now stops it, and a follow-up sent then waits behind it. Before, the Stop did nothing, and the follow-up started a second first turn next to it.
- **Start now** on a held turn after **Stop** now starts it. The follow-ups queued behind it stay held. When it can't start (a setup card is open), wisp says so instead of doing nothing.
- /btw, quick answers and goal checks: an answer whose last line has no newline after it is no longer lost, and the agent's process is stopped when the run ends early, fails to send its prompt, or is cancelled.
- Connectors signed in with OAuth keep working past their token's end. wisp refreshes the token a minute before it runs out, and again when the server refuses it, then tries the call once more. Before, it kept sending the expired token, and the issue picker and agents' calls failed until you signed in again. New runs get the new token. When the sign-in can't be renewed, the connector's row says **Needs sign-in** (also after a restart), and its menu offers **Sign in** beside **Sign out**.
- Settings → Connectors: signing in again from the same place now shows the connector as signed in as soon as the browser comes back, instead of **Signing in…** for up to 10 minutes. A sign-in you start and don't finish no longer changes the working one, and a new sign-in that registers wisp without a client secret no longer sends the old secret.
- **Secrets are never lost to a bad `secrets.json`**: a file wisp can't read for a moment (permissions, a disk error) or that isn't valid JSON is no longer taken as empty and then written over on the next sign-in or token, which cost every saved key. Reading it fails with the file's name instead, and wisp leaves the file alone until it's fixed. A namespace or value that isn't the right shape is skipped with a warning, and the rest is kept.
- Plugins: a plugin's HTTP route now runs within that plugin's permissions, and an agent's token is checked against its task before any of the route's code runs, however the request reaches it. An exec plugin (a child process) can only emit events under its own id or the types its manifest declares, and a call token that ended, timed out or was never issued is refused instead of running as a new call.
- Switching a plugin off while events were on their way to it no longer leaves `wisp` waiting forever for them to be handled. `wisp events retry` on an event parked for a plugin that's off now says to switch it on first, and keeps the event parked, instead of dropping it.
- Channels: a reply button under a finished task's report (**Ship it**, **Revise**, …) now sends that choice as the follow-up. Before, it sent an empty message that wisp refused. A button under an older turn says it's out of date and sends nothing.
- Slack: a button click that answers or follows up now updates the message: the buttons go and a line says who clicked. Before, every click got a private reply and the buttons stayed. A refused click (a stale button, a setup question only you can answer) still gets a private reply and keeps the buttons.
- Discord: a button press that is refused or fails keeps the message's buttons, so it can be pressed again. Before, they went whatever happened.
- Slack, Discord and Telegram: a task in Full auto has no reply buttons, and a tap on an old one asks you to say it in words (so it runs in Edits at most). Before, Slack and Discord never knew a task's mode, and Telegram never checked it. Reports from side discussions and delegated crew jobs now go to the thread of the task they came from.
- Shell, take over and Preview: a key typed after one that never reached the computer now goes in after a second, instead of waiting until the next key. A shell started again no longer holds back every key, and neither does a page whose count went on from before (its first key goes in at once). A page that falls behind the output reconnects and redraws the whole screen, and shows when the program ended, instead of silently missing output.
- Deleting a task now ends its shell, take over and running app, and stops its Preview proxy and frees its port. Stopping or restarting an app, or an app that exits by itself, also stops its Preview proxy. Turning the Terminal plugin off or reloading it ends its programs. An ended terminal is forgotten five minutes after it ended.
- Usage page: 7 days is now exactly 7 bars, and the last bar is your today. Days follow your time zone, not UTC. Before, the window started at this hour 7 days ago, so it had an extra part-day bar and a late-evening turn could land on the next day.
- Settings → Usage: a harness whose CLI reports no plan limits (Cursor, Copilot, Grok, opencode) now says so (**limits not reported by Cursor**), and its stop line can't be set, since there's no window to stop at. Each harness's stop line is now on its page in Settings → Agents; Usage links to it. Cursor, Copilot and Grok accounts now also show in the plan list: wisp misread their account ids before.
- Usage page: when no turn in the range has a price, the first tile is labelled **Tokens**, not Spend.
- Changes and diffs: a removed line that starts with `--` (such as a Markdown rule `---`) or an added line that starts with `++` is now shown as a removed or added line. Before, it was taken for a file header and left out, and reverting that hunk dropped the line from the file. The same fix applies to diffs in `wisp` CLI answers and in the terminal UI.
- Worktrees: a folder that several runs of a recurring job share is no longer removed while any of them is working in it. Before, the sweep, **Done**, archiving or the end of another run could remove it. A task that is waiting on your answer, or whose **Ship it** is pushing, now counts as working, so its folder stays too. A folder that stays for this reason shows as in use, not as failed.
- Worktrees: wisp no longer deletes a folder when git refuses to remove it (for example, a locked worktree), and it never removes a repository's own checkout, a full clone, or a folder outside wisp's worktrees folder that no task made. When git can't read a folder's status, the folder counts as holding work and stays.
- Merge (**Ship it**): the task's worktree stays, with a note that says why, when git can't read its status, when **Keep worktree** is on, when the task is still working, or when its branch has a commit that the pull request doesn't have. The folder is now removed through the same safe removal as every other clean-up. The branch is deleted, locally and then on the remote, only after the folder is gone.
- New worktrees: `.worktreeinclude` lines with `..` or an absolute path (`/x`, a drive path, `~`) are skipped with a note, and a symlink that points out of the repo is not followed. Until you trust a repository, a new worktree gets only its root `.env` files; the rest of its `.worktreeinclude` list is copied when you trust it.
- New task remembers the computer you picked under **Run on**: the next new task starts on that machine too, after a page change or a reload, until you pick another. If that computer is gone, it runs on this one.
- Answers: a `diff` block that names only its file (how agents are told to write it) said "No captured edits for this file" for every file: the block now shows the edits wisp captured for that file in the turn, as v1 did, and `files` rows without their own diff fold those edits open.
- **/sandbox with Claude Code signed in**: signing in to Claude for sandboxes now works when the browser finishes the sign-in by itself (Claude's page approves it with no code to paste). Before, the token never reached the grid, so every `/sandbox` still said *Sign in to Claude for sandboxes first*. Settings → Sandboxes shows **Signed in** as soon as the browser is done, and the sign-in link no longer comes out as two links run together.
- **/sandbox shows at once**: a sandbox task is in your task list from the moment you send it. Its row has a small glowing grid and says *Preparing the sandbox…*. Its page shows a larger glowing grid and each step: making the sandbox, starting it, copying your project in, starting the task. When the task starts, it opens. If the sandbox can't start, the row and the page say why, with **Try again** and **Dismiss**, instead of a bare error page.
- **Sign in from the task**: a `/sandbox` task that needs Claude's sign-in now asks for it on its own page, with a **Sign in to Claude** button, instead of sending you to Settings. Approve it in the browser and the task goes on by itself. A grid that can't be reached now says so, instead of asking you to sign in.
- Phone app: New task keeps the agent you picked last. With more than one computer (your cloud computer and your own), it read the pick from another computer's settings and went back to that agent each time you opened it. The order you dragged projects into has the same fix.
- Phone app: an answer's pictures and videos, and the files sent with a prompt, now show. Before, each one said "This picture can't be shown here." Over SSH they load through the tunnel. On wisp Cloud and computers reached through the grid, the grid gives the app a link that works for one hour and for that file only. The link stops working if the phone is removed from your account.
- Workflows: when a step fails, every step that depends on it, directly or not, is now skipped. Before, a step two links down still ran.
- Toasts now sit centred on the main area, not the whole window, so the sidebar no longer pushes them off-centre. They stay centred when you collapse or resize the sidebar.
- Side discussions (`/side`) now run with their main task's access on every harness: the main task's mode (never below Edits, Full auto when it is), its account, thinking, connectors and team, and commands without an Allow card. Before, a side on Codex and the other harnesses only read, and on Claude Code every command asked. A side's write to a file the main task changed, in an earlier turn or in the one it is running, isn't made: it shows on the main task as a card with the diff, **Apply** (written while the file still reads as it did, else nothing is written) and **Discard** (ADR-0123).
- Fork: a fork keeps its task's connectors, team, Try it two ways variant, lean setting and limits, so its first run has the same MCP servers; and none of the task's leftovers (a retry's grants, side questions, a blocked card).
- Fork from Claude Code: the forked conversation is saved, so the fork's second follow-up resumes it. Before, it resumed a conversation that was never written.
- Fork from an earlier turn: the fork goes on from that turn's conversation (Claude Code) and from its files as they were then, not with the later turns. A harness that can't go back to an earlier turn's conversation (Codex and others) says so, and the dialog offers the latest turn instead.
- Allow & retry: the grants or Full auto it gives go to its own retry only. Before, a follow-up queued ahead of the retry took them.
- Usage (Claude Code, Codex): a resumed turn counts only its own cost and tokens. Each turn now keeps the conversation's counters (`stats.totals`) and the next turn on the same harness and conversation starts from them, where every follow-up counted the whole conversation again. A chat brought in, adopted or restored, and a conversation whose last turn was stored before this change, takes its first result as the baseline (that turn counts 0 and the next one counts its own); another harness or a fresh conversation starts from zero.
- Usage: the cache share counts cache reads only. A write to the prompt cache (most of Claude Code's first turn) is input, and is no longer shown as a hit; turns keep their cache reads and writes apart (`cacheReadTokens`, `cacheWriteTokens`). Codex's cached input is recorded as cache reads and no longer counted twice in the input total; a Codex turn's input now leaves its cached part out, so Drain watch's per-turn tokens for Codex read lower than its older turns for a while after the upgrade.
- Task page (harness picker): switching a task to another harness hands it over as a follow-up's pick does: the new harness no longer resumes the old one's conversation id, gets its own default model and the nearest thinking level, and hears what it missed (the hand-off note). A turn still running when you switch no longer puts the old conversation back.
- Delegation: a lead told to delegate through wisp now runs with its harness's own subagent tool off (Claude Code's Task/Agent, opencode's `task`, Copilot's), as its system prompt says, and so does a side discussion (it has its main task's team); subagents and harnesses without wisp's tools keep theirs.
- OpenCode and Grok: a permission refusal on stderr shows as a blocked card with Allow & retry again (no refusal was ever detected), and a CLI's last stderr line without a newline reaches its turn's error.
- Desktop app on macOS, installed with `npm i -g wisp-agents`: the app now opens when you start it from the Dock, Spotlight or Finder. Before, it showed "wisp's files aren't in this app" and quit unless `wisp` was already running in a terminal. It also works when the Dock item is left over from an older version.
- Task list: **Mark done** on a task saved for later moves it to *Done*. Before, it stayed in *For later*. **Undo** puts it back in For later at its old place.
- Task page: the details now name the machine the task runs on (**This computer**, a computer by its name, or **Sandbox**), above the branch.
- Computers show by a name you know, not an id like `1959efa12b19`: the phone and the app keep the name you gave a computer (your grid box stays "Cloud"), and wisp names itself after the Mac's Computer Name or host name, never a container id (a grid box calls itself "Cloud").
- Terminal tasks (`/terminal …`, Babysitting mode): the agent's own CLI is now the task page, full width under the task's head, as in v1. Before, it opened in the side pane beside an empty chat column.
- Terminal tasks: **Mark as done** now moves the task to Done and closes its CLI. Before, it said the task was still working and did nothing. It works from the task's head, its row, the phone's bulk actions, and `runtime.setState` from an agent or the CLI. When the CLI later exits, the task stays in Done instead of going back to review.
- Home: the **Reach this wisp from anywhere** tip no longer shows once this computer is on a wisp Grid. Home shows another tip in its place.
- Android app: **Scan the code** in Add computer now opens a scanner (Google's code scanner from Play services, so the app still asks for no camera permission) and saves the computer with its key pinned. A phone without the scanner no longer shows a Scan button that always ended in "type it by hand": the typed address is the main action there.
- Android app: something shared to wisp from the share sheet now opens New task with the shared text or link in the box and its pictures or PDFs attached. Before, nothing took it and a toast said to update wisp on the computer. With no computer open yet, the share waits on the phone and wisp says to open a computer first.
- Android app: **Share** opens Android's share sheet. Before, it copied the link and did nothing else.
- Installed web app (phone Home Screen, desktop): an app left open or brought back now checks for a new wisp when it comes back into view, resumes, or reconnects. It no longer waits for a page load. *Update the app to reach …* has a **Reload** button. An update whose files fail to download keeps the working version, so the app no longer opens blank offline. Old versions' files are removed from the app's cache once a new one is in.

## [1.0.0-alpha.14] - 2026-11-25

### Added

- Markdown (answers, transcript, docs, the docs site): ```mermaid fences draw as diagrams (ADR-0118). mermaid is bundled with wisp (no CDN, works offline and in the phone apps) and loads only when a page shows a diagram, never with the first screen; it runs at its strict security level, follows light and dark, and a fence still streaming stays code until it closes. Each diagram has a bar to copy its source, switch to its code and back, and open it full size with zoom; a source that can't render shows as code with a short note saying why.
- Markdown: setext headings (`Title` over `=====` or `---`) are headings; `<details>` with an optional one-line `<summary>` is a native collapsible section (its body is Markdown; one still streaming shows open; every other HTML stays text); ```diff and ```patch fences colour added, removed, hunk and header lines like the diff view.

### Fixed

- Chat mode: a ```wisp block streaming into a message no longer flashes its raw JSON as a code block, then swaps the whole message: the message is an answer from the opener on, the block's place holds a skeleton named by its type until its JSON closes, and the prose before it keeps its place (and its text selection).
- Grid: every box's coding agents are now kept up to date. The grid rebuilds its box image daily with the latest Claude Code, Codex, Copilot (new on the grid), Cursor, Grok, Antigravity and opencode, checks that each one starts, and boxes move to it the next time they start. The grid's opencode is now v2 (from opencode's GitHub releases, checksum-checked); it was the 1.x line from npm. A grid box no longer offers an agent *Update* it couldn't apply (its agents live in the read-only image).
- First run without a coding agent: **Install opencode** now works on a computer without Node.js. With no npm, wisp downloads opencode's own binary for this OS from the npm registry, checks it against the registry's sha512 and unpacks it into `~/.wisp/tools`, so the free models answer a first task with nothing else installed (before, it stopped at "npm isn't on PATH… Install Node.js"). On Windows, wisp now finds the opencode it installed even when `HOME` isn't set (it looked under `/.wisp` and said "opencode was installed but doesn't start"). Every install attempt is now counted in usage stats with how it was tried and, when it fails, why, where only npm's successes were counted before.
- Grid (security): a request into a box or sandbox no longer carries a client's own copy of the grid's marks (`x-wisp-preview`, `x-wisp-route`, `x-wisp-upgrade`, `x-wisp-tunnel`, `x-wisp-proxy-*`, `x-forwarded-*`), and a box's `Domain=` cookies never reach the browser; a cookie-signed write to the grid's `/api/` from another origin is refused (403); a box's secret no longer reaches `/api/teams` or `GET /api/computers`; a grid cookie sent twice signs nobody in. A grid box's formula reads no secret through `file()`, the environment or a module folder in wisp's home.
- Answers: `chart` blocks draw their bars again (label, bar, value), with the numbers' table kept for screen readers only, and `tree` blocks draw as an indented file tree. A ```` ```wisp ```` block an agent left unclosed no longer hides the prose and code after it up to the next code fence. A fenced code block in markdown shows its language and a Copy button.

## [1.0.0-alpha.13] - 2026-11-25

### Fixed

- Antigravity (model picker): a model whose id holds its thinking level (`gemini-…-high`, `-low`, `-medium`) no longer shows the thinking-level selector, and wisp no longer passes `--effort` with it, so agy stops refusing the run with "--model … conflicts with --effort". A model's empty level list now means "no selector" again (as in v1) instead of "use the harness's levels", which also hides the selector for Grok models that take no reasoning effort.
- New task and reply box (Start later / Send later / Wait until the reset): with files attached (or review comments waiting on a reply) the task started, or the follow-up went, at once and the picked time was ignored. Such a send is now refused with a toast, leaving the text, files, comments and time as they were; with no time set, files go as before. New task also uploads attachments to the computer the task runs on rather than always this one, and uploads again if Run on changed after a file was added. A task on another computer now reads Settings → Chat from this computer, and `/btw` with its × show at once on tasks from other computers.

- Task view (`doc` blocks): ported v1 `keepDocs` snapshotting to `wisp-runtime` so finished turns capture each referenced markdown document (`text`, `abs`, and `truncated` past 512 KiB) from the task's cwd, worktree, extra project dirs, or outside doc paths (while refusing `.env`, binary files, and non-doc symlinks), wired `docText` live fallback via `worktrees.source` in `answerContext` for mid-turn Chat mode blocks and earlier turns, and fixed empty markdown files (`text: ""`) in `DocBlock` falsely showing *"The file wasn’t there when the agent finished"*.
- Task view (expanded actions): redesigned expanded action groups and step details in `Timeline` into a cohesive disclosure card with a full-width header, separated file-read chips with file icons, semantic edit icon tinting, crisp inline command badges, right-aligned diff stats/chevrons, and recessed high-contrast output panels instead of disconnected floating boxes.

## [1.0.0-alpha.12] - 2026-11-25

### Fixed

- Task view (side panel → Changes): fixed app freezes when opening the right panel on tasks with large diffs. Removed an O(N²) per-line array copy/reverse in `FileDiff`, switched hover and open-comment tracking to `createSelector` with per-file memoized comment marks and lazy hunk hashing, keyed file rows by `path` so streaming updates never teardown/remount open diffs, restored v1 auto-open rules (opening at most 3 eligible files or only the first, skipping generated and >300-line files) with per-file (300 rows) and per-list (200 files) expansion caps, skipped redundant turn-diffing for worktree files when `git.changes` is present, and added `statsOnly` to `git.changes` so the task head and background tab count no longer transfer full diff payloads.
- Performance (editor, streaming & compare): switched `FileView` syntax highlighting to per-line memoized chunks with `content-visibility: auto`, fixed the jump-to-line effect resetting the caret on edit, and decoupled `FilePane`'s why-gutter from per-keystroke splits and session-list updates; replaced `SidePanel`'s per-event full `subagentData` deduplication with a lazy per-task boolean check, made `builtinRows` single-pass O(N), and keyed `TeamList` rows by id; grouped `Compare` turns/events via `eventsByTurn` and cached finished `<Answer>` columns so sibling streaming never remounts them; pre-indexed per-file edit counts and memoized mid-turn messages in `TurnTranscript`, removed duplicate session-list subscriptions and hoisted `delegating` in `TaskView`; and replaced `JSON.stringify` block equality in `Markdown` with zero-allocation structural AST comparison.

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
