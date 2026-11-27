# ADR-0141: the crew editor is a hero with four tabs

- Status: accepted
- Date: 2026-11-27
- Source: the owner asked for a premium Add to your crew modal in the language of the member page they picked (the
  hero with tabs). Mockups A, B and C are in `design/mockups/crew-add-*.png`, and the owner picked C (D-0056).
- Changes: ADR-0016 (v2 looks exactly like v1) for one dialog, the crew editor (`crew.editor`, Add to your crew,
  Add this task to your crew, Edit crew member). v1's fields, values and behaviour all stay; only their layout changes.

## Context

v1's editor is one long column: Start from chips, the face with Name and Role inline, Their job, Checks in, Also
starts when, Task panel, the bar, then the foot. At 1440×900 the form runs past the dialog's 86vh, so the action
button sits below the fold, and nothing tells you what you're making: the member only takes shape once it's saved.

## Decision

1. **A hero is the dialog's head.** The view is `bare`. The editor draws its own eyebrow (the dialog's title, from
   `SurfaceTitle`) and ×. The spirit sits in a glow of its colour (`data-color` → `--k-pc`, over a faint dot grid),
   with the dice that rolls or opens the maker. The name is typed in large text into a bare input, with the role
   under it and the `@handle` line as a pill. Name stays `required`, so an empty Add focuses it from any tab.
2. **Four tabs under the hero**, numbered and ticked once they say something:
   - **Who**: Start from as spirit cards, the spirit maker and the Task panel.
   - **Their job**: the brief, in a larger box.
   - **When**: Checks in, Also starts when, and First check-in now.
   - **Where & how**: v1's bar drawn as a card of rows.

   The open tab is local state (`createSignal`). It never resets the draft, which stays the caller's.
3. **Every tab can be skipped.** The foot stays in view at a fixed dialog height (`min(44rem, 86vh)`). It has Back,
   Cancel, Next: <tab> and the action. The action saves from any tab once there's a name.
4. **Narrow screens**: under 520px, Start from is two columns of compact rows and Cancel is hidden (× stays).
   Under 400px, only the open tab shows its name (the others show their number or tick), and Next shows only its
   arrow, labelled by `aria-label`.
5. Kit-only (`packages/ui/src/crew/Editor.tsx`, `css/crew-editor.css`). The plugin passes the tab labels
   (`steps`) and keeps its commands, optimistic saves and rollbacks as they were.

## Consequences

- `qa/parity.mjs` no longer matches `design/v1-reference/crew.add` for this dialog. The crew editor is listed as an
  intended difference, and its parts (`crew.editor`, `.name`, `.role`, `.brief`, `.cron`, `.events`, `.tint`,
  `.bar`, `.foot`, `.templates`) keep their names. New parts are `crew.editor.hero` and `crew.editor.tabs`.
- A field that used to be on screen at once is now one tab away. The ticks and Next show where it went.

## Reaches (ADR-0040)

UI only: the same `crew.create`/`crew.update` commands that `wisp crew`, the MCP tools and `wisp.yml` use.
