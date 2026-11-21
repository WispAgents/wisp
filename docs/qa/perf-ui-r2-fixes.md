# UI performance round 2: fixes for findings 1, 2, 3, 4, 6 and 11 (2026-11-19)

The findings are in `docs/qa/perf-ui-r2.md`. coder-r155 fixes the others (5, 7, 8, 9, 10, 13, 14) at the same time.

## How it was measured

- **Same instruments as the report:** `/tmp/perf2u/run.mjs`, `trace.mjs` and `prof.mjs`. For the new build they are
  copied as `run2.mjs`, `trace2.mjs` and `prof2.mjs`, whose only change is the sourcemap directory, `/tmp/perf2u/web2`.
- **Server:** the report's scratch server (`/tmp/perf2u/serve.sh`, 5,000 tasks, :4900, a debug build). Its copy
  `/tmp/perf2u-fix/serve.sh` takes `WEB=` for the web directory.
- **Builds:**
  - **Before:** the report's own build, `/tmp/perf2u/web`.
  - **After:** `npx vite build --outDir /tmp/perf2u/web2`, built from this worktree.
- **Both measured in the same session on the same machine:** before first, then after, each with 3 tasks streaming
  (fake claude, `FAKE_STREAM=8`). The load average was 5–12 throughout. Desktop is Chrome at 1440×900, 4× CPU; phone
  is 390×844, 4×.
- **Caveat:** the after build also carries coder-r155's work in progress, since the worktree is shared. So numbers for
  open, J/K, the side panel and New task here are theirs, not this change's.
- **Extra scripts in `/tmp/perf2u-fix`:**
  - `probe-picker.mjs`: requests per harness picker open.
  - `recalc-src.mjs`: style recalcs per 3 s while streaming, with given animations paused.
  - `modal-exp.mjs`: which of a modal's writes restyles the page.
  - `orb-shots.mjs` and `diff.mjs`: orb screenshots and their pixel diff.
  - `devui.mjs`: a check in the real dev UI on :5180.
- **Logs:** `act-{before,after}-*.log`, `stream-*.log`, `trace-*.log`, `prof-*.log` and `probe-*.log`, all in
  `/tmp/perf2u-fix`.

## Before → after

| # | What | Before | After |
| --- | --- | --- | --- |
| 1 | ⌘K open, p95 / median (4×) | **940 / 735 ms**, 5 long tasks (up to 893 ms) | **61 / 59 ms**, 0 long tasks (first after-run: 69 / 67) |
| 1 | ⌘K open, trace | one 592 ms keydown task, 585 ms of it in `taskItems` | no long task |
| 1 | Phone search open, p95 / median | **839 / 662 ms** | **45 / 37 ms** |
| 2 | ⌘K typing "flaky sync test", per key, median / p95 | 45 / 331 ms, 8 long tasks, script 347 ms | **22 / 226 ms**, 4 long tasks, script 238 ms (first after-run: 19 / 206) |
| 2 | `rank` in the CPU profile, 15 keys | 569 ms (inclusive) | ~420 ms |
| 2 | Phone search typing, per key, median / p95 | 43 / 657 ms | **30 / 98 ms** |
| 3 | Requests: first open / second open / hover then open | 10 / 10 / 0 then 10 | 10 / **0** / 9 on the hover then **1** (the topic subscribe) |
| 3 | New task harness picker, each of 5 opens | 237, 169, 143, 143, 155 ms | 180, **24, 25, 25, 24** ms (the first is a cold chunk load; first after-run: 252, 32, 30, 33, 31) |
| 4 | Recalcs per 3 s while streaming, orb only (shimmer paused) | **235** (41 ms of style) | **10** (1 ms), the same as with every animation paused |
| 4 | Whole stream window: style per 10 s / recalcs a second | ~450 ms / 68 | ~265 ms / 68 (see "Left" below) |
| 4 | Orb look: max channel delta, 16 shots (light and dark, 8 points of the ripple, 4× zoom) | — | 0 at the ripple's ends; ≤ 14/255 mid-fade |
| 6 | ⌘K open, largest style recalc | **1,761 elements** (16.9 ms) | **209** (2.2 ms) |
| 6 | Crew add member, largest style recalc / p95 | 1,540 elements (20.3 ms) / 96 ms | gone (largest 369, the editor's own) / 67–74 ms |
| 11 | New task prompt, 38 keys: recalcs / layouts; p95 per key | 187 / 185; 22 ms | 152 / 114; 11–17 ms |
| 11 | Reply box, 31 keys: recalcs / layouts | 48 / 32 | 47–48 / 32 (already about 1 layout a key) |

## What changed

### 1. Bare id → key lookup, and the palette's first frame

- **The lookup:** `keyOfId` in `packages/client/src/ids.ts` is one `Map<bareId, key>` per keys list, kept in a
  `WeakMap`. `EntityReader.keys` gives a new array on every change, so every caller holding the same list shares one
  index.
- **Who uses it:**
  - `sessionKey` in `rows-view.ts`, which serves ⌘K's `taskItems`, `Sidebar.tsx:335`, Home, Compare and the fallback
    list;
  - the client's nav `taskKey` (`client.ts`).
- **The palette:** `Palette.tsx` draws the dialog in the key's frame and fills its list the frame after
  (`afterNextPaint`). A key or a typed character before then fills it at once.
- **Tests:**
  - `packages/client/test/key-of-id.test.ts`: 5,000 lookups read the list once, the first key in order wins, and each
    list gets its own index.
  - `packages/shell/test/palette-perf.test.ts` `building_5000_tasks_palette_items_reads_the_keys_list_once_not_once_per_task`:
    at most 5,000 key reads, against about 12.5 million for the old scan.

### 2. `createRanker`

- **What it does (`packages/shell/src/palette/match.ts`):**
  - lowercases each item's five fields once and keeps them by item id;
  - parses the query once per key;
  - when the query extends the last one, ranks only the last hits, plus any item that is new or whose fields
    changed. This holds across a rebuilt list too: with a task streaming, the store rebuilds the palette's items on
    every patch.
- **What stays the same:** `rank`, `fuzzy`, `matchOne` and `tagScore` keep their signatures and results.
- **Tests:** `typing_that_extends_the_query_ranks_only_the_last_keys_hits_and_answers_as_a_fresh_rank` and
  `a_list_rebuilt_while_typing_keeps_the_narrowing_and_ranks_what_changed_again`. Both check that the answers equal a
  fresh `rank`, and that the number of items scored falls as the query narrows. A third,
  `a_topic_item_left_out_while_its_topic_is_typed_comes_back_as_the_topic_grows`, covers a topic row left out while
  its own tag is typed (`#tag`): it stays a candidate, so `#tags` still finds it.

### 3. Harness picker data, kept per peer

- **`packages/shell/src/frame/picker-data.ts`:**
  - keeps `runtime.agents`, each harness's `budget.view` and `accounts.list` per client and peer for 30 s;
  - shares an answer still in flight;
  - applies the usage answers in one update, not seven;
  - keeps the harnesses topic watched for 30 s after the picker closes.
- **Hover:** it warms all of this from the pill (`HomeView.tsx` `onWarm`).
- **Test:** `packages/shell/test/picker-data.test.ts` checks that a hover warms it, that an open within 30 s asks
  nothing, that answers in flight are shared, that the topic is kept, then dropped, and that a failed answer isn't kept
  (the next open asks again).
- **Not done:**
  - **A single request for all harnesses.** No existing command answers for all of them: `budget.planWindows` leaves
    out windows with no traced task, so it isn't `budget.view`'s answer. One would need a new `wisp.budget` command,
    which is a Rust and API change.
  - **Moving `place()` to a rAF.** It would show the card at the wrong place for a frame.

### 4. The orb's ripple on the compositor

- **The change (`packages/ui/src/css/transcript.css`):** each live dot keeps its grey, and an accent `::after` layer
  fades in and out (`opacity`) in place of animating `background`.
- **The look:** crossfading over the grey is the same premultiplied mix the colour interpolation made. The only
  difference: Chrome interpolates `background` in Oklab, and opacity blends in sRGB. The 16 shots match exactly at the
  ripple's ends (0 delta) and differ by up to 14/255 on one channel mid-fade (`/tmp/perf2u-fix/{before,after}-*.png`).
- **The waiting state:** the amber pulse is unchanged, and its dot carries no layer.

### 6. A modal dialog without the restyle

- **The cause, measured on the dev UI (`modal-exp.mjs`):** the whole-document recalc came from Zag's modal dismissable
  layer writing `pointer-events: none` on `<body>`. That property is inherited.

  | Change made ahead of time | ⌘K open's recalcs (elements) |
  | --- | --- |
  | none | 106, **658**, 292 |
  | `pointer-events: none` on `<body>` | 106, 292 |
  | `aria-hidden` on the app root | 106, **658**, 292 |

- **`inert` wouldn't help:** setting it on the app root costs the same subtree recalc (346–361 elements of about 590).
  So the brief's "inert once on the app root" would not remove the restyle.
- **The fix (`dialog-machine.ts`):**
  - Zag's dialog runs with `modal: false`, plus `trapFocus: true` and `aria-modal="true"` on the content.
  - The positioner keeps the pointer, so a press outside still closes the dialog.
  - `hideOutside` marks the page outside `aria-hidden` and `data-aria-hidden`. These are the same nodes Zag marked:
    each sibling on the way from `<body>` down to the content, which includes the backdrop. It keeps nested-dialog
    counters, leaves live regions alone, and leaves what was hidden before.
  - The backdrop covers the page, so nothing needs a pointer block.
- **Check:** the overlay golden (`platform-browser-overlays.json`) is byte-identical.
- **Tests:** `packages/ui/test/dialog-page-quiet.test.tsx` checks that `<body>` gets no style, the page is
  `aria-hidden` while the dialog is open and read again after, focus stays trapped, and nested dialogs and pre-hidden
  nodes are handled. The existing dialog tests pass: focus return, first focus, scroll lock, and the overlay goldens.
- **Not checked:** the crew editor's follow-up layouts (34 of them) come from its own content, not this change; I
  didn't look further.

### 11. Autosize

- **`fitTextarea`:** it reads `getComputedStyle` once per box (`WeakMap`) and `scrollHeight` once per fit. When text
  was only added at the end and the box's width and rows are the same, it measures the box where it stands, with no
  shrink-to-min first, and writes nothing when the text still fits.
- **`growTextarea`:** one `scrollHeight` read, not two.
- **Test:** `packages/ui/test/fit-textarea.test.ts` checks for 28 keys, 28 layout reads and 1 `getComputedStyle` call,
  writes only when a line is added, and that the box still shrinks and still scrolls past its cap.

## Left

- **Streaming still restyles every frame because of `kit-shine`, not the orb.** The working title's shimmer
  (`transcript.css`, `[data-kit='working-title']`, animating `background-position`) costs about 50 recalcs a second on
  its own. The report missed it because `run.mjs` cuts its animation list at 8 entries (7 orb dots and `kit-rise` fill
  them). `background-clip: text` with a moving gradient has no compositor-only equivalent that keeps the same look. A
  fix needs a design choice: for example, a `transform`-animated highlight layer over the text, or no shimmer at 4×.
- **⌘K typing's p95 (~210 ms) comes from the past-work answers.** `runtime.search` starts after 3 letters, and keys
  3–4 and 8 are the slow ones. `rank` is about 28 ms a key at 4×. Most of that is the subsequence scan over each
  task's long `keywords` (its preview and live line), since most tasks match the first letters.

---

# Findings 5, 7, 8, 9, 10, 13, 14 and 15 (coder-r155, 2026-11-19)

## How it was measured

- **Builds:** before is `/tmp/perf2u/web3-before`, built from this worktree before any of these edits (it carries
  coder-r154's work in progress as it stood then). After is `/tmp/perf2u/web3`, built at the end. Both:
  `npx vite build --outDir … --emptyOutDir --sourcemap`.
- **Server:** a copy of the report's scratch server on **:4937**, `/tmp/perf2u/f5/serve.sh` (`WEB=` picks the build),
  with its own copy of the 5,000-task home (`/tmp/perf2u/f5/home`), so it never shared a store with coder-r154's :4900.
- **Scripts:** copies of `run.mjs`, `trace.mjs` and `prof.mjs` in `/tmp/perf2u/f5`, pointed at :4937 and `DIST=`.
  `run.mjs` gained phases:
  - `mine`: open, switch, J/K, side panel and tabs, submenu, New task, every Settings pane a second time;
  - `side`: `]` ten times;
  - `transcript`: click → transcript in the DOM, 8 opens.
- **Submenu timing is the flyout's own:** hover → the flyout's `data-state="open"`, measured in the page. The report's
  instrument stops at the row's highlight, a CSS change at once.
- **Settings panes:** each pane clicked twice after a first pass, the slower of the two kept. `act()` presses Escape
  before each rep when a dialog is open, which closed Settings in the report's run; here it doesn't.
- **Setup:** Chrome 4× CPU, 1440×900; phone 390×844, 4×. Each before/after row is two runs. The load average was 20
  during the before runs and 8–10 during the after runs.
- **Heap:** `/tmp/perf2u/f5/heap.mjs` (snapshots diffed by constructor), `heap-who.mjs` (objects by their keys),
  `retain.mjs` (a retainer chain), and `dbgstore.mjs` (the store's own counts, from a debug build that exposed the
  client; the build was thrown away).
- **Logs:** `before-*.log`, `after-*.log` and `out-*.json`, all in `/tmp/perf2u/f5`.

## Before → after

p95 / median in ms at 4×, two runs each.

| # | What | Before | After |
| --- | --- | --- | --- |
| 5 | J/K (16 steps) | 150 / 122, 150 / 120 | **159 / 68, 115 / 73** |
| 5 | Open a task (row click; p95 is the run's first open) | 161 / 83, 179 / 80 | 180 / 92, 148 / 78 (no change) |
| 5 | Switch with a row click | 92 / 74, 84 / 73 | 116 / 79, 88 / 82 (no change) |
| 5 | Click → transcript in the DOM, median / p95 (8 opens) | 868 / 1,206, 813 / 944 | 810 / 912, 814 / 907 |
| 5 | Open: CPU profile | `sse.ts` `push` 57 ms self + 22 ms; `persist-idb` `write` in the click | both gone |
| 7 | New task from an open task | 118 / 108, 127 / 119 | **91 / 88, 92 / 87** |
| 8 | Side panel `]`, 10 alone | 70 / 63, 70 / 63 | 80 / 64, 68 / 62 (no change) |
| 8 | Side panel tab (Changes ↔ Shell) | 108, 104 (one switch measured) | **54 / 49, 59 / 47** (6 switches) |
| 9 | Settings second visit, max of two passes: All settings | 163, 153 | **47, 47** |
| 9 | Usage | 117, 118 | **27, 24** |
| 9 | General / Agents | 86 / 83, 91 / 75 | 81 / 107, **24 / 25** |
| 9 | Settings `Usage` second click, trace | — | click task 22 ms, painted at ~30 ms; body built in the next frame (75 ms task) |
| 10 | Submenu: hover → flyout open, p95 | 255, 231 | **24, 29** |
| 10 | Submenu: hover a sibling with one open, p95 | 445, 440 | **48, 55** |
| 13 | Desktop scroll: script / dropped of ~490 frames / worst gap | 1,162 ms / 1 / 50, 1,271 / 1 / 50 | 1,140 / 1 / 50, 1,153 / 2 / 50 (no change) |
| 13 | Phone scroll: script / dropped / worst gap | 1,104 ms / 7 / 83, 1,102 / 9 / 83 | not changed (see 13) |
| 13 | WebKit phone scroll | 0 dropped, no "ResizeObserver loop" error | 0 dropped, no error |
| 14 | Heap while the open task streams | +2.35 MB in 6 min (0.39 MB/min) | the same while it's open (see 14) |
| 14 | Store 35 s after leaving a streaming task (events / turns) | kept for good | **0 / 0** (66 events and 1 turn while open) |
| 15 | Phone streaming, 2 min: long tasks / dropped of 7,200 frames / worst gap | 0 / 0 / 17 ms | 0 / 0 / 17 ms |

## What changed

### 5. Opening, switching, J/K

- **J/K (`packages/shell/src/builtins.tsx` `step`):** the selection and the navigation run in one `batch`. The row is
  scrolled into view after the frame paints (`afterNextPaint`), not read in the key.
- **The task's snapshot is read without walking every character (`packages/sync/src/sse.ts` `SseParser.push`):** line
  ends are found with `indexOf`. A task's snapshot is one `data:` line of a few MB, and the per-character loop was
  57 ms of self time in a 4× open, plus 22 ms more in `#onLine`.
  - The existing tests (every byte split, random chunk splits, CR, LF and CRLF across chunks) pass unchanged.
- **The last place is saved after the press (`packages/client/src/client.ts` `saveNav`):** each open and J/K step
  started an IndexedDB transaction in its own frame. Steps in a row now save once, the last, on a 0 ms timer.
  `client.flush()` writes a pending one.
- **A task page groups its events once per change (`packages/shell/src/frame/turn-events.ts`, used by
  `TaskView.tsx`):** each turn used to filter and sort every event in the store on every change. A turn whose events
  didn't change keeps its array, so only the turn a streamed event belongs to recomputes.
  - Test: `packages/shell/test/turn-events.test.ts`.
- **Not done:**
  - *Coalescing same-frame `onFrame` batches:* the snapshot is already asked for in the frame after the head
    (`TaskView` `watch` in `afterNextPaint`), and after the parser fix its apply is about 30 ms in its own task, not in
    the click.
  - *A warm cache of the previous and next tasks:* not built.
  - *Why open is still over budget:* the click's first frame is still ~68 ms of script at 4×. Half of it is
    TaskView's head (`TaskView.tsx:1136` 16 ms, `:1630` 9 ms, `:1588` 7 ms). The rest is Solid tearing down Home and
    repainting the list's rows. Getting under 50 needs the head split further.

### 7. New task from an open task

- **`HomeView.tsx`:** what's under the prompt box is built in the frame after Home's first, as the task page's body
  is: the usage row's content, Needs you, the cards, the tips, the keys and the feedback line.
- **Nothing jumps:** the usage row keeps its held room (`Reserved`).
- **Test:** `apps/web/test/phone-pages.test.tsx` `new_task_in_a_phones_browser…` now waits for the tips, which come
  the frame after.

### 8. Side panel

- **One panel across a task's tabs (`packages/shell/src/frame/side-key.ts`, `Panes.tsx` `SidePane`):** the panel was
  keyed by tab kind, so every tab switch built the whole panel again. Now it's keyed by the code that draws the tab
  and by the task.
  - Test: `packages/shell/test/side-key.test.ts`.
- **`plugins/shell-task/ui/src/side/SidePanel.tsx`:** the head and tabs paint in the press's frame. The body (and, on a
  tab switch, the next tab's body) is built the frame after.
- **The terminal fits itself in its `ResizeObserver`'s first callback (`packages/ui/src/terminal/Terminal.tsx`):** it
  no longer measures in `onMount`, right after the panel's writes.
  - Test: `terminal.test.tsx` `reads no layout as it mounts…`.
- **Inactive tab bodies were already lazy** (a `Switch`).
- **The `]` open itself didn't move** (63 → 62–64 median). Its first change comes when TaskSide's chunk resolves, and
  the body is no longer in that task. In the `mine` sequence it reads 118–136 because it runs right after the J/K
  steps. Measured alone (`side`), it is the same before and after.

### 9. Settings

- **New kit helpers (`packages/ui/src/core/paint.ts`, exported from `@wisp/ui`):**
  - `afterNextPaint`;
  - `useAfterPaint(source, equals)`: a value one painted frame behind its source;
  - `useProgressive(total, first, step)`: how many rows to draw, growing a step a frame.
  - Test: `packages/ui/test/paint.test.ts`.
- **`plugins/shell-settings/ui/src/frame.tsx`:** the picked row lights and the title changes in the click. The pane's
  body is built the frame after, over a skeleton (`shell-settings.pane.loading`).
- **`plugins/settings/ui/src/panes/All.tsx`:** All settings draws its first 16 rows at once and 16 more each frame.
  Groups stay keyed, so rows already drawn are never rebuilt.

### 10. Submenus

- **v1 (`web/js/ui/submenu.js`):** submenus drilled in on the press, at once, with no hover delay.
- **v2 (`packages/ui/src/controls/menu-flyout.tsx`):** a mouse move on a submenu row opens its flyout at once, where
  Zag waits 200 ms. A sibling's flyout closes at once as the next opens (`FlyContext.only`, `menu-machine.tsx`).
- **Unchanged:** a row crossed on the way to an open flyout doesn't open its own, because the safe triangle stops its
  moves. Click, →, Enter, ← and Escape are unchanged.
- **Tests (`packages/ui/test/menu-flyout.test.tsx`):**
  - `hovering the row opens the flyout at once…`, updated: it waited for Zag's delay before;
  - new: `hovering another submenu row with one flyout open swaps them at once`.
  - Both fail on the old code. They wait a fixed few tasks, not the clock (`check:allow timer-loop-in-test`), so a
    slow machine can't make them flaky.

### 13. Scrolling: not fixed

- **The profile:** row mounts are spread over the row parts (`ListRow` cells, `RowParts` pills and time, the
  per-row `MutationObserver` and time-width observer). No single hot spot.
- **Raising the phone's overscan (8 → 14) was tried and reverted.** Script 1,102–1,104 → 1,131–1,216 ms, dropped
  frames 7/9 → 5/7, and 38 rows in the DOM instead of 26. It moves when rows mount, not how much they cost.
- **Rebinding rows instead of remounting needs `TaskRow` to follow a changing key:** today it reads its row signal
  once by `k`, and the list's items are headings, tasks, groups and runs through `Switch`/`Match`. That's a bigger
  change than this round. Asked below.
- **WebKit's "ResizeObserver loop" while scrolling the phone list:** not reproduced with coder-r149's
  `observer-scope.ts` (0 errors, before and after).

### 14. Memory while streaming

- **What grows (two snapshots, 6 min apart, task open):** the open task's own event rows: +2,929
  `{body,id,kind,seq,session_id,t,turn_id,v}`, their step bodies and 2.1 MB of strings. The devtools log is a
  500-entry ring, so it stops at 500.
- **What was wrong:** a task's rows never left the store. Every task you had opened kept its turns and events for
  good.
- **Fix:** they now go 30 s after the last watch of the task's topic ends, unless it's opened again by then (Back,
  J/K and back).
  - `packages/client/src/peers.ts`: the watch refcount, `KEEP_UNWATCHED_MS`.
  - `packages/client/src/entities.ts`: `EntityWriter.dropTopic`. A row another topic also holds (the task's row on
    `list`) stays.
  - Test: `peers.test.ts` `a_closed_tasks_rows_leave_memory…`, covering a snapshot and a streamed append.
  - Checked in the app: 66 event rows and 1 turn while open; 0 and 0 thirty-five seconds after leaving.
- **Not done: a window of the open, running turn's events.** The working panel's counts ("N actions", read / edit /
  run) and the steps list come from the events held, so a client-side window would undercount a long run. The
  server's window already sends only the latest whole turns on a fresh open. Bounding one running turn needs the
  turn's step counts from the server. Asked below.
- **Open:** in the 4× heap snapshots, the total didn't fall within 45 s of leaving. A 571-event array is still reached
  through a list signal's cached value under a Map-held context, though the store itself reads empty. The retainer
  isn't identified yet.

### 15. Phone streaming

- **Re-checked on a quieter machine:** load 8–10, 2 min, Chrome phone 4×. Before and after: 0 long tasks, 0 of about
  7,200 frames dropped, worst gap 17 ms. Not reproduced, so nothing was changed.
