# ADR-0067: The first page loads only what the first screen draws

- Status: accepted
- Date: 2026-11-14
- Source: the web app's initial JS was 230.6 KB brotli after round 219 (`npm run build --workspace @wisp/web && npm run
  budgets`), against ADR-0011's 120 KB. Measured from the entry's module graph (rollup's rendered sizes, then brotli):
  English strings ~49 KB, Zag + floating-ui ~42 KB, plugin index modules and what they import ~43 KB, the rest the
  shell frame, the kit, Solid and the client.
- With ADR-0011 (performance budgets), ADR-0017 (every action answers within one frame), ADR-0003 (plugins, no
  private API), ADR-0007 (the shell contract).

## Context

- Every plugin's `index.ts` is in the entry (apps/web's eager glob). Its components were already `lazy()`, but the
  callbacks around them were not: a row menu item's `if` / `text` / `select`, the palette's sources, prompt sources,
  side tabs. Each pulls its plugin's data model and commands in, though the first screen (the task list and Home)
  calls none of them.
- Every catalogue was eager: ~3 200 plugin strings and 634 shell strings, most of them for dialogs, panes and the
  task page.
- The kit's overlays (Menu, Select, Dialog, Toast, Checkbox) imported their Zag machines at module level, and with
  them floating-ui, focus-trap and the dismissable layer, although the first screen opens none of them.

## Decision

1. **A plugin UI may declare `later`** (`PluginUiDef.later?: () => Promise<{ default: PluginUiLater }>`, additive in
   `packages/shell/src/contract.ts`). `PluginUiLater` holds only what a press reaches: `extensions` (a row menu's
   items, say), `palette`, `shortcuts`, `prompt`, `sideTabs`. The shell loads every `later` as the first screen
   renders (`shell.render`; a plugin registered after that loads its own at once) and joins it to the same plugin in
   its place (`Registry.extend`): its scope, its client, its extensions after its own. A plugin switched off before
   its part lands gets none of it. Views, Settings panes, harnesses, block renderers, the catalogue, `setup` and
   whatever the first screen draws (a row's pill, a list section, Home's cards) stay in the definition: a link or the
   last place must find its view or pane at start. Built-ins get nothing more than any plugin: it's the same field.
   A prompt box's send waits for parts still on their way (`ShellContext.laterReady()`, additive; a part whose load
   failed is tried again), so `/goal …`, `@member …` or `tomorrow 9am …` sent as the page loads still reaches its
   prompt source; it waits once, then goes. What reads registrations does so reactively (Ship's unsent-comments count
   over the prompt sources as they are, not as they were at setup).
2. **Strings load with the code that shows them.** Each package's English strings are split into the ones its
   first-page modules use (`i18n/en*.json`, the shell's `i18n/en.ts`) and the rest (`i18n/en.lazy.json`).
   `i18n/lazy.ts` adds the rest to the package's catalogue object when imported, and every module that uses one of
   those keys imports it for its effect (`vite.config.ts` `moduleSideEffects` keeps those imports, as it keeps the
   kit's `*styles.ts`), so the strings land in that module's chunk and `t()` answers synchronously once the code
   runs. The shell registers a catalogue by reference (a proxy for one with full-id keys), so strings added later
   show through `client.i18n` too. A key a first-page module, another package, the server or a run-time-built key
   (a template prefix) may use stays eager; a new key is eager until moved. Key types include both halves.
3. **Overlays draw at once and bind their machine when it's there** (`packages/ui/src/controls/lazy-part.ts`). Menu,
   ContextMenu, MenuOn, Select, Dialog/Sheet, Checkbox and the toaster render their trigger or surface with plain
   props (roles, `aria-expanded`, Escape and × for a dialog, a working native checkbox) and their Zag machine module
   loads apart. The app preloads every machine once the first screen is up (`preloadControls()` on idle); a trigger's
   first hover, focus or press fetches its own. A press before the machine shows the control pressed in the same
   frame (`aria-expanded`, which the kit styles) and is replayed on the same element once the machine binds, so it
   does what it would have (a click opens, ArrowDown opens on the first item, a right-click opens at the pointer); a
   trigger that already had focus is told so. Toasts wait for the store the same way: `show()` returns its id at
   once. AutoPop and the runner picker load the same way (`lazyView`, a lazy component under its own `Suspense`).
4. Smaller moves: ShipDialog's params, the goal's and ship's view ids and worktrees' Done view id moved to small
   modules (a module imported statically and dynamically stays in the entry); the remote UI host, Settings' frame and
   the notifier load on first use.

Result: initial JS **230.6 → 117.9 KB br** (raw 892 → 438 KB). The 120 KB budget is unchanged.

## Edge cases (each with its test)

| Case | Behaviour | Test |
| --- | --- | --- |
| A plugin's `later` before and after the first render | not there before; joins after, after the plugin's own extensions | `packages/shell/test/later.test.tsx` `a_plugin_s_later_part_joins_it_once_the_first_screen_draws_after_its_own_extensions` |
| Registered after the first render; switched off before its part lands | loads at once; the switched-off one gets nothing | same file, `a_plugin_registered_after_the_first_render_loads_its_later_part_at_once_and_one_switched_off_first_gets_none` |
| `extend` on the registry | in the plugin's place among plugins; unknown plugin: no change; removed with the plugin | `packages/shell/test/registry.test.ts` `a_later_part_joins_its_plugin_in_place_ADR_0067` |
| Strings added to a catalogue after registration, with full-id keys | both halves answer the new keys | `later.test.tsx` `strings_a_plugin_s_lazy_code_adds_to_its_catalogue_show_through_the_client_in_both_halves` |
| Every plugin's `later` | loads; holds only the allowed fields; ids in its namespace | `apps/web/test/plugins.test.tsx` |
| A key in both halves; a lazy half without its loader; a module naming a lazy key without importing it | none | `apps/web/test/strings-split.test.ts` |
| `lazy.ts` kept by the build, other i18n modules droppable | as stated | `apps/web/test/side-effects.test.ts` |
| Menu pressed before its machine | `aria-expanded` at once, opens as it lands, the machine owns the trigger after | `packages/ui/test/lazy-controls.test.tsx` |
| Select: ArrowDown before its machine, trigger focused before it | the list opens with the keyboard | same file |
| Dialog open before its machine | its role, title and body at once, focus inside; Escape and × close it | same file |
| Checkbox before its machine | a working native input; the machine binds the same box | same file |
| Right-click before the menu's machine | the browser's menu doesn't open; the menu opens at the pointer | `packages/ui/test/lazy-context-menu.test.tsx` |
| A toast and its dismiss before the store loads | the toast shows once it does, the dismissed one doesn't | `packages/ui/test/controls-toast.test.tsx` |
| A send while a `later` part is on its way, or after its load failed | the send waits for it (the failed one tried again), then goes through the prompt sources | `later.test.tsx` `laterReady_waits_for_parts_on_their_way_and_tries_a_failed_one_again_so_a_send_reaches_its_prompt_source` |
| Ship or Merge set up before shell-task's comments source lands | the count includes it once it does | `plugins/ship/ui/test/unsent-late.test.ts` |
| Mark done before the worktree watcher's module lands | asked about; one restored in flight at start isn't | `plugins/worktrees/ui/test/watch-late.test.ts` |
| A row menu opened before the `later` parts land | the menu opens without those items and they appear in place (the parts load as the first screen renders, within ~100 ms on a local wisp) | not tested (timing) |

## Consequences

- A plugin author keeps first-screen work in `index.ts` and puts press-only registrations in `later`; the budget
  (`npm run budgets`, CI) says when the entry grows again.
- Moving code out of the entry can leave strings eager that could now be lazy: the split is re-derived from the
  build's module graph (which modules are in the entry) and only ever errs towards eager.
