# wisp v2 UI performance audit, round 2 (2026-11-19)

Findings only: no repo file was edited except this report. Worktree `/Users/mokh/.wisp/worktrees/wisp-8fb64ec939b0` at
`c0d655ad`, plus the uncommitted working tree as it stood at build time (the kit menu fix, and fix-r330's in-progress
bundle split in `packages/shell/src/frame/{App,Panes}.tsx`, `packages/ui/src/controls/*`, `plugins/*/ui/src/index.ts`
and `tools/budgets.ts`). Round 1 is `docs/qa/perf-ui-r1.md`.

## How it was measured

- **Build:** `cd apps/web && npx vite build --outDir /tmp/perf2u/web --emptyOutDir --sourcemap`, then
  `node tools/budgets.ts --dist=/tmp/perf2u/web` (the working tree's `budgets.ts`, which now counts the chunks the
  first screen loads).
- **Server:** a real `wisp serve` (`target/debug/wisp`, a debug build) on :4900. `WISP_HOME=/tmp/perf2u/home`, a cleared
  environment, `HOME` and every CLI's home under `/tmp/perf2u/user`, and fake CLIs on `PATH` (wisp-testkit's
  `wisp-fake-agent` as `claude`, version-probe stubs for the rest). Script: `/tmp/perf2u/serve.sh`.
  - **Home:** 5,000 tasks, seeded through wisp.runtime's schema and `repo::save_session` (a copy of
    `spikes/server-budgets`' `seed()`, `/tmp/perf2u/seed`). Plus 12 finished tasks with real transcripts (fake claude,
    `FAKE_STREAM=40` for 5 s, agent mix).
  - **Streaming:** the server restarted with `FAKE_STREAM=8 FAKE_STREAM_MS=900000 FAKE_STREAM_MIX=agent` (8 agent lines
    a second for 15 min), with 3 to 4 running tasks.
- **Browsers:** playwright-core from `/tmp/pw`.
  - Chrome (system) with CDP `Emulation.setCPUThrottlingRate 4`.
  - WebKit unthrottled (Playwright can't throttle it).
  - Desktop is 1440×900. Phone is 390×844 with touch and r1's fake Capacitor bridge (Android look).
- **tap → frame:** the same instrument as r1. It runs from the input's `timeStamp` to the frame after the first DOM
  change the input caused, or to the next frame when nothing changed (typing in a textarea). Each action repeats 3–16
  times, and desktop and phone ran 3 full times each.
  - The table gives the median of the three runs' p95s.
  - The floor (a click that changes nothing) is 7–11 ms at 4× and about 20 ms in WebKit.
  - Event Timing durations are logged as a cross-check (`evP95`).
- **Long tasks, recalcs, layouts:** `PerformanceObserver('longtask')` and CDP `Performance.getMetrics` deltas per
  action, both Chrome only. Attribution comes from:
  - CPU profiles mapped through the sourcemaps (`/tmp/perf2u/prof.mjs`);
  - devtools.timeline traces (`/tmp/perf2u/trace.mjs`: style recalcs with element counts, layouts with dirty/total
    objects, a long-task breakdown and a timeline after the input);
  - a network-delay test (`/tmp/perf2u/slow.mjs`: the same clicks with `/api/events` and `/api/cmd` answered 400 ms
    late).
- **Scripts and raw output:** `/tmp/perf2u/run.mjs` (phases `load, actions, extra, pextra, scroll, stream`). Logs are
  `/tmp/perf2u/act-{desk,phone}-{1,2,3}.log`, `ext-desk-*.log`, `pext-1.log`, `wk-*.log` and `stream-*.log`; JSON is in
  `/tmp/perf2u/out-*.json`.
- **Caveat: the machine was loaded.** The load average was 9 to 120 during the runs (a game and other agents were
  running). Single runs varied, for example opening a task measured 316 ms p95 in one run and 119 ms in the next, so
  every number below is from repeated runs. Attributions come from profiles and traces, not wall times. The server
  is a debug build, which only affects how fast data arrives (finding 3 checks this).

## Numbers: budget, round 1, round 2

Chrome 4× unless marked. "r1" is `perf-ui-r1.md`; "—" means not measured that round.

| Action | Budget | r1 | r2 desktop (p95 / median) | r2 phone (p95 / median) | r2 WebKit 1× desktop / phone |
| --- | --- | --- | --- | --- | --- |
| Open a task (row click) | ≤ 50 | 399 (phone 124) | **165** / 90 | **144** / 58 | 36 / 42 |
| Click → transcript in the DOM | — | — | 165–192 | 100–151 | 53–62 |
| Switch task (click another row) | ≤ 50 | — | **136** / 81 | — | 34 |
| J / K (next / previous task) | ≤ 50 | ~263 | **137** / 120 | — | 37 |
| Side panel open (`]`) | ≤ 50 | 194 (WebKit 285) | **100** / 97 | — | 35 |
| Side panel tab (Changes ↔ Shell) | ≤ 50 | — | **119** | — | 22 |
| ⌘K open (phone: search) | ≤ 50 | 283 (phone 159, WebKit 195) | **826** / 667 | **820** / 656 | **210 / 237** |
| ⌘K typing, per key | ≤ 50 | 31–36 (first keys 95) | **45–66** per key once open (p95 236–306 while the open is still finishing) | 37–66 | 64 / 115 |
| Typing in the reply box, per key | ≤ 50 | — | 15 | 10 | 54 / 19 |
| Typing in the New task box, per key | ≤ 50 | — | 16 | — | 16 |
| Typing in the reply box while streaming (Event Timing p95) | ≤ 50 | — | **88** | 32–48 | — |
| Menu: task More | ≤ 50 | 130 | **85** / 42 | 48 (⋮) | 27 / 23 |
| Submenu (hover) | ≤ 50 | — | **254** (200 ms of it is a hover delay) | — | 216 |
| Menu: list-row More | ≤ 50 | 198 | **68** / 51 | 24 (list ⋮) | 29 |
| Composer mode flyout | ≤ 50 | 106 | 47 | — | 25 |
| Composer harness picker | ≤ 50 | 130 | **118** / 95 | 54 (sheet) | 40 / 20 |
| New task (from a task) | ≤ 50 | 357 | **149** / 139 | **193** / 74 | 55 / 49 |
| New task harness picker | ≤ 50 | 150 | **253** / 201 | — | 58 |
| New task mode flyout | ≤ 50 | 111 | 31 | — | 23 |
| New task project picker | ≤ 50 | 256 | **92** / 70 | — | 38 |
| My Stuff | ≤ 50 | — | **61** / 58 | 40 (bottom nav) | 51 |
| Crew: org chart | ≤ 50 | — | **67** / 55 | — | 22 |
| Crew: add member | ≤ 50 | — | **111** / 108 | — | 41 |
| Settings pane, first visit | ≤ 50 | 74–98 (WebKit This computer 139) | 37 / 30 | 39 / 25 | 40 |
| Settings pane, second visit | ≤ 50 | — | **120** / 46 (All settings 160, Usage 120) | — | 33 |
| Phone: Back to the list | ≤ 50 | — | — | 50 / 44 | 23 |
| Phone: turn More, me menu, list menu | ≤ 50 | — | — | 46, 27, 24 | 28, 25, 23 |

| Budget | r1 | r2 |
| --- | --- | --- |
| Initial JS ≤ 120 KB br (what the first screen loads) | 308.7 KB in 94 files (phone 302.9 KB in 79) | **175.2 KB in 30 files** (phone **145.2 KB in 28**); `budgets.ts`: 172.2 / 166.1 |
| Initial CSS ≤ 30 KB br | 36.0 KB (phone 43.5) | 28.6 KB, passes (phone **38.2 KB**: index 28.6 + `android-styles` 9.6) |
| Time to first row, 4× (cold / warm) | desktop 1,052 / 778; phone 609 / 512 | desktop 1,209 / 691–846; phone 1,056 / **302–334** |
| Long tasks before TTI, 4× (cold / warm) | 9, longest 422 / 9, 372 | 3, longest 175 / 2, 83 (phone 3, 99 / 1, 59) |
| Phone cached open ≤ 400 ms (plan §8) | 512 | **302–334, passes** |
| Streaming: 0 long tasks (desktop, 10 min) | 0 (6 s sample) | **0 in 10 min**, 0 of 36,021 frames dropped, worst gap 17 ms |
| Streaming: 0 long tasks (phone) | 1 (85 ms) | 3 (≤ 91 ms) in one of two 2-min runs; 0 in the second run and in a 75 s trace |
| Streaming: scripting ≤ 100 ms / 3 s | 34.5 | **11** |
| Streaming: style recalc | 907 ms / 6 s (453 / 3 s) | **176 ms / 3 s**, 68 recalcs a second (finding 4) |
| Memory over 10 min of streaming | — | JS heap 29.1 → 32.9 MB; CDP nodes 3,239–3,284 (flat); listeners ~100 (flat); DOM 1,181 (flat) |
| Scroll, desktop 4× (frames dropped, worst gap, long tasks) | 62 of 432, 67 ms, 1 (54 ms) | **4 of 500**, 50 ms, 1 (53 ms) |
| Scroll, phone 4× | 81 of 378, 100 ms, 2 (80 ms) | **5 of 494**, 83 ms, 1 (70 ms) |
| Scroll, WebKit (desktop / phone) | 1 dropped | 0 / 0 dropped |
| Rows in the DOM while scrolling | 42–44 | 26–27 |

### What round 1 fixed

Each of these was checked this round:

- **r1 #5 (attribute-only selectors):** 1,967 of the main sheet's 2,179 selectors now use `.k-` classes, and 10 are
  `data-kit` only. Style time on open-task fell from 309 ms to about 24 ms (trace).
- **r1 #6 (focus during render):** the task body is built a frame after the head (`task-paint.ts` `afterNextPaint`).
- **r1 #13 (time-width writes):** `time-width.ts` now skips unchanged widths.
- **r1 #14 (VirtualList full-list work):** keys and indexes are memoised per change of items.
- **r1 #1 (first screen):** 94 → 30 files; the bundle split is in progress (fix-r330).
- **Streaming scripting:** 34.5 → 11 ms per 3 s.

Everything in the table above still over 50 ms is below.

## Findings

### 1. ⌘K and phone search take 820 ms to open: `sessionKey()` makes the task list O(n²)

- **Measured:**
  - ⌘K open p95 826 ms at 4× (three runs: 826, 833, 820), as one long task of 778–807 ms.
  - Phone search: 820 ms.
  - Chrome 1×: 151–178 ms on a repeat open. WebKit 1×: 210 ms (phone 237 ms).
  - r1 measured 283 ms at 4×, so this is a regression of about 3×.
- **Trace (`node /tmp/perf2u/trace.mjs k`):** one 787 ms task, of which 786 ms is the keydown's `FunctionCall`. CPU
  profile, self time (`SELF=1 TOTAL=30 node /tmp/perf2u/prof.mjs k`):
  - **550 ms** in `packages/shell/src/builtins.tsx:411` `items: () => taskItems(ctx)`, which inlines
    `packages/shell/src/palette/tasks.ts:42` `taskItems`.
  - Everything else is under 4 ms.
- **Cause:**
  - `taskItems` calls `sessionKey(keys, s.id)` for every unarchived task (`tasks.ts:54`, and again in `run` at `:65`).
  - `packages/shell/src/frame/rows-view.ts:284` `sessionKey` is `keys.find((k) => k.slice(k.indexOf(':') + 1) === id)`:
    a linear scan that allocates a substring per key.
  - With about 4,500 unarchived rows that is about 10 million `slice` calls per open.
  - The bare-id → key lookup was added on 2026-11-17 ("a bare id opened a blank page"), after r1.
- **Also reached by:** phone search (the same palette), and `sessionKey(untrack(keys), …)` on each list row's model
  word in `Sidebar.tsx:335` (O(n) per rendered row, not per task).
- **Fix:**
  - Build a `Map<bareId, key>` once per `keys()` change (a memo next to `entities.keys`) and look up in O(1).
  - Better: give `SessionRow` its entity key so no lookup is needed.
  - Separately, the palette builds all 4,500 items inside the keydown task. Draw the dialog first and fill the list in
    the next frame (design/feedback.md: the navigation answers at once), or build only the first `GROUP_LIMIT` rows
    until the user types.

### 2. Typing in ⌘K costs about 30 ms of script per key at 4×; `rank()` re-lowercases every field of 4,500 items each key

- **Measured:**
  - Per key, once the palette has opened: 45–66 ms wall at 4× (median 33–54), 37–66 ms on the phone.
  - WebKit 1×: p95 64 ms (phone 115 ms).
  - In the runs the first keys overlap the 800 ms open, so p95 reads 236–306 ms.
- **CPU profile (`node /tmp/perf2u/prof.mjs ktype`, 15 keys):** 498 ms in `Palette.tsx:151` `onInput` →
  `match.ts:149` `rank`. Of that:
  - `match.ts:80` `fuzzy`: 311 ms.
  - `match.ts:42` `matchOne`: 230 ms, 208 ms of it self time (`text.toLowerCase()` and the scans).
  - `tagScore`: 46 ms.
  - `score`: 25 ms.
- **Cause:** `rank` runs `fuzzy` over 5 fields (label, sub, tags, keywords, group) for each of about 4,500 items on
  every keystroke. `matchOne` lowercases the haystack on every call, and `tagScore` re-splits the query for each item.
- **Fix:**
  - Lowercase and join each item's fields once, when `items` is built (a parallel array or a `WeakMap`).
  - Parse the query once per keystroke, not once per item.
  - When the new query extends the previous one, rank only the previous hits.
  - Stop scoring a group once it has `PER_GROUP` sure hits.

### 3. The harness picker sends 10 requests on every open and applies their answers before it settles

- **Measured:**
  - New task harness picker: p95 253 ms at 4×, median 201 ms, 8–10 long tasks of up to 129 ms. r1 was 150 ms, so this
    regressed.
  - Composer harness picker: p95 118 ms.
  - WebKit 1×: 58 ms and 40 ms.
- **Not the debug server:** with `/api/*` delayed by 400 ms (`/tmp/perf2u/slow.mjs`, Chrome 1×) the picker still shows
  in 16–58 ms, so the cost is client CPU.
- **Requests per open (`/tmp/perf2u/probe6.mjs`):** 10 POSTs to `/api/events`:
  - `runtime.agents`;
  - `accounts.list {agent: claude}`;
  - a `sub` to `plugin:wisp.runtime:harnesses`;
  - **seven `budget.view`, one per harness**.

  This happens on every open, even with `onWarm: warmPicker` set (`HomeView.tsx:958`).
- **CPU profile (`prof.mjs nharness`):** about 45 ms of `peers.ts:211/222` `onFrame` → `batch` applying the answers.
  Plus about 46 ms in `HomeView.tsx:957` `onClick` → the picker's mount:
  - `HarnessPicker.tsx:125` `place()` `getBoundingClientRect` ×2: 8.6 ms forced layout.
  - `:135` `focus()`: 5.4 ms.
  - List rows re-render (`RowParts.tsx:187` `pills`, `Card.tsx:64`).
- **Fix:**
  - One `budget.view` for all harnesses, or a budget topic the picker subscribes to once and keeps.
  - Let `warmPicker` (hover) send the requests and the open reuse them.
  - Make sure opening the picker doesn't re-render list rows (the `pills` work in the profile).
  - Measure `place()` in a rAF after mount instead of in `onMount` before paint.

### 4. Streaming: the "working" dots force a style recalc and repaint every frame

- **Measured over 10 min of streaming (desktop 4×):**
  - Main budgets pass: 0 long tasks, 0 dropped frames, script 11 ms per 3 s.
  - But **68 style recalcs a second (one per frame) and 176 ms of style per 3 s**, plus 9 layouts a second.
  - Phone and WebKit show the same 68–69 recalcs a second.
  - A 75 s phone trace has 4,513 recalcs, each touching about 10 elements and taking 1–2 ms at 4×.
- **Evidence:** `document.getAnimations()` in every 10 s window lists `kit-orb@orb-dot` ×7–8 and
  `kit-rise@task.working`.
- **Cause:** `packages/ui/src/css/transcript.css:101,107`. The rule
  `[data-kit='orb'][data-live] [data-kit='orb-dot'] { animation: kit-orb 1.4s … infinite }` animates `background`
  (`color-mix(…)` ↔ `var(--accent)`). That can't run on the compositor, so for as long as any task is live, Blink
  recalcs style and repaints those dots every frame.
- **Fix:** animate `opacity` (or `transform: scale`) of an accent-coloured `::after` layer over each dot, which is
  composited and needs no recalc. Or animate one element with `steps()`. Check it in a trace: recalcs a second while
  streaming should fall to about the stream's patch rate (about 9 a second here).

### 5. Opening, switching and stepping through tasks are still 2–3× over 50 ms at 4×

- **Measured (4×):**
  - Open a task: p95 165 ms (the first open of a run is 150–168; then 68–100), median 90. Phone p95 144, median 58.
  - Switch with a row click: p95 136, median 81.
  - J/K: p95 137, median 120, every one of 48 steps between 97 and 145.
  - Click → transcript in the DOM: 165–192 ms.
  - WebKit 1× passes: 25–37 ms.
  - Better than r1 (399 and about 263 ms), but still over budget.
- **Traces (`TL=600 node /tmp/perf2u/trace.mjs next` / `open`):**
  - **J:** a 37 ms keydown task:
    - `builtins.tsx:194` `step` → `ctx.selection.set(next)` (the list repaints the selection);
    - `querySelector(...).scrollIntoView()` (`:202`, a forced layout);
    - `navigate(openTask)` → `TaskView.tsx:265`.
    - Then 14 ms of style and layout, a 13 ms microtask, and a 26 ms rAF that builds the body (`afterNextPaint` →
      `TaskView.tsx:239`). The first DOM change is the head, about 115 ms after the key at 4×.
  - **Click:** a 55 ms task, then 20 ms of microtasks, then the 45 ms body-build rAF (31 ms in TaskView's chunk), then
    a 43 ms microtask. The CPU profile shows `peers.ts:222` `onFrame` taking 79 ms in all, applying the
    `session:<id>` topic's snapshot and the `TELEMETRY_TOPIC` frames the open subscribes to (`TaskView.tsx:256`).
- **Fix:**
  - J/K: set the selection and navigate first; defer `scrollIntoView` to the next frame (or use the VirtualList's own
    scroll-to-index, which needs no layout read).
  - Apply a topic's first snapshot in the frame after the head paints (it is already requested there), and coalesce
    `onFrame` batches that land in the same frame.
  - Keep a small kept-view cache warm for the previous and next tasks so J/K reuses a built body.

### 6. Opening ⌘K and the crew editor restyles the whole document (`inert` / `aria-hidden` on everything outside)

- **Measured:**
  - Crew "add member": p95 111 ms at 4×, with 84–90 ms of style and about 30 layouts per open. WebKit: 41 ms.
  - Trace (`trace.mjs crewadd`): **one style recalc of 1,526 elements (20.5 ms)** and another of 366 elements
    (11.4 ms), then 27 layouts (2–8 ms each, 1,688 objects in all) over the next 300 ms.
  - ⌘K's open shows the same full recalc: **1,792 elements, 17 ms**.
- **Cause:** opening a modal walks every node outside the dialog to mark it hidden or inert (`@zag-js/aria-hidden`
  `walk-tree-outside.mjs:135` in the ⌘K profile; the dialog machine, `dialog-machine.ts:17`). Each attribute write
  invalidates that subtree's style, so the next frame restyles nearly the whole document. The crew editor's later
  layouts are its own content settling (`CrewEditor` chunk, `kit-rise` on `ce-maker`).
- **Fix:**
  - Set `inert` once on the app root (the dialog portal sits outside it) instead of `aria-hidden` on each sibling
    branch.
  - Confirm the 27 follow-up layouts in the crew editor come from a measurement loop (a textarea fit or a
    ResizeObserver) and batch them.

### 7. New task from an open task: p95 149 ms at 4× (phone 193 ms)

- **Measured:** p95 149 ms, median 139 (r1 357). Phone p95 193, median 74. WebKit 55 / 49 ms.
  - When Home is already showing, the same press costs 11–16 ms, because the launcher is already mounted.
  - From a task view it rebuilds Home: about 47 recalcs, 5–12 layouts and 105–127 ms of script per open (CDP deltas).
- **Suspected cause:** Home's launcher and its panes mount in the click task, after the task view unmounts.
  `HomeView.tsx` is 10.8 KB br of the first screen and the largest non-entry chunk after `TaskList`.
- **Fix:** keep the launcher mounted (hidden) beside the task view, or render its head in the click frame and the
  panes (`HomePanes`, usage, cards) in the frame after, as `task-paint.ts` does for the task body.

### 8. Side panel and its tabs: about 100 ms at 4×

- **Measured:** opening the panel p95 100 ms (r1 194), switching tabs 96–123 ms. WebKit 22–35 ms.
- **Trace (`trace.mjs side`):** a 21 ms keydown task, then layouts of 10.4 ms and 6.7 ms over about 1,405 objects,
  plus a 4.6 ms recalc of 138 elements. The Shell tab's terminal measuring itself in the same frame (r1 #8) is the
  likely source of the second layout.
- **Fix:** mount the inactive tab's body lazily, and fit the terminal in a ResizeObserver callback rather than reading
  `clientWidth`/`getComputedStyle` right after the panel's writes.

### 9. A Settings pane opened a second time builds in the click: All settings 160 ms, Usage 120 ms

- **Measured (4×):**
  - First visits: p95 37 ms. The first change is the nav item's highlight; the pane's chunk loads after it.
  - Second pass, all chunks cached, the pane built in the click task: p95 117–121 ms, with long tasks of 112–135 ms.
    All settings 160 ms, Usage 120–127 ms, General 87–105 ms, Agents 92 ms, Plugins 96 ms.
  - WebKit: 33 ms.
- **Suspected cause:** All settings renders every setting row at once, and Usage builds its charts synchronously.
- **Fix:** highlight the nav item in the click frame and render the pane in the next (the same pattern as
  `afterNextPaint`). Virtualise or chunk All settings' rows.

### 10. Submenus open after a fixed 200 ms hover delay

- **Measured:** hovering a task More submenu shows it after 245–258 ms at 4×. WebKit: 214–216 ms.
- **Cause:** `@zag-js/menu` `menu.machine.mjs:553` `waitForOpenDelay` is a hard-coded `setTimeout(…, 200)`. The
  hovered item itself highlights through CSS at once.
- **Impact:** this is a deliberate hover-intent delay; clicking or pressing → opens the submenu at once. It reads as
  slow next to v1. Check v1's delay before changing it.
- **Fix, if wanted:** open at once when the pointer moves from an item that already had a submenu open, and keep the
  delay only for the first one. Zag's delay isn't configurable, so this needs a patch or a wrapper.

### 11. The textarea autosize reads `scrollHeight` twice per keystroke: 5 recalcs and 5 layouts per key

- **Measured:** typing 38 characters in the New task box made 187 recalcs and 185 layouts (CDP deltas). That's about
  5 of each per key; the reply box made 47 and 32 for 31 keys.
  - Typing is still within budget: p95 10–18 ms at 4×.
  - While a task streams, a key in the reply box shares its frame with the stream's patches and the orb recalcs, and
    Event Timing p95 rises to 88 ms on desktop (finding 4).
- **Cause:** `packages/ui/src/core/autosize.ts:11-14` and `:23-25`. Each fit sets `height` and reads `scrollHeight`,
  then sets `height` and `overflowY` and reads `scrollHeight` again (`fitTextarea` also calls `getComputedStyle`).
- **Fix:** read `scrollHeight` once, set both styles after the read, skip the fit when the value's line count can't
  have changed, and cache the computed line height per element.

### 12. The first screen is still over its JS budget (phone CSS too)

- **Measured:**
  - **JS:** 175.2 KB br in 30 files before the first row (desktop) and 145.2 KB in 28 (phone), against 120 KB. In r1 it
    was 308.7 KB in 94 files.
  - **CSS:** desktop 28.6 KB, which passes. Phone 38.2 KB: `index` 28.6 KB plus `android-styles` 9.6 KB.
  - **Largest chunks before the first row (br KB):** `index` 99.0, `TaskList` 16.2, `HomeView` 10.8, `pins` 5.8,
    `HomePanes` 5.2, `composer-plus` 5.0, `Home` 4.9, `lazy` 4.2, `launcher-tool` 3.9.
  - `node tools/budgets.ts --dist=/tmp/perf2u/web` agrees: initial JS desktop 172.2 KB, phone 166.1 KB; CSS phone
    39.5 KB; exit 1.
  - **First row at 4×:** desktop cold 1,209 ms, warm 691–846 ms. Phone cold 1,056 ms, warm 302–334 ms (the 400 ms
    phone target passes).
- **Status:** fix-r330 is splitting this bundle now; these are the numbers it starts from.
- **Phone CSS:** `android-styles` loads before the first row (9.6 KB). Splitting the platform sheet by screen (list
  versus task and sheets) would bring the phone under 30 KB.

### 13. Scrolling 5,000 tasks: smooth, but row mounts still cost about 400 ms of script per 150 wheel steps at 4×

- **Measured (150 wheel steps of 400 px, about 1,000 rows):**
  - Desktop 4×: 4 of 500 frames dropped (r1 62 of 432), worst 50 ms, 1 long task (53 ms). 592 layouts, 900 recalcs,
    style 1,019 ms, layout 604 ms, script 1,397 ms.
  - Phone 4×: 5 of 494 dropped (r1 81 of 378), worst 83 ms, 1 long task (70 ms).
  - WebKit: 0 dropped on desktop and phone.
- **CPU profile (`prof.mjs scroll`):**
  - 565 ms inclusive in `VirtualList.tsx:320` / `:205`, mostly `@solid-primitives/keyed` mounting rows:
    `TaskList.tsx:162` `children` 375 ms, `:189` 219 ms, `:204` 193 ms.
  - `VirtualList.tsx:225` `defer` has 30 ms of self time.
  - The r1 offsets problem is gone.
- **Fix:**
  - Recycle row components across scroll: rebind data instead of unmounting and mounting per key.
  - Keep a row's time format and pills as memos that don't run until visible.
  - Raise `overscan` on the phone so mounts spread over more frames.
- **Also:** WebKit on the phone still logs "ResizeObserver loop completed with undelivered notifications" while
  scrolling the list (`VirtualList.tsx:98-124`'s note says QA r1 #16 / r2 #15 fixed this; it reproduces here).

### 14. Heap grows about 0.38 MB a minute while a task streams

- **Measured (desktop 4×, task open, GC before each sample):**
  - JS heap: 29.1, 29.5, 30.2, 30.6, 29.9, 30.8, 31.2, 31.2, 32.1, 32.5, 32.9 MB at minutes 0 to 10.
  - CDP `Nodes`: 3,239–3,284, flat. Listeners: about 100, flat. DOM: 1,181, flat.
  - The live view is bounded (a working card with the last 5 actions), so the DOM isn't what grows.
- **Suspected cause:** the open task's events kept in the client: 8 lines a second × 400 B is about 0.2 MB a minute
  raw, so the growth matches keeping them. An hour's run would hold about 23 MB, and AGENTS.md says only open or
  running sessions stay in memory, which this is.
- **Fix:** confirm with two heap snapshots (`HeapProfiler.takeHeapSnapshot` at minutes 1 and 10, diff by
  constructor). If it is raw events, keep the activity as pages fetched on demand (`runtime.activity` has `sinceSeq`)
  and hold only the rendered window.

### 15. Phone streaming: 3 long tasks in one 2-minute run, not reproduced

- **Measured:**
  - Phone 4×, first 2-min run: 3 long tasks (91, 72 and one more, all ≤ 91 ms) between 101 s and 111 s.
  - Second run: 0. A 75 s trace of the same state: 0.
  - Desktop: 0 in 10 min.
- **Status:** unattributed. The machine's load average was 20–30 at the time, so this is likely external. It's the
  only long-task sighting while streaming; re-check it on a quiet machine (`TAG=x MINUTES=5 PHONE=1 TASK=<id>
  node /tmp/perf2u/run.mjs chrome stream`).

## Not measured

- **r1 #16 (open 50 tasks, detached nodes):** the memory phase hung on the real server and was stopped; not re-checked.
- **J/K on the phone, and phone menus beyond those in the table.**
- **Selector-level costs (r1's `/tmp/perf-sel.mjs`):** not rerun, since style time is no longer dominant except in
  findings 4 and 6.
- **Long tasks in WebKit:** it has no long-task API.

**15 findings.**
