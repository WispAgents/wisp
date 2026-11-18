# wisp v2 UI performance audit, round 1 (2026-11-18)

Findings only: no repo file was edited. Worktree `/Users/mokh/.wisp/worktrees/wisp-8fb64ec939b0` at `1b971518`.

## How it was measured

- **Build:** `cd apps/web && npx vite build --outDir /tmp/perfweb --emptyOutDir --sourcemap`, then `node tools/budgets.ts --dist=/tmp/perfweb`.
- **Server:** `vite preview` of that build with the mock peer, `MOCK_SESSIONS=5000`, on :5288 (stopped at the end). All timings are on the minified production bundle, not dev mode.
- **Browsers:** playwright-core from `/tmp/pw`.
  - Chrome with CDP `Emulation.setCPUThrottlingRate 4` (labelled "4×").
  - Chrome unthrottled ("1×").
  - WebKit unthrottled (Playwright can't throttle it; this Mac's speed).
  - Phone look: 390×844, touch, the fake Capacitor bridge (`window.Capacitor = { isNativePlatform: () => true, getPlatform: () => 'android' }`), Chrome 4×.
- **tap → frame:** from the input event's `timeStamp` to the frame after the first DOM change it caused (rAF, then a MessageChannel hop). A click that changes nothing measures 12–16 ms at 4×, so the numbers below are the app's own cost. Chrome's Event Timing durations agree within about 20 ms.
- **Long tasks** come from `PerformanceObserver('longtask')`, which is Chrome only (WebKit has no API).
- **Script, style and layout times** come from CDP `Performance.getMetrics` deltas. Callers come from CPU profiles mapped through the sourcemaps; selector costs from a trace with `disabled-by-default-blink.debug` selector stats.
- **Scripts:** `/tmp/perf-run.mjs` (phases), `/tmp/perf-first.mjs`, `/tmp/perf-split.mjs`, `/tmp/perf-callers.mjs`, `/tmp/perf-sel.mjs`, `/tmp/perf-dig.mjs`, `/tmp/perf-map.mjs`. Raw output: `/tmp/perf-out-*.json`.

## Summary of budgets

| Budget | Measured | Verdict |
| --- | --- | --- |
| Initial JS ≤ 120 KB br (as `tools/budgets.ts` counts it) | 118.3 KB (1 file) | passes, 1.7 KB headroom |
| Initial JS ≤ 120 KB br (what the first screen actually loads) | **308.7 KB in 94 files** (desktop), 302.9 KB in 79 (phone) | **2.6× over** |
| Initial CSS ≤ 30 KB br (as counted) | 28.8 KB | passes, 1.2 KB headroom |
| Initial CSS ≤ 30 KB br (actually loaded on Home) | **36.0 KB** desktop, **43.5 KB** phone | **over** |
| Phone open → cached paint ≤ 400 ms (plan §8) | first row 512 ms phone warm, 778 ms desktop warm (4×) | **over** |
| tap → frame ≤ 50 ms p95 (4×) | 74–399 ms on every action measured | **over everywhere** |
| qa/feedback: visible change ≤ 100 ms | open a task 116 ms (Chrome 1×), 167 ms (WebKit 1×) | **over on a fast Mac** |
| Streaming: 0 long tasks, ≤ 100 ms scripting / 3 s (4×) | desktop 0 long, 34.5 ms / 3 s; **phone 1 long task (85 ms)** | desktop passes, **phone fails** |
| VirtualList past 200 rows | 42–44 rows in the DOM out of 5,000 | passes |

## Findings

### 1. The first screen loads 2.6× the JS budget; `tools/budgets.ts` measures only the entry file

- **Measured:** before the first list row is in the DOM, Chrome fetches **94 JS files, 308.7 KB brotli** (desktop) and 79 files, 302.9 KB (phone look). The budget is 120 KB. 138 JS files load in all (452 KB br) without opening anything.
- **Reproduce:** `node /tmp/perf-first.mjs` (or `PHONE=1 node /tmp/perf-first.mjs`) against the preview. It lists every `.js` resource fetched before the first `[data-part~="card"]` and brotli-11s each from the dist.
- **Suspected cause:**
  - `tools/budgets.ts:70-77` (`initialFiles`) only counts the `<script>` / `modulepreload` / stylesheet tags in `index.html`, so lazy chunks that load before the first paint never count.
  - `packages/shell/src/shell.tsx:389-390` sets `rendered = true` right after `render()` returns (synchronously, before the browser paints) and starts every plugin's `later` part at once, so 9 plugins' later chunks compete with the first screen.
  - `apps/web/src/plugins.ts` imports every plugin UI's index eagerly; the plugin index modules add about 60 KB raw to the entry.
- **Impact:** high. Over the phone's SSH tunnel each of the 94 requests is a round trip, and the budget gate passes while the real first screen is 2.6× over it. This also feeds finding 4.

### 2. Home loads 36 KB br of CSS (phone 43.5 KB) against a 30 KB budget

- **Measured:**
  - Desktop: `index-*.css` 28.8 KB, plus `settings-*.css`, `TeamPop-*.css`, `Automations-*.css` and `RunnerPicker-*.css`, all fetched before the first row: 36.0 KB br.
  - Phone: plus `android-styles-*.css` (9.7 KB) and `phone-pages-*.css`: 43.5 KB br.
- **Reproduce:** same script as finding 1 (`cssBrBeforeKB`).
- **Suspected cause:** the Settings panes' and Automations' CSS comes with their lazy chunks, started at boot by the eager registration and the `later` loads (finding 1). Nothing on Home shows Settings or Automations.
- **Impact:** medium. Render-blocking bytes on every open, and more style rules for every recalc (finding 5).

### 3. The entry chunk and its CSS sit right at their limits

- **Measured:** JS 118.3 of 120 KB br, CSS 28.8 of 30 KB br. The entry chunk is 433 KB raw. By source (sourcemap byte attribution, `/tmp/perf-map.mjs`):
  - `packages/shell` 161 KB, `packages/ui` 122 KB, `packages/client` 40 KB, solid-js 26 KB.
  - Largest files:
    - `packages/shell/src/frame/HomeView.tsx`: 25.7 KB
    - `packages/ui/src/content/icons.ts`: 24.2 KB (every icon, used or not)
    - `packages/shell/src/i18n/en.ts`: 17.5 KB (the whole catalogue, including strings for screens that load lazily)
    - `packages/ui/src/app/Home.tsx`: 13.0 KB
    - `packages/ui/src/app/Stage.tsx`: 11.3 KB
    - `packages/shell/src/builtins.tsx`: 9.8 KB
    - `packages/shell/src/frame/Sidebar.tsx`: 8.8 KB
    - `packages/shell/src/frame/commands.ts`: 7.6 KB
- **Suspected cause:** icons and the shell catalogue are whole modules in the entry rather than split by screen.
- **Impact:** medium. The next feature that touches the first screen breaks the budget gate.

### 4. First paint and time to interactive are slow, with long startup tasks

- **Measured (Chrome 4×):**

  | Case | First row | TTI | Long tasks before TTI |
  | --- | --- | --- | --- |
  | Desktop, cold | 1,052 ms (FCP 608) | 2,458 ms | 9, longest 422 ms (then 239) |
  | Desktop, warm (IndexedDB) | 778 ms | 1,870 ms | 9, longest 372 ms |
  | Phone, cold | 609 ms | 966 ms | 5, longest 350 ms |
  | Phone, warm | 512 ms | 963 ms | 6, longest 219 ms |

  The plan §8 target for a cached phone open is 400 ms. WebKit unthrottled: first row 318 ms cold, 217 ms warm.
- **Reproduce:** `node /tmp/perf-run.mjs chrome load` (and `PHONE=1 …`).
- **Suspected cause:** parsing and evaluating the 433 KB entry plus about 93 chunks (the 422 ms and 239 ms tasks are script evaluation at boot), and the `later` loads started before the first paint (`shell.tsx:389-390`).
- **Impact:** high. Every app open, worst on the phone.

### 5. Style recalculation dominates every interaction

The kit's attribute-only selectors make every recalc test about 2,000 rules per element.

- **Measured (Chrome 4×, `/tmp/perf-split.mjs`, 1.2 s after each action):**

  | Action | Style | Recalcs | Script | Layout |
  | --- | --- | --- | --- | --- |
  | Open a task | 309 ms | 160 | 115 ms | 59 ms |
  | Next task (J) | 275 ms | 147 | 69 ms | 58 ms |
  | Side panel | 273 ms | 45 | 31 ms | 51 ms |
  | ⌘K | 198 ms | 26 | 43 ms | 14 ms |
  | Project picker | 188 ms | 42 | 38 ms | 16 ms |
  | Close side panel | 130 ms | 19 | 15 ms | 10 ms |

  Streaming spends 907 ms on style recalcs per 6 s against 69 ms of script (finding 15).
- **Root cause:** the main stylesheet has **2,297 selectors, 1,976 of them `[data-kit="…"]` attribute selectors and none by class**. Blink buckets attribute selectors by attribute *name*, so every element with a `data-kit` is tried against nearly all of them. The selector-stats trace (`/tmp/perf-sel.mjs`, opening a task) shows rules that match nothing, such as `[data-kit="split-main"]` and `[data-kit="ov-x"]`, each attempted about 1,972 times per recalc. The time is spread over 2,871 selectors, so no single rule is hot.
- **Where:** `packages/ui/src/css/*.css` (ADR-0007's "the kit styles itself through `data-kit`").
- **Impact:** high, systemic. It multiplies every other finding: each forced recalc (findings 6, 8, 9, 13) costs 2–3 ms at 4× even when little changed.

### 6. Opening a task: 399 ms at 4×, and over 100 ms even unthrottled

- **Measured:** 399 ms at 4×, 116 ms in Chrome 1×, 167 ms in WebKit 1×, 124 ms on the phone look at 4×. Budget 50 ms (4×); qa/feedback 100 ms. It adds 1,336 DOM nodes.
- **Reproduce:** `node /tmp/perf-run.mjs chrome actions`, or click any `[data-part~="card.title"]` with the instrumentation in `/tmp/perf-run.mjs`.
- **Suspected cause (CPU profile, `node /tmp/perf-callers.mjs open`):**
  - `packages/shell/src/frame/TaskView.tsx:349`: `queueMicrotask(() => … input?.focus())` runs during the render, before the first frame, and `focus()` forces a synchronous style and layout: 39 ms at 4×.
  - `packages/ui/src/app/hold-view.ts:18` (`size()`: `getClientRects()` + `offsetHeight`, called from `:27`): 31 ms of forced layout while the task view sets up.
  - Then the style recalcs of finding 5 (309 ms, 160 recalcs).
- **Impact:** high. It's the most frequent action.

### 7. J / K (next and previous task): 135–286 ms per step at 4×, slower as you go

- **Measured (4×):** per step 162, 169, 170, 135, 141, 150, 155, 197 (J ×8), then 185, 165, 160, 158, 245, 226, 263, 286 (K ×8), so p95 is about 263 ms. Also 155 ms in WebKit 1×, 49 ms in Chrome 1×, and up to 155 ms on the phone at 4×. Each step adds about 1,322 nodes and 275 ms of style.
- **Suspected cause:** each step builds the whole task view again (as in finding 6, `TaskView.tsx`); the later steps getting slower points to work growing with the number of tasks visited (see finding 16).
- **Impact:** high for keyboard users; the budget is 50 ms.

### 8. Opening the side panel: 194 ms at 4×, 285 ms in WebKit unthrottled

- **Measured:** 194 ms (4×), 285 ms (WebKit 1×), 38 ms (Chrome 1×); style 273 ms; +988 nodes.
- **Suspected cause (`node /tmp/perf-callers.mjs side` with `SELF=1`):** the Shell tab's terminal mounts and fits synchronously as the panel opens.
  - `packages/ui/src/terminal/Terminal.tsx:114` (`size()`: `getComputedStyle(box)`, `clientWidth`/`clientHeight` after the panel's writes): 87.6 ms of forced layout.
  - `Terminal.tsx:151` (`paint`): 30.6 ms.
  - `hold-view.ts:27`: 14.9 ms.
- **Impact:** high. It's WebKit's worst action (the Mac app's engine).

### 9. ⌘K opens in 283 ms at 4×; Zag forces a full style recalc

- **Measured:** 283 ms (4×), 195 ms (WebKit 1×), 61 ms (Chrome 1×), 159 ms on the phone at 4×. Typing in it: the first keys take 95, 80 and 65 ms, then 31–36 ms per key at 4× (57 ms max in WebKit).
- **Suspected cause (`node /tmp/perf-callers.mjs k` with `SELF=1`):**
  - `node_modules/@zag-js/dismissable/dist/layer-stack.mjs:94` (`syncLayers`) reads `getComputedStyle(layer.node).zIndex` right after setting styles, forcing a whole-document style recalc: 74.9 ms self time at 4×.
  - Plus `@zag-js/dom-query` `node.mjs:24` (16 ms) and `overflow.mjs:33` (11 ms).
  - `packages/shell/src/palette/tasks.ts:39` builds the task items: 8 ms.
- **Impact:** high. The same layer stack runs for every dialog, sheet and popover the kit opens (findings 10 and 11).

### 10. New task: 357 ms at 4×; the project picker 256 ms

- **Measured:**
  - New task: 357 ms (4×), 101 ms (WebKit 1×), 54 ms (Chrome 1×); +1,235 nodes.
  - Project picker (`launcher.place`): 256 ms (4×), 170 ms (WebKit 1×), 55 ms (Chrome 1×); style 188 ms; longest task 214 ms.
- **Suspected cause:**
  - `packages/shell/src/frame/HomeView.tsx:301` focuses during the render (forced layout, as in finding 6).
  - The picker is a Zag dismissable layer (finding 9), on top of the style cost of finding 5.
- **Impact:** medium-high.

### 11. Menus, flyouts and Settings panes: all over 50 ms at 4×

- **Measured (4×):**
  - Task More: 130 ms. List-row More: 198 ms.
  - Composer harness picker: 130 ms. New task harness picker: 150 ms.
  - Composer mode flyout: 106 ms. New task mode flyout: 111 ms.
  - Settings panes: 74–98 ms each (13 panes).
- **Measured (WebKit 1×):** list-row More 83 ms; Settings panes 45–64 ms, except **This computer at 139 ms**.
- **Suspected cause:** finding 5's recalc cost per frame, and Zag's layer stack (finding 9) for the menus.
- **Impact:** medium. Every one fails the 50 ms p95 budget; none fails qa/feedback's 100 ms in Chrome 1×.

### 12. A closed menu rebuilds all its DOM on every task change

- **Measured:** one 👍 on a turn makes 108 DOM mutations. 68 of them remove and re-create the open task's *closed, hidden* More menu: 18 items, 8 separators and 4 submenus, rebuilt twice. Any change to the session row does the same.
- **Reproduce:** `node /tmp/perf-dig.mjs` and `/tmp/perf-dig2.mjs` (MutationObserver log by `data-part`).
- **Suspected cause:**
  - `packages/ui/src/controls/menu-machine.tsx:205`: `<For each={tidySeparators(props.items)}>` is keyed by object identity.
  - `packages/shell/src/frame/TaskView.tsx:822`: `taskMenu({…})` returns new item objects on every session change, so every row's nodes are thrown away.
  - The menu's content stays mounted while closed.
- **Impact:** medium. Wasted work on every patch to the open task: while it streams, every row patch rebuilds it.

### 13. List rows are restyled by changes that aren't theirs

- **Measured:**
  - A 👍 in the open task rewrites the `style` attribute of 6 unrelated list rows (`card.head`, 4× each).
  - One sync patch to another task (s0005) does the same to 5 other rows (4× each), on top of the moved row itself.
- **Suspected cause:** `packages/ui/src/composites/time-width.ts:13`. One shared ResizeObserver calls `row.style.setProperty('--k-time-w', …)` for every observed time on each callback, without checking whether the width changed. Each re-observe (`:21`) and removal (`:25`) writes too. Every write invalidates the row's style; with finding 5, each costs a recalc over the row's subtree.
- **Impact:** medium. It adds to the 616 style recalcs while scrolling (finding 14) and to every list update.

### 14. Scrolling 5,000 tasks drops frames at 4×

- **Measured:**
  - Desktop 4×, 120 wheel steps over about 3 s: **62 of 432 frames dropped** (gaps over 34 ms), worst 67 ms, 1 long task (54 ms), 615 layouts, 616 style recalcs, 638 ms of script.
  - Phone 4×: **81 of 378 frames dropped**, worst 100 ms, 2 long tasks (80 ms).
  - WebKit 1×: 1 dropped.
  - Virtualisation works (42–44 rows in the DOM).
- **Reproduce:** `node /tmp/perf-run.mjs chrome scroll` (opens Done, then wheels the list).
- **Suspected cause:**
  - `packages/ui/src/layout/VirtualList.tsx:114`: every newly measured row height calls `setVersion`.
  - `VirtualList.tsx:248-251`: `offs` then maps **all 5,000 items** (`props.key(item)` and a Map lookup each) to rebuild the offsets. That's full-list work on every frame that scrolls unmeasured rows in (the "no full-list work per event" rule).
  - The rows mounted while scrolling also write `--k-time-w` (finding 13), and the spacers' heights change every frame.
- **Impact:** medium on desktop, high on the phone.

### 15. Streaming: within budget on desktop, one long task on the phone; style recalc every frame

- **Measured (400 tokens at 16 ms into the open task):**
  - Desktop 4×: 0 long tasks; scripting 69 ms per 6 s (34.5 ms per 3 s, within 100); but **907 ms of style recalc in 6 s** (367 recalcs, one per frame) and 297 ms of layout; 1 dropped frame (50 ms).
  - **Phone 4×: 1 long task (85 ms; budget 0)**, worst frame gap 100 ms.
  - WebKit 1×: 1 dropped frame.
- **Suspected cause:** findings 5 and 12 (the open task's menus rebuilt on each patch) and the rows' time-width writes (finding 13). Each token restyles more than the growing text.
- **Impact:** medium. The phone misses its streaming budget.

### 16. Opening 50 tasks keeps about 2,300 detached nodes (bounded)

- **Measured:** after opening 50 tasks and going back Home, with GC:
  - JS heap 30.5 → 35.4 MB.
  - CDP `Nodes` 3,578 → 5,901, while nodes in the document stay 1,427 → 1,427.
  - Event listeners 203 → 420.
  - A second round of 50 tasks leaves it flat (5,901 nodes, 36.0 MB), so it's a bounded cache, not a leak.
- **Reproduce:** `node /tmp/perf-dig.mjs` (end of its output).
- **Suspected cause:** a kept-view cache of recently opened tasks; confirm it's intended and sized for the phone.
- **Impact:** low.

## Not measured

- Phone menus and flyouts: the desktop selectors don't exist in the phone layout. Open a task, J/K, ⌘K and typing were measured on the phone.
- Component re-run counts: the production build has no instrumentation, so DOM mutations stand in for them (findings 12, 13).
- Long tasks in WebKit: no API.

**16 findings.**
