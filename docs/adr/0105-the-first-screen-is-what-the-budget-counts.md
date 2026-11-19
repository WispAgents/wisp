# ADR-0105: The first screen is what the budget counts, and deferred work waits for it

- Status: accepted
- Date: 2026-11-18
- Source: the UI performance audit (`/tmp/qa-keep/perf-ui-r1.md` #1–#4, 2026-11-18); the lead's job ("fix the
  measurement at the cause … the number stays 120 KB br; only the measurement gets honest"); the owner's delegate,
  2026-11-19 (§4: land the honest measure as a ratchet at today's numbers plus 2 KB; 120 / 30 stays the goal). Follows ADR-0011 (the
  budgets), ADR-0067 (plugins' later parts), ADR-0065 (the phone looks). Changes how ADR-0011's initial budgets are
  measured and when ADR-0067's later parts load; the numbers stay.

## Context

ADR-0011 budgets the first page: initial JS ≤ 120 KB br, initial CSS ≤ 30 KB br. `tools/budgets.ts` counted only
the tags `index.html` names (the entry and its modulepreloads, the stylesheet it links). Every lazy chunk the first
screen asks for as it draws was missed: the gate passed at 118 KB while the browser fetched **94 JS files, 309 KB br**
before the first row (phone 303 KB), and **36 KB br of CSS** (phone 43.5 KB). On top of that, every plugin's `later`
part (ADR-0067: "as the first screen draws") and the app's own deferred work (the overlays' machines, the task page's
warm-up) started at `render()`, competing with the first screen for the network and the main thread.

## Decision

### 1. The first screen, defined

The first screen is drawn when the shell has rendered, the home computer's `list` topic is here (its snapshot landed
this run, or first paint restored it as whole), or the home computer is away (asleep, unreachable, incompatible:
nothing more is coming), and two frames have painted after that. The shell marks it with
`performance.mark('wisp:first-screen')` (`packages/shell/src/shell.tsx` `whenFirstScreen`).

### 2. Deferred work waits for it

After the mark, when the page is idle (`requestIdleCallback`, 1 s timeout), these start, in this order:
- the plugins' later parts (ADR-0067 otherwise unchanged: what only a press reaches);
- the app's deferred work through `Shell.afterFirstScreen` / `ShellContext.afterFirstScreen` (additive): the overlays'
  machines (`preloadControls`), the task page warm-up (F-phone-8), the notifier, the harness picker's code;
- the kit's own (`afterFirstScreen` / `firstScreenDrawn` in `@wisp/ui`): the phone's Settings pages.

`laterReady()` (a test, or a screen that needs every later part now) starts them at once.

### 3. The budget counts the first screen, honestly

`tools/budgets.ts` measures with `tools/first-screen.ts`: the built app, served by `vite preview` with the mock peer,
opened cold (a fresh browser context) in headless Chrome at 1440×900 and as the phone app (390×844, touch, the
Capacitor look), at `/?first-screen`. In that mode the shell starts none of the deferred work above, so every JS and
CSS file fetched is the first screen's, its lazy chains followed to the end: the tool waits for the mark, then for no
JS or CSS in flight for 1 s (`QUIET_MS`), and counts every one fetched (brotli 11, as the edge serves). Each look is listed and gated
(§4: on a ratchet toward ADR-0011's 120 KB br JS, 30 KB br CSS).

- Deterministic: the set depends on the code, not on timing (six runs gave identical sets; a load triggered by a
  timer or an idle callback would make it vary, so deferred work goes through §2, never a bare timer).
- Fast and dependency-free: Chrome's DevTools protocol over Node's own `WebSocket`; Chrome from `$CHROME_PATH`, the
  macOS app, or `google-chrome` / `chromium` on the PATH (GitHub's Linux runners have it). About 3 s for both looks.
- `--entry-only` keeps the old count (index.html's tags) for the tool's own tests; CI never passes it.

### 4. The gate is a ratchet toward 120 / 30 (2026-11-19)

Cutting the first screen to 120 / 30 is longer work than one change (see Consequences), so the honest measure lands
now as a CI gate on a ratchet, and the target stays:

- **Each look's first screen** is gated at `FIRST_SCREEN_BUDGETS` in `tools/budgets.ts`: the numbers measured on
  2026-11-19 plus 2 KB br. Measured: desktop JS 171.1, phone JS 164.9, desktop CSS 28.6, phone CSS 39.5. Gates:
  **desktop JS 173, phone JS 167, desktop CSS 30, phone CSS 40.5 KB br.**
- **The ratchet only goes down.** A cut that lands lowers its look's number to the new measure plus 2 KB, in the same
  PR. Raising it needs a new ADR that says why. A ratchet at or under ADR-0011's numbers is replaced by them.
- **120 KB br JS and 30 KB br CSS stay the goal** for each look's first screen (ADR-0011).
- **index.html's own files** (the entry, its modulepreloads, its stylesheet) stay gated at 120 / 30 as before. They
  measure 98.9 / 28.6 KB br today.
- The CI step (`npm run budgets` after the web build, `.github/workflows/suite.yml`) measures both looks in the
  runner's Chrome and fails on any of the six. The job summary lists each against its budget.

The next cuts, in order, from the coverage of the first screen (2026-11-19: 550 KB of minified JS loaded on the
desktop, 52% of it run):
1. **Home's unpressed handlers.** HomeView, builtins, frame commands, Sidebar and Home hold about 40 KB of minified
   code that runs only on a press. It goes behind `lazyPart` with the press replayed, as the kit's controls do, so a
   press still answers in the frame.
2. **The list.** TaskList is 16 KB br: its row menu chain (task-menu, the list's menu and actions) and the rows'
   less-used parts go lazy.
3. **A pinned stuff pane.** About 15 KB br, when one is pinned: the pane host and its strings come after the first
   screen, behind the pane's skeleton.
4. **CSS layers, then the phone split.** The kit's sheets move into ordered nested layers (`@layer kit.base,
   kit.screens, kit.after`), so a sheet loaded with its screen's code (Home's, the task page's) keeps its place in the
   cascade. Then a phone's first screen loads only what it draws (39.5 → under 30 KB br). This needs a `qa/parity.mjs`
   run.

Each split is weighed in brotli per file: the gate counts files compressed one by one. On 2026-11-19 the desktop's
first-screen files came to 194.8 KB br compressed one by one, against 174.2 KB br joined. Moving Home into its own
chunk saved the phone 20 KB and cost the desktop 7.

## Consequences

- The gate now fails where the first screen really is over budget. Measured on this change (2026-11-18): desktop JS
  197 KB br, CSS 28.6; phone JS 202 KB, CSS 39.5 (the audit's honest numbers were 309 / 36 and 303 / 43.5). The JS
  left is mostly the entry chunk (≈119 KB br: shell, kit, client, Solid) plus what Home and the list draw; getting under
  120 needs the entry split by screen (icons, the shell catalogue, Home's own code), and the phone's CSS needs the kit
  sheet split so a phone doesn't carry desktop-only rules. Cuts landed on 2026-11-19, before the ratchet:
  - the icons split by use;
  - the shell's lazy strings kept off page 1;
  - the touch binders and the row Swipe load only with the phone look;
  - the phone overlays load after the mark;
  - the machine chip loads only when it shows;
  - Spare, the update pill and the Home feedback note are drawn after the mark;
  - Home is its own chunk (the desktop frame waits for it; the phone fetches it after the mark or at the first touch);
  - the worktrees Done asker loads on a Done.

  Together these brought the first screen to 171.1 / 164.9 KB br JS. The step is green on §4's ratchet.
- What waits for the mark is never on the first screen: a press before it still works (the picker, a menu's machine
  and a later part load on demand, as before).
- New first-screen work must not hide behind a timer: a bare timer or idle callback that loads code makes the gate's
  set vary. Use `afterFirstScreen`.

## Places it reaches (ADR-0040)

UI only: the web app, the desktop app and the phone apps load less before their first screen. CI: the bundle step.
