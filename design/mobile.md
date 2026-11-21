# Phones

wisp runs on a phone in three ways. Part A is the Android and iPhone apps (ADR-0065): each in its platform's
look, built from the kit's platform variants. Part B is wisp in a phone's browser and the Home Screen web app:
v1's look and rules, kept exactly (ADR-0016), and still the list of everything the apps must also do.

| | Phone browser, Home Screen web app | Android app | iPhone app |
|---|---|---|---|
| Look | v1 (Part B), `qa/parity.mjs` | Material 3 (Part A) | Apple HIG (Part A) |
| How the kit knows | no `data-platform` | `<html data-platform="android">` | `<html data-platform="ios">` |
| Checked by | parity, layout, feedback | layout, feedback, `qa/platform.mjs`, JUnit | layout, feedback, `qa/platform.mjs`, XCTest |

# Part A: the phone apps

The owner, 2026-11-13: Android and iPhone don't copy v1's look; each follows its platform, built from `@wisp/ui`
with per-platform variants on wisp's tokens, no app CSS, every v1 feature kept; gestures where they fit better
than taps, each with a visible control too.

Rules that hold on both:
- **Features don't know the platform.** A screen is the same kit calls on both; the kit draws Material 3 or HIG.
  The only code that reads `data-platform` is in `packages/ui` and `packages/shell/src/platform` (`tools/check`
  rule `platform-branch`).
- **Every gesture has a visible twin** (§A.4): a button or menu entry that does the same thing, on the same screen.
  Nobody has to know a gesture to use wisp.
- **Every action answers within one frame** (`design/feedback.md`), and **control text never wraps**
  (`design/layout.md`), at text scale 100–200 %.
- **The words are v1's** (`design/copy.md`): *Mark done*, *Save for later*, *New task*, *Computers*. A platform's
  own words are used only for the platform's own controls (*Edit*, *Done* and *Cancel* on iOS; *Select* on both).
- **Safe areas and the keyboard:** nothing pinned to an edge sits under the status bar, the notch, the home
  indicator or the gesture bar (`--safe-*`); the composer rides above the keyboard; a sheet's field is never covered.
- **Light and dark follow the system** (Settings can still force one, as v1). Contrast ≥ 4.5:1 for text.

## A.1 Navigation

### Android (Material 3)

- **Top app bar** on every screen. The list: a *small* bar with the computer filter (*All* or a computer's name) as
  its title button, search and the me avatar at the end. A pushed page (a task, a setting): a *medium* bar whose
  title takes two lines expanded and collapses to one line on scroll, Back at the start, the page's actions at the
  end, folding label → icon → ⋮ by priority (`Toolbar`'s rules).
- **Navigation bar** at the bottom on phones (< 600 dp): **Tasks · Crew · Stuff · Computers**. **Navigation rail**
  from 600 dp; from 840 dp the list and the open task sit side by side (M3 list-detail), as the browser's split.
  Settings is in the me avatar's menu (v1's me menu), not a destination.
- **FAB:** an extended FAB *New task* over the list, shrinking to its icon while the list scrolls down and growing
  back on scroll up. It gives way to a question's choices on a row (v1 `fab.js giveWay`: fades and sinks, comes back).
  Crew has *New job*; Stuff has none (items come from tasks; *Upload* is in the top bar).
- **Back:** the system Back (gesture or button) closes the newest sheet, menu or dialog, then the open page, then the
  computer filter, then leaves the app. Predictive Back (Android 14+): the page shrinks and follows the gesture's
  progress before letting go, and lands on what Back will show.
- **Motion:** M3 *emphasized* for pages (container transform from a row into its task page; shared axis X between
  tasks; fade through between destinations); *standard* for small things. Durations from the tokens (§A.8).

### iOS (Apple HIG)

- **Navigation bar with large titles** on the root of each tab (*Tasks*, *Crew*, *Stuff*, *Computers*), collapsing to
  an inline title on scroll; the computer filter is the large title's menu (a chevron after it). Search is the
  bar's search field, revealed by pulling the list down a little.
- **Tab bar:** **Tasks · Crew · Stuff · Computers**, the same four. On iPad (regular width) a sidebar and the list and
  task side by side (split view).
- **New task** is the compose button (`square.and.pencil` metrics) at the navigation bar's trailing end, as Mail and
  Notes. It opens New task as a **sheet** (large detent, grabber), the prompt focused. Swiping the sheet down keeps
  the draft (v1: *the draft stays*); the sheet's *Cancel* does the same.
- **Back:** the back button (the previous title, shortened to *Back*) and the **interactive edge swipe** from the
  left edge, which follows the finger and can be cancelled halfway.
- **Sheets** for anything that isn't a place: New task, the row's actions on long-press menus, pickers, the project
  sheet, sign-in. Medium and large detents where the content is short.
- **Motion:** springs (the tokens' `--spring-*`, approximated with `linear()` easing), the standard push/pop, sheets
  that follow the finger.

### Both

- **The app opens with the Computers destination selected** (v1's behaviour, the default until the owner answers
  ADR-0065 open question 3), never inside a task; Tasks is one tap away. With no computer saved,
  Computers shows *Continue with GitHub* / *Google* (*Apple* on iOS) first, *Connect over SSH instead* under it.
- A **share from another app** opens New task from the first paint, with the shared items attached, the keyboard
  away so Send is in view; *Shared from outside wisp* stays above it until sent; nothing leaves the phone before
  Send (v1).
- A **notification tap** opens its task directly (Back returns to the list where it was).
- **Deep links** `wisp://task/<computer>/<id>` and the grid's `https://…/task/<id>` open the same page.

## A.2 Screens

Each screen lists what it holds (v1's features, Part B and docs/v1-features.md) and how each platform draws it.

### Tasks (the list)

- Rows from every saved computer in one list (the client's N peers), grouped as the web list groups them (need
  you, running, recent, For later); the computer filter narrows it (*All*, a computer); v1's project pick and its
  chips (docs/v2-queue.md item 7) as a horizontal chip row under the bar.
- A row: harness avatar, title (one line, `…`, full title on its page), the status pill and the time in fixed
  columns, the computer's name when the filter is *All*, unread as a dot. A question's choices on the row (v1
  `rowChoices`). A running row's live line (one line).
- **Android:** M3 two-line list items (72 dp), leading avatar 40 dp, trailing time over the pill; unread as bold
  title + dot; dividers off, section headers as M3 *title small*.
- **iOS:** a plain list with inset separators (Mail-like): title in *headline*, the live line in *subheadline*
  secondary, time trailing in *footnote*; unread as a leading blue dot; a disclosure chevron off (as Mail).
- Empty: the kit's `EmptyState` with *New task*. Offline: a slim *Offline · last updated 12:04* bar under the top bar,
  with *Retry*; rows stay.
- 4,000 rows: `VirtualList`; first paint from the cache (ADR-0065 §4).

### Task page

- Head: Back, the title (two lines expanded, one collapsed), the status, the actions in priority (*Ship it*, the side
  panel, ⋮/⋯ with the full v1 menu: Rerun, Fork, Rename, Save for later, Mark done, Archive, Delete, Copy link,
  Share…). The working panel and its status line (one line, v1 rules).
- The transcript and answer blocks (every block type; `preview` opens in the platform's in-app browser view, Custom
  Tabs / `SFSafariViewController`, with *Open in wisp* back), the composer pinned above the keyboard, the queue.
- **Side panel** (F-phone-4: changes, shell, PR, preview, panes): Android a modal side sheet from the right that
  follows a leftward swipe on the page; iOS pushed as a page that follows the same swipe, and the edge swipe returns.
  Its button stays in the head.
- The terminal opens inline (v1). Keyboard-only actions are hidden (v1 F-set-6: no shortcuts sheet in the app).
- **Android:** the composer is an M3 docked bar: a filled text field (28 dp radius), the attach and mode icons at its
  start, Send as a filled icon button (Stop in its place while running, same size). The mode menu is an M3 menu.
- **iOS:** a Messages-like field: rounded (18 pt), `+` for attach/camera/dictation at its leading side, Send as the
  filled up-arrow circle inside the field's trailing end. The mode is a pull-down menu button on the bar above it.

### New task

- v1's home as a page: the prompt, the project (and *Also work in* chips), harness, mode, model, place (the computer
  and its worktree), attachments (files, camera, photo, video, dictation), schedule, the spells/recipes.
- **Android:** a full-screen page from the FAB (container transform from the FAB), *Send* in the top bar's end and
  in the composer. Choosers (project, harness, model) are M3 modal bottom sheets.
- **iOS:** a sheet (large detent) with *Cancel* leading and *Send* trailing in its bar; choosers are pull-down menus
  where short, pushed lists inside the sheet where long (projects).
- Sending goes back to the list where the new task shows at once (*Starting*); Back/Cancel keeps the draft.

### Settings

- Every pane the web has (Agents, Phone, Channels, Appearance, Budgets, Machines, Keys, About, …) plus the app's own:
  *Computers*, *Keys on this phone*, *Notifications* (on/off per kind: finished, asks, approval; *Open settings* when
  the system refused), *Stay connected* (Android foreground service), *Swipe actions* (which action each side runs).
- **Android:** M3 list settings: section headers in the primary colour, one/two-line items, M3 switches with
  icons, single choice in a dialog with radio buttons, sliders M3.
- **iOS:** inset grouped lists, footers for help text, `UISwitch`-style switches tinted with wisp's accent,
  single choice as a pushed checkmark list, values trailing in secondary text with a chevron.

### My Stuff

- Docs, plans, images, videos, links, files and apps (ADR-0036), search, tags, versions; apps launch inside wisp in
  their sandboxed frame (F-stuff-10), full screen with the platform's top bar and *Close*; *Pin beside a task* in the
  item's menu.
- **Android:** filter chips (M3 filter chips) for kinds; a grid of M3 filled cards (two columns on phones) with the
  thumbnail; *Upload* in the top bar.
- **iOS:** a segmented control for *All · Docs · Media · Apps*, a grid like Files with names under thumbnails;
  *Upload* in the bar's ⋯ menu.

### Crew

- Members (avatar, role, what they're on), job cards (ShellApi `taskCards`), a member's page with its brief, wakes and
  history; *New job*.
- **Android:** a list of two-line items with avatars; the member page with M3 primary tabs (*Jobs · Brief · Wakes*).
- **iOS:** an inset grouped list; the member page with a segmented control for the same three.

### Computers (machines and cloud)

- The app's own screen (v1 `mobile/www/`): each wisp Cloud account is one card (picture, GitHub/Google/Apple mark,
  handle, account under it) with its computers as rows in that card; SSH computers as rows; each row's state as a
  dot and a word under its name (*Online*, *Offline*, *Over SSH*, *Asleep*), never a disabled button.
- Actions: *Continue with GitHub/Google/Apple*, *Use your own wisp Grid*, *Connect over SSH instead* (address,
  password once or a key, *Scan from your computer* for the QR, nearby computers from Bonjour), *Keys on this phone*
  (make, import, copy the `authorized_keys` line), *Sign out*, and under it the quiet *Delete your account* link
  (48 dp tall, `--bad`, opens `/account#delete`).
- A connecting SSH computer shows its steps (SSH, wisp, tunnel) in its row as v1's card did, then *Online*.
- The desktop's *Machines* (reaching other computers from a computer) is a setting pane here, as in v1.
- **Android:** M3 outlined cards per account, list items inside; the connect form as a full-screen dialog.
- **iOS:** inset grouped sections per account; the connect form as a sheet.

### Search and the palette

- One search over tasks, Stuff and commands. **Android:** the M3 search view, full screen from the top bar's search.
  **iOS:** the navigation bar's search field, results in place, scopes as tokens. No keyboard hints on either.

## A.3 Phone width, tablets, foldables

- Under 600 dp: one pane, the bottom bar. 600–839: rail (Android) / tab bar (iOS), one pane. From 840: list and task
  side by side (Android list-detail; iPad split view), the side panel as a third pane from 1200.
- A foldable's hinge is never under a pane's edge (Android `WindowLayoutInfo`, through the platform module).

## A.4 Gestures and their visible twins

| Where | Gesture | Does | Visible twin |
|---|---|---|---|
| A list row | swipe right (start → end) | *Mark done* / *Reopen* (v1; Settings → Swipe actions can pick *Archive*, *Mark read*, *Save for later*, *Delete task*) | the row's long-press menu and the task page's ⋮/⋯ menu; *Select* in the list's bar |
| A list row | swipe left (end → start) | *Save for later* / *Unsave* (v1; pickable as above) | same |
| A list row | long-press | Android: a bottom sheet of the row's actions with *Select* first; iOS: the context menu with a preview of the task | *Select* in the list's bar; the task page's menu |
| Selection mode | tap rows, or drag down the leading edge to add a range | act on several (*Mark done*, *Archive*, *Save for later*, *Delete*) | the selection bar's buttons and *Select all* |
| The list | pull down | reconnect every computer now (skips the backoff); clients never poll | *Retry* on the offline bar; *Reconnect* in the me menu |
| A task page | swipe left on the page body, starting outside the edge zones and not in a scroller | the side panel comes in (F-phone-4) | the side-panel button in the head |
| A task page's head | swipe left / right on the title | the next / previous task in the list's order (shared axis X) | ↑/↓ *Next task* / *Previous task* in the page's menu |
| Any pushed page | edge swipe (iOS left edge; Android either edge, the system's) | Back | the Back button |
| The queue (follow-ups) | long-press then drag | reorder | the drag handle on each item and *Move up* / *Move down* in its menu |
| A sheet | drag down | close (a draft stays) | *Cancel* / *Close* / *Done* |
| A message or block | long-press | Copy, Quote in reply, Select text, Share | the block's own ⋯ button (shown, not on hover) |
| An attachment chip | swipe up / tap × | remove | the × |
| The composer | swipe down on its bar | fold the keyboard | the system's keyboard dismiss; tapping outside |
| A notification | its actions (*Answer* choices, *Reply*, *Open*) | answer without opening the app | the same choices on the row and the task page |
| An image or video | pinch / double-tap | zoom | the viewer's zoom buttons |

Rules for gestures:
- **The edges are the system's.** A horizontal drag starting within 24 px of an edge is Back (Android both edges,
  iOS the left), so the side-panel swipe and row swipes start inside that margin or not at all; a vertical drag is a scroll; a drag that starts in a horizontal scroller (code, a diff, a chip row)
  scrolls it.
- **A swipe says what it'll do** before it does it: the band under the row names the action in the menu's words,
  warms with the pull and fills past the line (v1: a third of the row, or a flick) with a haptic tick. A row the
  action can't apply to (a running task) only gives a little, with no label.
- **Destructive swipes don't ask; they undo.** A swiped *Delete task* waits for its Undo snackbar/banner (v1).
- **Platform shape:** Android draws the M3 swipe band (icon at the leading edge, tonal colour); iOS draws Mail's
  swipe actions (buttons revealed from the edge; a full swipe runs the first one).
- **No nudge teaches gestures;** a tip on the Computers screen does (v1), once.
- Gestures live in one registry in the kit (`packages/ui/src/gestures`), each entry naming its twin's command; a
  gesture without a twin doesn't type-check.

## A.5 Feedback on a phone

- The press shows as the finger goes down: Android the M3 ripple and state layer; iOS the highlight (dim to
  `--state-pressed`) and a slight scale on cards. Never only on lift.
- Haptics through the platform module's `haptic` (`Haptic` in `packages/shell/src/platform/types.ts`): `select` on a swipe crossing its line and on picker changes, `success` on Send,
  `warning` on an undoable destructive action, `tap` on long-press opening a menu. Off with the system's setting.
- A pending control shows its spinner in place (`design/feedback.md` §4): M3 circular indicator / the iOS activity
  indicator, same size as the icon.
- Toasts: Android an M3 snackbar above the FAB and the navigation bar, with *Undo*; iOS a compact capsule banner above
  the tab bar with *Undo* (HIG has no toasts; this is the least surprising equivalent).

## A.6 Text size and accessibility

- Type follows the system size: Android `sp` scale, iOS Dynamic Type, to 200 %. Control text never wraps: it
  truncates or the bar folds (`design/layout.md`), and the list row grows in height, never in columns.
- TalkBack and VoiceOver: every gesture is also an accessibility action on its element (*Mark done*, *Save for
  later*, *Next task*), so screen readers get them without the gesture.
- Touch targets: 48 × 48 dp (Android), 44 × 44 pt (iOS), always; a smaller visible control grows its hit area.

## A.7 Kit variants

Each component keeps its API; `data-platform` picks the variant. Structure changes are listed; the rest is tokens.

| Component | Android (Material 3) | iOS (HIG) |
|---|---|---|
| `AppFrame` / `Frame` (shell) | top app bar (small/medium) + navigation bar / rail; edge-to-edge with insets | navigation bar (large title, collapsing) + tab bar; sidebar on iPad |
| **`NavigationBar`** (new) | M3 navigation bar 80 dp, active indicator pill 64 × 32; rail 80 dp wide from 600 dp | tab bar 49 pt + home indicator, filled icon when selected |
| **`Fab`** (new) | FAB 56 dp / extended FAB, shrinks on scroll, gives way | not drawn: becomes the navigation bar's trailing compose button |
| `Toolbar` / bars | top app bar actions, ⋮ overflow | bar buttons, ⋯ menu; bottom toolbar on pushed pages where HIG puts it |
| `ListRow` | M3 list item, 1/2/3-line (56/72/88 dp), leading 40 dp, trailing supporting text | table cell, 44 pt min, inset separators, chevron where it pushes |
| `Swipe` | M3 swipe band (tonal, icon leading), threshold ⅓ | Mail-like swipe actions, full swipe runs the first |
| `Section` / `SettingsGroup` | header in primary, no card | inset grouped section with header and footer |
| `Card` | M3 filled / outlined, 12 dp radius | grouped background, 12 pt continuous corners |
| `Button` | filled / tonal / outlined / text, 40 dp, full radius, *label large* | filled / tinted / gray / plain, 50 pt large / 34 pt small, 12 pt radius |
| `IconButton` | standard icon button, 48 dp target, state layer | plain bar button, 44 pt target, highlight |
| `Switch` | M3 switch 52 × 32 with thumb icon | iOS switch 51 × 31, wisp accent tint |
| `Tabs` | M3 primary tabs with indicator | segmented control |
| `Input` / `Field` / `PromptBox` | M3 filled text field (floating label); composer as docked bar | rounded field; labels as row titles in forms; composer Messages-like |
| `Select` / `RadioGroup` / `Combobox` | exposed dropdown menu; dialog with radios for long sets | pull-down menu button; pushed checkmark list for long sets |
| `Menu` (incl. row menus) | M3 menu from a button; long-press opens an M3 bottom sheet | pull-down menu from a button; long-press opens the context menu with preview |
| `Dialog` | M3 basic dialog, 28 dp radius, buttons end-aligned text | alert (270 pt), stacked or side by side, destructive red, *Cancel* bold |
| **`Sheet`** (new; today's overlays/`Popover` on phones) | M3 modal bottom sheet, drag handle, 28 dp top corners | sheet with detents (medium, large), grabber, 10 pt corners |
| `Popover` | becomes a menu or a bottom sheet | becomes a popover on iPad, a sheet on iPhone |
| `Toast` | M3 snackbar | capsule banner above the tab bar |
| `Tooltip` | M3 plain tooltip on long-press of an icon button | none: long-press on a bar item shows the large content viewer's label |
| `Banner` | M3 banner under the top bar | inline banner row in the list's header |
| `Pill` / chips | M3 assist / filter / input chips, 32 dp, 8 dp radius | capsule tags, *footnote* |
| `Skeleton` | M3 placeholder blocks (surface container) | redacted placeholders |
| `EmptyState` | M3 icon + *title large* + body + tonal button | SF-metrics icon + *title2* + body + filled button |
| `VirtualList` | unchanged (pull to refresh with the M3 indicator) | unchanged (pull to refresh with the iOS refresh control) |
| `Avatar`, `Pill`, `Dot`, `Time` | sizes from tokens | sizes from tokens |
| `Icon` | Material Symbols Rounded | wisp's icon set at SF metrics (ADR-0065 open question 5) |
| Blocks (`blocks/*`), `Markdown`, `Code`, `Diff` | content: unchanged layout, platform type roles and colours | same |
| `AppFrame` for My Stuff apps (`StuffFrame`) | full screen, top app bar *Close*; tokens passed as CSS vars | full screen, nav bar *Done*; same |

## A.8 Tokens to add

In `@wisp/tokens`, one set per platform, set under `:root[data-platform=android|ios]`. Colours are derived from wisp's
own (accent, surfaces, good/bad/warn), light and dark; numbers are the platform spec's, cited in the token file.

| Token | Android | iOS | Spec |
|---|---|---|---|
| `--tap-min` | 48px | 44px | M3 accessibility; HIG hit targets |
| `--bar-h` / `--bar-h-large` | 64px (small) / 112px (medium) | 44px / 96px (large title) | M3 top app bar; HIG navigation bar |
| `--nav-h` / `--rail-w` | 80px / 80px | 49px (+ home indicator) / sidebar 320px | M3 navigation bar/rail; HIG tab bar |
| `--fab` / `--fab-ext-h` | 56px / 56px | n/a | M3 FAB |
| `--row-h-1` / `-2` / `-3` | 56 / 72 / 88px | 44 / 60 / 76px | M3 lists; HIG tables |
| `--inset` | 16px | 16px (20px grouped) | both |
| `--radius-xs…xl` | 4 / 8 / 12 / 16 / 28px | 6 / 8 / 10 / 12 / 18px (continuous) | M3 shape scale; HIG |
| `--radius-sheet` / `--radius-dialog` | 28px / 28px | 10px / 14px | M3 sheets, dialogs; HIG |
| `--type-*` roles | display/headline/title/body/label × L/M/S (e.g. body-l 16/24, title-m 16/24 w500, label-l 14/20 w500) | largeTitle 34/41, title1 28/34, title2 22/28, title3 20/25, headline 17/22 w600, body 17/22, callout 16/21, subhead 15/20, footnote 13/18, caption 12/16 | M3 type scale; HIG Dynamic Type (large) |
| `--font-ui` | `Roboto, system-ui` (the system font) | `system-ui` (SF Pro) | system fonts, not shipped |
| `--surface-0…4` | surface, container lowest/low/default/high/highest | systemBackground, secondary, tertiary; grouped variants | M3 tonal surfaces; HIG backgrounds |
| `--on-surface`, `--on-surface-var`, `--outline`, `--outline-var` | M3 roles from wisp's colours | label, secondaryLabel, separator (0.5px hairline), opaqueSeparator | |
| `--state-hover/focus/pressed/drag` | 0.08 / 0.10 / 0.10 / 0.16 opacity layers | n/a / 0.10 / highlight 0.15 / 0.16 | M3 state layers |
| `--elev-1…3` | M3 tonal elevation + shadow levels 1–3 | sheet and menu shadows | |
| `--ease-standard` / `--ease-emph`, `--ease-emph-in`, `--ease-emph-out` | (0.2,0,0,1) / (0.2,0,0,1) with emphasized path, (0.3,0,0.8,0.15), (0.05,0.7,0.1,1) | `--spring-snappy`, `--spring-smooth` as `linear()` curves | M3 motion; HIG springs |
| `--dur-short/medium/long` | 150 / 300 / 500ms | 250 / 350 / 500ms | M3 durations |
| `--swipe-line` | 0.33 (v1) | 0.5 (full swipe) | v1; Mail |
| `--grabber` | 32 × 4px handle | 36 × 5px grabber | both |
| `--safe-top/right/bottom/left` | `env(safe-area-inset-*)` + the gesture bar | `env(safe-area-inset-*)` | |

## A.9 Checking it

- `design/checklist.md`, plus: each screen seen on an Android emulator (Pixel 8, API 36) and an iOS Simulator
  (iPhone 16, iOS current), light and dark, text at 100 % and 200 %, and once on a real phone of each.
- `qa/platform.mjs` (ADR-0065 §5) on the phone views; `qa/layout.mjs` and `qa/feedback.mjs` too. `qa/parity.mjs`
  doesn't run them.

# Part B: the phone browser and the Home Screen web app (v1, kept)

Everything below is v1's own phone design, and stays the spec for wisp in a phone's browser and the Home Screen web
app (ADR-0016). Its *The app* section is v1's Android app: in the v2 apps its **features and behaviour** stay
(what opens where, Back, the FAB giving way, sharing in, cloud cards, the delete link), and Part A replaces its look.

wisp runs on a phone in two ways, and they're not the same:

| | Narrow browser (≤ 720px) | Android app (`mobile/`) |
|---|---|---|
| How to tell | CSS `@media (max-width: 720px)`, `(hover: none) and (pointer: coarse)` | JS `MOBILE` (`web/js/core/state.js`, the `wispMobile` user agent), CSS `body.wisp-mobile` |
| Layout | split view, stacked: the list, a task as a page with Back | the same, always (even on a tablet) |
| What it connects to | the wisp it's served from | *Computers* (desktop's *Machines* isn't shown) |
| First paint | the server | the service worker's cache (`web/sw.js`) and the last snapshot (`cache.js`) |

A fix for phones usually belongs in **both**: the app's rules under `body.wisp-mobile` and the
narrow browser's under the media query (the reply box's mode button is icon-only in both, #221,
#269). Desktop and wide windows must not change.

## B: Fit the width

- Check at 390px (and 360px for tight rows). Nothing may stick out of its box: Send stays inside the
  prompt box, badges stay whole, long URLs wrap.
- Labels fold to their icons before anything gets squeezed to a sliver (place, harness, mode,
  *Ship it* in the reply bar, #223). Keep the label in `title`/`aria-label`.
- When a row runs out of room, what's least important shortens first (a computer name before *All*;
  a branch name before the recipe chip), and a badge moves to the next line rather than breaking.
- Titles get two lines instead of `do a full audi…`.
- A bar with more buttons than fit scrolls sideways with Send pinned at the end (`.c-bar` in the
  app).
- A bar of 44px buttons holds buttons only. What it says (*Side question · …*, a read time) is a row
  of its own above it: one line that ends in `…` (`.c-hint`, which `composer.js` moves out of `.c-bar`
  in the app and in a browser up to 720px, `body.hint-row`). Flex text inside a bar that doesn't wrap
  squeezes into a word-wide column. Anything
  that changes with what's typed keeps its slot, so Send and the input never move sideways.
- A folded reply box is one quiet line. A line that only informs (*Cache gone cold*) folds with the
  bar, on the bar's own conditions, so it arrives with the buttons and a kept draft doesn't move. A line
  you set yourself (a send time) stays.
- A status line stays one line at any width and text size: what doesn't fit drops whole, least
  important first (the working panel's action count, then its time), and the name ends in `…`
  (`.w-status`: wrap onto a second line the box clips). A placeholder stays one line and fades out
  at the edge instead of doubling an empty box that shrinks on the first key.

## B: Touch

- In the app, touch targets are at least 44px (an invisible `::before` can grow a 32px pill's hit
  area). Buttons get a press state (`scale(0.96)`), rows an active background. In the app it shows as
  the finger goes down (`.pressed`, `web/js/ui/press.js`), not when it lifts as `:active` does: style
  both (`:is(:active, .pressed)`).
- Menus, the project list and search's rows are 44px on any touch screen (`@media (pointer: coarse)`,
  `responsive.css`), not only in the app: a phone browser's thumb is the same size.
- Search (⌘K's palette) is a sheet from the top on a narrow screen, as tall as the screen allows, so
  the keyboard never covers the end of its list.
- Menus open over the keyboard's half of the screen otherwise: `popover()` (`web/js/ui/popover.js`)
  puts the keyboard away when a button opens one, and `fitToScreen` keeps every popover inside the
  visual viewport (above or below its button, scrolling inside), again on every resize. Pick with
  `refocus(back)`, not `back.focus()`, so a touch screen doesn't get the keyboard back.
- A name the user chose (a computer, a machine) has a max-width and ends in `…` wherever it sits in
  a bar or a menu row: `Mokhless-MacBook-Pro-Studio-Workstation` must not push buttons away.
- A panel with a leading icon (the working panel's orb) keeps the icon beside its first line only:
  what's under it (a question, its answer box) takes the panel's whole width.
- An icon-only button sits centred in its 44px; its pressed or hover background is a smaller square
  (`background-clip: content-box`) whose edge lines up with the content above it. A tap leaves no
  hover background behind (`@media (hover: none)` in `responsive.css`).
- Nothing is hover-only on touch: a control that appears on hover on desktop is always shown, or
  reachable another way.
- No keyboard hints on touch screens: `kbd`, `.key-hint`, the palette footer, *Press ⌘L*, the
  *Keyboard shortcuts* entry. Wrap key text you add in `.key-hint` so the existing rules hide it.

## B: Swiping rows

- In the app only (`MOBILE`, `swipe.js`): a phone browser keeps its own sideways gestures.
- Right is *Mark done* / *Reopen*, left is *Save for later* / *Unsave*: both undoable, so neither
  asks. A spell may pick *Archive* or *Delete task* (`card.swipeRight` / `swipeLeft`); a swiped
  delete has no dialog, it waits for its Undo toast instead.
- The band under the row says the action in the same words as the menus, warms with the pull and
  fills (with a tick, `VIBRATE`) past a third of the row or on a flick. A row the action can't apply
  to (a running task) only gives a little, with no label.
- Swipes starting within 24px of the screen's edge are Android's Back; a vertical drag is a scroll.
- No nudge teaches it; a tip on the computers screen does.

## B: The app

- The app's own screen (`mobile/www/`) and the grid's device pages (`/activate`, *Device connected*)
  use **48px** targets, Android's own size (Play's pre-launch accessibility check flags less); wisp's
  page in the app keeps 44px.
- Signing in to wisp Cloud is one tap everywhere in the app (*Continue with GitHub* / *Google*);
  a grid address is only asked for behind *Use your own wisp Grid*.
- Each wisp Cloud account is one card: its picture with the GitHub or Google mark it signs in with,
  the handle, and that account (`@login` or the email) under it; never *wisp Cloud* on every row
  (a grid's name shows only when it isn't wisp Cloud). Its computers are rows in the same card.
- Where a row stands is a dot and a word under its name (*Online*, *Offline*, *Over SSH*), never a
  disabled button. *Over SSH* stays whole; the address after it ends in `…`.
- Store-required destructive paths (*Delete your account*) are a quiet `--bad` underlined link
  under *Sign out*, 48px tall, never a filled button; they open the grid's own page for it
  (`/account#delete`), which does the confirming.
- It opens on the **computers list**, never straight into the last computer. The wisp logo in the
  top bar goes back to it (a browser's logo stays the way home); the list has no Back button of its
  own, the phone's Back returns.
- **Back** closes the newest dialog, menu or open task, then leaves the computer. Anything that opens
  and should close on Back registers with `onBack(close)` (`web/js/ui/back.js`); `popover()` and
  overlays already do.
- The list fills the screen. A task opens as a page of its own (`split.js`), its head Back, the
  title (two lines at most) and the `split.head` slot; Back returns to the list where it was. It
  takes one history entry while it shows (`onBack`), however many tasks you go through on it.
- *New task* is the floating button (`fab.js`) over the list (the list's own New task row is hidden
  in the app): it opens home as a page, the prompt box focused. Sending goes back to the list, where
  the new task shows; Back leaves it, and the draft stays.
- *New task* gives way to a question's choices on a row (`rowChoices`): while any are on screen it
  fades and sinks out of reach (`fab.js giveWay`), and comes back when they go. A floating button
  that only sometimes sits on content gives way like this rather than moving. The choices read from
  the left like the open card's answers (`.opt`).
- The app starts on the list, never inside a task it had open.
- Sharing to wisp from Android's share sheet (`features/share-in.js`, `Shared.java`) is the exception: it opens
  New task, from the first paint, with it in the box and attached, without the keyboard, so Send is
  in view. Nothing starts by itself, and nothing leaves the phone before Send (its files are chips
  that upload then). *Shared from outside wisp* stays above it until it's sent: any site can post the
  Home Screen app's form too, and neither the worker nor the page can tell. With no computer open the list says it's waiting.
- Name the computer you're on with `HERE` / `here` (`state.js`), never a literal *This computer*: in
  the app it's the computer's name (*Studio*).
- Hide desktop-only actions with `MOBILE` (*Open project in editor*, the layout switch, keyboard
  shortcuts), not by width; or give them a phone version (the terminal opens inline).
- Keep `server/machines/bootstrap.js` and `server/shq.js` free of Node imports: the app runs them.
- The page is cached for good by content hash: a new module or stylesheet needs nothing extra, but
  test that a stale cached page reloads onto the new build.
- Check it with `node qa/platform.mjs` (the phone app's page, qa/phone.mjs) and the `MOB-*` scenarios in
  `qa/scenarios/mobile.md`.

## B: The iPhone Home Screen app (wisp Grid)

- On a Grid, iPhone Safari's *Add to Home Screen* opens wisp full screen. That's `STANDALONE`
  (`web/js/core/state.js`, `html.standalone`), not `MOBILE`: the Android app's code paths stay its own.
- It draws under the status bar and around the notch (`viewport-fit=cover`, set only there by
  index.html's first script). Anything pinned to an edge of the window keeps clear with
  `env(safe-area-inset-*)`, scoped to `html.standalone` so browser tabs don't move. `.stage` already
  pads for the list, the task's head and the reply box (split.css).
- No `new Notification()` there (iOS has none for home-screen apps): alerts come from ntfy.
- The manifest and Apple's tags go in from `homescreen.js` on a Grid only, so a desktop browser on
  someone's own computer is never offered to install wisp.
