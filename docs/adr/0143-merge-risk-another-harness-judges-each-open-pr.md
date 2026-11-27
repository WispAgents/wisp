# ADR-0143: merge risk, another harness judges each open PR

- Status: accepted
- Date: 2026-11-27
- Source: the owner asked for "merge risk" on a task's PRs, whether Ship it opened them or the agent attached them.
  It comes from another agent judging the risk. Settings can turn it off, and it is off by default (D-0023). The UI
  works with several PRs. After seeing the first build, the owner picked the card's layout: a row under the Steward,
  built the same way (D-0056).

## Context

Review by another harness (wisp.review) reads the task's own PR and lists what it would fix. It doesn't say how
risky merging is, and it reads only one PR. A task often has several open PRs, across repos, some attached with
`wisp_pr`. Before Merge, the user wants one judgment per PR: low, medium or high, and why.

## Decision

1. **A new plugin, `wisp.risk`** (`plugins/risk/{api,core,host}`), with its own setting `risk.harness`: `off`
   (default), `other` (a harness that didn't write the task), or a harness id, the same values as `review.harness`.
   It owns the task's `ext["wisp.risk"]` slot: `{prs: [Verdict]}`, one verdict per PR URL, of one head
   (`running | done | failed`, a level, a 0–100 score, a one-line summary, up to 8 reasons, who judged, whether the
   diff was cut).
2. **What it judges.** It judges every open PR with a head in wisp.ship's slot, in the repos' order first and the
   attached ones after, each URL once. A head with no verdict is due. A push makes a verdict stale, and that head is
   judged again. `force` (Judge again) judges a head that was judged already, but never starts a second run of a
   head being judged. A run still `running` 7 minutes after it started reads as failed and is due again, so no boot
   sweep is needed. The heads are claimed in one compare-and-set (`update_ext`). An answer lands only on the run it
   belongs to (same URL, same head, still running).
3. **How.** For each PR, one after another (never one judge per PR at once on the user's account): a new read-only
   ship command `ship.pr_diff {session, url}` (`gh pr diff`, only for a PR the task has) gives the diff. It is cut at
   60 000 characters, at a line's end, and the prompt says so. The judge is then asked with `runtime.quick` (JSON, 5
   min timeout) on its current account, never another. Its answer is read whether it is JSON or words around a JSON
   object: with only a score, the level comes from the score.
4. **When.** `pr.opened`, a turn's end (`turn.ended`), ship's slot writer seeing a new head on any open PR
   (`slot::new_open_heads`, asked after the slot is written), and by hand (`risk.judge` with `force`, the PR pane's
   Judge again). Not while a turn runs, unless forced. A subagent's PRs are judged with its lead's.
5. **Where it shows.** It shows only while the setting is on. The row has `riskOn` and `risk` (`risk.view`'s
   entries).
   - **The PR card:** a *Merge risk* row under the Steward's. It has the worst open PR's verdict in a line ("High ·
     sync-lib #44: …", a high outweighs one still being judged) and the level on the right ("High · 2" when two share
     it). Clicking it opens the PR pane.
   - **Each PR's chip** adds its own word ("#44 · high risk").
   - **The PR pane:** a Merge risk section for the picked PR, with its level, score, line, reasons, who judged, a
     stale or failed state, and Judge again.
   - **Merge's confirm:** a PR judged high turns it red (Merge anyway), and one judged medium adds a line.
   - **Settings → General:** *Merge risk*, next to Review by another harness.
6. Every surface reaches it: `risk.judge` and `risk.view` are MCP tools like every command (ADR-0040), and
   `risk.judged` is a durable event.

## Consequences

- wisp.ship depends on `wisp-risk-api` (it asks `risk.judge`), and wisp.risk depends on `wisp-ship-api` (`PrOpened`,
  `ship.pr_diff`). Both are api crates, so there's no cycle (the review pattern).
- A push to an open PR costs one quick judge run per PR whose head moved, on the judge's account. The setting is off
  by default.
- Clicking the card's row opens the pane on the task's own PR first. Opening it on the riskiest one needs a "which
  PR" field on the pane's navigation (a later change).
- Edge cases, each with a test: a merged PR or one with no head isn't judged; an attached copy of the task's own PR
  counts once; an older head's answer is dropped; a closed PR's verdict goes at the next start; a run cut short is
  due again; every harness wrote it (`other` has none); unset is off; a non-JSON answer is not a verdict; a long diff
  is cut at a line's end.
