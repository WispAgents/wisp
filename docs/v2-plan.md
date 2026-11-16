# wisp v2: the plan, the features and where everything stands

**This is the one file to read.** The lead (Mokhles) updates it every turn and works from it, not from memory.
Every item is marked: `[x]` done, `[ ]` to do, `[~]` dropped (an ADR says why), **in progress** (who), **blocked**
(why). Details live in the files it points to (`docs/v2-queue.md` for the long notes per item, `docs/v1-features.md`
for each feature's evidence, `docs/adr/` for decisions, `CHANGELOG.md` for what users see).

_Last updated: 2026-11-15 01:40, round 234._

---

## 1. Goal

**wisp v2 architecture done with every wisp v1 feature (or better), no bugs, no UI smells, no code smells.**
Rust for everything but the UI, Solid/TS for the UI, everything a plugin (ADR-0003, ADR-0013). PR #756.

## 2. Status at a glance

| | Now |
|---|---|
| v1 features | **315 built · 7 partial · 5 missing · 4 dropped** of 331 (§6) |
| Regressed | none (F-comp-1 picker restored to v1 in round 233) |
| CI, last full run (56405a3a) | Linux + macOS: one brittle test (platform-controls read only the last reduced-motion block) — fixed in the test; Windows running |
| Budgets | initial JS 107.2 KB br (≤ 120) · CSS 27.1 KB (≤ 30) |
| Weekly usage | 0 % (reset applied) · **owner 2026-11-15: one coder at a time** |
| Coders running | none |

## 3. Owner decisions (every one, dated; read before any change)

The owner's decisions, from every conversation since v2 started (2026-11-07). Each coder brief says "read §3 first".
A decision here wins over an older doc, a v1 habit or a coder's idea. When a decision is new, it goes here the same
turn. When a screen or feature contradicts one, that's a bug: fix it toward the decision. Quotes are the owner's words.

### Product and positioning

- **wisp is a harness wrapper** for every harness (Claude Code, Codex, opencode, Cursor, …); the user doesn't read long
  agent sessions: wisp shows a presentation of what was done; you follow up from wisp (2026-10-16).
- **Wisp as code** (2026-11-08): everything declarable in files (agents, crew, machines, tools, MCP, skills, tasks,
  formulas), imported, run with `--formula`, deployed to the grid; GUI, TUI and CLI are wrappers over it. Not "one
  file": references and modules like Terraform (11-08 23:52). Import happens in the UI too (11-09).
- **Three pillars: Declare · Run · Track** (11-08 23:49, 11-09 00:55); "Manage the code your agents write"; Run =
  "every harness, any machine, from anywhere"; Track = remember, rerun, blame. Hero leads with Track and open source,
  Declare and Run support (11-09 00:53). Site examples show every way: CLI, SDK/code, GUI, TUI (11-08 23:02).
- **`.wisp/` in a repo** holds the tasks that produced the code (11-08 22:58); the user can add a task to the repo's
  `.wisp` (11-08 23:31); `wisp ls` has a repo flag (23:45); a wisp-init'ed project shows its tasks apart (another
  tab) in GUI and TUI (23:45). `wisp blame` takes commit ids and PRs, not only file:line, shown creatively in GUI and
  TUI (23:09). Turn on tracking in our own repo once v2 runs our tasks; don't backfill now (11-10 00:05).
- **Save the full transcript always**, not just the report (11-08 23:59); "You read a report, not a transcript" is
  an option (default in the apps), not the headline (11-09 00:05).
- **Sharing goes through the grid** (wisp Cloud) as the easy option; files still work (11-09 00:42). The grid is the
  hub for shared things and remote machines (and does the tunnelling) (11-09 01:20).
- **Meet users where they work**: every feature works inside wisp and from the user's harness (skill, MCP), git, CI,
  chat; never require a flow or harness change (11-09 01:19). Test wisp's skills and MCP from other harnesses in a
  wisp-init'ed repo (11-09 21:39).
- **"Open in <harness>"** replaces "Take over" everywhere (11-09, option 1).
- **docs.trywisp.dev**: big docs on every aspect, like big projects' docs (11-09 00:48). Site inspirations: warp.dev,
  openclaw.ai, herdr.dev (11-09 00:52). Full docs/site rework at the end.
- **Sandboxes**: in the plan; they use the main box's subscriptions; usable as the main prompt, a delegate and judges
  (11-11 19:52).
- **Crew and triggers** get more ways to start tasks and crew work; an event system that is fast and clear (11-09).
- **PostHog events as in v1**, behind a swappable analytics interface (11-08 18:17).
- **The CLI does everything the GUI does** (11-08 18:35), can read the terminal it ran from ("check the error above")
  (18:36), shows blocks well, not raw JSON (18:48); **a TUI mirrors the GUI** (18:49), inspired by opencode / Claude
  Code, not simple (11-09 20:52). CLI, TUI and GUI are plugins (18:49).

### Architecture and code

- **Rust for everything but the UI**: backend, local runners and every plugin's backend; Solid + TypeScript only for
  UI (11-07 22:54). Everything is a plugin and swappable (11-08 18:49, 11-09 01:30).
- **The phone app is a separate UI**, not a remote webview (11-07 22:58).
- **monoflow always** (11-07 22:16). **Performance is a priority**: the fastest dev tool (11-07 22:29).
- **Code quality and architecture respected strictly** (11-08 00:46).
- **No v1 back-compat** (11-08 21:20).
- **Port the intent, not the bugs**: check v1 bugs whenever porting and fix them in v2 (11-08 18:19). Check v1's
  changes often and port them (11-10 01:39).
- **Keep every v1 feature and UI detail**: "v1 ui and features are amazing, make sure we don't lose any" (11-10
  01:01); docs/v1-features.md is the inventory.

### UI (browser and desktop)

- **v2 looks exactly like v1** (11-08 01:12): spacing and paddings consistent, states right (hover/active padding);
  the base kit must be right first (11-08 01:08).
- **No sloppy wrapping**: rules for when components/text wrap and when never; icons instead of labels when tight
  (11-07 22:46).
- **Immediate feedback on every action**: something happens at once (loading, navigate, something appears) (11-08
  01:19).
- **One choice = one control**; UI fixes go toward v1's exact control, never a new arrangement (11-15 00:04, the
  harness picker: v1's one pill and one two-pane popover).
- **Project selector** (11-12 01:00): **a selector instead of the chips/cards**, plus a shortcut opening a palette to
  search and switch (⌘P), done "the way good project-management apps do it". Built 10d8f30f; recorded as an allowed
  difference from v1 (design/v1-reference/allowed.md).
- **Multi-project pick** (11-13 22:44–46): a task can work in several folders; **clicking a project selects only that
  project**; a **+ button** (or similar) adds other projects; clear and easy. Built round 133 (click = only, + adds,
  Only/All strip). **This applies everywhere a project is chosen, phones included: no chip rows of projects.**
- **Badges** ("Done" etc.) as small as v1's (11-11 23:34). "This computer", never "This Mac" (11-08 19:16).
- **Task history: direction A, the ledger**, inline expand on phones too (11-15 00:24).

### Phone apps

- **Native design, not v1's look**: Material 3 on Android, Apple HIG on iPhone, built from our kit and tokens
  (11-13 20:34, 20:37). Every v1 feature kept.
- **Gestures where they fit better than taps**, be creative (11-13 21:40); each gesture also has a visible control.
- **Crew as stories, direction C: stories you can reply to** (11-15 19:14): stories row under the Tasks top bar,
  full-screen story, reply box (the task page's composer, compact). Design first, the owner approves, then build
  (11-15 19:10): the rule for every redesign.
- **The one project selector with multi-pick is used everywhere a project is chosen, desktop New task included**
  (owner, 2026-11-15 20:05): it replaces v1's place row ("in <project> · No folder · Other folder…") in the
  desktop and browser New task box and the reply box, the phone Tasks chips and the phone New task sheet. Recorded as
  an allowed difference from v1 (design/v1-reference/allowed.md).
- **Removing (−) the only picked project switches New task to No folder** (owner, 2026-11-15, confirming round 288).
- **Projects on phones follow the project-selector decisions above**: one selector (with multi-pick: tap = only, +
  adds), not chips (owner, 11-15 19:52, on the New task sheet showing the old selector).

- **Long threads naming** (owner, 2026-11-15): **Ledger** stays the task history (ADR-0068). The long-threads state
  file (goal, constraints, decisions, failed, state, open, files) is called **Notes**, shown in a **Thread** tab.
  Its tool is `wisp_notes` (v1's `wisp_ledger`), its CLI `wisp notes`. See docs/v2-long-threads.md.

- **The decision ledger** (owner's #802, merged 2026-11-16): `decisions/D-NNNN.yml` records with hooks that bring each
  decision to the agent before it edits a file in scope. D-0001..D-0046 are v1's AGENTS.md rules (scoped to v1's
  files; advice only, no checks). **D-0043 ("no new dependencies") contradicts v2** (a pinned dependency with an ADR is
  allowed): to supersede for v2's paths. v2's decisions are here in §3 until they're recorded as decisions too.

### How the work runs

- **Coders code; tests in CI**: coders write code and their named tests; full suites, e2e and gates are the outer loop
  (11-09 15:24, 15:34). Move fast, verify after.
- **One coder at a time** (latest: 11-15 17:40, "Dont use 3 coders please one coder"). A fresh session per job, one
  small concern, small briefs so context stays small (11-12 18:39, 19:00). Medium effort, never high (11-07 23:31).
- **The coder is always working**; delegate the next job when a report lands (11-09 20:57).
- **Coders may use Chrome to see their UI** (11-10 00:40).
- **No QA now**: finish v2 to 100 %, then the Linear TODO tickets, then QA by separate low-context agents, a bug list,
  fixes (11-14 17:59, 18:28 "i said not now"). When QA starts: strict rounds on every scenario (11-14 18:27).
- **When 100 % and tests pass repeatedly: work the Linear TODO tickets** (11-14 10:53).
- **Never schedule check-ins** in wisp; check at the start of each turn (11-15 00:11).
- **Questions don't reorder the plan**: answer, record the direction on its future item (11-13 20:41; 11-09 18:21).
- **This file is the memory**: everything goes in docs/v2-plan.md, marked done or to do, updated every turn; never
  trust memory (11-15 00:17, 19:52).
- **Update the watch page every round**, with "round X" in its title (11-13 16:11–16:12).
- **Developer experience is first-class**: easy to use, debug, test and work on; a dev mode for the desktop app;
  documented commands must work (11-14 23:40, 23:48).
- **Repo lives at github.com/WispAgents/wisp** (11-09 14:19).

## 4. The plan

### 4.1 Backend and integrations

- [x] Events + triggers (ADR-0043, 0044)
- [x] Grid leftovers: GitHub/Google sign-in, boxd container driver, sandboxes as peers, `wisp ssh`, teams in the CLI
- [x] Terminal context (ADR-0023)
- [x] Rest of the backend: chat transports, computer pane, update host, telemetry, settings, mentions, setup, copilot reader
- [x] GitHub Action and image, git hook (ADR-0040)
- [x] Every v1 server feature ported (see §6); cross-machine delegates (ADR-0059), sandbox seeding (ADR-0060), several
      grid accounts (ADR-0061), cloud API keys (ADR-0058), cloud previews (ADR-0064), harness detection topic (ADR-0066)
- [x] Process hygiene: nothing outlives a killed server (guard, Windows jobs, exec plugins ADR-0063, probes)
- [x] `.env` masking everywhere a value can leave (ADR-0050, 0054, 0055)
- [ ] **v1 sync** (`docs/v1-sync.md`): #789 with phone step 13. #797 (Shell paste after selecting) done; #795 (lead's
      check-in while waiting on subagents) and #796 (compact follow-ups list) done in r246; #794 (Plan on Claude Code runs
      commands, side discussions' action buttons in the main task's folder) done in r253; found there: v2 has no side gate
      or clash check (v1 side-gate.js): a side discussion runs in a scratch folder with no side note or main-task digest
- [x] **Lead waits on its subagents and gets their reports** (v1 #750, F-task-51, r247): `runtime-core::owed` + `runtime/host/src/more/report_back.rs`; `Facts.owes` set while owed delegates work (`waitsOn` on the slot, wisp.later reads it), each delegate's report goes back as soon as it ends and the lead's run is off, as v1 `#delegateDone`/`#reportBack` (r250: one follow-up per batch; those ending while the lead runs come in one at its end; the rest go on with that follow-up's turn; a stopped one rides with the next report and doesn't wake it alone; queued in the write that marks them read; restart-safe), `subagent_results` marks read, Stop stops delegates and sends nothing, a follow-up's turn takes owed ones over with the still-working note; finish.rs merges the turn slot. Left (see the open item below): delegates on another machine / sandbox / crew jobs aren't waited on
- [x] **Lead waits on its remote and crew subagents too** (F-task-51, v1 `#awaiting` counts `child.relay`, r251):
      `owed::is_far`/`far_open` (pure) + `runtime/host/src/more/far_watch.rs`. Local crew jobs (`ext["wisp.crew"].from`
      = the lead) are read like children and their end runs `check_lead` (`finish.rs`, `lead_of`); jobs elsewhere
      (`<machine>|<task>`: machine, sandbox, crew there) are asked about through `machines.route` (`runtime.get`/`turns`,
      no new wire message) on the kernel clock on the plugin's scope while owed (1 s, backing off to 10 s when it doesn't
      answer or the lead's run is on), armed at every Hold (so `start` → `check_lead` re-arms after a restart); the
      record's `turnId` is the turn started there, and that turn's status counts (not the task's last: a later turn
      there neither holds the lead nor ends the job); status is asked and changes brought before the lead's lock
      (`far_ahead`), never a machine's link under it; changes come back once (`bring_far`: one bringer per lead, the
      mark on the lead and the note on the owed record written before the next looks); Stop routes `runtime.stop`
      there. Left: a machine that never answers (or a task deleted there) keeps the lead in Working until Stop (no
      give-up timeout yet); a sandbox member's second job's changes still aren't brought (marked by key, pre-existing).
      Tests: runtime host `report_back.rs` (crew here, machine, sandbox, stop, restart, a later turn there), runtime-core `owed.rs`, crates/wisp
      `serve_delegate_machine.rs` `a_lead_whose_delegate_elsewhere_outlasts_its_turn_hears_its_report_and_gets_its_changes`
- [x] Later foot sits inside the reply box over its text, as v1 .c-queued (r261: kit `ReplyBox` `top` → `[data-kit='c-top']` `data-part=composer.top`, new additive input `shell.task:composer-top`; `wisp.later.foot` moved there; same box geometry as v1 at 1280/390, light/dark, 1 and 4 follow-ups, `target/r261/`)
- [ ] **TUI** (ADR-0024, `docs/v2-tui-design.md`): first slice done (list, answer, composer). Left: `--on <machine>`,
      For later and Repo tab, blame gutter, hunk jumps, terminal images, the real-terminal pass.

- [x] **v1 sync #799-#801** (phone fixes in v1: subagent time inside the lead card, a pasted task ID becomes a chip, a tests card's long command wraps): check v2 has each, port what's missing (r295: #799 and #801 already held in v2, guarded by css-rules tests; #800 ported: a clipboard insert with no paste event becomes the task's mention)
- [ ] **Long threads** (owner, 2026-11-15, from wisp task d8ee0a207d52; plan `docs/v2-long-threads.md`): the task
      ledger (goal, constraints, decisions, failed, state, open, files; rewritten, never appended; `wisp_ledger` MCP
      tool), the note before each follow-up, Fresh thread from an editable brief, the health meter, the compaction
      audit, the Bash output offload (`wisp hook offload`). v1 is building it now (~/Downloads/wisp-long-threads):
      **port once v1's PR merges**; before that: the ADR draft (naming settled: Notes in a Thread tab)
      and the UI mock for the owner's approval

### 4.2 GUI (browser and desktop), v1 parity

- [x] Every screen wired to the real server (the 2026-11-09 "mounts no task view" gap is closed)
- [x] Multi-project pick (click = only, + adds, strip with Only/All) (round 133)
- [x] **Phones break the project-selector decisions (§3):** done round 288 (2026-11-15): the one selector (kit `ProjectSelect`, the sidebar's) replaces the desktop and browser New task place row (it opens the ⌘P list on the box's place: name = only, + adds, No folder and Other folder… in the list), the phone Tasks project chips (the selector is in the Tasks top bar beside the computer filter, M3 filter chip / iOS pull-down look) and the phone New task place chips (the same launcher); the task page's reply box has no place control (its place chips only show where it works). Tests `apps/web/test/project-selector.test.tsx`; allowed.md entry; F-comp-47, F-task-26/27/30
- [x] Composer `@` and `/`
- [x] Harness picker back to v1: one pill, one two-pane popover (round 233, 6e3fe42e)
- [x] "What will Auto run" back on the pill as v1 (#autoPv: Auto's pick is the pill's own segment, opens the AutoPop); extra compare runs open the same two-pane picker with their own model · thinking · account and What's different (round 236, c390aca5)
- [x] Try it two ways folds as v1 + Default on compare runs (round 245, 1d79d6e9)
- [ ] After Escape closes the harness picker v2 focuses the pill (focus ring on the runner group); v1 appears to return focus to the prompt (blue box): check v1 and match
- [ ] **Parity gate** (`qa/parity.mjs`): baseline 0/96 on 2026-11-10; per-view burn-down in `docs/v2-ui-burndown.md`;
      **not re-measured as a whole since**: the batched pass (§4.8) runs it on every view, width and theme
- [ ] Layout and feedback gates on every view (browser and phone views): to run in the batched pass
- [x] Task history redesign, direction A: the Ledger (round 235, b4b43212, ADR-0068): outcome rows, notice folds, day
      cards, search + filters, scrubber above 30 turns, inline open on every width

### 4.3 Performance

- [x] Budgets in CI (initial JS ≤ 120 KB br, CSS ≤ 30 KB)
- [x] First download 290.6 → 117.9 KB JS, 46.4 → 29.7 KB CSS (ADR-0067)
- [x] 4,000 tasks scroll smoothly on a 4× slowed phone; first tap on Slow 4G 1.3 s blank → 171 ms
- [x] Initial JS 119.4 → 107.0 KB br: Compare's table math out of the entry (`frame/runs-small.ts`), 756 strings no first-page module uses moved to their lazy halves (ADR-0067 re-derived; every eager half keeps a key of each kind: `strings-split.test.ts`)
- [ ] Warm tap on a phone ~90 ms (budget 50 ms) (F-phone-8)
- [x] The server budgets gated in PR CI, step 1 (ADR-0080): `spikes/server-budgets` (own workspace) seeds 5k tasks through wisp.runtime's writer, runs the real `wisp serve` (`budgets` profile: release, thin LTO) with fakes, gates cold start / idle RSS / the `fresh=list` snapshot on `tools/budgets.ts` `SERVER_BUDGETS`; CI step *server budgets* (Linux, job summary). **All three miss today** (2026-11-16, Mac): cold start 410–515 ms (150), idle RSS 145–160 MB after a list read (120; 95 before), list ~76 ms for a 4.5 MB snapshot (5)
- [x] Make the server budgets pass (ADR-0080 §Consequences): ~~cold start~~ **passes** (2026-11-16, Mac, 5k tasks: 427 → 67 ms median; `wisp.review`'s boot clear of reviews a restart cut short now runs on its scope after startup, one `runtime.list` read and `runtime.get` only on rows whose `review.status` is running, a task a run of this server claimed left alone; tests `startup_reads_again_only_the_tasks_whose_row_says_a_review_is_running`, `a_run_this_server_started_before_boot_reached_its_task_keeps_its_record`); ~~list~~ and ~~RSS~~ **pass** (ADR-0081, 2026-11-16, Mac, 5k tasks, two runs): `list` is read from the session table in pages in one read transaction (`PagedSource`: the 100 latest unarchived first, then 500 at a time), never by replaying the change feed (which also read empty once retention pruned it); pages go as read, are never shed past the high-water mark, and the UI draws the first (`snapshotPage`); first page **76.6 → 1.3 ms**, every task 76.6 → 40 ms, idle RSS **146 → 98–101 MB**, cold start 65–70 ms; the gate times the first page and reports the last. Tests in ADR-0081's table **Linux CI** (2026-11-16, ADR-0084; run `pr` on db637559: RSS 297–300 MB, cold 139–174 ms, list 5.03–5.07 ms): glibc kept each list read's garbage (~5 MB a read) where macOS returns it; mimalloc measured worse (v3 188 MB, v2 331 MB on Mac), so `wisp serve` on glibc trims the heap (`malloc_trim`, `wisp_proc::trim_heap`) 300 ms after each burst of first looks and once after the start; the start does less before the first answer (summaries parsed once from text, wisp.machine's `sysctl`/`date` after start, harness rows not rewritten for `at` alone: 42 → 14 writes a restart): cold start Mac 63–70 → 48–53 ms, E-cores 300–334 → 166–214 ms; list first page unchanged (next: stored-text rows). Linux unmeasured here (no docker): PR CI proves it. Tests in ADR-0084's table
- [x] The server budgets in CI, step 2 (ADR-0082): the same `spikes/server-budgets` run gates p99 patch latency (10 tasks streaming through the fake agent's new `FAKE_STREAM`, 4 clients on `list` + all 10 topics), append rate (`spikes/events`' `append_rate`, now its lib) and write amplification (WAL pinned during the stream + db growth ÷ kept activity) on `SERVER_BUDGETS`. **Two miss** (2026-11-16, Mac, 5 runs): patch p99 **301–303 ms** (150; min 1.5, p50 156: the Coalescer's 250 ms commit window + the outbox's 50 ms flush), write amplification **36.0–36.8×** (15; ~10 pages a commit, each of 10 tasks committing alone every 250 ms with ~5 events); append ~1M/s passes. Next: one commit for all running tasks per window (shared coalescer / group commit) at ≤ 100 ms fixes both
- [ ] Make patch latency and write amplification pass (ADR-0083): ~~patch latency~~ **passes** (2026-11-16, Mac, 3 runs): one coalescer shared by every running turn, 100 ms window, at once on a tool's result / a wait / turn end; p99 **303.9 → 102.8–103.2 ms**; write amplification **37.0 → 30.5–31.8×, still misses 15** (commits 315 → 161, but each activity commit touches one leaf per running task in `runtime_event_seq`, `runtime_event_turn`, `change_topic`; the db stores each step twice). Trims (drop `runtime_event_turn`, the change feed's AUTOINCREMENT) estimate ~24×; 15× needs append change rows that don't repeat the row, or a budget ADR: owner's call. Tests in ADR-0083's table
- [x] Harness detection: 45 s → 15 s with hung CLIs, ~1 ms cached

### 4.4 Phone apps (ADR-0065, `design/mobile.md` Part A)

1. [x] Platform look plumbing (round 215)
2. [x] `qa/platform.mjs`, and layout/feedback on phone views (rounds 216, 224)
3. [x] Kit: frame and navigation (round 217)
4. [x] Kit: rows and surfaces (round 220)
5. [x] Kit: overlays (Sheet, Dialog, Menu, Popover, Toast, Tooltip) (round 227, reviewed by r241)
6. [x] Kit: controls (Button, IconButton, Switch, Tabs, Input, Select) (round 265)
7. [x] Gestures with visible twins (round 225); seams (GestureHostProvider, row long-press) in round 227's work
8. [x] The app's frontend (`@wisp/mobile` routes, launch on Computers, several peers) (round 281: its own entry `mobile/web` `npm run dev`/`build`; Stage → kit Frame + NavigationBar under a phone look; `Client.setHome` runs the shell on the launched computer, every peer stays connected; seams `secureStore`/`scanCode`/`registerPush`)
9. [x] Android native: Keystore-sealed keys and tokens, QR host-key pinning, a port per computer (round 226)
10. [ ] iOS native: **blocked: Xcode is not installed on this Mac**
11. [x] Push (`wisp.push`, HPKE, grid relay) (round 228, reviewed by r241; no UI calls it yet: step 13)
12. [x] Offline and bad network: core + shell hookups (round 229, reviewed by r241)
13. [x] Screens part 1 (Tasks, task page, New task, search; Add computer with QR, Trust new key, Stay connected ON by default without push). Done (round 284): Tasks (root TopAppBar with the computer filter `shell.computerFilter` as title, Search, the me menu; project chips row; FAB / iOS compose for New task; shell-list filters by computer), Add computer (QR via `scanCode`, typed fallback, Trust the new key with confirm, Stay connected ON for SSH without push, `mobile/web/src/{add,stay}.ts`). Done (round 286): the task page's app bar (M3 medium / iOS navigation bar: Back + the task's menu from the one menu model, with the details bar's Transcript, Rerun, Open in, Changes, Shell, Ship it, Mark done and Details: the details column as a sheet; Rename as a dialog; plugins' `shell.task:head` items in the bar; the ledger and the reply box as they are, the composer M3 docked / Messages-like, `PhoneTaskBar` in `frame/PhoneHead.tsx`), New task as Android's full-screen dialog / iOS's sheet (Close × or Cancel, New task, Send; the prompt box unchanged, `PhoneCompose` in `frame/PhoneHead.tsx`, Frame `behind`), Search as the M3 search view / the iOS field under the large title (the palette's task search, Back / Cancel; `TopAppBar.search`, `PaletteSheet.close`); browser initial JS 107.8 KB br and CSS 27.1 KB as at HEAD; tests `apps/web/test/phone-pages.test.tsx`, `packages/ui/test/platform-pages.test.tsx`. Done (round 293): selection mode in the phone Tasks list (a row's long-press sheet's Select first on Android, Select in the Tasks bar on both; the kit's `SelectionBar`: Android's contextual top app bar that leaves with the last row unpicked, iOS's edit mode with Select All / Deselect All, n Selected, Done and a bottom toolbar; a tap toggles a row, its check in the dot's column (`ListRow.picked`); bulk Mark done, Save for later, Archive, Delete (asks first) on the tasks each fits, optimistic with the client's rollback toast and Undo, `frame/bulk.ts`; state `ShellContext.listSelect`, `frame/list-select.ts`; ends on navigation and Esc), the iOS computer filter as a pull-down from the title (`TopAppBar.titleMenu`, choices checked; Android keeps its sheet), and the task head's swipe to the next / previous task with Next task / Previous task in the task's menu (disabled at the list's ends), in the order the list shows now (`ShellContext.listOrder`, published by shell-list; `frame/task-step.ts`: a filter re-orders it, a task gone meanwhile keeps its place). Tests: `apps/web/test/phone-step13.test.tsx`, `packages/ui/test/selection-bar.test.tsx`, `packages/shell/test/{task-step,list-select,bulk}.test.ts`; screenshots `target/r293/`
13b. [x] **Crew on phones as stories** (owner, 2026-11-15: "like stories in Instagram and Snapchat"): replace the phone Tasks list's Crew section
      with a stories row under the top bar (one avatar per member, ring = working / unread report / waiting on you / idle;
      Org and + as first/last bubbles); tap opens a full-screen story (now, last report, what waits on you with the task
      page's buttons, next check-in), tap/swipe to step, swipe down to close, each gesture with a visible control.
      Mock done: docs/design/crew-stories.html (A stories row, B + Waiting for you strip [recommended], C + reply box);
      **Owner picked C (stories you can reply to), 2026-11-15.** Phones only; browser/desktop keep v1.
      Done (round 287): `shell.list:crew` draws under the phone Tasks top bar (shell `PhoneListTop`; shell-list leaves
      it out of the list on phones); the crew's phone chunk (`plugins/crew/ui/src/Stories.tsx`, rules in `stories.ts`)
      with kit `CrewStories`/`CrewStory`/`StoryCard` (`packages/ui/src/crew/Stories.tsx`, its own lazy
      `crew-stories.css`); rings wait › unread report › working › idle; story: bars that fill (8 s, none under reduced
      motion), ←/→ + arrow keys + tap halves, × + Esc + drag down, Pause + held press, ⋯ (page, Edit, Check in,
      Pause, Share, Track record, Remove); Waiting for you is shell-task's own `shell.task:foot`; reply is the kit
      `ReplyBox compact` (follow-up on the running job, else `crew.assign`); 👍 Good / ↻ Try again follow up on the
      report. Mock seed `crew` (MOCK_CREW=1, `?seed=crew`), mocks `crew.assign`, `runtime.answer`. Browser initial
      JS/CSS unchanged (396.7 KB raw / 199.7 KB raw). Tests: `apps/web/test/crew-stories.test.tsx`,
      `packages/ui/test/crew-stories.test.tsx`, `plugins/crew/ui/test/stories.test.ts`.
14. [x] Screens part 2 (Settings, My Stuff, Crew, Computers and cloud sign-in). Done (round 297): **Settings** from the me menu is a pushed page over the whole screen (Overlays: a bare viewer on `platform.phone` is a `Dialog size="full"`): the kit's `SettingsFrame` draws its phone half (`packages/ui/src/settings/SettingsPhone.tsx`, loaded with the look): the panes as an M3 settings list with primary section headers / an iOS inset grouped table with chevrons, in sections by pane group (`phoneGroups`), Back titled Tasks; a pane pushed with its own bar and Back to Settings, the panes as they are (their switches/selects already the kit's phone controls); Keys hidden in the apps (F-set-6, `offeredPanes`). **My Stuff** on the Stuff tab is the kit's `StuffPhone` (root bar with Search (a field under the bar), Add files (Android's icon / iOS's ⋯), ⋮/⋯ with Make a pane, By time/By task, Cards/List; kinds as M3 filter chips / iOS segmented control; two-column tiles or swipeable rows keyed by id), every v1 item action (`page-run.ts`, shared with the browser page) plus Share (platform share sheet, copy fallback); a row swipes right to Share and left to Remove (`stuff.swipe`, twin: the row's ⋯ `stuff.phone.row.menu.trigger` on screen); Remove leaves at once with Undo (which outlives the screen) and runs `stuff.remove` after 5 s; an app launches inside wisp full screen in its sandboxed frame with Close/Done (`StuffPhoneApp`, F-stuff-10 in the app). Browser initial JS 108.9 KB br (HEAD 108.7; +0.2 KB raw: the gesture registry entry, Overlays' phone branch), CSS 26.9 KB unchanged; Settings' phone pages load with the PhoneStage chunk, the phone-only Settings rules in `phone-nav.ts`. Tests `packages/ui/test/phone-screens.test.tsx`, `apps/web/test/phone-step14.test.tsx`, `plugins/shell-settings/ui/test/nav.test.ts`; screenshots `target/r297/`. Done (round 299): **Crew** on the Crew tab is the platform screen (`plugins/crew/ui/src/CrewPhone.tsx`, its own chunk, rules in `phone.ts`): the root bar titled Crew with Org and Add member, ⋮ / ⋯ with Import crew, Share your crew, Track record; the members as M3 two-line rows with faces / an iOS inset grouped list in the stories row's order (asks you › new report › working › idle), each pushing its page: the page bar with Back to Crew, Pause / Resume and ⋮ / ⋯ (Edit, Check in now, Org, Share, Share your crew, Track record, Remove); its face, role and what it's doing; M3 primary tabs / an iOS segmented control for Jobs · Brief · Wakes (jobs newest first keyed by id; brief, notebook (edit in place), how they work, skills and instructions; checks in, also starts when, next, so far, Check in now, Change when…); New job is the task page's reply box (compact); Org (`crew.org` with `{ chart: true }`) pushed with Back; a story's Open page opens the same member page; a phone's browser keeps v1's pages. **wisp Cloud** on Computers (`mobile/web/src/{cloud,cloud-accounts}.ts`, `ui/Cloud.tsx`): Continue with GitHub / Google (one tap, the grid's device flow run by the app: `/api/device/code`, the provider in Custom Tabs / ASWebAuthenticationSession via `openInApp` back to `/activate`, `/api/device/token` polled, asked at once on resume), Use your own wisp Grid behind a link; a sheet at once with the code, Open again, Copy code, Cancel; one card per account (several, ADR-0061) with its cloud wisp and computers from `GET /api/computers` as rows that open like saved ones (`GridComputer.account`), Sign out (card and computers gone at once; a computer the phone couldn't forget stays listed) and the Delete your account link (`/account#delete`); a revoked token marks the card signed out with Sign in again; the device token sealed in `platform.secureStore` (a phone without one refuses), never web storage; every computer saved again with a new token on a new sign-in (retried on each refresh until it lands); a refresh dropped if the account was signed out meanwhile; logic in `createCloudAccounts`, reviewed by a delegate (2 passes). Browser initial JS 108.7 KB br / 399.8 KB raw and CSS 26.9 KB, both as at HEAD (the phone screens are their own chunks; Org's params are parsed in OrgPage's chunk). Tests `apps/web/test/crew-phone.test.tsx`, `plugins/crew/ui/test/phone.test.ts`, `mobile/web/test/cloud.test.ts`; screenshots `target/r299/`. Done (round 300, ADR-0078): Sign out revokes the device token on the grid first (`POST /api/device/revoke`, authenticated by the token revoked, atomic, idempotent, audited like the account page's Remove), offline queued as a secret and told on the next refresh with a toast saying so; the desktop's `grid-client` sign-out does the same (told at the next start or sign-in); tests `apps/grid/tests/device_revoke.rs`, `mobile/web/test/cloud.test.ts`, `plugins/grid-client/host/tests/accounts.rs`. **Left (later rounds):** Continue with Apple on iOS (the grid has no Apple sign-in yet: ADR-0065 open question 1), the app's own Settings panes (Computers, Keys on this phone, Notifications per kind, Stay connected, Swipe actions), My Stuff thumbnails for images (the file URL must go through the computer's transport), My Stuff's search scopes in the palette, and the iOS compose button showing on the Stuff root bar
15. [ ] Store and release (android.yml, ios.yml, listings, privacy)
- Open questions with defaults (ADR-0065): Sign in with Apple before iOS ships · push needs a free Cloud sign-in ·
  the app opens on Computers · no wallpaper colours · wisp's own icons on iOS

### 4.5 Windows

- [x] Clippy clean for Windows; `\\?\` paths; CRLF in wisp's own copies; path separators; `.cmd` shims; Windows base
      env for plugins; read-only `.git` removal; ConPTY cursor reply (rounds 184–207)
- [ ] The remaining Windows test groups from the newest full run (§8)
- [ ] Windows by hand once: update relaunch, desktop window checks (part of §4.8)

### 4.6 Developer experience (owner, 2026-11-15)

- [x] `npm run dev` and `npm run dev:desktop` (round 230)
- [x] Check rule `commands`: every script and documented command resolves (round 231)
- [x] `npm run doctor` (round 232)
- [x] `docs/dev.md`: clone to running wisp, debugging (logs, `RUST_LOG`, inspector, attaching a debugger), one test per area
- [ ] Dev runs never collide with an installed wisp, everywhere (desktop, CLI, grid)
- [ ] Tauri dev window removes its lock file on Ctrl-C. Plan (r283): macOS only (`/tmp/dev_wisp_dev_si.sock`; Linux DBus, Windows mutex); tools/dev.ts removes it after the desktop child exits, only for the dev identifier and only if nothing answers on it
- [ ] `tools/bundle.ts` so `npm run build` builds the published bundle again. Plan (r283): web build, then `cargo build -p wisp` (release, `--debug` in CI), then `npm/scripts/build.mjs` for this target, then `npm pack` into npm/dist; pure plan in tools/bundle/plan.ts with a test

### 4.7 Cutover

- [ ] Turn on `.wisp/` tracking for this repo once our own tasks run on v2
- [ ] Delete v1 (`server/`, `web/`, v1 `grid/`, v1 `desktop/` packaging, v1 tests and workflows)
- [ ] CI workflows for v2 only; fewer test binaries (one integration-test binary per crate)
- [ ] Budgets: binary ≤ 15 MB
- [ ] README for v2 (most of it is still v1 prose) and CHANGELOG
- [ ] Re-point the npm trusted publisher (repo moved to WispAgents)
- [ ] **release.yml's npm job still publishes v1** (`npm publish` from the root; the root package.json is private now) and never runs `npm/scripts/build.mjs`: switch it to v2's npm packages before any v2 release

### 4.8 One batched verification pass

- [ ] CI green on every OS, several runs in a row (no flakes): first all-green run f0db9105 (2026-11-15); need more in a row
- [ ] Parity, layout and feedback gates on every view, width and theme; plus `qa/phone-selection.mjs` (first row holds still in selection)
- [ ] Perf budgets, all of them
- [ ] Windows by hand once
- [ ] wisp from other harnesses in a wisp-init'ed repo (real Claude Code and Codex runs, offline MCP, the CI twin)

### 4.9 Docs and site

- [ ] docs.trywisp.dev (ADR-0039) and the site rework (`marketing/positioning-v2.md`)

### 4.10 After v2 is 100 %

- [ ] The Linear TODO tickets
- [ ] Continuous QA rounds by fresh low-context agents → `docs/qa/bugs.md` → one fix per coder

## 5. Waiting for the lead right now

- [x] Verify + commit round 227 (gesture seams, phone overlays) and round 229's shell hookups (r241)
- [x] Verify + commit round 228 (push plugin, grid relay, `hpke` dependency) (r241)
- [x] Review coder-r233's picker against `design/v1-reference/home.runner` (side by side): matches; committed 6e3fe42e
- [x] Push the local commits after coder-r234's CI fix
- [x] Merge origin/main (#792-#797) into #756: CHANGELOG conflict was blocking PR CI (8efe765b)
- [ ] Rebuild the owner's v2 desktop preview with the latest fixes (port 4791, `~/.wisp-v2-preview`)

## 6. Every v1 feature

Source: `docs/v1-features.md` (each row's evidence and test). `[x]` built, `[ ]` partial or missing, `[~]` dropped.

| Area | Built | Partial | Missing | Dropped |
|---|---|---|---|---|
| Tasks and the list (`TASK`) | 50 | 0 | 0 | 0 |
| Composer: New task and the reply box (`COMP`) | 46 | 1 | 0 | 0 |
| Task page: answers, questions, side panel (`PAGE`) | 44 | 0 | 0 | 0 |
| Goal and judges (`GOAL`) | 8 | 0 | 0 | 0 |
| Crew (`CREW`) | 20 | 0 | 0 | 0 |
| Automations, triggers, schedules (`AUTO`) | 15 | 0 | 0 | 2 |
| Worktrees (`WT`) | 7 | 0 | 0 | 0 |
| Ship, PR, review (`SHIP`) | 9 | 0 | 0 | 0 |
| My Stuff, apps and panes (`STUFF`) | 9 | 0 | 1 | 0 |
| Sharing (`SHARE`) | 3 | 0 | 1 | 0 |
| Machines, cloud and the grid (`MACH`) | 21 | 0 | 0 | 0 |
| Channels: phone, chat, mail (`CHAN`) | 15 | 0 | 0 | 0 |
| Connectors and MCP (`CONN`) | 9 | 0 | 0 | 0 |
| Agents, accounts, usage (`ACCT`) | 18 | 0 | 0 | 0 |
| Settings (`SET`) | 7 | 1 | 0 | 0 |
| Keyboard (`KEY`, from `web/js/features/keys.js`) | 10 | 0 | 0 | 0 |
| CLI and integrations (`CLI`) | 9 | 0 | 0 | 2 |
| Desktop app (`DESK`) | 5 | 1 | 0 | 0 |
| Phone app and phone web (`PHONE`) | 3 | 2 | 3 | 0 |
| Updates, install, telemetry (`UPD`) | 3 | 1 | 0 | 0 |
| Look and feel (`LOOK`) | 3 | 2 | 0 | 0 |
| **All** | **314** | **8** | **5** | **4** |

### Tasks and the list (`TASK`)

- [x] F-task-1: a project, when I send a prompt, then a task starts in the background and its answer shows as cards (hero, diff, tests…)
- [x] F-task-2: Home, when I press Send, then the box clears and the task shows in Working as *Starting up* at once
- [x] F-task-3: tasks in several states, when I look at the sidebar, then they sit in Needs you, Working, Done and Archived
- [x] F-task-4: the list, when I click *Needs you*, *Crew* or *Working* (or Tab + Enter), then that section folds
- [x] F-task-5: more than 60 done tasks, when I scroll Done, then its count is the real total and older ones load
- [x] F-task-6: a task row, when I read it, then a third line names harness logo, model and where it ran
- [x] F-task-7: a task, when I right-click it, then a menu offers open, rename, mark done, for later, archive, delete, rerun, fork, mention
- [x] F-task-8: a task, when I press A (or Archive), then it moves to Archived and a new question brings it back
- [x] F-task-9: a task, when I rename it (M, Enter saves, Esc cancels), then I stay on the task
- [x] F-task-10: a done task, when I press L (or swipe left on phone), then it's parked under For later; L again unparks
- [x] F-task-11: the open task, when I mark it done, archive or delete it, then wisp goes to the next task that needs me (feed) or New task
- [x] F-task-12: a running task, when I delete it, then it's gone without flashing through Needs you or notifying
- [x] F-task-13: a task a recipe or trigger started, when I see its row, then it has a bolt (recipe) or the trigger's mark
- [x] F-task-14: a task that crashed, hit a limit or was refused, when I see its row, then the pill says which
- [x] F-task-15: a new task, when its first answer lands, then the task keeps a name from what I asked, not the agent's headline
- [x] F-task-16: an open task, when I reload or copy the address, then `#/s/<id>` opens the same task
- [x] F-task-17: I scrolled a task, when I reload and open it again, then I'm back where I was
- [x] F-task-18: I opened a task before, when I open it again, then it shows at once from cache
- [x] F-task-19: many tasks, when I press ⌘K and type, then a fuzzy search finds tasks, projects and commands
- [x] F-task-20: past work, when I type 3+ letters in ⌘K, then it finds words inside prompts and answers
- [x] F-task-21: agents tagged tasks (`wisp_tag`), when I search a topic, then tagged tasks match
- [x] F-task-22: ⌘K or the project picker, when I search, then recently touched items rank first
- [x] F-task-23: two tasks, when I tap ⌃Tab, then I'm back on the previous one; hold Ctrl and Tab to step through
- [x] F-task-24: split view, when I press [ or the sidebar button, then the list hides and the task takes the window
- [x] F-task-25: the list head, when I click the round avatar, then account, Machines, Automations and Settings are there
- [x] F-task-26: no folder, when I pick *No folder* and send, then the task runs without a project
- [x] F-task-27: an API and a web repo, when I add a second folder, then one task works in both
- [x] F-task-28: a GitHub URL, when I paste it as a project, then wisp clones it (or reuses the clone) and adds it
- [x] F-task-29: a project in any picker, when I right-click it, then I can edit or remove it
- [x] F-task-30: 30 projects, when I look at the launcher's row, then chips stay on one row and the rest fold into More
- [x] F-task-31: an empty prompt box, when I press ↑, then my last sent prompt comes back (last 50)
- [x] F-task-32: a half-typed prompt or follow-up, when wisp reloads or updates, then the text is still there
- [x] F-task-33: a long task, when I press C or U, then turns collapse or I jump back to the top
- [x] F-task-34: a task, when I press V (or turn on *Full transcript*), then every tool call shows
- [x] F-task-35: a running task, when it runs a shell command, then the ticker says it in words (*Reading lines 15–50 of a.js*)
- [x] F-task-36: a run quiet for 12 min, when I look at it, then it says *No output for 12 min* and keeps counting
- [x] F-task-37: wisp stopped, when 4 s pass, then the page says it lost wisp instead of *working*
- [x] F-task-38: a reply sent while wisp doesn't answer, when it fails, then the text comes back to the box with a toast
- [x] F-task-39: two active tasks in one repo changing the same files, when I open one, then a note says so
- [x] F-task-40: a task, when I right-click → *Mention in prompt*, then a reference to it goes in the box
- [x] F-task-41: an agent names one of my tasks, when its answer shows, then a `task` card opens it with a click
- [x] F-task-42: a task, when I fork it (⇧F), then a new task carries its conversation
- [x] F-task-43: a task, when I pick Rerun… (⇧R), then a dialog with the prompt, attachments, mode and harness reruns it
- [x] F-task-44: the rerun dialog, when I pick another computer, then it reruns there
- [x] F-task-45: a turn, when I click Restore, then the worktree goes back to its checkpoint
- [x] F-task-46: an open task, when I see its details column, then Mark done/Reopen, Ship it, Rerun, Fork and More are there
- [x] F-task-47: menus, when I read them, then they show key hints (⌘L on Mac, Ctrl+L elsewhere)
- [x] F-task-48: a finished answer, when I open it again, then its entry animation doesn't replay
- [x] F-task-49: a reply box open, when I click a suggestion or a button above it, then one click works
- [x] F-task-50: a task with wisp subagents, when I open it on a desktop, then its details show its plan share with theirs added and how full its context is, and a click shows all of it

### Composer: New task and the reply box (`COMP`)

- [x] F-comp-1: the launcher, when I open the harness · model picker, then harnesses with logos are left and models right — built: round 233 (6e3fe42e), one pill, one two-pane popover
- [x] F-comp-2: the picker, when I drag the thinking slider, then the level goes from Default to the harness's top
- [x] F-comp-3: Codex or Claude, when I open models, then they come from the CLI's own catalogue (Fable included)
- [x] F-comp-4: the prompt, when I press Shift+Tab or the mode button, then it cycles Plan, Edits, Full auto
- [x] F-comp-5: a new task with no mode, when it launches (UI, API, CLI, triggers), then it's Full auto
- [x] F-comp-6: Auto, when I type, then the launcher shows what will run (*Opus · Medium*) and picks harness, model and effort
- [x] F-comp-7: Auto failed tests at low effort, when it retries, then it goes again at high before handing over
- [x] F-comp-8: Auto Best, when I enable *Best of two*, then two picks run and the better is marked
- [x] F-comp-9: *Run on*, when I pick a computer, then the harness list shows that computer's harnesses
- [x] F-comp-10: *Run on*, when I click a machine, then only it is picked and the menu closes
- [x] F-comp-11: a git repo, when I launch with Worktree on, then the task runs in its own worktree on `wisp/<task>-<id>` from fresh `origin/main`
- [x] F-comp-12: a file, when I drop or paste it next to the prompt box, then it's attached once (not opened)
- [x] F-comp-13: the Android app, when I tap the paperclip, then Take photo / Record video / files are offered
- [x] F-comp-14: the Android app, when I tap the mic, then what I say is typed in
- [x] F-comp-15: a project, when I type `@` and part of a file name, then matching files are offered
- [x] F-comp-16: a prompt, when I type `@Ada`, then the task is Ada's job (a follow-up `@Ada` goes to her)
- [x] F-comp-17: My Stuff, when I type `@` in a prompt, then kept items are offered and attached
- [x] F-comp-18: the prompt box, when I type `/`, then `/goal`, `/btw`, `/side`, `/sandbox`, `/terminal`, `/bug` and plugin commands show
- [x] F-comp-19: a prompt starting *in 5 minutes…* or *remind me … in 1 hour*, when I send it, then it's held and starts then (or becomes a reminder)
- [x] F-comp-20: an empty box, when I pick a time on the clock, then *Starts at …* shows under the box and Send holds it
- [x] F-comp-21: a schedule, when I type *every monday 9am*, then it reads it in plain words
- [x] F-comp-22: a prompt with `/draft` or *after #12*, when I send it, then it waits as a draft or starts after the other task
- [x] F-comp-23: an agent is working, when I send a follow-up, then wisp interrupts and resumes with my message
- [x] F-comp-24: a queued follow-up, when I click ×, then it's cancelled; drag reorders; Send now sends it
- [x] F-comp-25: queued follow-ups, when I press Stop, then they stay marked *Held after Stop* and nothing goes by itself
- [x] F-comp-26: a follow-up, when I switch mode or harness in the reply box, then the next turn uses it and the box names it
- [x] F-comp-27: a hand-off to another harness, when it continues, then my rules and effort go with it
- [x] F-comp-28: a running task, when I type `/btw did you update the docs?`, then the agent answers on the side without stopping
- [x] F-comp-29: a task, when I type `/side`, then a side discussion opens on its own page and can run commands in the task's folder
- [x] F-comp-30: the launcher, when I turn on vs and pick Codex and opencode, then the same task runs on each, shown as one task with a row per run and honest totals
- [x] F-comp-31: a harness, when I click *Try it two ways*, then one task runs twice with one setting changed
- [x] F-comp-32: the launcher, when I add subagents (a team), then the lead hands parts to other harnesses
- [x] F-comp-33: a task without a team, when I add subagents in a follow-up, then the next turn has them
- [x] F-comp-34: a task, when I pick many delegates, then there's no cap at 6
- [x] F-comp-35: the launcher before my first task, when I look, then it's only prompt, mode, harness and Send
- [x] F-comp-36: New task, when I open *Issue*, then my GitHub, Linear and Jira issues are listed and one fills the prompt
- [x] F-comp-37: a recipe `Prepare release {version}`, when I click it, then a dialog asks the fields and runs it
- [x] F-comp-38: the harness picker, when I open it, then each harness shows how much of its plan is gone
- [x] F-comp-39: a task that would take my plan past the line, when I send, then wisp asks (Send anyway / wait for the reset)
- [x] F-comp-40: no harness on this computer, when I open the launcher, then *No coding agent here yet* offers to install opencode
- [x] F-comp-41: a signed-out harness, when I pick it in Run with, then the sign-in opens right there
- [x] F-comp-42: a run fails because the harness is signed out, when I sign in from the turn, then wisp resends my request
- [x] F-comp-43: I type in the launcher, when I press Enter, then the repo was already fetched (warm) so the agent starts at once
- [x] F-comp-44: wisp starting, when the first frame paints, then the launcher is there (placeholders, not an empty strip)
- [x] F-comp-45: Safari with an IME, when I press Enter to pick a word, then nothing is sent
- [x] F-comp-46: Send from a finished task, when the row moves to Working, then the keyboard stays in the reply box
- [x] F-comp-47: New task with a project picked, when I press + beside another project (or ⌘-click it), then the task also works in that folder (v1 launcher.js `lp-also`, project-list.js `pm-also`; the task's places list every folder)

### Task page: answers, questions, side panel (`PAGE`)

- [x] F-page-1: an agent asks a question, when I click a choice or type, then it answers and the run goes on
- [x] F-page-2: Edits mode and a blocked tool, when I click Allow / Always / Deny, then the agent goes on (or not)
- [x] F-page-3: a question or approval, when I type in the reply box, then that's my answer (one box, not two)
- [x] F-page-4: a repo with hooks, MCP servers or setup, when a task first runs there (not Full auto), then wisp asks before trusting them
- [x] F-page-5: an `actions` block, when I click an item, then its command runs (or its reply is sent)
- [x] F-page-6: answers with `app`, `doc`, `video`, `preview`, `map`, `graph`, `flow`, `chart`, `compare`, `stuff`, `task`, `blocked` blocks, when they show, then each renders
- [x] F-page-7: Settings → *wisp blocks* off, when an agent answers, then it's plain markdown
- [x] F-page-8: a quick question, when the agent answers, then it's plain text, not a report
- [x] F-page-9: a malformed `wisp` fence, when the turn ends, then the blocks are salvaged and raw text kept
- [x] F-page-10: an answer, when I select words or hover a block and click Comment, then the comment goes with my next follow-up
- [x] F-page-11: unsent comments, when I open Ship it or Merge, then it says *2 comments not sent yet*
- [x] F-page-12: a finished turn, when I read its trail, then a receipt says *38k tokens · 71% cache · 3% of weekly*
- [x] F-page-13: Claude ran subagents, when I read the trail, then it says which and where the tokens went by model
- [x] F-page-14: a Claude task ≥100k tokens idle past its cache, when I reply, then a line warns the cache went cold
- [x] F-page-15: a Claude task ≥200k tokens, when I pick *Compact now*, then the context shrinks
- [x] F-page-16: a task, when the CLI changed between turns, then *What changed* says version, tools, Codex models
- [x] F-page-17: Claude refuses (usage policy), when the refusal card shows, then *Hand to …* moves the task to another harness
- [x] F-page-18: a Claude task, when I click *Run subagents on Haiku*, then built-in subagents use Haiku
- [x] F-page-19: a task, when I press ] or the panel button, then a side panel opens with Changes, PR and Shell
- [x] F-page-20: Changes, when I open it, then every file changed since the base shows with a diff (first one open)
- [x] F-page-21: Changes in a worktree, when I tick Stage and commit, then it commits
- [x] F-page-22: a hunk, when I click Revert (or tick Viewed), then that hunk or file goes back
- [x] F-page-23: a diff line, when I comment on it, then the comment goes with the next follow-up
- [x] F-page-24: a changed file, when I click Edit, then it opens in wisp's editor and saves
- [x] F-page-25: the editor, when I hover or go to definition, then a language server answers (TS, Python, Go, Rust)
- [x] F-page-26: Changes, when I click *Open file*, then any file of the task opens (search too)
- [x] F-page-27: the side panel's Shell, when I type, then a login shell runs in the task's worktree (End shell keeps it ended)
- [x] F-page-28: a task's PR, when I open PR in the panel, then checks, reviews and merge state show without noise
- [x] F-page-29: the side panel *Run*, when I click Run, then the task's app runs and its page shows beside the task (Logs, Restart)
- [x] F-page-30: the preview, when I pick an element, then its HTML and styles go in the reply box
- [x] F-page-31: a `preview` block, when it shows, then the live page is in the answer (cloud: the cloud's page)
- [x] F-page-32: a long job, when the agent calls `wisp_watch`, then a live page shows beside the task
- [x] F-page-33: a task, when I press T (Take over), then the agent's session opens in a terminal inside wisp (or a terminal app/cmux)
- [x] F-page-34: *Babysitting mode*, when I send a task, then it opens straight in the agent's own terminal
- [x] F-page-35: `/terminal …`, when I send it, then the task runs in a terminal in wisp
- [x] F-page-36: `/sandbox`, when I send it, then the task runs in a fresh cloud box
- [x] F-page-37: a picture or video in an answer, when I click it, then it opens full screen and arrows go through them
- [x] F-page-38: a picture in an answer, when I hover → Copy, then it pastes elsewhere
- [x] F-page-39: a picture, when I click *Show in folder*, then Finder/Explorer opens with it selected
- [x] F-page-40: an image an agent saved in `/tmp`, when I reboot, then it still shows (copied to `~/.wisp/shots`)
- [x] F-page-41: a `.env` in a diff or activity, when it shows, then values are hidden
- [x] F-page-42: a finished turn, when I click 👍/👎, then feedback goes to the wisp team
- [x] F-page-43: Home or `/bug`, when I click *Tell us*, then a feedback dialog sends a report
- [x] F-page-44: a task, when it lands while I'm reading it, then the view stays put (no jump to the new turn)

### Goal and judges (`GOAL`)

- [x] F-goal-1: *Done when …*, when a round ends unmet, then wisp sends the task back until the check passes
- [x] F-goal-2: a running task, when I add a goal from the reply box, then it's checked from the next round
- [x] F-goal-3: a goal, when I add up to five judges (harness, model, thinking), then each votes and the band shows votes
- [x] F-goal-4: a judge's vote, when I click it, then its check opens like a task with everything it said
- [x] F-goal-5: a goal, when I pause, resume or drop it, then the band follows and the reason stays after quit
- [x] F-goal-6: Goal in the prompt box, when I send, then the Goal button lights while on and starts off next time
- [x] F-goal-7: `/goal …` right after a task lands, when sent, then the goal is checked
- [x] F-goal-8: a goal, when it asks for judges in a sandbox (or the task runs in one), then its judges run in that sandbox

### Crew (`CREW`)

- [x] F-crew-1: Crew → Add, when I give a name, role, brief and schedule, then a member exists and checks in on its clock
- [x] F-crew-2: a member, when I build their spirit (body, glow, eyes, mouth, extra), then the face shows everywhere
- [x] F-crew-3: a member, when I click them, then their thread fills the main pane with every job as a conversation
- [x] F-crew-4: a wide window, when I look at the sidebar, then a rail shows each member's face; clicking filters the list to their jobs
- [x] F-crew-5: the rail, when I hover a face, then a card shows name, role, status and tasks
- [x] F-crew-6: members, when they work, then a notebook keeps facts between jobs without filling context
- [x] F-crew-7: a member, when I open *Also starts when*, then it starts on events (PR opened, CI failed…)
- [x] F-crew-8: any task, when I ask *add a crew member who reviews PRs hourly*, then it's added
- [x] F-crew-9: a member on another machine, when they get a job, then it runs there
- [x] F-crew-10: a member, when I edit *Skills and instructions*, then their own skills and AGENTS.md go with each job
- [x] F-crew-11: a delegated crew job, when I click its row in the lead's team panel, then the job opens
- [x] F-crew-12: a member or the whole crew, when I click Share, then a `.crew.json` or link is made; dropping one imports it
- [x] F-crew-13: a member, when I pick *See what X used*, then their usage and what ate my limits show
- [x] F-crew-14: the Org page, when I drag cards, then the chart keeps my layout
- [x] F-crew-15: a member's job that asks me something, when I look at the list, then it's in Needs you with their face
- [x] F-crew-16: the Crew section, when nobody is busy, then two members stay and the rest fold (busy first)
- [x] F-crew-17: a member's check-in, when it schedules a follow-up and finds nothing, then the follow-up still comes
- [x] F-crew-18: the API, when I `POST` a job or check-in, then the member runs it
- [x] F-crew-19: a member's kit, when it builds its own, then the kit is saved
- [x] F-crew-20: a member's finished job, when its toast or notification is clicked, then that job opens

### Automations, triggers, schedules (`AUTO`)

- [x] F-auto-1: Automations → Triggers, when I add a cron schedule, then a task starts on time
- [x] F-auto-2: a *CI failed* trigger, when a GitHub Actions run fails, then a task starts (or follows up the branch's task)
- [x] F-auto-3: Slack, Linear, Sentry, PagerDuty or a webhook trigger, when it fires, then a task starts
- [x] F-auto-4: a Slack trigger with `users`, when someone not listed posts, then nothing starts
- [x] F-auto-5: a task, when I type *in 10 min check again* in the reply box, then the follow-up goes then
- [x] F-auto-6: scheduled items, when I look at the list, then a Scheduled section holds tasks, reminders and check-ins
- [x] F-auto-7: a waiting item, when I click Edit, then I change its text or time before it goes
- [x] F-auto-8: an agent scheduled a check-in, when its task finishes, then it moves from Needs you to Scheduled
- [x] F-auto-9: a plan near the limit, when work would pass it, then it waits for the reset and goes after
- [x] F-auto-10: *Automations stop at* per harness, when usage passes it, then automations wait
- [x] F-auto-11: low free memory or high CPU, when a check-in or trigger is due, then it waits
- [x] F-auto-12: a banked Codex reset lands early, when it does, then waiting work goes
- [x] F-auto-13: a PR conflicts, when I click *Fix conflicts*, then a follow-up resolves it (auto-fix off unless set)
- [x] F-auto-14: *Auto-fix PR conflicts* on, when main moves, then wisp fixes shipped PRs itself
- [x] F-auto-15: a webhook URL in config, when a task finishes or asks, then wisp POSTs it
- [~] F-auto-16: a spell, when I cast *a floating button that opens my PRs*, then wisp changes itself — dropped
- [~] F-auto-17: a spell with `crew`, when installed, then its members join — dropped

### Worktrees (`WT`)

- [x] F-wt-1: `.worktreeinclude`, when a worktree is made, then ignored files listed are copied (copy-on-write on a Mac)
- [x] F-wt-2: a repo with `.conductor/settings.toml`, when a task starts, then its setup runs
- [x] F-wt-3: Conductor worktrees, when I open the project, then *Bring in* lists them
- [x] F-wt-4: I mark a task done, when it has a worktree, then wisp asks to remove it (and its delegates')
- [x] F-wt-5: old worktrees, when the hourly sweep runs, then merged/closed and old Done ones go, under a cap
- [x] F-wt-6: Settings → Worktrees, when I open it, then I see every worktree with Keep and Remove
- [x] F-wt-7: a task archived right after it finishes, when its worktree is clean, then it's removed

### Ship, PR, review (`SHIP`)

- [x] F-ship-1: a finished worktree task, when I click Ship it (S), then it commits, pushes and opens a PR
- [x] F-ship-2: nothing to ship, when the task finishes, then Ship it doesn't show
- [x] F-ship-3: Ship it, when I click it, then the dialog opens at once and shipping happens in the background
- [x] F-ship-4: a PR, when I click Merge, then it confirms and merges
- [x] F-ship-5: a PR with running checks, when they change, then the PR state updates faster
- [x] F-ship-6: agents open PRs, when they call `wisp_pr`, then the PRs attach to the task
- [x] F-ship-7: a task's PR merged, when more changes are made, then Ship it comes back
- [x] F-ship-8: *Review by another harness* on, when a PR is open, then another harness reviews before Merge (sees what I ruled out, says if partial)
- [x] F-ship-9: a PR conflict, when I see the PR row, then Fix conflicts is last, in the same place

### My Stuff, apps and panes (`STUFF`)

- [x] F-stuff-1: an agent's picture, doc or file, when I click Save to My Stuff, then it's on the My Stuff page
- [x] F-stuff-2: an agent saves with `wisp_save`, when it does, then a `stuff` card shows in the answer
- [x] F-stuff-3: a kept item, when an agent saves with its id, then a v2 appears in place
- [x] F-stuff-4: an app folder, when it's saved, then Launch opens it in wisp
- [x] F-stuff-5: `"db": true`, when the app runs, then it keeps data (seed.sql once, no 5 s cut on a slow disk)
- [x] F-stuff-6: an app, when I click Publish, then it's at a grid link; secrets and source maps stay off
- [x] F-stuff-7: *make me a pane that…*, when I send it, then an agent builds a pane and it pins beside tasks or on Home
- [x] F-stuff-8: a pane, when it asks for `tasks:write`, then it can only propose a prompt, loses access when code changes, and can't grow during Full auto
- [x] F-stuff-9: Agent board and Allowance board starters, when I add one, then it shows tasks and periods
- [ ] F-stuff-10: the phone app via wisp Cloud, when I open My Stuff, then I can launch apps — **missing**: no phone frontend

### Sharing (`SHARE`)

- [x] F-share-1: a task, when I pick Share…, then it's on my wisp Grid at a link anyone can open
- [x] F-share-2: a share link, when I import it, then the task is in my list
- [x] F-share-3: a share, when I stop it, then the link stops working
- [ ] F-share-4: a phone share sheet (Android), when I share a link or PDF to wisp, then New task opens with it attached — **missing**: no Android frontend

### Machines, cloud and the grid (`MACH`)

- [x] F-mach-1: another computer over SSH, when I add it in Machines, then agents run there
- [x] F-mach-2: `~/.ssh/config` and Tailscale, when I open Machines, then found hosts are listed
- [x] F-mach-3: a machine that doesn't know my key, when I click *Let me in*, then my password once installs it
- [x] F-mach-4: a machine with only SSH, when I add it, then wisp installs Node and itself there
- [x] F-mach-5: a machine on an older wisp, when I click *Update to X*, then it restarts on this version
- [x] F-mach-6: any project, when I pick a machine, then it runs there; a follow-up can move the task to another computer
- [x] F-mach-7: another machine, when I delegate to it, then the delegate runs there and says where
- [x] F-mach-8: Settings → Machines → This computer, when I check, then it says whether SSH answers here
- [x] F-mach-9: Settings → Cloud, when I sign in to a wisp Grid with a code, then my cloud computer joins
- [x] F-mach-10: a terminal, when I run `wisp computer on`, then this computer is on my grid
- [x] F-mach-11: my computers on the grid, when I open the grid's account page, then each is listed with Open and state
- [x] F-mach-12: `wisp grid serve`, when people sign in (GitHub, Google, OIDC), then each gets their own wisp
- [x] F-mach-13: a busy host, when someone signs in, then the grid admits by memory and waitlists the rest
- [x] F-mach-14: my grid account, when I type my handle and delete, then the account and box are gone
- [x] F-mach-15: an API key, when I call `https://<handle>.<domain>/api/…` with `Bearer wgk_…`, then it works without a browser
- [x] F-mach-16: a computer on the grid, when I open a preview from my phone, then its localhost page shows
- [x] F-mach-17: `/sandbox`, when I run it, then a fresh box runs the task and I can remove it
- [x] F-mach-18: Settings → Cloud, when I click Sign in for Claude on a cloud computer, then I sign in its agents from my desktop
- [x] F-mach-19: Settings → This Mac, when I open it, then I see free memory, CPU, disk, each task's RAM/CPU, harness folders, leftover processes with Stop
- [x] F-mach-20: a grid, when I open it after sign-in, then a real home page shows (not one long column)
- [x] F-mach-21: several grids or accounts, when I add an account, then each is listed

### Channels: phone, chat, mail (`CHAN`)

- [x] F-chan-1: ntfy set up, when a task finishes or asks, then my phone gets it; tapping an option answers (no repeat after restart)
- [x] F-chan-2: a Telegram bot, when I message it, then a task starts; replying to a question answers it
- [x] F-chan-3: *Say what you'd type in wisp* on, when I send `@ada in api on laptop with opus high plan: …`, then it reads like the launcher
- [x] F-chan-4: a forwarded Telegram message, when it arrives, then it doesn't run as my words
- [x] F-chan-5: Slack two-way, when I set it up from the manifest, then threads start tasks and buttons answer questions (linked people only)
- [x] F-chan-6: Discord, when I use `/wisp`, then a task starts and replies come back
- [x] F-chan-7: a GitHub comment starting `/wisp …` on my repo, when posted, then a task starts and answers there
- [x] F-chan-8: a secret mail address on the grid, when I mail it, then a task starts from the subject
- [x] F-chan-9: a sleeping cloud wisp, when Telegram/Slack/Discord posts, then the grid holds it, wakes the box and delivers once
- [x] F-chan-10: a channel-started task, when it runs, then it stays at Edits or lower and says when it stalls on me
- [x] F-chan-11: a report with a token in it, when it goes to a channel, then the secret is redacted
- [x] F-chan-12: Settings → Notifications, when a task finishes or asks, then a desktop notification shows
- [x] F-chan-13: Settings → Phone, when I open it, then one dialog sets up ntfy, Telegram, Slack, Discord, GitHub and email
- [x] F-chan-14: a channel message that names no project, when it starts a task, then it runs in the project picked as Default project, else the last used, else the first (a deleted pick falls through)
- [x] F-chan-15: Settings → Notifications, when I open Default project, then it offers Last used (naming that project) and every project

### Connectors and MCP (`CONN`)

- [x] F-conn-1: Settings → Connectors, when I add an MCP server once, then every harness's tasks get it
- [x] F-conn-2: a remote connector (PostHog, Linear), when I click Sign in, then OAuth replaces pasting a token
- [x] F-conn-3: a task, when I ask *add PostHog*, then the agent adds the connector
- [x] F-conn-4: Claude Code or Codex, when I run `wisp mcp`, then wisp's MCP tools (launch, wait, get, reply, list, stop, agents…) work
- [x] F-conn-5: an agent, when I run `wisp skill install --agent claude`, then the wisp skill is installed
- [x] F-conn-6: a task, when I ask *which tools do my tasks use?*, then `wisp_stats` answers
- [x] F-conn-7: a task, when I ask *have codex review this*, then the agent drives wisp (launch, delegate)
- [x] F-conn-8: Plan mode in Claude Code, when the agent asks or tags, then wisp's tools still work
- [x] F-conn-9: Codex in Edits or Plan, when it calls wisp's tools, then they aren't refused

### Agents, accounts, usage (`ACCT`)

- [x] F-acct-1: Claude Code, Codex, opencode, Cursor, Antigravity, Grok, Copilot installed, when I launch, then each runs headless with its mode flags
- [x] F-acct-2: Settings → Agents, when I click Sign in, then the CLI's own login runs with its link and code
- [x] F-acct-3: two plans for Claude Code or Codex, when I add a second account, then tasks can pick it
- [x] F-acct-4: opencode Zen/Go, when I paste the API key, then opencode runs on it
- [x] F-acct-5: Settings → Agents → Re-check, when I click it, then harnesses and models are found again (all machines)
- [x] F-acct-6: a missing harness, when I look in Agents, then a one-line install for my OS shows
- [x] F-acct-7: only the Codex or Claude desktop app, when I launch, then wisp uses their bundled CLI
- [x] F-acct-8: a harness's program changed, when wisp runs it, then it says so
- [x] F-acct-9: Settings → Usage, when I open it, then spend, turns, success by project and harness · model show
- [x] F-acct-10: Plan budget, when I open it, then each account's 5-hour and weekly windows show, with banked Codex resets
- [x] F-acct-11: usage reports, when windows move, then wisp shares the change out among tasks (*Where your plan went*)
- [x] F-acct-12: *Drain watch* on, when an account drains fast, then wisp warns
- [x] F-acct-13: a window changes length or reset, when wisp reads it, then it says so
- [x] F-acct-14: two accounts, when I open Usage, then tokens per model per account show
- [x] F-acct-15: Codex says 429 with room left, when it does, then wisp tells limits apart
- [x] F-acct-16: Codex *model at capacity* in Auto, when it happens, then Auto moves on
- [x] F-acct-17: finished tasks, when they go quiet, then a retro learns preferences and lessons, shown in Memory
- [x] F-acct-18: Settings → Auto, when I set goal, allowed harnesses and best-of-two, then Auto follows

### Settings (`SET`)

- [x] F-set-1: Settings, when I open it, then it's a sidebar of panes (General, Notifications, Agents, Auto, Usage, Memory, Automations, Phone, Machines, Cloud, Connectors, This Mac) with the X pinned
- [x] F-set-2: General → Theme, when I pick System, Light or Dark (or ⌘K), then the theme switches
- [x] F-set-3: General → Terminal, when I pick in-wisp, Auto-detect or an app, then Take over uses it
- [x] F-set-4: General → Editor, when I pick one, then Open in editor uses it
- [x] F-set-5: General → Worktree per task, when I turn it off, then tasks run in the repo
- [ ] F-set-6: the Android app, when I look for shortcuts, then the shortcuts sheet is hidden — **partial**: needs the Android frontend
- [x] F-set-7: Windows, when settings save while a file is held, then it retries instead of failing
- [x] F-set-8: Settings → Keys, when I open it, then shortcuts are listed

### Keyboard (`KEY`, from `web/js/features/keys.js`)

- [x] F-key-1: anywhere, when I press ⌘K or ⌘L, then the palette opens or the launcher gets focus
- [x] F-key-2: the list, when I press N, then New task opens
- [x] F-key-3: a menu open, when I press Esc, then only it closes
- [x] F-key-4: anywhere, when I press ?, then the shortcuts sheet opens
- [x] F-key-5: a focused task, when I press J/K, then focus moves down/up; O or Enter opens
- [x] F-key-6: a focused task, when I press E, A, L, M, Delete, then done, archive, for later, rename, delete
- [x] F-key-7: a focused task, when I press S, ⇧R, ⇧F, T, V, D, P, ], /, then ship, rerun, fork, take over, transcript, changes, PR, panel, reply
- [x] F-key-8: a long feed, when I press U/Home or C, then back to top or collapse all
- [x] F-key-9: the desktop app, when I press Ctrl+` , then the terminal toggles
- [x] F-key-10: ⌃Tab, when tapped or held, then the switcher

### CLI and integrations (`CLI`)

- [x] F-cli-1: a terminal, when I run `wisp "fix the test" --wait --json`, then the task runs and prints its result
- [x] F-cli-2: tasks, when I run `wisp ls --state=review`, `show`, `wait`, `reply`, `stop`, then they work (a bad state errors)
- [x] F-cli-3: a task, when I run `wisp ship <id>` or `wisp merge`, then it ships or merges
- [x] F-cli-4: `--compare codex,opencode:model`, when I launch, then it runs on each
- [~] F-cli-5: the JS SDK `WispClient`, when I script launch/wait/reply, then it works — dropped
- [x] F-cli-6: `GET /api/openapi.json`, when I fetch it, then the API is described
- [~] F-cli-7: v1's HTTP routes, when a script calls them, then they work — dropped
- [x] F-cli-8: `~/.wisp/server.json`, when a tool reads it, then it finds the server
- [x] F-cli-9: `npx wisp-agents`, when I run it, then the desktop app (or `--browser`) opens
- [x] F-cli-10: any plugin, when I run `wisp <plugin> <verb>`, then that command runs (help, completions)
- [x] F-cli-11: an old `~/.wisp`, when I run `wisp import`, then tasks, crew and settings come over once

### Desktop app (`DESK`)

- [ ] F-desk-1: a Mac, Windows or Linux, when I open wisp.app, then wisp opens in its own window (Tauri, a third of Electron's size) — **partial**: window checks are macOS only; Windows/Linux CI check the server and quitting
- [x] F-desk-2: a release, when I download, then there's a .dmg, Windows installer and Linux package
- [x] F-desk-3: `curl … install.sh \
- [x] F-desk-4: the app, when I right-click text, then spelling, copy link, cut/copy/paste show
- [x] F-desk-5: a slow `~/.zshrc`, when the app first opens, then the window shows within 5 s
- [x] F-desk-6: the app with no task open, when I drag the top, then the window moves

### Phone app and phone web (`PHONE`)

- [ ] F-phone-1: the Android app, when I add a computer over SSH, then wisp opens on it — **missing**: no v2 Android frontend (ADR-0031: v2 bundles only; queue item 8)
- [ ] F-phone-2: the app in the background, when a task finishes or asks, then an Android notification shows; tapping opens the task — **missing**: as above
- [x] F-phone-3: the app's list, when I swipe right/left, then mark done / save for later
- [ ] F-phone-4: a task page, when I swipe left, then the side panel comes in — **partial**: kit `task.body-swipe` gesture built and tested (twin: the side-panel button); no phone task page mounts it until step 8 (round 225)
- [ ] F-phone-5: wisp Cloud, when I sign in from the app (GitHub/Google), then my cloud wisp and computers are listed — **missing**: 
- [x] F-phone-6: iPhone Safari on my cloud wisp, when I Add to Home Screen, then it runs full screen
- [x] F-phone-7: a browser under 900 px, when I tap a task, then it opens as its own page
- [ ] F-phone-8: 4,000 tasks on a bad connection, when I use the app, then it stays quick — **partial**: spikes/phone-bench: fling 110/141 slow frames → 2/298, first tap on Slow 4G 1,273 ms blank → 171 ms; left: warm tap ~90 ms vs 50 budget, initial JS 288.7 KB br vs 120 (round 214)

### Updates, install, telemetry (`UPD`)

- [x] F-upd-1: a newer wisp, when I open wisp, then an Update button shows and Update now updates however I installed it, on the same port
- [x] F-upd-2: the update channel, when I pick beta or nightly, then I follow it (semver prerelease order)
- [ ] F-upd-3: Homebrew or the curl line, when I install, then wisp installs — **partial**: install.sh and the brew formula need a published v2 release to verify
- [x] F-upd-4: telemetry on, when I use wisp, then anonymous stats and errors go to PostHog (off switch)

### Look and feel (`LOOK`)

- [ ] F-look-1: any screen at 320–1440 px, light and dark, when I compare with v1, then it looks the same — **partial**: needs the outer-loop parity run
- [x] F-look-2: toasts, when several come, then they stack as light cards with a glyph
- [x] F-look-3: a working task, when I look, then the dot-grid ripple shows
- [x] F-look-4: the app, when I look at the logo, then it's the handwritten w with the orb
- [ ] F-look-5: control text (buttons, pills, tabs), when the window narrows, then nothing wraps and bars collapse to icons then More — **partial**: needs the outer-loop layout run


## 7. Bugs and known gaps

### Bugs (`docs/qa/bugs.md`)

- [ ] B1 Sending from Home left a blank page: fixed (round 198), to re-check by QA
- [ ] B2 Harness picker showed Claude Code twice: fixed (round 198); the picker is being redone (F-comp-1)
- [ ] B3 "Mark read needs a newer wisp.runtime": fixed (round 198), to re-check by QA

### Known gaps left by rounds (to do)

- [ ] A refused command stored before masking ranges is hidden whole after a restart until its task's values load
- [ ] A Windows terminal joins its job a moment after it starts (ADR-0063 gap)
- [ ] The cause of the >30 s sandbox seed stall on Linux CI (keep box logs on failure)
- [ ] Learn list items edited by the user or aged out are masked only for `.env`-shaped lines (ADR-0055)
- [ ] Crew notebooks are not `.env`-masked (no single task; ADR-0055)
- [ ] F-phone-8: warm tap 90 ms vs 50 ms budget
- [ ] Cloud API keys can't be made inside the cloud wisp's own Settings (`/_grid/keys` sign-in)
- [ ] A stream already open with a revoked API key isn't cut
- [ ] Share links on another grid aren't revoked by Share again
- [ ] `uncarryStuck` note isn't shown on the Machines card
- [ ] Ledger keys (j/k, /, [ ]) work only with focus inside the ledger (they mean other things page-wide)
- [x] `wisp import` drops v1 per-turn changes (importer.rs), so imported tasks show 0 files on day cards (r267: kept in `ext["wisp.runtime"].changes` like a live turn; `a_v1_turns_changes_*` tests)
- [x] Ledger scrubber day labels; filter row never clipped (round 276)
- [ ] Ledger not yet in design/v1-reference/allowed.md for the parity gate
- [ ] `rows-view.ts` forces `en-GB` (peer-note fixed in r241) dates: check v1 (later/format.ts is en-GB on purpose, as v1) and use the app locale where v1 did
- [ ] Popover reads its width once (an iPad rotating while open doesn't switch popover/sheet)
- [ ] Android long-press sheet has no Select row (list selection mode comes with step 13)
- [ ] `phone.tsx` copies iOS token values as numbers (IOS_SEP has no token); 466 lines
- [x] Pull to refresh may never show its spinner (reads `connecting()` right after `reconnect()`)
- [ ] Decide: should push notifications answer tool approvals in place (like ntfy)?
- [ ] A machine that never answers keeps a lead in Working until Stop (no give-up timeout for far subagents)
- [ ] ~300 strings no code uses (shell-home tips/setup/getapp, unused wisp.later.*, shell.key.switcher, shell-settings.diag.*): delete, with a check that fails on unused keys
- [ ] CI: Windows still cancelled at 60 min on 708ba936 (cargo test ended 04:19 with 6 failing targets, then a leaked process held the pipe): coder-r257
- [x] `wisp tasks` with no flags starts a task with the prompt 'tasks' (a known command word taken as a prompt): a command word alone prints its usage, a typo of one says *did you mean*, a plugin's name lists its verbs; `wisp -- <word>` is the task (`crates/wisp/src/words.rs` `not_a_task`, `serve_cli.rs` `a_command_word_or_a_typo_of_one_starts_no_task`)
- [ ] README's Development lines still describe v1's npm run dev
- [ ] No tested `.vscode/launch.json` (CodeLLDB) for wisp serve
- [x] ADR-0074 gaps: a command's events go out with its writes (every `Store::write` in a command stages what it emitted so far; the rest at its return), and budget drain/early, channels `startedFrom`, machines `changed`/`removed` go in an outbox with their write (ADR-0075; `kernel_events.rs` `a_commands_events_go_out_with_its_write_so_a_reader_of_its_row_finds_them`, `durable_events.rs`, `plan_free.rs`, `machines/host/tests/events.rs`)
- [x] `wisp add` of a second folder with a project's name (`/a/wisp`, `/b/wisp`) replaced the first project; Run on did the same on the other computer (it misread `resources.export`): the second is `wisp-2`, the same folder again stays one project (ADR-0076; `serve_cli.rs` `wisp_add_of_a_second_folder_with_the_same_name_keeps_both_projects`, `runtime/api/tests/paths.rs`, `kinds/project.rs` `two_same_named_folders_*`, `run-on.test.ts` `…_r288_run_on_same_name`)
- [x] Every client but `wisp add` and Run on still named a new project itself through `resources.apply` (an agent, Projects' Add, the phone, a script), overwriting a same-named one, and two adds at once could pick one free name: `runtime.projectAdd { path, name? }` names it on the server in one store transaction (`StoredKind::put_with`), with its MCP tool, refused to capped tasks; `wisp add`, Projects' Add and Run on use it (ADR-0077; `runtime/host/tests/project_add.rs` `two_concurrent_adds_*`, `the_same_folder_twice_*`, `a_caller_given_name_*`, `a_relative_path_*`, `a_capped_task_*`, `a_name_with_separators_*`; `serve_mcp_tools.rs` `an_agent_adds_a_project_*`; `run-on.test.ts`, `projects.test.ts` `…_adr0077_guess`, `mock-commands.test.ts`)
- [x] machines.removed reaches a handler only after the peer is cleared from memory (r279: memory first, then the write that carries the event; `plugins/machines/host/tests/events.rs` `a_machines_removed_handler_reading_the_list_finds_the_peer_gone`)
- [x] serve_task follow_up_queue_and_stop_keep_the_queue relies on a 1.5 s fake run (fails under Windows load): hold the fake on a gate (r277: `FAKE_GATE` file gate in fake_agent; also serve_task ask/stale-ask/mcp/delegate and serve_crew_delegate)
- [x] serve_channels_cap and serve_delegate_machine still use FAKE_DELAY (convert to FAKE_GATE)
- [ ] Same-named phone chips share a highlight when one is pressed (give chips the computer in their id)
- [ ] A removed peer's pending optimistic writes aren't cleared (resetPeer)
- [x] Phone Tasks: a crew member's avatar touches the task title (no gap like the dot's): the phone feed row's leading column is one fixed width (`--size-avatar-sm`), dot / face / mark icon centred in it, so every title starts at one x (r291; `platform-css.test.ts` `a_crew_face_rows_title_starts_at_the_same_x_as_a_dot_rows`; Chrome: title x 47 on face and dot rows, android and ios, 390 and 320)
- [x] Phone New task sheet shows the whole Home page under the composer (Needs you, Tips): a sheet should hold just the composer: the phone sheet holds the prompt box alone, caret in it (no greeting, Needs you, usage, cards, tips, keys, feedback line); an empty Recipes list hides on phones as in v1 responsive.css (Start from an issue stays) (r291; `phone-pages.test.tsx` `the_new_task_sheet_on_{android,ios}_holds_only_the_composer_focused_with_no_home_list_or_tips`)
- [x] Lead's call (2026-11-16): agents (agent:session) and API keys may add projects through runtime.projectAdd (they already start tasks; capped tasks are refused)
- [x] Tests keep timing out on an `await import()` inside the test body under CI load (pane-host, machine labels): a check rule against it (r301: `dynamic-import-in-test`, 46 imports hoisted, 8 lazy-chunk waits allowed)
- [ ] Sign in with Apple on the grid, then on iOS (ADR-0065 open question 1)
- [ ] Windows: a plugin reinstall while its binary runs fails (`plugins/plugins/host/src/install.rs` ~215: remove_tree errors ignored, rename onto a non-empty dir); `wisp update` runs `npm install -g` while wisp.exe runs (EBUSY/EPERM likely) (r310)
- [ ] Write amplification 30x vs 15x on the 10-task stream (ADR-0082/0083): measure at real step sizes, trim the schema, then decide the budget with evidence
- [ ] `ui-only`/MCP: nothing open

## 8. CI

- **Shape (ADR-0069, r252):** suite.yml checks the Cargo workspace once (fmt, clippy --workspace --all-targets, test
  --workspace --no-fail-fast, its log kept for the result), then `node tools/ci-verify.ts --no-cache`: one monoflow
  verify per package that isn't a workspace member (43 npm workspaces, sdk/python, spikes/events, spikes/sync-bench-rs),
  after a failing Rust step too. `verify --all` had re-run each of the 149 crates' check, clippy and tests (23 of 33
  min on Linux, macOS past 60). Windows hung 38 min in `serve_desktop::with_no_wisp_running_the_app_starts_one`: the
  detached app inherited the test's stdout pipe (fixed: `wisp_proc::keep_std_handles`). To confirm on the next
  `ci:full` run: every OS under 45 min.
- **The other packages' step (ADR-0072, r269, proof = next `ci:full`):** 9ea69e14 Windows node 22 cancelled at 60 min
  with the step 26 min in, 28 of 46 packages done (plugin UIs 60-345 s each; Linux 7.7 min, macOS 16.4). Cause: 43
  per-package `tsc -p` runs, each re-checking the kit `npm run typecheck` had just checked, and 40 vitest starts. Now
  `tools/ci-verify.ts` credits a package's typecheck from the typecheck step's log when one of its programs holds the
  config's files with the same options (tools/ci-plan.ts `covers`: all 43 do; fallback: runs tools/typecheck.ts), runs
  the 39 plain-vitest packages' tests in one vitest process (tools/vitest.ci.config.ts projects, report split by
  folder), and sends the rest through monoflow (`--test` for the 4 node --test packages; whole verify for sdk/python
  and the spikes, started first). Locally all 46: 128.6 → 38.5 s. To confirm: the step < 8 min on Windows, the job < 45.
- **Windows 708ba936 (r257, proof = next `ci:full`):** the Rust test step hung 26 min after `6 targets failed`: an
  orphaned `opencode` probe held the `tee` pipe. Diagnosis: a child joins its own job only after `CreateProcess`
  returns; a server killed in that window (many serve tests stop theirs within a second of its start, while its ~10
  startup probes spawn) left it suspended outside every job. Fixed by ADR-0070 (`wisp serve` joins a nursery job,
  `wisp_proc::nursery`; detached starts break away), test `wisp-proc/tests/nursery_windows.rs`. The 6 targets:
  `serve_detect` (the test removed `agy.exe`, a copy on Windows, then copied from it: now rewrites its bytes),
  `serve_events` (compared a raw path inside JSON, where `\` is doubled), `serve_sandbox` (`ends_with("/code/proj")`
  on a `\` path: compared by parts), `serve_kinds` (8.3 `RUNNER~1` vs the long name: product fix, a task's folder and a
  project's path are stored long, `wisp_proc::long_path` / `GetLongPathNameW`), `serve_open` (os error 32: the folder
  was removed while the run's process, its cwd in it, was still exiting after its answer; the test waits for that pid),
  `serve_cli` Ctrl-C (the runner hands "ignore Ctrl-C" down: the attached CLI turns it back on,
  `wisp_proc::take_ctrl_c`, and listens from its start). Only Windows CI can confirm: `nursery_windows`, Ctrl-C under
  ConPTY, and `serve_open` (if a post-turn `git` in the worktree also holds the folder, the next step is a product one:
  the turn's end waits for the run's release, not the test).
- **Windows 8bb8ccd0 (r263, proof = next `ci:full`):** 14 serve tests failed: one folder spelled two ways (`%TEMP%` is
  `C:\Users\RUNNER~1\…`; r257 stored task folders and projects long, nothing else). ADR-0071: one helper,
  `wisp_proc::plain_path` (pure rule `spell_out_with`, tested on every OS with a mapped 8.3 name), applied where a path
  enters (the homes, the CLI cwd, `runtime.create` cwd/dirs before the project match, projects and triggers on apply,
  `projectRun`, `machines.changes`, the sandbox); the tests' `temp()`/`tmp()` are spelled the same. Causes: test-side
  only (expected the short path the product now stores long): `serve_crew_delegate`, `serve_crew_machine`,
  `serve_delegate_machine` ×2 (copy under the home), `serve_mcp_tools` ×2. Product: `projectRun` (short cwd vs a long
  project), `serve_kinds` worktree + `a_failing_setup_holds…` (the project was matched before the cwd was spelled: no
  worktree, no setup), `trigger:after` (the trigger's project kept short; and `C:\…` filters were an exact `glob` match:
  now by parts), `serve_delegate_machine` "HTTP 403" (home short, copy long: not a copy), `serve_sandbox` ("only a
  project under ~/code": `$HOME` short). `serve_history` setState: not a path: Mark done was refused while the CLI
  lingered after its result (`Run::Lingering`); fixed in `ops::set_state`, test
  `serve_task::a_task_whose_turn_ended_is_marked_done_while_its_cli_is_still_exiting` (fails before). **Left:**
  `serve_open::a_folder_gone_opens_at_its_base` os error 32 is not diagnosed (no Windows here): the run's CLI pid is
  gone, so another process of wisp's works in the worktree after the turn (candidates: ship's `TurnEnded` git/gh,
  the worktree/git row refresh). The test now prints every live process with its command line when the remove fails,
  so the next Windows run names the holder; then fix it in the product (the folder is free once the run ends).
- **Windows 9ea69e14 (r268, proof = next `ci:full`):** `cargo test --workspace` failed 3 targets (was 14). (1)
  `wisp-machines --test carry` ×2, HTTP 403 "Only a copy wisp carried…": r264 spelled `machines.changes`' cwd long and
  left the copies root as `Paths` was given (short in the test; in a product the same goes for any home not spelled at
  entry), so `is_copy` compared long against short. Product fix: `carry::copy_at` spells both sides with
  `wisp_proc::plain_str_blocking`; test `carry::tests::a_copy_is_one_whichever_side_is_spelled_short` (mapped 8.3 name,
  fails before on every OS). The plugin host tests' temp helpers now return the long spelling as `crates/wisp/tests/common`
  does (machines carry and bootstrap_install, sandbox seed, grid-client accounts, plugins install, channels setup; git and
  worktrees were already canonical; push has no wisp-proc and compares no path). (2) `wisp-sandbox --test seed` ×2:
  test-side only: the expected path was built from the short temp dir and joined `code/my-app` with `/`; now long and
  `join("code").join("my-app")`, plus an assertion that a returned Windows cwd has no `/`. The sandbox's own `~/.sandbox/…`
  and `.wisp/sandbox.sh` joins are now per part. Elsewhere `.join("a/b")` remains for files only read or written (Windows
  takes `/` there), never for a stored or compared path. (3) `serve_open::a_folder_gone_opens_at_its_base` os error 32:
  no handle is kept: no file watcher in the tree, no `set_current_dir`, no file kept open under a task's folder, the
  claude run uses pipes (no ConPTY), and the dump showed no child of `wisp serve` alive. The holder is transient: ship's
  `TurnEnded` recheck (`plugins/ship/host/src/wire.rs` `turn_ended` → `follow::recheck` → `git rev-parse`, `git status`,
  `git rev-list --count`, all with their cwd in the worktree) starts as the turn's row turns ended, exactly when the test
  removed the folder, and Windows refuses to remove any process's cwd; those gits were gone before PowerShell listed.
  Not a product bug (wisp checks what a task has to ship after its turn; a user's `rm` in that second meets the same).
  The test now stops wisp, waits until nothing it started is alive (by parent pid, `wisp_proc::processes`: a job's kill
  is asynchronous), removes the folder, and opens on a restarted wisp. If it is still held then, the dump says what it
  can: Restart Manager over every file left (or that no file is held: then a process has the folder itself open), what
  is alive under the test and under the stopped server, every git/wisp/harness/console process alive anywhere, command
  lines naming the folder, and the last 40 lines of serve.log. A process holding only the folder as its cwd is named
  only if it is one of those. Not seen on Windows yet: only the next run can confirm (2) and (3); Defender holding a
  fresh file is the next suspect if (3) still fails with wisp stopped.
- **Windows db8f01ad node 24 (proof = next Windows run):** `serve_orphans::a_probe_whose_caller_hung_up_ends_at_its_time_limit`
  panicked at "the start's probes outlived their time limit" (node 22 and every Unix job green). Not a product bug: the
  probes end at their 15 s limit with their whole tree (`wisp_proc::local::collect` → `kill_now` → `TerminateJobObject`
  on the per-child job). Cause: the test watched them by pid (`tasklist /FI "PID eq n"`), and Windows hands a freed pid
  to the next process at once. A timeline of the start (this Mac): 9 hung probes start at 0 s and end at 13.8-14.1 s;
  at 14.3 s wisp.accounts' sign-in check (`agy models`, `HarnessLogin.status`, 8 s limit) starts a new hung fake, while
  the 5 sibling tests start their own hung probes and the watch loop itself starts a `tasklist` per pid per 100 ms. Any
  of those taking a freed pid kept a dead probe "alive" past the 25 s budget. Fix (test-side): every process the file
  watches holds an exclusive lock on a file of its own for its whole life (`fake_agent.rs` `hold_live`: a record's
  `live`; `FAKE_GRANDCHILD`'s `<file>.<pid>.live`: the run writes the pid line once the sleeper says on its pipe that it holds
  its lock), and
  `serve_orphans` `Watched::running` asks that lock (`try_lock`), never the pid; a leftover is ended by pid only while
  its lock is held. Test `a_probe_that_ended_is_told_apart_from_a_process_that_got_its_pid` (a killed probe's record
  with a live process's pid reads as ended; fails with the old `ps` check on every OS).
- **Last full run 03c0046d:** Linux and macOS fail only `packages/ui/test/css-rules.test.ts` (2 tests; fixed in round 244, 8e60cdf5 + c8173ea6); Windows jobs cancelled after the others failed. Earlier: 2790d627's probe test fixed in round 234.
- **Windows groups still open** (from the last Windows results, b7499c23 → 8c2a17ed): most fixed since; to re-count on
  the next full run. Logs in `target/ci/*-failed.log`.
- **Earlier green:** 0a22f297, 3fcb1196, ed7924d1 had Linux and macOS fully green.

## 9. Blocked

- **iOS app (phone step 10):** needs Xcode installed on this Mac (owner).

## 10. Decisions (ADRs)

- 0001: monorepo monoflow
- 0002: typescript node zero deps
- 0003: everything is a plugin
- 0004: store sqlite change feed
- 0005: sync topics cursors
- 0006: client state
- 0007: ui solid kit
- 0008: capability sets
- 0009: rust wisp native
- 0010: peers transports
- 0011: performance budgets
- 0013: rust for everything but the ui
- 0014: grid control plane
- 0015: rust dependencies
- 0016: foundation metrics
- 0017: immediate feedback
- 0018: frame harnesses pending
- 0019: openapi from the route table
- 0020: per app sqlite
- 0021: analytics sinks
- 0022: cli covers the gui
- 0023: terminal context
- 0024: frontends are plugins
- 0025: jiff for time zones
- 0026: wisp as code
- 0027: task specs
- 0028: code first
- 0029: history and formula reruns
- 0030: formula crates
- 0031: no v1 compatibility
- 0032: share signatures
- 0033: repo carries its agent history
- 0034: formulas as modules
- 0035: adopt outside sessions
- 0036: my stuff and apps
- 0037: hosts for peers channels and keys
- 0038: share through wisp cloud
- 0039: docs site
- 0040: meet users where they work
- 0041: grid is the hub
- 0042: swappability of new features
- 0043: triggers and crew wakes
- 0044: events one log two speeds
- 0045: grid mail dns resolver
- 0046: plugins installed at runtime
- 0047: sandboxes
- 0048: untrusted plugin tier
- 0049: preview proxy and element picker
- 0050: env values masked on the server
- 0051: task topic is a turn window
- 0052: crew shares through the share plugin
- 0053: a tasks folder is ready before each turn
- 0054: other plugins data about a task is env masked
- 0055: env masking prose vs code and what stays out
- 0056: try it two ways is a linked pair of tasks
- 0057: the desktop app hands its server the login shells path
- 0058: api keys for the cloud wisp
- 0059: a delegate on another machine works on a copy
- 0060: a sandbox gets its project raw and a delegates copy goes with its lead
- 0061: several grid accounts one current
- 0062: server errors reach telemetry through kernel failures
- 0063: exec plugins die with wisp through a contain hook
- 0064: a box s pages show at the grid s preview address
- 0065: phone apps are native shells on the kit
- 0066: harness detection lands on a topic as it settles
- 0067: the first page loads only what the first screen draws

## 11. Work log (recent rounds)

| Round | What | State |
|---|---|---|
| 283 | DX: build bundle + Tauri lock: stopped before edits; plan in §4.6 | to redo |
| 282 | Server perf budgets: stopped before edits; plan in §4.3 | to redo |
| 310 | Windows: fakes placed race-free (no rename over a running exe) | committed |
| 309 | glibc heap trimmed after first looks (ADR-0084); start does less: cold start -25% | committed |
| 308 | One commit window for every running turn (ADR-0083): patch p99 304 -> 103 ms | committed |
| 307 | Patch p99, append, write amplification gated (ADR-0082); FAKE_STREAM | committed |
| 306 | Task list first look in pages from the session table (ADR-0081): first page 1.3 ms, RSS 99 MB | committed |
| 305 | Cold start 427 -> 67 ms: review's startup clear after serve, only running reviews | committed |
| 304 | Server budgets gated in PR CI on a real wisp serve with 5k tasks (ADR-0080) | committed |
| 303 | `npx vitest run <file>` from the repo root uses the file's package config (root vite.config.ts projects, shared with CI's); a bare root run refused by the hook | committed |
| 302 | pull.test.ts: not load: run from the repo root, solid-js resolves its server build (no effects); the test now says so | committed |
| 301 | Check rule `dynamic-import-in-test`: 46 test imports hoisted, 8 lazy-chunk waits allowed with a reason | committed |
| 300 | Sign out revokes the device's grid token (`POST /api/device/revoke`, ADR-0078); offline queued; desktop grid-client too | committed |
| 299 | Phone step 14 done: Crew tab + wisp Cloud sign-in on Computers | committed (1febaf4e) |
| 298 | "This Mac" labels follow the OS (owner 11-08); My Stuff says each item's computer; iOS compose only on Tasks | committed |
| 297 | Phone step 14 part: Settings + My Stuff platform screens (left: Crew tab, cloud sign-in) | committed |
| 296 | Team rows as v1: machine chip, ticking time, time taken (F-crew-11) | committed (f1dcea35) |
| 295 | v1 #800 ported (keyboard-clipboard task ID -> mention); #799, #801 already held, now tested | committed (9032c602) |
| 294 | iOS selection keeps the bar (no jump); stepping replaces history (Back -> Tasks); desktop j/k too, as v1 | committed |
| 293 | Phone step 13 done: selection + bulk, iOS filter pull-down, Next/Previous task | committed (357aec16) |
| 292 | Phone platform CSS bans in npm run check (shared module) | committed |
| 291 | Phone New task sheet = composer only; crew avatar aligned; lead fixed raw px (4a43d45e, 73a52eb0) | committed (2891737c) |
| 290 | runtime.projectAdd: server-side naming in one write; CLI, UI, Run on, MCP use it (ADR-0077) | committed |
| 289 | Same-named folders: wisp-2 instead of replacing (CLI + Run on; ADR-0076) | committed (bdcdb92c) |
| 288 | One project selector with multi-pick everywhere (owner decision) | committed (d11a93ad) |
| 287 | Crew on phones as stories with a reply box (owner's pick C) | committed (9da2cf94) |
| 286 | Phone task page, New task, search (step 13; left: selection mode, iOS filter pull-down, swipe between tasks) | committed (cb800d63) |
| 285 | Strings fix; phone row time/pill lines; peers.remove drops rows (duplicate tasks bug); same-name chip labels | committed |
| 284 | Phone step 13 part: Tasks screen (top bar, filter, chips, FAB) + Add computer (QR, Trust key, Stay connected); list-at-top scroll fix | committed |
| 281 | Phone step 8: the app's frontend (opens on Computers, all computers connected) | committed (ae086063) |
| 280 | Pull to refresh shows its spinner until the reconnect settles | committed |
| 279 | machines.removed after the peer is cleared from memory | committed |
| 278 | serve_channels_cap + serve_delegate_machine on gates (FAKE_GATE_ALL) | committed (b4d85054) |
| 277 | 'While running' tests held on a gate (serve_task x5, crew delegate) | committed |
| 276 | Ledger: scrubber day labels; filter row one line (collapse, then scroll) | committed |
| 275 | serve_orphans: Windows pid reuse (test tracked pids); fakes hold a life lock | committed (488e7813) |
| 274 | A command's events go out with its writes; budget/channels/machines via outbox (ADR-0075) | committed (d60b14dc, f4679d2d) |
| 273 | 4 tools tests on Windows: 2 product bugs (commands ignored-file check off on Windows; dev home path rules) + 2 tests | committed (a7297cd7, 8921f5ae) |
| 272 | A write's events queued before its rows can be read (outbox, ADR-0074) | committed (d1a32104) |
| 271 | Budget counts a turn that just ended: compare/view wait for earlier events (kernel Bus::delivered, ADR-0073) | committed (3cd4ffd7) |
| 270 | Two load-sensitive UI tests + a Select stuck-open bug (product) | committed (b532ec4e, 302cff77) |
| 269 | CI's other-packages step runs each check once (ADR-0072): 128.6 s -> 38.5 s locally | committed (836287a4); proof = CI on 836287a4 |
| 268 | Machines copy check spells both sides (product); sandbox joins; long temp in plugin tests; serve_open | committed (d3165656) |
| 267 | wisp import keeps v1 per-turn file changes (ledger 0 files) | committed (3d55a9e2) |
| 266 | Every CSS rule shared by check and css-rules.test (the :is() rule passed check, failed CI) | committed (608cedcb) |
| 265 | Phone kit controls (ADR-0065 step 6), browser unchanged; lead fixed queued.css :is() | committed (a6c727ac, ed23c434) |
| 264 | Paths spelled once at entry (ADR-0071): 6 product bugs + Mark-done race; 13/14 Windows serve failures; serve_open holder still unknown | committed (46437ddb, 236c6c04, 9ea69e14) |
| 263 | Windows nursery test: pid marker mid-line (libtest prefix), deadlines on every blocking step | committed (ff809a38) |
| 262 | Queued follow-ups inside the reply box; the queue now shows on a real server (queue_item rows) | committed (5d72c830) |
| 261 | Scheduled follow-ups inside the reply box (kit top slot, shell.task:composer-top) | committed (f58828bd) |
| 260 | A command word never starts a task; typos say 'did you mean'; `wisp -- <word>` | committed (75b53b16) |
| 259 | `commands` rule resolves paths against tracked files | committed (ef174895) |
| 258 | Windows file-URL paths fixed repo-wide (web tests, qa, mobile, tools/check); new rule file-urls; python SDK test | committed (46c87560) |
| 257 | Windows CI: nursery job (ADR-0070), long paths, Ctrl-C, 4 test fixes | committed (7ccc17f3, d90e5992, 85841811; lead-verified on macOS + windows-target clippy; proof = next ci:full) |
| 256 | Dialog focus return: a real bug (Zag trap picks its target late; opened before the machine loaded, focus stayed on body), fixed in Dialog | committed (45b105df) |
| 255 | docs/dev.md: clone to running, debugging, one test per area, troubleshooting (every command run) | committed (b3b5b3d9) |
| 254 | Port v1 #797: paste after selecting text reaches the Shell | committed (3f45f724) |
| 253 | Port v1 #794: Plan runs commands for your own tasks; outside/capped/side stay read-only | committed (f0b27570; lead-verified: plan 3, plan_mode 2, clippy, fmt) |
| 252 | CI checks the Cargo workspace once (ADR-0069, tools/ci-verify.ts); Windows serve_desktop 38-min hang fixed (keep_std_handles); lead fixed a unix.rs clippy error | committed (a3d7e1b0, 708ba936); proof = next ci:full run |
| 251 | Lead waits on remote, sandbox and crew subagents (v1 child.relay) | committed (5dee4072; lead-verified) |
| 250 | Each subagent's report goes back as it ends (v1 timing) | committed (e9755bab) |
| 249 | Offline bar strings eager (strings-split CI failure) | committed (301a9cf5) |
| 248 | Initial JS 119.4 -> 107.2 KB br (Compare split, 756 strings lazy); lead restored offline bar strings | committed (59a460b2, 5d23195a) |
| 247 | Lead waits on its subagents + report-back + slot merge (F-task-51) | committed (2ec6192c, 25500b26, 66ca5968) |
| 246 | Port v1 #795 + #796 (compact follow-ups list, kit ScheduledList) | committed (58ade620, 81eac943) |
| 245 | Try it two ways folds (≤1099/≤700) + Default on compare runs | committed (1d79d6e9; lead-verified) |
| 244 | css-rules failures fixed; check and test share @wisp/tokens/css-rules | committed (8e60cdf5, c8173ea6) |
| 243 | Launcher bar as v1 at every width | committed (a6ba4cac) |
| 242 | Settings and org CSS lazy: initial CSS 30.0 -> 26.9 KB br, looks unchanged | committed (826b025f) |
| 241 | Review of rounds 227, 228, 229: one fix (offline time in app locale); all committed | committed |
| 240 | Picker account row wraps as v1 (Wrap group); task clock locale | committed (af6dd52b, f838e783) |
| 239 | Harness pill folds at v1's breakpoints (8 rules aligned) | committed (441ea737) |
| 238 | Auto pick segment: v2 already matches v1's shortLabel; test pins it | committed (61ddba34) |
| 236 | "What will Auto run" + compare runs on the picker | committed (c390aca5; its app-picker.css .pk-vary waits for r237) |
| 237 | 3 check errors fixed (dev proxy in @wisp/web, picker-h token, no wrap) + dayName locale | committed (4fda9295, 85c563ac, c659fbee) |
| 235 | Task history: direction A, Ledger + scrubber | committed |
| 234 | CI: probe test after the probe memo (ends at its limit) | committed |
| 233 | Harness picker back to v1 (one pill, one two-pane popover) | committed |
| 232 | `npm run doctor` | committed |
| 231 | Check rule `commands`; 13 broken commands fixed | committed |
| 230 | `npm run dev`, `npm run dev:desktop` | committed |
| 229 | Offline and bad network (client, sync, shell) | committed |
| 228 | Push plugin + grid relay (HPKE) | committed |
| 227 | Gesture seams + phone overlays | committed |
| 226 | Android native: Keystore, QR pinning, ports per computer | committed |
| 225 | Gestures with visible twins | committed |
| 224 | Layout/feedback on phone views | committed |
| 223 | Install manifest only on a grid box | committed |
| 222 | First download under budget (ADR-0067) | committed |
| 221 | Mock Status shape; sandbox seed waits | committed |
| 220 | Kit rows and surfaces (phone) | committed |
| 219 | CSS under budget; budgets in CI | committed |
| 218 | Harness detection topic (ADR-0066) | committed |
| 217 | Kit frame and navigation (phone) | committed |
| 216 | `qa/platform.mjs` | committed |
| 215 | Platform look plumbing | committed |
| 214 | Phone web: scroll, first tap, Home Screen, task page < 900 px | committed |
| 213 | ADR-0065 phone apps decision + design | committed |
| 212 | Harness detection speed | committed |
| 198–211 | Owner's preview bugs, telemetry, Windows rounds, process leaks | committed |

Older rounds: `git log` and `docs/v2-round-*.md`.
