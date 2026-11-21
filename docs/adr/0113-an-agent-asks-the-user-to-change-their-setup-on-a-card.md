# ADR-0113: an agent asks the user to change their setup, on a card

- Status: accepted
- Date: 2026-11-20
- Source: owner's delegate, 2026-11-20 (owner-list row 66): v1's `wisp_crew_add`/`_edit`/`_remove`, `wisp_connector_add`
  and adding a project are v1 features (D-0049) the owner's own crew uses; refusing them silently lost them. Hardened
  after review r194 (row 66, findings 1–8).
- Supersedes: ADR-0112's consequence that these tools are refused to an agent (its first bullet under *Consequences*,
  and "`wisp_connector_add` from a task no longer reaches the user's card"). ADR-0112's decisions 1–6 stand: an agent
  never raises a mode, its capability set still denies the setup writes themselves, and nothing it wrote runs unasked.

## Context

ADR-0112 took the user's setup out of an agent's capability set: a probe showed an agent storing `sh -c id` as a
connector, which every later task then started. It also left the agent no way to do what v1 let it do with the user's
say-so. v1 asked the user before a local connector (`mcp-ask.js connectorTool`: "Add it" / "Don't", in every mode); its
crew tools wrote at once, and it had no "allow always" for any of them.

## Decision

1. **A setup change an agent wants is a card the user answers.** `runtime.setup.ask { id, command, args }` puts a card
   on the agent's task (`ext["wisp.runtime"].ask` with `setup`, which the task page, the phone and channels show) and
   the task waits. `command` is one of `connectors.agent.add`, `crew.agent.add`, `crew.agent.edit`,
   `crew.agent.remove`, `runtime.projectAdd`; anything else is refused before a card is made.
2. **The card is the write.** The server first makes the ask into the exact writes that would run
   (`wisp_runtime_core::setup::normalize`): only fields the card can show (any other field is refused: nothing unseen
   is stored), defaults filled in as they'd be stored (a member's folder), a connector either a URL (remote) or a
   command (local), never both, with a `transport` that agrees, key names only. A crew `from` a link or a pasted file is
   read first (crew's own dry run) and becomes the import of that read bundle: Allow imports exactly the members the
   card listed, with no second fetch. The card (`setup::card`) is built from those writes alone: who asks, what changes,
   every field that will be stored one a line, every command verbatim, and the whole write in Show all (kit bodies,
   briefs, a shared crew's members). The writes, the card and a SHA-256 digest of both are stored together.
3. **An answer names its card, and nothing replaces an open one.** `runtime.answer { id, text, askId }`: an answer to a
   question that isn't the open one is refused, and a setup card is answered only with its id (the card's buttons,
   a channel's and the phone's signed taps carry it). While a setup card is open, a new question or approval on that
   task is refused, and a follow-up queues behind the card (it stays) and goes on once the card is answered.
4. **Allow runs exactly what was shown.** On Allow the stored writes must still give the card that was shown and its
   digest; then they run, under the task's lock (a second Allow finds nothing asked), with wisp.runtime's own call,
   whose manifest grants just these writes (`crew:write`, `connectors:agent.add`; `runtime.projectAdd` is its own). A
   typed answer is a Deny with the user's words. No Allow always (v1 had none).
5. **Who answers is an allow-list.** The user's own page and harness (`user:*`); a grid box's own page (`box`, its
   owner: the box's wisp is theirs); a plugin that declared `runtime:setup.answer` word for word in its manifest
   (wisp.channels, wisp.push: they relay the user's own tap; `runtime:*` doesn't count); an API key's set only when the
   owner listed it in `runtime.setupAnswerSets` (none by default). Never an agent, a crew member's set, an app pane or a
   preview.
6. **A channel answers only for the linked person.** A channel answers a setup card only when its transport checked the
   tap came from the person linked there (`ActArgs.person`): Slack and Discord gate each press by the presser's own id;
   Telegram keeps the id of whoever linked the chat and checks Telegram's `from` (a private chat's id is its person's);
   ntfy can't tell who tapped, so its taps never answer a setup card. Elsewhere the reply says to answer it in wisp.
7. **Every mode asks, Full auto included**; Full auto covers the task's own work, not the user's setup (v1 asked for a
   local connector in every mode). A capped task (a channel's) gets no card: v1 offered it none of these tools.
8. **The agent reads the outcome plainly**: what the write said once allowed, that it didn't go through and why, or
   that the user said no, with their words. The MCP tools wait as `ask_user` does; a raw call with the agent's token gets
   `{ askId, text }` and reads `runtime.pollAnswer`. Calling a setup write itself with the token stays refused, and the
   refusal says to ask on a card.

## Threat model: what the card stops, and what it doesn't

wisp's local server treats a request with no token on its loopback port as the user (the page, the CLI, `wisp mcp`).
An agent that can run a shell as the user on that computer can make such a request, or edit the user's files, or run
`wisp` itself: a card is **not** a boundary against a hostile agent that has a shell on the user's machine, and neither
is any other check in wisp. The boundary for that is a sandbox.

The card does stop:

- an agent changing the user's setup off-plan **through wisp's tools and its own token** (MCP or raw API): nothing runs
  without the user's Allow on the card that shows it, and the agent can't answer the card;
- an agent **without a shell**: a Plan task (Claude Code's Plan runs no command; Codex's is read-only), a task whose
  every command waits on the user's approval card (Claude Code's Edits; the command is shown before it runs), a
  channel-started capped task (never Plan-that-runs-commands, `plan::runs_commands`; held to Plan or Edits);
- a **sandboxed** agent: it runs on its own grid machine (ADR-0047) whose wisp is that sandbox's, so a request with no
  token there reaches only the sandbox's wisp, never the user's computer; Codex's own sandbox runs with
  `network_access=false` outside Full auto;
- a **remote** agent (another machine, a channel, the phone): it reaches this wisp only with a token or a signed tap,
  never as the loopback user.

It doesn't stop an agent in Full auto (or Plan that runs commands, or a harness whose shell isn't gated) from using
its shell as the user. Other harnesses' Edits modes (opencode, Cursor, Grok, Copilot, Antigravity) gate commands with
their own permission prompts; wisp doesn't verify each here. SECURITY.md says this in the user's words.

## Places it reaches (ADR-0040)

The task page's card (shell-task `AskCard`: headline, lines, commands in mono, Show all, Allow / Deny), the phone and
the channels (the ask's question carries the headline and every line; buttons carry the card's id), the agent's MCP
tools and its raw token.

## Consequences

- `runtime.setup.ask` joins the agent's writes (`serve_mcp_tools::an_agents_capability_set_writes_only_its_own_work`).
- wisp.runtime's manifest grants `crew:write` and `connectors:agent.add`, used only from a setup card's Allow.
- `runtime.answer` takes `askId` (additive); `crew.import`'s review returns the `bundle` it read (additive).
- A channel tap now answers with its option's label (it sent an empty answer before) and names its question.

## Tests

- `plugins/runtime/core/src/setup.rs`: `setup_card_connector`, `setup_card_url_and_command`, `setup_card_crew_fields`,
  `setup_card_import`, `setup_card_crew_project`, `setup_card_refusals`, `setup_card_answers`.
- `plugins/runtime/host/src/more/setup.rs`: `setup_answerers` (the allow-list), `setup_card_digest` (changed writes run
  nothing).
- `crates/wisp/tests/serve_mcp_tools.rs`: `an_agents_setup_change_is_a_card_the_user_allows_or_denies_never_the_agent`,
  `…_review_1` (the card's id, nothing replaces it), `…_review_2` (the URL card is the stored write), `…_review_4`
  (every field, `from` read first), `…_review_7` (a follow-up waits behind the card).
- `plugins/channels/host/tests/answer.rs` (a tap names its question; a setup card only for the linked person),
  `plugins/telegram/host/tests/messages.rs` (`a_tap_is_the_linked_persons_only_…`), `plugins/push/host/tests/push.rs`
  (`a_notification_answer_lands_once`: the question's id).
- `plugins/shell-task/ui/test/setup-card.test.tsx`: the card's model, what it shows, Show all, the answer's `askId`.
