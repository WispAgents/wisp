# ADR-0145: a PR link in a prompt attaches the PR, and a PR's link reads as its live chip

- Status: accepted
- Date: 2026-11-27
- Source: the owner pointed at Google Antigravity's new GitHub PR feature (paste a PR link into chat, it shows
  inline; hover shows its status, branches and comments; an overview lists the linked PRs) and asked for the same
  idea built the wisp way, not a copy.

## Context

wisp already follows PRs on a task: Ship it opens them, the agent attaches them with `wisp_pr`, `ship.attach` does it
from the UI, and the row's pill, the PR card (`shell.task:details`) and the PR pane show every one of them with
checks, reviews and Merge. Two things were missing:

1. A person who pastes a PR link ("review https://github.com/acme/api/pull/12") had to wait for the agent to call
   `wisp_pr`, or attach it by hand. The link already says what the task is about.
2. In the turns a PR link was a plain blue URL. You had to open the pane, or GitHub, to see if it was green.

## Decision

1. **A PR link in a prompt attaches the PR.** wisp.ship handles `turn.started`: it reads the turn's prompt
   (`runtime.turns`, a permission it already has) and attaches each GitHub PR link in it through the same
   `Ship::attach` that `wisp_pr` uses, so the PR is followed and kept current like the others. The pure part is
   `wisp_ship_core::links::pr_links`: `github.com/<owner>/<repo>/pull/<n>` with any tail (`/files`, `#discussion…`,
   `?w=1`), with or without a scheme or `www.`, each PR once, at most 5 per message. Look-alike hosts, issues and
   bad numbers are left alone. Before calling gh it skips the PRs the task already has. A link gh can't read stays
   text and nothing is said. Turns wisp wrote itself (`by: wisp | compact`) are skipped. Channel, crew and later
   turns count, because a person wrote them somewhere. The setting is `ship.attachPasted`, **on by default**: it is
   no new integration (D-0023 doesn't apply), it only fills in what `wisp_pr` would have done. Its switch is *Follow
   PRs I link* in Settings → General → tasks (`shell.settings:tasks`).
2. **A new UI input, `shell.task:link`.** Every link in the open task's turns asks the first extension on it whose
   `if` holds for the link's `href` (a new additive `ExtensionProps.href`). If none does, the link stays exactly as
   it was. The shell provides it with `TaskLinks` around the Ledger. The kit gets the mechanism with no feature
   inside it: `LinkRenderer` (a context), which `Markdown` asks for each non-`#` link with the anchor as fallback.
   `LinkedText` does the same for a prompt's plain text, which stays text unless a renderer claims a URL in it.
   Without a renderer nothing changes, so the v1 look holds everywhere else.
3. **wisp.ship draws its PRs there: `PrPeek`**, a new kit piece. It is the PR pill's `ToneChip`, a little lower so
   it sits on the line, with a card on hover or focus: the title, the state line (`prLine`: "Open · 3 checks passed ·
   Approved"), head → base, the check counts, and "Click to open the PR beside the task". A click opens the PR pane.
   The card is the kit `Tooltip` with a new additive `card` (read-only, on a popover's surface). Nothing in it takes
   a click, so no hover-card dependency is needed. Phones have no hover: a tap opens the pane. The extension's `if`
   is `prOfLink` (`model/link.ts`, light, matched case-blind on `owner/repo#n`), so the chip shows only for the
   task's own PRs (shipped, attached or pasted). Any other PR link stays a link.
4. **The overview is the one that's there.** The PR card and the pane already list every PR of the task, so a
   pasted PR shows there like the rest. No second list.

## Where it reaches (ADR-0040)

- Inside wisp: the composer, follow-ups, the turns' chips, the PR card and pane, Settings.
- From the harness: `wisp_pr` is unchanged, and `wisp_task` / `wisp_launch` prompts with a PR link attach it, because
  they go through `turn.started`.
- From the CLI (`wisp task "…"`) and channels (Telegram, Slack, Discord, email, a `/wisp` GitHub comment): the same
  event.

## Consequences

- A pasted link costs one `gh pr view` per new PR, off the event's way (spawned), at most 5 per message, and none
  for one the task has.
- `ExtensionProps` and `Scoped` gain `href`, and `INPUTS` gains `shell.task:link`. All are additive.
- A plugin can now redraw any link in a task's turns (an issue, a Linear ticket, a deploy). Extensions are ordered,
  and the first whose `if` holds wins.
- A chip's click opens the PR pane on the task's first PR, as the card's row does (ADR-0143). Opening it on the clicked
  PR needs a "which PR" field on the side panel's navigation, a later change for both.
- `TaskLinks` hands each link's extension only `session`, `id` and `href`. Spreading the provider's props reads its
  `children` getter and draws the whole transcript again inside each link, which recursed forever while it was built.
- The web mock's `?seed=prlink` shows the chips: the prompt links two PRs and one the task doesn't have.
- Edge cases, each with a test: a link with a tail is the same PR; a markdown link, an autolink and a scheme-less
  one count; issues, look-alike hosts, `pull/0` and `..` don't; more than 5 links take the first 5; multi-byte text
  around the needle never panics; a PR already on the task asks gh nothing; a PR gh can't read is skipped; the
  setting is on unless `false`; wisp's own prompts never attach; with no renderer an answer and a prompt render as
  before; a renderer that doesn't know a link leaves the anchor.
