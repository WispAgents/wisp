# Changelog

All notable changes to `wisp-agents` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Push a saved task from Save to repo.** Once saved, the line under its answer has **Push** (and ⋯ has **Push to
  repo**), so its `.wisp/` files don't stay on your computer. While the task's work isn't shipped they're committed on
  its branch and pushed (its PR gets them). Once its work is merged, they go out as a pull request from
  `wisp/history-<id>`, built on the remote's main without touching your checkout, branch or staged files; the local
  copies are taken out so your next `git pull` doesn't stop on "untracked files would be overwritten", and they come
  back with it. The line then says where they went, with **Open PR**. Agents can do the same with the `history_push`
  tool.

### Changed

- **The task list has a Shipping section**, under Needs you and before Working. A task you clicked Ship on moves there while wisp commits, pushes and opens its PR, and stays while that PR is open. Once the PR merges or closes it comes back to Needs you. If the task runs again (a fix for its checks), asks you something or is marked done, it shows in that section instead. Home's Needs you leaves these tasks out too.

### Fixed

- **A key pressed the moment a task opens now works.** ⇧F, ⇧R, M and the other task and list keys arrive a beat
  after the page draws; one pressed before that did nothing. It now runs once they're in, if you're still where you
  pressed it.
- **Re-applying a `wisp export` with `${secret:NAME}` references works again.** A reference with a long name
  (`${secret:LINEAR_WEBHOOK_SECRET}`) was masked like a token in `wisp export` and `wisp plan --json`, so applying
  the export was refused as "a masked value". References now print as written; a real token beside one is still masked.
- **Push to repo keeps your saved files when no pull request opened.** Without `gh` (or when it failed), the files
  were deleted from your checkout though nothing would bring them back. They stay now, and the line says *Branch
  pushed to …: open the pull request*, with a button to open it.
- **Where a push went now shows after a reload and from ⋯.** The line under the answer says where the files went
  (with **Open PR**) after a reload, ⋯ → **Push to repo** toasts the branch and its PR, and pushing again after the
  files went out answers the earlier push instead of "Save to repo again". Saving again after a push offers **Push**
  again instead of the old "Pushed to …".
- **A PR you just shipped no longer shows green before its checks start.** Read the moment it opened, GitHub listed no checks yet, so the PR row said passed in green and the Steward said "green"; the checks only showed later (or after a reload). Now the row says *Waiting for checks* in amber, wisp asks GitHub every 5 s until they show, and the Steward says green only once checks have run on the head it judged.

## [1.0.0-alpha.17] - 2026-11-27

### Changed

- **trywisp.dev on a phone offers what a phone can do.** In place of `npx wisp-agents` and the desktop download, the hero and the closing section offer **Send it to my computer** (the share sheet, or the link copied where there's none) and **Try it in the cloud** (Wisp Agents Cloud). On a computer nothing changes. Six in ten readers came on a phone, where neither command nor download works.
- **The site counts its install funnel.** Each event says whether it came from a phone or a computer, and copies and downloads say where on the page they were. New: sending the page to a computer, opening the docs or the cloud from the hero, the sections a reader reached, and joining the phone app waitlist (without the address). docs/privacy.md says so.
- **Settings → Computers → Sandboxes lists every sandbox your grid holds**, with how many you have of the most it keeps ("3 of 5 sandboxes", amber when full). One made from another computer, or whose machine here went, shows as "Not on this computer" with a Delete that removes it on the grid: before, it held one of your slots where nothing showed it. A sandbox with no owner now says it's yours and stays until you delete it (it said the sweep removes it, which it never did).
- **The Sandboxes list stays live.** A launch's sandbox appears as it lands, one dropped goes, and online/asleep follows its link while the pane is open; it looks again when a launch ends and when you come back to the window.
- **Sign in to Codex there shows on the sandbox's row**: starting at once, then an Open the sign-in page button and the device code with a copy button, then done or why it failed. It was a 4-second toast, and the first click showed nothing.

- **Each `wisp` exit code means one thing.** 0 it worked, 1 wisp refused or the task failed, 2 the command line didn't fit (nothing ran), 3 the task waits on you, 4 no task with that id, 124 a wait timed out. A task that asks you something now makes `wisp wait` and `--wait` exit 3 (it was 2, the same as a typo). `wisp show`, `wait`, `reply`, `btw`, `stop` and `attach` with no id, a launch with a misspelled flag or a bad `--mode`, `--effort` or `--model`, and a bad `wisp share`, `wisp trigger` or `wisp cloud` argument now exit 2 (they exited 1), and an unknown task id exits 4. `wisp --help` and the wisp skill list the codes.

- **New headline: "Your coding agents, as code."** It replaces "Manage the code your agents write" on the site, the docs, the README, the npm package, llms.txt and the link preview card, with "Declare them. Run them anywhere. Track every line they write." under it.

### Fixed

- **`--prune` removes only what this formula applied.** Each apply now records where it came from (the `.wisp/` folder and its workspace, or the set of `-f` files), and `wisp apply --prune` deletes a resource only if this same source applied it before. It used to delete every agent, task or trigger of the kinds the formula names that it didn't declare, so another repo's, another file's or one made in the app went too. The plan lists the ones it keeps (`agent/helper (not from this formula: kept)`); `--prune-all` keeps the old behaviour and says so. Without `--prune`, the plan says what this formula applied before and no longer declares (`2 no longer declared here: --prune removes them`). Resources applied before this version have no recorded source: the next apply that declares one (changed or not) records this formula as its source, so `--prune` removes it once the formula drops it; one no apply declares since is never pruned by `--prune`, so remove those with `--prune-all`.
- **A token written into a formula is caught before it's stored.** `wisp validate`, `wisp plan` and the app's Import check every string of every spec for tokens and keys (GitHub, OpenAI, Anthropic, AWS, Slack, a private key, …) and name each one with its fix: `connectors.gh.headers.Authorization looks like a GitHub token: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}`. Apply refuses until it's fixed. `${env:NAME}` in a connector's `headers` or `env` is no longer filled in on your machine (its value went into wisp's store and every export): it's a problem with the same `${secret:NAME}` fix. `wisp export` and `wisp plan --json` mask anything that still looks like a token (`ghp_…(masked)`), and a plan refuses a masked value written back (`connectors.gh.headers.Authorization is a masked value from wisp export: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}`), so applying an export never overwrites a stored token with the mask.
- **`wisp mv` renames the bare names that point at what it renames.** `wisp mv agent.builder agent.maker` now rewrites `agent: builder` in a workflow's steps, `skills: [tidy]`-style lists and the other fields that name that kind, in the folder's files and in what wisp has stored (its success line says how many stored resources named it). The same word in a prompt, in another field or as another kind's name is left alone. `wisp validate` and `wisp plan` now check the references of every declared resource, not only the ones they change, so a workflow left as it is that names an agent no longer there is a problem.
- **The GitHub Action fails the job when the task didn't end well.** It failed only when the task's state was `error`; a task that was stopped, blocked, waited on an answer nobody in CI gives, or timed out passed. With `fail-on: failed` (the default) each now fails the job with its reason, after the summary and the pull request comment are written. New inputs: `task-file` runs `wisp run -f` (`formula` still works, with a warning), and `check: validate` or `check: plan` checks the repo's `.wisp/` setup and posts its plan on the pull request, failing on problems or secrets that aren't set, with no prompt or task needed. A launch that fails (a task file with no task) now says wisp's own message. `action/README.md` lists every input.
- **`wisp apply --json` applies.** It printed the plan and stopped. Now `validate`, `plan`, `diff` and `--dry-run` print the plan, and `apply` and `destroy` go through the same `--yes` and `--trust` checks as without `--json` (asking on stderr in a terminal, refusing with a JSON error otherwise), then print `{"plan": …, "applied": N}`. Every `--json` exit is 1 while there are problems or secrets that aren't set (`validate --json` exited 0 with a secret missing).
- **A goal set in a follow-up sets at once.** The Done when line said "Enter sets it", but Enter did nothing and the goal only went out with a message. Enter now sets it (with your message if you typed one), a small **Set** at the line's end sets it without one, and Escape closes the line. While a goal is going, the reply's Goal chip names its condition and opens Edit goal… instead of an empty line.
- **The side panel's PR tab looks like v1's again.** Checks were 44px rows with a box on hover, file paths wrapped onto two lines with grey `+n −n` out of line, and the folds were centred buttons. Checks are 30px lines again, section titles are v1's small uppercase heads with their count, each file stays on one mono line (the folder gives way first) with `+n` / `−n` in the diff colours in aligned columns, *2 passed · 1 skipped* and *and 13 more* sit on the rows' left edge with a chevron, and the foot sits under a hairline.
- **An agent's question on its task reads cleanly again, as in v1.** The card showed the question's markdown marks (`**`, backticks) in one muted paragraph and cut each answer off with "…". It now has v1's orange Needs your call header and border, the question itself as the headline, the agent's explanation under it as smaller text (lists, bold and code rendered), and numbered answers that wrap to show their whole text. Approvals get the same card.
- **A workflow run cut by a wisp restart lands its task.** Its view already said it stopped; the task it held in Working (the `<name> (workflow)` task, or a lead whose turn ended while its run went) now lands in Needs review, unread, instead of saying Working for ever.
- **A task with a goal stays in Working while its judges vote**, and lands once when the goal is met. It used to land in Needs review, unread, with a notification, every round, while the judges were still voting; another round now starts without passing through Review. A wisp restart in the middle of a round no longer leaves the task in Working for ever: it is judged again, or lands if its verdict was in.
- **A goal's judges vote at once.** A round took the sum of every judge's time; now it takes the slowest one's, and with vote `any` (one yes) or `all` (one no) the rest stop as soon as the outcome is fixed, their records saying they weren't needed.
- **A crew member given two jobs at once while idle runs one at a time.** Both started past their parallel limit; now one starts and the other waits first in line (a lead's second job is refused as busy). A job that fails to start waits first in line instead of being lost.
- **A workflow's budget counts every step's cost**, including a step stopped for its timeout (its stopped turn's cost was dropped) and a step whose end was missed under load. Each step's cost is read from its task's turns.
- **Wisp Agents Cloud counts its sign-ups again.** The grid's usage stats weren't ported to v2, so nothing from the hosted grid reached PostHog after it moved (2026-11-21): no sign-in page visits, no sign-ups, no boxes started. With `"telemetry": true` the grid again sends sign-ups, sign-ins and refused sign-ups, and boxes started (with how long they took), stopped (why, after how long) or failed, each user as a hash. On grid.trywisp.dev the sign-in page loads PostHog again, and each sign-in step is counted as that page's visitor: sent to the provider, back signed up or in, or turned away and why (the provider, a refusal, an expired sign-in). `WISP_TELEMETRY=0` or `DO_NOT_TRACK=1` turns it all off; a self-hosted grid loads no script.
- **A follow-up sent while a task works keeps the page at the bottom.** Sending one interrupts the run: that turn folds as it stops and the follow-up's turn starts. The browser moved the view up as the page shrank and grew again in one go, the page took that for you scrolling up and stopped following, so it sat on the interrupted turn. In Cards mode the interrupted turn finishing also took the view back to its start. Now only your own wheel, keys, touch or scrollbar leave the end, and a turn already on the page when you sent never pulls the view up.
- **Codex tasks start again after `wisp skill install --agent codex`.** It wrote wisp's history server into `~/.codex/config.toml` as `[mcp_servers.wisp]`. That is also the name of each run's own `wisp` server, so Codex refused its whole config ("url is not supported for stdio in `mcp_servers.wisp`") and no Codex task ran. The table is now `[mcp_servers.wisp-history]`, the name runs already use, and wisp renames the one an older version wrote the next time it finds Codex. A `wisp` server you wrote yourself for something else is left alone.
- **Toasts centre on the prompt box.** On a task page or New task, a toast sits over the middle of the reply box or New task's box, not the middle of the whole main area, so a side pane beside the box no longer pulls it off the input.
- **`wisp tui`'s side panes scroll, show under 100 columns, and the Terminal shows only the terminal.** With a pane focused (Tab), j k, PgUp PgDn, g g / Home and G / End scroll it and the wheel over it scrolls it, instead of moving the transcript beside it; the Terminal follows its newest output and says how much is below when you scroll back. Under 100 columns `d`, `t` and `p` open the pane in the task's place (Esc or Tab goes back), and closing a pane gives the focus back to the task. The Terminal lists shell commands with their own output under each, and no longer shows what a Read or a fetch returned. `/` from a pane searches the transcript (typing there used to close the pane or ask to stop the task), ↓ ↑ Ctrl-N Ctrl-P and Enter jump to the next or previous match with a "2/5" count, and an open step with a long output says how many lines are cut, with Enter (or Space again) showing all of it.
- **An agent can only delete or re-own its own task's sandboxes** (ADR-0136). `sandbox.drop` and `sandbox.own` from an agent reached any sandbox, another task's included; now they're held to the agent's own task, and a delegate's sandbox still holding work is yours to delete.
- **Sandbox env vars, Claude's code and Sign out say why they failed, where it happened.** A name the grid refuses (starting with a digit, `PATH`, `HOME`, `WISP_…`, `LD_…`) is said before it's sent; a variable the grid turned down comes back into its inputs with the grid's words under them, and one whose removal failed comes back too. A refused Claude code and a failed Sign out show their error instead of nothing.
- **Deleting a sandbox asks first.** Settings → Computers → Sandboxes' trash opens a confirm that names the sandbox and whose it is, and warns that what wasn't brought back from it is lost; only Delete drops it (Cancel, Escape or a click outside keep it).
- **The Sandboxes pane says when it can't reach your sandboxes.** While it loads it shows a skeleton, and when the grid doesn't answer it says why with Try again, instead of "Not signed in" and "None now" as if nothing were set up.
- **A failed sandbox setup's log fits on the page.** The end of what `.wisp/sandbox.sh` said is a dozen lines tall, scrolled to the error, so Open its task stays in view on a phone.
- **A connector's notes wrap.** "Codex uses its own …" and "can't reach an SSE server" show whole on narrow screens instead of being cut with "…".
- **Settings rows:** a meter as a row's control keeps its width (it drew 0px wide), and an empty notes band under a row no longer draws a stray line.

- **`wisp reply <id> "…" --wait` waits as `wisp wait` does.** It honours `--timeout` (a bad value is said before anything is sent) and exits with how the task ended; it used to wait a fixed 30 minutes and exit 0.
- **`wisp ls --watch` follows the list you asked for.** `--state`, `--agent`, `--crew`, `--project`, `--since` and `--limit` narrow it at each redraw (it showed every task), it fits your terminal's height, and `--json` prints one JSON object per change. `wisp ls --watch` and `wisp show --watch` refuse a flag they don't take instead of ignoring it.
- **`wisp serve --host localhost` works**, and a bad `--host` or `WISP_HOST` is said (naming which) before anything in your wisp home is touched. A wisp bound to `::1` or another address now advertises the URL it answers on, so `wisp ls` and the agents reach it.
- **`--host` is refused outside `wisp serve`** (`wisp --host 0.0.0.0`, `wisp ls --host …`) with "--host is wisp serve's alone"; it was silently ignored.
- **`wisp help` and `wisp completions` never start a background wisp.** With none running they use the commands the last one listed, or the built-in ones.
- **`wisp tui` never stops a task by accident.** Esc only goes back now (to the list, out of the composer); it used to stop a running task. `s` and `/stop` ask "Stop <task>?" first (y or Enter stops it, n or Esc doesn't), and do nothing on a task that isn't running.
- **y, a and n in `wisp tui` answer only tool approvals**, with the approval's own Allow, Always or Deny. On a question ("Postgres or SQLite?") they used to send "allow" or "deny" as the answer.
- **Answers from `wisp tui` and `wisp attach` name the question they answer.** Setup cards can now be answered from the terminal, and an answer to a question that was already answered or replaced is refused with that reason instead of landing on the next one.
- **`/btw` works in `wisp tui`.** It was refused every time; now it asks the open task on the side and shows the answer in the transcript, and says so when there's no question or no task open.
- **The `?` keys in `wisp tui` match what the keys do.** It shows the rows that were missing (`s`, `p`, `h`/←, `/`, Ctrl-A), and the goto row lists every view's letter, a plugin's included (Blame's `b`).

- **Plan mode can't change your files, on every agent.** A Claude Code task in Plan still runs commands, tests and builds, but its shell now runs in a read-only sandbox: a write into the project, an added folder or your home fails, and /tmp stays writable. Where your computer has no sandbox for it (Linux without `bwrap` and `socat`, Windows, wisp Cloud), Plan is Claude Code's own read-only plan mode instead. An opencode task in Plan can't edit even when your opencode config allows it, and its shell only runs reads (`git status`, `git diff`, `ls`, `cat`, `rg` and the like); a Plan task a trigger or a channel started has no shell.
- **A sandbox subagent's changes are never lost on the way back.** When they clash with yours, the patch is kept in a file and the lead's report says where (`git apply <path>` puts it in). A sandbox that's asleep or unreachable no longer counts its changes as brought: they're tried again on the next look. A deleted lead's sandbox that still holds work it never got stays until you delete it in Settings → Computers → Sandboxes.
- **Every job of a sandbox subagent brings its own changes back.** The second and later jobs you gave the same subagent used to come back with nothing (or with the first job's changes again, which then clashed). Each job now brings only what it changed.
- **A subagent whose sandbox is gone gets a new one** on its next task, instead of failing until you remove it from the team.
- **A lead in Plan mode never gets a subagent's changes in its folder**, and neither does any lead from a subagent job that stopped or failed. Those changes are held in a file the report names.
- **A crew job that meets your grid's sandbox limit waits its turn** instead of being dropped. The grid's refusal now says to free a sandbox in Settings → Computers → Sandboxes.
- **The CLI no longer runs your commands on another home's wisp.** With two homes (say `WISP_HOME=~/.wisp-work` beside `~/.wisp`), `wisp ls` and every other command used whichever wisp held the default port, so they listed and changed the other home's tasks. Each wisp now says which home it serves (a hash, never the path), and the CLI and `wisp desktop` refuse one that isn't theirs in one line: "port 4477 is the wisp for another home, not …: pass --port or run wisp serve". An older wisp that doesn't say is still used.
- **`wisp mcp` answers requests at once.** A `wisp_wait` no longer holds up the editor's other calls (a `ping` behind it went unanswered for minutes), a cancelled request (`notifications/cancelled`) stops, and a line that isn't JSON gets a parse error (-32700) instead of silence. Its tool list is the full catalogue even when wisp wasn't running when the editor started (wisp is started for it), and the editor is told when the list changes.
- **`wisp tui` without a terminal** (a pipe, CI) says "wisp tui needs a terminal: try wisp ls --watch or wisp attach <id>" and exits 2, instead of panicking.
- **Scripted commands refuse flags they don't take.** `wisp ls --stat done` or `wisp ls --bogus` showed the whole list as if nothing was typed; now they print the command's usage and exit 2. `--limit` and `--days` take a whole number above 0 (`--limit abc` was dropped without a word), and `--host` is refused anywhere but `wisp serve`.
- **Connectors ask before they act, and stay out of Plan mode.** In Edits a connector's tools now ask you first (wisp's approval card on Claude Code; Codex, which can't ask, refuses them outside Full auto), where every one ran unasked. Plan mode gets a connector only when its `modes` name plan. wisp's own tools still run without a card, and a connector can't take one of their names (`wisp`, `browser`, `computer`), which would have let it run unasked.
- **Connectors meet the harnesses' own servers.** A harness whose own config already has a server by a connector's name keeps its own (the row says "Codex uses its own linear"), an SSE server stays SSE on import, on paste and in the run's config (Codex, which can't speak SSE, goes without it, and the row says so) and is checked over its event stream (also a server that answers the newer transport with a 404 or 405), so its row lists its tools instead of "Didn't start", a sign-in whose token is due is refreshed before a run starts (also after the computer slept), a refresh that fails for now reads "Couldn't refresh, retrying" and is tried again instead of asking you to sign in, and servers you hid with **Not now** stay hidden after a restart and are one click away under **Show hidden**.
- **Sandboxes keep their work and their quota.** A crew job's sandbox is no longer deleted while it's still being set up (the minute's check asked about a task that didn't exist yet), and one crew never named goes after half an hour instead of staying for good. The minute's check only asks sandboxes that are awake, so an idle one finally gets to sleep, and a slow box no longer holds up the others. A sandbox that never came up, or whose owner couldn't be saved, is deleted at once instead of filling one of your 5. A crew member's own sandbox gives each job its own folder (`~/code/<project>-<id>`), so a job no longer wipes another job's live folder or the last job's unpushed work.
- **A failing `.wisp/sandbox.sh` is shown, and runs again.** A `/sandbox` launch whose setup script failed now says so before it opens the task, with the end of what the script printed. A failed run no longer marks the setup as done, so the next seed tries it again.

- **`wisp tui` polish.** A reply follows the open task's mode (the status bar shows it, and a capped task says so) instead of sending the bar's mode, which lifted a channel task's cap without you choosing it; Shift-Tab picks one and warns when that lifts the cap. `Ctrl-E` gives your editor the whole text (long pastes put back) in a private 0600 file, keeps images whose `[image N]` is still there, and says so when the editor didn't save. ↑ history recalls a long paste whole. A new task that didn't start puts your prompt back in the box with a plain "wisp isn't answering" instead of jumping to another task. `/` search moves between matches with ↑ ↓ (Ctrl-N, Ctrl-P), and Enter with no match does nothing and says "No task matches". `o` opens the task in its harness on the TUI's own wisp (`--port`, `--on`) and says when it couldn't. A palette command goes before your draft instead of replacing it, and `i` with no task starts one.

- **`--help` after any wisp command prints its usage and never runs it.** `wisp destroy --help` planned a destroy, `wisp graph --help` printed the graph, `wisp workspace new --help` made a workspace named `--help`, and `wisp run -f x.task.yml --help`, `wisp apply -f x.yml --help` and `wisp secret set FOO --help` ran the command. Now each prints that command's lines and exits 0, whatever follows; a plugin verb's `--help` (`wisp crew create --help`) still lists its flags, and `wisp -- "explain --help"` is still a task. `wisp ship --help` shows `wisp ship <id>` and its `--draft` first, then the ship plugin's verbs under "more:". `wisp tui` and `wisp schema` have lines of their own in `wisp --help`, `wisp list --help` prints what `wisp ls --help` does, and `wisp output`, `wisp graph` and `wisp mv` each have their own line.
- **A mistyped command line never starts wisp in the background.** `wisp serve --bogus` (or any `--help`) started a background wisp just to read the plugin list, then said serve's usage. Help and refusals now read the list from the wisp that's running, or the one it last saved, and otherwise print the built-in usage.
- **Mark done no longer says a worktree "holds work" when it doesn't.** An agent that switched to its own branch in a task's worktree (`git checkout -b fix/x`) left its commits on that branch, so removing the folder loses nothing, but the Done dialog warned "Commits off its branch" and wouldn't remove it unless you ticked it. Now only commits that no branch keeps (a detached HEAD) count as work the folder holds.
- wisp Cloud computers' usage stats (app started, task turns) now carry the address you reach your computer from, so they're placed where you are instead of all showing the grid's server in Germany.
- **`/wisp` comments on GitHub work again.** A task started from a comment on an issue or PR is now told which one it's on (repo, number, link and the `gh` command to read it), runs in that repo's project instead of your default one, and posts its report back on the issue or PR. A later `/wisp` there follows up the same task. Slack, Discord, Telegram and email lost the same link: a task started from a message there is tied to its thread or chat again, so replying to it follows that task up.
- A command wisp runs with a time limit (a harness probe, a git read) now always stops at that limit. One that closed its output and kept running used to hold wisp waiting for as long as it ran.
- **Stop on This Mac never signals the wrong process.** The list can be up to a minute old, and by then a process that ended may have handed its number to another. Stop now checks it's still the same process (by when it started) before it sends anything, and before the kill that follows 5 seconds later. If it isn't, Stop says it already ended.
- A terminal that failed to start halfway no longer leaves its program running in the background, outside every way wisp ends its processes.
- wisp's memory no longer grows with every deduplicated event for as long as it runs: keys older than the 24-hour dedupe window are now dropped.
- An opened step in the transcript stays open when the turn ends, when Show thinking or a hidden tool call moves the rows, and a step opened in a folded group keeps the group open. Before, the open step could jump to its neighbour, or close as the turn finished.
- **Antigravity's models pick like every other harness's.** The picker used to list one row per model and thinking level (`Gemini 4 Neon (Low)`, `(Medium)`, `(High)`, `(XHigh)`). Now each model is one row ("Gemini 4 Neon"), and its thinking-level selector shows only the levels agy has for that model. A model saved as `…-xhigh` no longer fails with "conflicts with --effort".

### Added

- **Find in task**: press ⌘F (Ctrl+F) on a task, or pick **Find in task** in its menu on a phone. wisp searches every step of the task: labels, commands, outputs, and the agent's messages, including steps inside folded groups and closed steps. It also searches older turns that haven't loaded yet, which the server searches for you. **Enter** and **Shift+Enter** (or the arrows) go to the next and previous match, which opens its turn, group and step and highlights the words. A hidden .env value is never matched unless you revealed it.
- **Copy a step's output**: an opened step has a Copy button in its corner. It copies what the step shows: a .env file's masked text, or the value once you revealed it.
- **Copy turn as Markdown**: a turn's tools (in the answer's menu on a phone) copy the prompt, each step with its output in a code fence, and the answer as the agent wrote it.

## [1.0.0-alpha.15] - 2026-11-26

### Added

- **Keep it green (Steward)**: tick **Keep it green** in Ship it, or press **Keep green** on the PR row, and the task's PR is looked after until it's green. When a check fails, the task gets the end of its log. When someone leaves a review comment, the task gets it too. wisp then pushes the fix. A second box, **Another vendor reviews each push**, has a harness on another company's model review each push, and its serious findings go back to the task too. The Steward never merges and never re-runs a check. It does nothing when the check fails on `main` too. It stops and tells you when the same failure comes back, when a pass changes nothing, or after 3 tries per kind of problem (6 in all). The PR row says what it's doing and has **Stop Steward**. Settings → Ship sets the default. From the CLI: `wisp ship <id> --steward fix|review|off`.
- **A browser for every agent**: Settings → Agents → **Browser** picks what agents drive: **Default** (Claude Code follows its own `/chrome` choice, as before), **Off**, **On (Chrome)** (Claude in Chrome on Claude Code, Playwright on your Chrome for Codex, opencode, Copilot and Grok) or **Headless** (Playwright's own hidden browser, a fresh profile each run). Each harness can have its own, from its page in Settings → Agents or the **Browser** row in the harness picker. Cursor and Antigravity can't take one and the picker says why. In a sandbox, any choice but Off uses the sandbox's own browser, the one you can watch; On (Chrome) is unavailable there because Claude in Chrome needs your own Chrome. In Edits mode, Claude in Chrome's clicks and screenshots no longer each ask for approval, and a browser step that does ask names the page ("open https://example.com").
- **Computer use for every agent**: switch on **Computer use** in Settings (`computer.enabled`) and every task's agent can see and use this computer's screen, whatever its harness (Claude Code, Codex, opencode, Copilot, Grok). The agent gets a `computer` tool set: screenshots, clicks, typing, keys, scrolling, windows, apps and menus, through [Cua Driver](https://github.com/trycua/cua). A task in Plan mode can only look at the screen. wisp never offers the driver's own settings, installs or recordings, and turns off its telemetry. Install Cua Driver first (`curl -fsSL https://cua.ai/driver/install.sh | bash`); `wisp computer status` says whether it's ready.
- Cloud computers and sandboxes come ready for computer use: their agents can use the box's screen and Chromium (the one you watch in the task's browser view) with no setup. In a sandbox the agent's browser tools drive that same Chromium, so it can open what it builds and check it on screen.
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

- When a task hits a usage limit, the card's **Hand to** button now picks the harness with the most plan left (from Settings → Usage's windows) and skips one that's used up too. Before, it picked the first one set up. The button's tip says how much is left.
- The npm package, the README and the site's structured data now say **Wisp Agents** and point to trywisp.dev, so searches for "Wisp Agents" find the project.

### Fixed

- Task chat: a long tool output you open has **Show all N lines** under it. The step shows the start and end of a long output, and the server keeps the whole text, so you can still read the part in the middle. wisp loads it only when you click. An output that read a .env file, or that quotes one of its values, is never kept whole.
- Task chat: a failed tool call keeps the end of its error, where a test run or a build says what went wrong. Before, only its first 400 characters were kept. The error now shows on the call that failed, marked red, with the error as the call's output, instead of as a row of its own under a call that looked fine. This works on Claude Code, Codex and opencode.
- opencode 2 tasks get wisp's tools again (`ask_user`, delegating, saving, connectors, the browser). When another opencode was already running on the computer (yours, or one wisp started to list models), a task's run joined it and none of the tools wisp gives a run reached the agent. Each run now has its own opencode server.
- Phone browser and the installed web app: a task's notification shows again (Chrome on Android refused the page's own, so none appeared), and tapping it opens the task, in the open tab or a new window.
- Android app: a task's newer notification replaces its earlier one instead of piling up beside it, and opening the task clears it. The installed web app clears it too.
- The installed web app is in the phone's share sheet again: what you share opens New task with it in the box and its files attached, ready to send.
- **Task chat: the agent's thoughts show as messages.** A thought no longer folds into the commands pill or shows as a one-line step to expand; it reads in full on the rail like the agent's other messages, in Chat and Transcript, running or finished.
- Crew: two jobs given to a member at once both wait in their queue (one was lost), an edit saved while a waiting job started or a new one was queued (from wisp or from an agent's `wisp_crew_edit`) no longer puts that job back in the queue or drops the new one, and a member removed while a job for them was being placed or a check-in ran stays removed.
- Crew: a wake-up a member asked for is kept until its check-in starts. Before, a wake-up that came while they were still checking in, or with their queue full, was dropped.
- Crew: a message that tags several members (`@ada @bob`) wakes every one who can take the job. Before, one with a full queue stopped the rest from waking.
- Crew: `wisp validate`, `wisp diff` and `wisp apply --dry-run` report a member's `cron` that can't be read and a wake the crew would refuse (`crew.ada: wakes[0]: a schedule needs cron`), instead of failing half-way through the apply. Saving a member with a wake on an event no source offers (`on: not.an.event`) is refused with the list of sources, as a trigger is, from wisp and from an agent's `wisp_crew_add` and `wisp_crew_edit` alike.
- A crew check-in or a scheduled run keeps its worktree until you mark it done or archive it. Its worktree and branch were deleted as soon as a turn ended, even when the run had just asked you something. A follow-up then failed with "its branch is gone: it was merged", though nothing was merged.
- A follow-up to a task whose worktree and branch are gone runs. It goes on in a fresh worktree from main, and a line in the turn says the worktree was deleted. It was refused.
- wisp Grid, shared links: a password-protected share takes 10 tries a minute from one address and 60 a minute in all, then says *Too many tries. Wait a minute and try again.* The person who shared it can always open it. Checking passwords no longer slows the rest of the grid: when it's busy, it answers *try again in a second*. Unlocking a link to one version (`/s/<id>?ref=v1`) now opens that version, not the latest.
- wisp Grid, shared links: the 200-share limit counts only live links. Revoking a link, or letting it expire, frees a place, and the message says so: *You have 200 live shares, as many as this grid keeps. Revoke or let one expire to make room.*
- wisp Grid, signing in a device: the approve page says *A device calling itself “<name>”*, shows where the request came from and how long ago, and warns when it came from a different network than yours. An address can ask for 10 codes a minute and have 20 waiting at once, expired codes are cleared at once, and `/activate` takes 30 code look-ups a minute.
- wisp Cloud: a share you didn't publish to the gallery no longer opens by its name alone. Anyone who knew your handle could open `grid://<you>/crew` (your whole crew) or a setup by its name. An unlisted share now opens by its link, or by a name that carries its key, `grid://<you>/crew#…`, which is what the share shows you; you, and the people or teams it's for, still open it by its plain name. A published share keeps its plain name. An old plain name for an unlisted share now says "this link is unlisted: ask its sharer for the new link". `wisp init` keeps the `#…` in a module's source.
- Share again on a task with a passkey, without typing the passkey, keeps the passkey. It made the link open to anyone. The "copied, with its passkey" note now says what the link really is.
- Sharing the crew again keeps who can open its link (some people, a team, or a password set with `wisp cloud access`) instead of opening it to anyone. The Share dialog says who opens it, with **Open to anyone with the link** to change that.
- `wisp wait` and `wisp "…" --wait` exit with how the task ended: 0 done or review, 1 failed, stopped or blocked, 2 when it asks you something, 124 when the wait timed out. So does `wisp "…" --try … --wait`, with the worse of its two runs. They exited 0 every time. `--timeout 30s|10m|2h` sets how long they wait (30 minutes by default), and `--json` says `"timedOut": true` when the wait ran out.
- Installed from npm, one Ctrl-C in `wisp attach` detaches again, as it says. The launcher sent the terminal's Ctrl-C to wisp a second time, and a second Ctrl-C stops the task. `kill <pid>` and supervisors still reach wisp through the launcher.
- Only one wisp server runs on a home (`~/.wisp`). A second `wisp serve` says `wisp is already running for this home at <url> (pid N)` and exits, instead of running every trigger and schedule a second time and taking over `server.json`. A command finds the running server instead of starting another.
- `wisp merge last`, `wisp ship last` and a short id merge or ship that task. They launched an agent task with those words. `wisp held <id> start` with an id that names no task, and `wisp merge x`, say `no task x` and launch nothing.
- A new task from the TUI gets its own worktree in a git repo and is filed under its project, as `wisp "task"` from the same folder is. It edited the checkout in place.
- wisp Cloud: opening your cloud wisp while it sleeps shows its waiting page at once instead of a blank tab for up to 45 seconds. The page says *Waking your wisp*, or *Waiting for room* with your place in line when the grid is full, and opens your wisp as soon as it runs. If it can't start, the page says why, with **Try again** and **Home**, instead of a dead-end error page. A full grid now puts the start in line and starts it as soon as another box stops (people ahead of scheduled tasks), instead of refusing it. Scripts and the API still get 503, now with `retry-after`. When two boxes stop at once, everyone in line who may start does, even when one person's sandboxes are at their limit. A start that no one waits for any more (its tab closed) leaves the line after `hostd.queueMs` (2 min) instead of taking a slot later. Opening a box whose start just failed shows why straight away, and **Try again** starts it again without showing the old failure.
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
- Factories: a run wisp restarts in the middle of starting a station picks up where it was. It starts only the station tasks it hadn't started and sends only the follow-ups it hadn't sent. Before, it could end failed with "the spec station made no task", or take round 1's review as round 2's.
- Factories: a run that fails now stops its station tasks, as a stopped run does. A build whose second agent couldn't start no longer leaves the first one running. A finished run still keeps its build for you to read or merge.
- Factories: `factory.set` takes the loop as `back_to` (and `backTo`). Before, its tool said `backTo`, and a factory written that way was refused.
- Factories: the run's band shows the start of the spec next to **Approve the spec?**. Before, it showed only the question.
- Your harness can no longer answer its own **Your harness wants to…** card by naming the card's task inside `args` (`{"args": {"id": …}}`), nor by `sessionId` or `session`: wisp refuses it as it does a top-level `id`.
- Your harness adding or changing a connector's command or URL, testing or importing one, or setting a project's test command now asks you first on a **Your harness wants to…** card, as switching off a safety guard does. Nothing is stored or started until you allow it. Switching a connector on or off still applies at once.
- Connectors: a `${secret:…}` in a connector's headers or environment is read once and only from that connector's own secrets or a formula's (`wisp secret set`): another connector's token or a plugin's secret stays as written, and a secret whose value names itself no longer hangs wisp. A sign-in token is sent as it is.
- Connectors: changing a signed-in connector's URL to another server signs it out (its row shows **Sign in**), so the first server's token never goes to the new one. A sign-in token is never sent over plain `http://` to another computer.
- Slack, Discord and Telegram: a link code links one person and is then used up (**Relink** shows a new one), and five wrong codes from anyone change it, as Settings then shows. `/wisp link` on Discord and codes sent to an unlinked Telegram bot are limited to three tries a minute per person, as Slack's and Discord's `link` already were.
- With more than one computer connected, the project order (⌘1–⌘9, ⌘K, the task list, Works in, Repo, Run a recipe) and the crew tint now come from the computer you are on, not from whichever computer answered first. On the phone, switching computers moves the task list's project order and its Scheduled and Drafts to that computer without a reload.
- Settings search opens the pane that shows the setting (**Accept tasks from channels** and **Default project** open Notifications, General's and Chat's rows open General and Chat), scrolls to its row, lights it for a moment and focuses its control. A search hit in Settings → Agents (a harness's default model, mode or stop line) lands on that row too, also when Agents is already open on another harness, which it now switches to.
- Changes: **Show more lines** on a very long diff now adds 1,000 lines per click and says how many are left, instead of showing the whole file at once and freezing the panel. Preview finds the app's address once as its output comes, so it keeps working after a megabyte of logs, and each request no longer re-reads the terminal's output. ⌘K and the project counts look up each task's project faster, and the server opens faster on a long event log.
- **@ mentions are kept**: a task, crew member or My Stuff item you picked after **@** still goes with the prompt after a reload, after switching to another task and back, when **↑** brings back a prompt you sent, and when a send or a New task launch is refused and the words come back. The box shows `@Title` again, never the stored `@[Title](wisp:…)` form. `/side` no longer carries its mentions into the next prompt. When two picked titles overlap (`Auth` and `Auth bug`), each `@` is matched to the longest whole title, and every place you typed it is sent as the mention.
- **@ offers only tasks the agent can read**: the task list after **@** now shows only the tasks on the computer the prompt goes to (New task's **Run on**, or the task you reply to), with that computer's name when you have more than one. A pasted task id from another computer is pasted as text, not taken as a mention that would be dropped.
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
