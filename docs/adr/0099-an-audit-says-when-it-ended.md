# ADR-0099: A compaction audit says when it ended (`threads.audited`)

- Status: accepted
- Date: 2026-11-18
- Source: CI, 2026-11-18: `the_audit_is_off_by_default_and_on_says_what_the_compaction_lost_once` failed on Linux
  (it read the audit's result while the audit still ran in the background). Follows ADR-0096 (the compaction audit),
  D-0053 (a flaky test is fixed at its cause).

## Context

The audit runs in a task of wisp.threads' scope (`ctx.spawn`), so a probe that takes up to 2 minutes never holds the
plugin's event queue. Nothing said when it was over: `Kernel::idle` waits for events and follow-ups, not for a
plugin's own tasks, and a test that yielded a few times and then read the result passed on a fast macOS runner and
failed on Linux. Anything else that wanted to know (a channel, a UI that isn't on the task's topic) had to poll.

## Decision

- **`threads.audited { audit }`** (`wisp_threads_api::Audited`, declared in wisp.threads' manifest `events`, subject
  the task, not durable): emitted once per audit, after its final `threads_audit` row (done, or skipped with why) is
  saved. An audit that never wrote a row (no Notes to check, the task already being audited, a store error) emits
  nothing.
- Tests wait on it (a watching plugin hands each one to the test over a channel), never on time or a number of
  yields; their 30-second bound only turns a lost event into a failure instead of a hang. The fake probe answers after
  50 ms, so a test that doesn't wait fails every time instead of only on a slow machine.

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| A done audit is heard with what it kept; a skipped one with its status | `plugins/threads/host/tests/fresh.rs` `the_audit_is_off_by_default_and_on_says_what_the_compaction_lost_once` |
| A compaction a harness's hook counted is audited, heard by its end | `compactions_a_hook_counted_are_kept_on_the_turn_and_audited` |

## Consequences

- One more event type in `wisp events ls`; nothing else listens to it yet.
