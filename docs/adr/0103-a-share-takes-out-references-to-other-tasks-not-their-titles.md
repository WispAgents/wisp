# ADR-0103: A share takes out references to other tasks, not their titles

- Status: accepted (supersedes ADR-0027's "other tasks' titles and crew names" redaction)
- Date: 2026-11-18
- Source: QA round 12 #3 (`/tmp/qa-keep/qa-round12-phone-grid.md`): with a task titled "login" on the wisp, sharing a
  task whose prompt is "fix the login bug" published "The ask: fix the <task> bug".

## Context

ADR-0027 has a share take out "other tasks' titles and crew names the sharer didn't choose", so what else the sharer
works on doesn't leave with it. `wisp-share-core` did that by replacing each title and name wherever its text
appeared. A title is ordinary words ("login", "start", "config", "Ada"), so any shared text that used them lost them.
Meanwhile the real references went out: a mention chip kept the other task's id (`@[<task>](wisp:01a1…)`), and so
did a bare id.

## Decision

A share (and `wisp history export`, which uses the same scrub) takes out **where the text refers to** another task, or
to a crew member the sharer didn't choose:

- its mention chip, `@[Title](wisp:<id>)` / `@[Name](crew:<id>)`, becomes `<task>` / `<crew>`;
- its id, standing on its own (not inside a longer word), becomes `<task>` / `<crew>`.

Titles and names as words stay as written. The share's own task, and a crew member the sharer chose, are never taken
out. Each is a `name` redaction in the preview, showing the chip's title or the id. `Scrub` takes `tasks` and `crew`,
both as ids, in place of `names`. Secrets, env values and paths are unchanged.

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| "fix the login bug" stays with a task titled "login" on the wisp; its chip and its id go; the share's own chip stays; a crew chip goes and the crew name as a word stays; an id inside a longer word stays | `plugins/share/core/src/lib.rs` `other_tasks_and_crew_go_where_they_are_named_never_as_words` |
| The same through `share.preview` on a real `wisp serve`: the words stay, the other task's id never leaves (it failed on the old scrub with exactly the QA result) | `crates/wisp/tests/serve_share_redact.rs` `a_share_keeps_words_that_are_another_tasks_title_and_takes_out_a_mention_of_it` |

## Consequences

- A shared prompt can say "the payroll migration" while a task with that title exists. A title is something the
  sharer wrote in this text, not a reference; the sharer sees the preview before anything leaves.
