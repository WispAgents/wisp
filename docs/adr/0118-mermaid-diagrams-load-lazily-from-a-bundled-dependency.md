# ADR-0118: ```mermaid fences draw as diagrams, from a bundled mermaid loaded lazily at securityLevel 'strict'

- Status: accepted
- Date: 2026-11-25
- Source: the render audit, round 2 (item `user-mermaid`): agents write ```mermaid fences in answers and docs, and
  wisp showed them as C-highlighted code.

## Context

Agents (Claude Code, Codex, Cursor…) answer with ```mermaid fences for flows, sequences and state machines. The kit's
Markdown (`packages/ui/src/content/markdown`) showed every unknown language as code under the C-like rules, so a
diagram read as a wall of arrows; the docs site (`apps/docs`, Starlight) did the same. Drawing a diagram needs a
layout engine: mermaid is the de facto one (GitHub, GitLab, Obsidian and the agents themselves render it), and
writing our own is out of scope.

mermaid is large: at 12.3.0 its core is 133.9 KB br in the web build, with each diagram type, its layouts (dagre,
elk, cytoscape) and KaTeX as further chunks (elk alone 347.2 KB br). The first screen is gated at its ratchet
(ADR-0105) and only goes down, so none of it may load with the first screen. Answers are agent-written, so the source
is untrusted input; mermaid's default `securityLevel` ('strict' in recent versions, but configurable per diagram
through `%%{init}%%`) and its `htmlLabels` put HTML from the source into the page if left loose.

## Decision

1. **A bundled dependency, never a CDN.** `mermaid` is an exact-pinned (`12.3.0`) npm dependency of `@wisp/ui` and of
   `@wisp/docs`. No page fetches it from anywhere but its own origin, so it works offline, in the desktop app, in the
   phone apps (Capacitor serves the bundle) and behind a strict CSP. A version bump is its own PR.
2. **Lazy, once.** `content/Mermaid.tsx` imports `mermaid` through one module-level promise on the first diagram's
   mount; a page with no diagram never loads it. `tools/budgets.ts` gives mermaid's core chunk its own ceiling
   (`LAZY_CHUNK_BUDGETS`, 140 KB br) and fails the build if any look's first screen (or index.html) loads it; every
   other lazy chunk is held under `LAZY_CHUNK_MAX_BR` (360 KB br).
3. **Only a closed fence draws.** The Markdown AST's code block carries `open` (a fence with no closer yet: a
   streaming answer). An open fence stays code; a closed ```mermaid fence (any case) becomes `<Mermaid>`. A
   half-written graph never errors mid-stream, and the component keeps its node while the source changes, redrawing
   after 150 ms of quiet.
4. **Strict.** `initialize({ startOnLoad: false, securityLevel: 'strict', htmlLabels: false, suppressErrorRendering:
   true, … })`: no click handlers, no HTML labels, the output sanitised by mermaid (DOMPurify). That sanitised SVG is
   the one piece of markup the kit inserts as markup (`innerHTML` on `[data-kit=mermaid-svg]`); the Markdown source
   itself still never becomes HTML (ADR-0006's rule for content stays). A source that fails shows as code with a
   muted note, "Diagram couldn't render: <first line of the error>".
5. **Themed.** The theme follows `currentScheme()` (mermaid's `default` or `dark`) and the frame tokens
   (`readTheme()`: surface, ink, line, muted, font) as `themeVariables`; `watchTheme()` redraws on a theme or scheme
   change and is disposed with the component. SVGs are cached by source, scheme and tokens (64 entries).
6. **A bar on every diagram:** copy the source (the blocks' copy button), View code / View diagram, and Open full size
   in the existing lazy media viewer (zoom with the wheel, + and −, Escape closes). Each is a labelled button reachable
   by keyboard; every string is a `block.mermaid.*` key.
7. **The docs site** draws the same fences with a small island (`apps/docs/src/mermaid.ts`, loaded from `Head.astro`)
   that finds Expressive Code's `pre[data-language="mermaid"]` and only then imports the bundled package, with the
   same strict config and the site's light/dark theme. Astro 7's default Markdown processor runs no remark plugins
   without `@astrojs/markdown-remark`, so no build-time rewrite: until it draws, and if it can't, the fence stays a
   code block with its copy button.

## Where it reaches (ADR-0040)

Everything that renders Markdown through the kit: the transcript's messages, answers (`markdown` blocks and any
block's Markdown text), the `doc` block, `callout`, Compare's columns, My Stuff windows, RemoteView, and the phone apps
(Capacitor, same bundle); and the docs site. An agent needs no new tool or block type: a ```mermaid fence in any reply
is enough, and in a harness that doesn't draw it, it stays readable text.

## Consequences

- The repo carries mermaid and its dependencies (d3, dagre, cytoscape, elk, KaTeX, DOMPurify) in the lockfile; they
  load only on a page with a diagram. Their licences: MIT (mermaid, cytoscape, dagre-d3-es, KaTeX), ISC (d3),
  MPL-2.0 or Apache-2.0 (DOMPurify), EPL-2.0 or GPL-3.0 (elkjs, shipped unmodified under EPL-2.0); listed by
  `package-lock.json`, as THIRD-PARTY-NOTICES.md says code dependencies are.
- A diagram's first draw waits for the chunk (a fixed-height skeleton holds its place, so nothing jumps).
- `%%{init: …}%%` directives in a source can change the look but not `securityLevel` (mermaid refuses `secure` keys
  from a diagram).
- Tests: `packages/ui/test/mermaid.test.tsx` (mermaid mocked: an open fence never imports it, a closed one draws,
  a failure falls back to code and the note, a theme change redraws in the other theme, the debounce, the bar),
  `packages/ui/test/markdown.test.tsx` (`open`), `tools/test/budgets.test.ts` (the ceilings and the first-screen rule).
