# Long task history: how the best products show it

Research for the v2 task page (2026-11-14). The problem today (v1 `web/js/features/turns.js:232`):
the latest turn is open, every older turn is a one-line row (chevron, first line of the message,
time), "Show 426 earlier turns" sits at the top, and wisp's own system messages ("Subagents you
delegated to earlier have finished…") are rows in the same list as the owner's messages. On a
phone with 430+ turns there is no way to find a moment, no sense of what each turn achieved, and
the owner's words drown among automated ones.

"Verified" means a doc, changelog or PR says it. Items marked *(from use)* are well-known
behaviour I did not find a doc for this time; check before quoting them in marketing.

## 1. What each product does

### Chat assistants

**ChatGPT** — One long scroll; long user messages get "Show more". The web app has a small
prompt minimap or table of contents for long chats, but the mobile and desktop apps don't
(users ask for it: [1](https://community.openai.com/t/feature-request-add-conversation-navigation-to-chatgpt-apps/1392883)).
The forum is full of requests for an outline, bookmarks, a prompt-first view that folds the
answers, and next/previous message controls
([outline](https://community.openai.com/t/feature-request-in-thread-prompt-navigation-panel-table-of-contents-for-the-current-conversation/1385831),
[bookmarks](https://community.openai.com/t/feature-request-bookmarks-pinning-for-long-chatgpt-conversations/1375695),
[prompt-first view](https://community.openai.com/t/feature-request-prompt-first-view-collapse-assistant-responses-for-long-chats/1380760),
[prev/next](https://community.openai.com/t/feature-request-add-message-by-message-navigation-controls-to-long-chatgpt-conversations/1389024),
[minimap](https://community.openai.com/t/highlight-mini-map-for-conversations/1401686)).
Lesson: the biggest chat product still gets this wrong, and its users want **an index of their
own prompts**.

**Claude.ai** *(from use)* — Same single scroll. Tool use and thinking fold into a one-line
summary you can open in place. Artifacts open in a side panel, so the chat stays short. Branches
from an edited message show as `< 2/3 >` on the message instead of extra rows. No outline, no
search inside a chat.

**Claude Code CLI** — Each tool call is a one-line summary ("Read 3 files", "Queried {server}"
since v2.1.81). Ctrl+O toggles the full transcript. The fullscreen focus view (v2.1.97) shows only
the prompt, a one-line tool summary with edit diffstats, and the final answer. Transcript search
arrived in v2.1.83: `/` to search, `n`/`N` to step through matches
([summary](https://www.memorylake.ai/en/blogs/claude-code-transcript-view),
[search](https://agentpatterns.ai/observability/transcript-search/)). Users keep filing requests
to collapse or group tool output so the answer isn't buried
([CC#24311](https://claudeissues.com/issue/24311-complex-turns-important-text-output-gets-buried-between-tool-calls),
[#49646](https://claudeissues.com/issue/49646-feature-vscode-extension-configurable-tool-call-visibility-hide-collapse-group)).
Lesson: **a turn has three parts: ask, work, answer.** Fold the work, keep the ask and the answer.

### Agent IDEs and session UIs

**Cursor** — Each code-changing prompt gets a checkpoint. Hover an earlier message to get
*Restore checkpoint* ([docs](https://docs.cursor.com/agent/chat/checkpoints)). Forum threads say
the button keeps disappearing, and cloud/background agents don't have it
([1](https://forum.cursor.com/t/restore-checkpoint-missing/167908),
[2](https://forum.cursor.com/t/cursor-3-where-are-the-checkpoints/157319)). Tool calls fold in
place *(from use)*. Lesson: **actions belong on the turn row**, and they have to be reliable.

**Windsurf** *(from use)* — Cascade is like Cursor: steps fold inline and you can revert to a
step. Nothing new for long-history navigation.

**Devin** — A step list sits beside a Progress view that combines shell, editor and browser for
the selected step. You can scrub the timeline (steps after the scrub point go grey) and press
*restore checkpoint* ([Sept 24 update](https://cognition.ai/blog/sept-24-product-update),
[session tools](https://docs.devin.ai/work-with-devin/devin-session-tools),
[guide](https://fast.io/resources/devin-session-tools-guide/)). After the run, Session Insights
adds an Issue Timeline: a colour-coded strip of key events
([insights](https://docs.devin.ai/ja/product-guides/session-insights.md)). Lesson: **a scrubber
with coloured events** works for runs of hundreds of steps. The opened step shows in a separate
pane, not inline.

**Factory** — The app lists sessions across projects and machines, filters and groups them, and
lets you preview output and comment on specific changes
([overview](https://docs.factory.ai/factory-app/overview.md)). I found no doc for how it shows
a long session.

### wisp's direct competitors

**Conductor** — Has done the most work here, all in its changelog:
- Long Claude messages collapse automatically ([0.2.2](https://www.conductor.build/changelog/0.2.2-bugfixes)).
  A later "more accurate algorithm" collapses less-important agent messages, including Claude
  Code background agents ([changelog](https://www.conductor.build/changelog/page/6)).
- A **table of contents of overview bars**, one per turn, capped at 50 turns. Hovering it shows
  a short summary of the chat
  ([0.34.2 chat summaries](https://www.conductor.build/changelog/0.34.2-chat-summaries-re-run-actions-latex),
  [listing](https://www.conductor.build/changelog/page/8)).
- ⌘F find searches the thinking too, and jumping to a match **opens the folded group that hides
  it** ([changelog](https://www.conductor.build/changelog/page/7)).
- Subagent tool calls nest under their parent ([0.22.1](https://www.conductor.build/changelog/0.22.1-gpt-5-1)).
  "Garry mode" turns off folding for people who want everything open (0.41.0).
- Long chats load the newest messages first (0.92.0, Oct 2026).

**T3 Code** — Settled turns fold their work behind one row labelled with how long it took
("Worked for 30s"). While folded, only the newest entry shows (it was 6)
([PR #3022](https://github.com/pingdotgg/t3code/pull/3022), merged June 2026). Work-log groups
start folded ([PR #1345](https://gh.r-mj.com/pingdotgg/t3code/pull/1345)). Lesson: **the
folded row states an outcome** ("Worked for 30s"), not the first line of the prompt.

**Paseo** — Runs agents on a local daemon and shows them on phone, desktop, web and CLI
([README](https://cdn.jsdelivr.net/gh/getpaseo/paseo@main/README.md), [docs](https://paseo.sh/docs)).
Nothing documented on long-history UI. Its CLI `attach` streams live output.

**cmux** *(from use)* — A terminal multiplexer for agents. Its history is the terminal scrollback,
and it marks panes that need you with a notification ring. No turn model, so nothing to copy for
history.

### Work trackers and code review

**GitHub PR / issue timeline** *(from use, plus the
[changelog](https://github.blog/changelog/2026-10-08-screen-readers-can-navigate-timelines-as-lists))*
— A long timeline shows **the start and the end, with "N hidden items · Load more" in the
middle**. Load more pulls a batch from both sides. Resolved review threads fold to one line
naming who resolved them. Bot and system events ("added label", "force-pushed") are small grey
rows with an icon, clearly unlike comment cards. Pain: you can't load everything in one click.
Extensions exist only to press Load more over and over
([Better GitHub](https://addons.mozilla.org/firefox/addon/a-better-github),
[userscript](https://greasyfork.org/scripts/470520-auto-expand-github-issues/code)).
Lesson: **fold the middle, not the top.** The first post (the ask) is the anchor.

**Linear** — Similar events in a row are grouped ("changed status 3 times"), and older activity
between comment threads folds so the feed stays on the conversation
([changelog 2025-04-03](https://linear.app/changelog/2025-04-03-collapsed-issue-history)).
Comments are full cards; history lines are small text with no avatar bubble.

**GitLab** — Has an explicit switch: *Show all activity / Show comments only / Show history
only* ([system notes](https://docs.gitlab.com/user/project/system_notes)). Lesson: **one
filter to hide machine events**, remembered per user.

### Messaging and mail

**Gmail** — A thread opens with the first message and the newest one or two open. The read
messages between them fold to a single "N older messages" line, and each message is a one-line
row with **sender, avatar, snippet and date**. Expand all is in the corner
([Stony Brook guide](https://it.stonybrook.edu/help/kb/understanding-conversation-view-in-google-mail),
[Wikipedia](https://en.wikipedia.org/wiki/Gmail_interface)). Same middle-out fold as GitHub,
and **a collapsed row says who and a snippet of the content**, not just a title.

**Superhuman / Front** — Same model as Gmail, driven by the keyboard. Front: `;` expands all,
`Shift+;` collapses all ([Front help](https://help.front.com/en/articles/2416)). I found no
Superhuman doc. *(From use:* Superhuman folds older messages to a sender-and-snippet row and
lets you step through them with the keyboard.)

**Slack threads** *(from use)* — In the channel, a thread is one row: "N replies", reply
avatars, "last reply 2h ago". Opening it shows a side panel on desktop and a full-screen page on
mobile. Bot and app messages carry an `APP` badge, and "joined the channel" lines are greyed
and can be hidden. Jump to date sits on the date divider. Lesson: **open a turn somewhere
else**, which keeps the main list short.

**iMessage / Telegram** *(Telegram partly verified)* — Sticky day separators. Telegram's
in-chat search has a calendar to jump to a date, and you can long-press a date header to get
"Jump to date" ([tracker](https://bugs.telegram.org/c/63155),
[tracker](https://bugs.telegram.org/c/39495/2),
[tdesktop source](https://upd.dev/telegramdesktop/tdesktop/src/branch/dev/Telegram/SourceFiles/data/data_search_calendar.cpp)).
Search results show as a list with ▲▼ to step through matches in place. A floating ↓ button
with an unread count brings you back to the bottom. Lesson: on a phone, **time and search are the
jump tools**, and a "back to latest" button is essential.

### Version history

**Figma** — Autosaves group into folded runs you open with a click. Named versions stand out
with a title and description. A filter shows or hides autosaves
([help](https://help.figma.com/hc/en-us/articles/360038006754),
[forum tip](https://forum.figma.com/share-your-feedback-26/tip-copy-and-paste-from-version-history-38485)).
Lesson: **machine checkpoints fold and human milestones stand out**, which is exactly wisp's
"system rows vs the owner's rows".

**Notion** — Version history is a side list of snapshots by time with a full preview. Restore
the whole page, or copy blocks from an old version
([help](https://www.notion.com/help/duplicate-delete-and-restore-content),
[guide](https://thomasjfrank.com/learn-notion/edit-history-trash/)).

**Arc** *(from use)* — Arc has no conversation history. Its relevant ideas are Little Arc and
peeking: open something in a light overlay and dismiss it without losing your place.

### Terminals

**Warp** — Each command and its output form a block. Arrow keys move between blocks,
⌘⇧↑/↓ jumps to a block's top or bottom, **⌘B bookmarks a block** so you can jump back, and a
**sticky header** keeps the command visible while you scroll its output. Compact mode tightens
spacing ([block basics](https://docs.warp.dev/terminal/blocks/block-basics),
[actions](https://docs.warp.dev/terminal/blocks/block-actions),
[blocks behavior](https://docs.warp.dev/terminal/appearance/blocks-behavior.md)).
Lesson: **keyboard stepping between turns, plus bookmarks.**

## 2. Patterns at a glance

| Question | Best answers seen |
|---|---|
| Compress old items | Fold the **middle**, keep the first ask and the latest (Gmail, GitHub); group similar machine events (Linear, Figma); fold a turn's work behind an outcome row (T3 Code, Claude Code) |
| Jump around | Overview-bar TOC with hover summary (Conductor); coloured scrubber (Devin); date jump (Telegram, Slack); search that opens folds (Conductor, Claude Code `/`); bookmarks (Warp) |
| Show an opened item | Inline for short items (Gmail, Claude Code); side panel or full page for big ones (Slack, Devin, Notion) |
| System vs human | Small grey rows with an icon and no bubble (GitHub, Linear); a filter for comments only (GitLab); an APP badge (Slack); autosave vs named (Figma) |
| Outcome per item | "Worked for 30s" (T3 Code); diffstat in the summary (Claude Code focus view); resolved-by line (GitHub) |
| Mobile | Thread as a full page (Slack); sticky dates, calendar jump, ↓ with count (Telegram, iMessage); ChatGPT's mobile app has no navigation, and users complain |

## 3. Ideas for wisp, ranked by value

**1. Outcome rows, not first-line rows** (from Gmail, T3 Code, Claude Code focus view). *Size S/M.*
Each folded turn becomes one fixed-column row: who (you / agent / wisp icon) · **your ask's
first line** · **what came of it**: `edited 4 files +120 −8` · `tests 48✓ 1✗` · `PR #812` ·
`4m 12s`, or a status pill (asked you, failed, stopped). The data is already on each turn
(changes, receipts, duration). Columns stay fixed, so rows never jump (owner rule). This fixes
the main complaint: today a row repeats the prompt and doesn't say what happened.

**2. Split the owner's turns from wisp's own** (from GitHub/Linear grey rows, GitLab filter,
Figma autosaves). *Size S.*
wisp-originated turns ("Subagents … have finished", crew follow-ups, auto-continue after a reset)
render as small grey one-line rows with no bubble, and runs of them group ("3 wisp updates ▸").
A sticky segmented control at the top: **All · Mine · Milestones**. Remember the choice per
device and expose it as a Settings option and spell prop. A 430-turn task where most turns are
automatic becomes a list of the few dozen things the owner actually asked.

**3. Middle-out folding, with the original ask pinned** (from Gmail, GitHub, Warp sticky header). *Size S.*
Show turn 1 (the ask), then `▸ 426 turns · Nov 2 – Nov 13`, then the last 3 turns. Load more
opens 20 from each edge, plus *Open all*. Turn 1's first line stays as a sticky header under the
task title while you scroll, so you never lose what the task is for. Today's
"Show 426 earlier turns" link at the top is the worst of both: the ask is hidden and the count
means nothing.

**4. Turn rail / scrubber with day marks** (from Conductor overview bars, Devin coloured timeline, Telegram calendar). *Size M.*
Desktop: a thin rail on the right of the task, one tick per turn, coloured by kind (yours, wisp,
failed, asked you, has a PR or commit), with day labels. Hovering a tick shows the outcome row
from idea 1; clicking it jumps there and opens the turn. Phone: a **Jump** button opens a bottom
sheet grouped by day ("Today · Yesterday · Nov 11 …") listing only Mine and Milestones, with a
calendar at the top. Add a floating ↓ *Latest* button with a count of new turns. Better than
Conductor's: it scales past 50 turns by grouping by day, and it marks wisp-specific events that
no single harness has (cross-harness handoffs, subagents, resets, review by another model).

**5. Search in the task that opens folds** (from Conductor ⌘F, Claude Code `/` + `n`/`N`, Telegram ▲▼). *Size M.*
⌘F (or a search icon on phones) searches every turn's ask, answer, tool output and file names.
Results show as a count with ▲▼ stepping. Each match opens the folded turn or work group it's
in and highlights it. On the server this is a query over the stored session, so the page never
loads 430 turns to search them. Plain Claude Code only searches the live session; wisp searches
the whole task across harness handoffs and resumes.

**6. Milestones and pins** (from Figma named versions, Warp ⌘B bookmarks, Devin restore checkpoint). *Size S/M.*
Mark turns as milestones automatically: commit made, PR opened or merged, tests went green,
handoff to another harness, the owner marked it Done. The owner can also pin any turn (long-press
on a phone, ⌘B on desktop). Milestones feed the Milestones filter (idea 2) and the rail (idea 4).
Each milestone row carries its actions: *Fork from here*, *Restore checkpoint*, *Open diff*
(wisp already has fork-from-any-turn and per-turn checkpoints). Cursor's restore button is
hover-only and unreliable; here it's always visible on the row that earned it.

**7. Open a turn as a focus page** (from Slack threads, Devin's Progress pane, Claude Code focus view). *Size M.*
Tapping a turn row opens it on its own page on phones (Back returns to the same scroll spot)
and in a side pane on wide screens. The page shows the ask, then the answer, then the work
folded by step, with ‹ › to move to the previous or next turn. The main list stays rows only,
so opening a turn never pushes the list around. This replaces today's inline open, which on a
phone pushes the list off screen.

**8. Day dividers and smart default focus** (from iMessage, Slack, Gmail "open the unread"). *Size S.*
Add sticky day dividers in the list. Open the task at the **first turn the owner hasn't seen**
(or the latest one that asked them something), not always at the bottom. Cheap, and it makes
coming back to a long task feel like reading a thread.

My pick to build first: **1 + 2 + 3 together**. All three are S, they need no new data, and
together they turn the screenshot's 430 identical rows into the ask, a dated fold, a handful of
the owner's turns with outcomes, and grey wisp updates. Ideas 4–5 are the navigation layer on
top; 6–7 give it depth.

## 4. Open questions for the owner

- Should Mine be the default filter on phones (fewer rows) while desktop defaults to All?
- Should wisp-originated turns count toward the turn number shown on rows and in the rail?
- Is search across a task's history a Settings option, or always on? (It costs a server
  index per task.)
