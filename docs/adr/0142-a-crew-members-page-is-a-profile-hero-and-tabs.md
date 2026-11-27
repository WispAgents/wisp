# ADR-0142: a crew member's page is a profile, a hero and tabs

- Status: accepted
- Date: 2026-11-27
- Source: owner, 2026-11-27: "redesign the crew members to look more premium, check other bots apps in the market to
  get inspired by, don't copy". Three mocks were shown (D-0056: `design/mockups/crew-premium-{A,B,C}-*.png`); the
  owner picked B.
- Supersedes, for the crew member's page and the rail's faces only: ADR-0016 ("v2 looks exactly like v1").

## Context

v1's member page was a task page with a side column: a 60 px head (small face, name, role, a status line), the thread
of jobs, and the facts, Notebook, Brief and kit down the side (folded above the thread when the pane was narrow). It
read like a log, not like a teammate. Agent and bot apps present an agent as someone with a profile: Linear's agents
have a full profile marked as an app user, Relevance AI gives agent teams pictures, and GPT-store-style pages lead
with a large avatar and suggested first prompts. Background-agent apps (Devin, Cursor) show what the agent is doing
right now. The mock took ideas from these and copied none of them.

## Decision

1. **The hero.** The page opens on the member: their face at 82 px on a disc in their colour's glow, the name at
   28 px, the role, then chips: what they're doing (the status line, pulsing while a job runs, amber while they ask
   you), their harness (logo), what wakes them (while busy, when the status chip doesn't already say it), their
   folder. The harness, cadence and folder chips open the member editor.
2. **Numbers, only real ones.** Under the chips: jobs done, jobs this week, how long a job takes (the mean of each
   run's `endedAt − startedAt`), and who they hand work to. Each is left out when nothing backs it, so a new member
   shows none. No number is invented (no "PRs merged": nothing records it per member).
3. **Tabs instead of the side column.** Activity (the thread, as before), Kit (with its count), Notebook, How they
   work (the facts, Check in / Edit / Remove, the brief). A panel is drawn only while its tab is open. The data-parts
   plugins restyle stay: `crew.page`, `crew.thread`, `crew.compose`, `crew.details` (How they work), `crew.notebook`,
   `crew.brief`, `crew.facts`; new ones are `crew.hero`, `crew.chips`, `crew.stats`, `crew.tabs`, `crew.suggest`.
4. **Again, above the box.** The last three prompts you gave the member (distinct, newest first) are chips above the
   job box on Activity; a click puts one back in the box and focuses it, so nothing is sent without Enter.
5. **The rail rings a busy member.** A face on the rail gets a ring of its colour while one of its jobs runs (the
   shell reads it from the task rows' `wisp.crew` note; no new data).
6. The head keeps only its buttons (back, Pause, More, Close) over the hero's glow.

## Consequences

- `qa/parity.mjs` no longer compares the member page or the rail's faces with v1: `design/v1-reference/allowed.md`
  §5 names both (`crew.page`, `crew.avatar`). Everything around them is still checked.
- The phone apps' native member page (`CrewPhone.tsx`, ADR-0065) is unchanged; a phone browser gets the hero page,
  which narrows under 520 px of its own width (smaller disc and name, the tab row scrolls).
- Kit: `CrewView` takes `color`, `chips`, `stats`, `tabs`, `activity`, `tabsLabel`, `suggestions` and no longer takes
  `state`, `side` or `about`; `Face` has a `hero` size; `RailMember` has `live`. Only the crew plugin and the shell
  use them.
- Where it reaches (ADR-0040): the page is UI only. The numbers come from data the MCP tools (`wisp_crew`,
  `crew_stats`) already return, so an agent sees the same facts.
