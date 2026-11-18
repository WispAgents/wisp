# ADR-0104: The kit styles through a class mirror of `data-kit`

- Status: accepted (refines ADR-0007's "the kit styles itself through `data-kit`")
- Date: 2026-11-18
- Source: UI performance audit round 1 (`/tmp/qa-keep/perf-ui-r1.md`) finding #5: style recalculation dominates every
  interaction (open a task 309 ms of style at 4× CPU, ⌘K 198 ms, the side panel 273 ms).

## Context

The kit's stylesheet had about 3,000 selectors, nearly all `[data-kit="…"]`. Blink buckets attribute selectors by
attribute *name* only, so every element carrying a `data-kit` (about 750 on a task page) was tried against all of them
on every style recalc; the selector-stats trace showed rules that match nothing attempted ~2,000 times per recalc.
Class selectors are bucketed by class name, so an element is tried only against its own kit's rules. An A/B on the
production build (same mock, Chrome 4×, median of 3) with the stylesheet rewritten to classes cut style time per
action by 60–80 %.

Rewriting the source would break what reads it: about 500 assertions in 27 kit tests match `[data-kit=…]` in the raw
CSS, the shared CSS rules (`packages/tokens/src/css-rules.ts`) parse kit names out of selectors, and `data-kit` is the
attribute every kit test, every `querySelector` in the shell and the readable contract use.

## Decision

The source stays as it is: components write `data-kit`, the kit's CSS selects `[data-kit=…]`. A kit-owned Vite plugin
(`packages/ui/build/kit-classes.ts`, `@wisp/ui/vite`, used by `apps/web` and the gallery) changes the bundles only:

- **JSX:** an element (or Solid's `Dynamic`) with a `data-kit` also gets `class="k-<kit>"`, static or from the same
  expression; a `'data-kit'` key in a props object gets a `class` key. Kit components (capitalised) and elements that
  already have a class are left alone. Insertions only, on the same line, with a sourcemap that keeps every column.
- **CSS:** once Vite has assembled it (the kit's sheets come through `@import`s Vite inlines without other plugins),
  `[data-kit=x]` becomes `.k-x`: the emitted CSS files in a build, the served CSS module in dev.
- **Kept as attributes:** the kits whose elements are written outside JSX (HTML strings, `setAttribute`) and so have
  no class, listed in `KEEP_ATTR` with the file that writes each. Prefix (`^=`) and presence (`[data-kit]`) selectors
  stay too.

`data-kit` stays on every element. `data-part` (the plugin contract, ADR-0007) is untouched. Unit tests (vitest) run
the source as it is.

## Consequences

- Style time per interaction at 4× (same tree, class mirror off → on): open a task 277 → 123 ms, next task 223 →
  105 ms, side panel 177 → 60 ms, task More 115 → 37 ms, ⌘K 142 → 69 ms. Screens are pixel-identical within the noise
  floor (light and dark: Home, a task, More, ⌘K).
- The entry chunk grows by 0.7 KB br (118.8 → 119.5 of 120 KB): every template carries the class. The entry CSS
  shrinks 0.2 KB br.
- A new imperative `data-kit` writer must be added to `KEEP_ATTR`, or its rules stop matching in the bundles while
  tests still pass: `packages/ui/test/kit-classes.test.ts` scans the kit, the shell and the plugin UIs for
  `setAttribute('data-kit', …)`, `dataset.kit =` and HTML strings, and fails on an unlisted kit (and on a listed one
  with no writer left).
- A Vite config that builds the kit must use `kitClasses()`; without it the app still renders correctly (every element
  keeps `data-kit` and the CSS stays `[data-kit=…]`), only slower.

## Edge cases (each with its test, `packages/ui/test/kit-classes.test.ts`)

| Edge case | Test |
| --- | --- |
| A static `data-kit` | `kit_classes_static` |
| A conditional or template expression | `kit_classes_expr` |
| `Dynamic` and SVG elements | `kit_classes_dynamic_svg` |
| A kit component, an element with its own class | `kit_classes_skip` |
| A props object spread onto an element | `kit_classes_object` |
| Columns after an insertion still map back | `kit_classes_map` |
| Quoted, unquoted, inside `:where()` / `:not()` | `kit_classes_css` |
| `KEEP_ATTR` kits, `^=` and presence selectors | `kit_classes_keep` |
| Every kit sheet leaves only `KEEP_ATTR` kits as attributes | `kit_classes_sheets` |
| An imperative writer not in `KEEP_ATTR`, or a stale entry | `kit_classes_guard` |
