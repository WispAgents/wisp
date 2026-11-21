# ADR-0092: An unjudgeable timed budget takes more rounds, up to a cap, before its verdict

- Status: accepted
- Date: 2026-11-16
- Source: docs/qa/bugs.md B24 (CI red on runner noise). Amends ADR-0089 (how many rounds); the budgets
  (`tools/budgets.ts` `SERVER_BUDGETS`, ADR-0011), the calibration, the 1.5× line and the verdicts are unchanged.

## Context

ADR-0089 takes a fixed number of rounds (cold start and the list 5 each, patch latency 3), leaves a disturbed round out
while the clean rounds are most, and calls a run over budget with most rounds disturbed *too noisy to judge* (exit 3,
red). On the shared Linux runners that verdict has become the usual way the step fails:

| run | the list's rounds (ms, † disturbed) | calibration, worst | verdict |
|---|---|---|---|
| 780f67fc | 5.96† · 3.62† · 5.82 · 4.17† · 7.22† | 45.4× | too noisy (list) |
| d58d7621 | 5 of 5 † (median 5.38) | 15.1× | too noisy (list); patch p99 155 ms, 2 of 3 † |
| cac4bec1 | 4.51† · 3.99 · 2.27 · 4.28 · 4.44† | 32.7× | pass |

Local runs of the same build: the list 2.5–3.0 ms, patch p99 ~104 ms. The verdicts are right (a slowed runner can't say
wisp is fast), but a run that stops after 5 rounds when none was clean judges nothing and costs a full CI rerun, which
is as likely to land in another slow spell. The slow spells last seconds to a minute (the calibration's spread over
one job), so the cheap answer is to keep measuring until the runner is quiet, inside the same job.

## Decision

1. **More rounds while a budget can't be judged** (`Rounds::wants_more`): cold start and the list keep taking turns
   after their `ROUNDS` (5) while either one's rounds so far are over budget with most of them disturbed, up to
   `MAX_ROUNDS` (15). The stream takes its unpinned rounds the same way (at least 2, up to 5) before the last, pinned
   one (write amplification), so at most `MAX_PATCH_ROUNDS` (6).
2. **The judging is ADR-0089's, over every round taken**: the median of the clean rounds while they are most, a pass is
   a pass, over budget with clean rounds most is a miss at once (no more rounds to wait it out), and still disturbed at
   the cap is *too noisy to judge* (exit 3), never a pass. "Disturbed" is against the job's fastest calibration as it
   stands when the round is judged, so a round can turn disturbed once a quieter calibration comes.
3. The table shows every round taken (the count is in each row), so a run that needed 9 rounds says so.

## Consequences

- A step that hits a slow spell takes longer (a cold/list round ~2 s, a stream round ~12 s): at worst ~20 s more for
  cold and the list and ~40 s for the stream, against a rerun of the whole job (~25 min).
- A real regression is still caught at once: it slows the clean rounds too, so they are most and over budget (a miss).
- A runner slow for the whole job still ends *too noisy to judge*, red, as before.
- Not a retry of a test: every round is kept and shown, the verdict reads all of them.

## Places it reaches (ADR-0040)

PR CI's *server budgets* step and anyone running `spikes/server-budgets` by hand. Nothing users run.

## Edge cases and their tests

- The fixed rounds first; five disturbed rounds over budget ask for one more; clean rounds that come and outnumber the
  disturbed ones end it (here a pass); still disturbed at the cap stops and stays *too noisy*; over budget on a quiet
  runner is a miss at once: `spikes/server-budgets/src/main.rs`
  `an_unjudgeable_budget_takes_more_rounds_until_a_clean_majority_or_the_cap`.
