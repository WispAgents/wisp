# ADR-0066: Harness detection lands on a topic, one row per harness as it settles

- Status: accepted
- Date: 2026-11-14
- Source: round 212's last open concern (harness detection, `plugins/runtime/host/src/{probes.rs,more/agents.rs}`):
  the runner picker and Settings → Agents waited for `runtime.agents`, which waits for every harness's detect; with
  one hung CLI the first ask still took up to 15 s (`probes.rs` `HUNG_FOR_MS`; v1 had the same wait in
  `runner-picker.js` → `GET /api/agents`).
- With ADR-0005 (topics and cursors: clients never poll), ADR-0011 (performance), ADR-0017 (every action answers
  within one frame), ADR-0004 (one write path).

## Context

- `runtime.agents` detects every harness at once (round 211) and each probe runs once per CLI file (round 212's
  memo), so a second ask is instant. The first ask is not: it answers when the slowest harness answers, and a CLI
  that hangs (`agy --help` on a broken install, round 211) costs its whole 15 s timeout.
- Its UI callers only need each harness's own answer: the picker's row for Codex doesn't depend on Cursor's CLI.
  Waiting for the whole list kept every row (and New task's "No coding agent" check) behind the slowest one.
- The picker renders "Looking for harnesses…" for the whole wait; Settings → Agents rendered wisp.accounts' rows,
  rebuilt after a whole `runtime.agents` too.

## Decision

1. **A new topic, `plugin:wisp.runtime:harnesses`** (`wisp_runtime_api::TOPIC_HARNESSES`), with one entity kind,
   `runtime.harness` (table `runtime_harness`, migration 5): one row per registered harness, `HarnessRow { id, ord,
   state: checking | installed | missing, agent: AgentStatus, at }` (ts-rs: `bindings/runtime/HarnessRow.ts`).
   Rows are written through the store like every other row (the change feed records them); nothing is polled.
2. **A detection round writes in two steps.** First, in one write, every harness's row as `checking` (with what was
   known: the stored last answer, else its declared name and models) and a delete for a harness no longer
   registered. Then each harness's row again, in its own write, the moment its own detect lands (`installed` when
   found, signed in or not; `missing` otherwise, `agent.error` says why). A hung CLI holds back only its own row.
3. **No flash:** a harness this process already settled isn't set back to `checking` by a plain round; only a
   Re-check (`fresh` naming it, or `all`) or its first round since wisp started (a stored row may be stale) shows
   `checking`.
4. **Newest wins:** each round has a number; a round's answer is written only if no later round has already settled
   that harness (Settings' Re-check while the picker's older ask still waits on a hung CLI). The check and the write
   are under one lock, so two settles never land out of order.
5. **Who fills it:** wisp.runtime at start (off the start's way), and every `runtime.agents` call (the picker's ask,
   wisp.accounts' publish and Re-check, `wisp_available`). What `harnesses:` declares (switched off, pinned) is
   applied to each answer before it's written, so the topic and the command agree.
6. **`runtime.agents` is unchanged** for CLI and MCP callers: it still answers the whole list when every harness has.
7. **The UI renders from the topic.** The runner picker (`packages/shell/src/frame/RunnerPicker.tsx`) and
   Settings → Agents (`plugins/accounts/ui/src/panes/Agents.tsx`) watch it while they show: a row per harness at
   once, `Checking…` until its own answer, then updated in place. Settings → Agents keys its rows by harness id and
   merges wisp.accounts' row (sign-in, keys, accounts, Install) over the detection row (`words.ts`
   `mergeHarnesses`); the picker keeps each row's place while it's open (`runner.ts` `keepOrder`) and counts a
   checking harness with the set-up ones. A wisp with no such topic (an older peer) shows what `runtime.agents`
   answered, as before.

## Edge cases (each with its test)

| Case | Behaviour | Test |
| --- | --- | --- |
| Three harnesses, one hung | all three `checking` at once, in registration order; the other two settle while it hangs; `runtime.agents` answers only after it | `plugins/runtime/host/tests/detect.rs` `the_harnesses_topic_shows_every_harness_checking_at_once_then_each_as_it_settles` |
| An older round's answer lands after a Re-check's | the Re-check's stays; the older caller still gets its own answer | `detect.rs` `an_older_rounds_late_answer_never_overwrites_a_newer_one` |
| A plain round over a settled harness | the row never goes back to `checking`; the new answer lands in place | same test |
| Picker: rows checking, then settling | Checking… on each, set-up and checking ones first, no row moves while open, a new one goes last | `packages/shell/test/runner.test.ts` `the_picker_shows_every_harness_at_once_as_checking_and_each_row_settles_in_place` |
| Settings → Agents: never seen, then found | Checking… (neutral), no install tip or Not found before its answer; wisp.accounts' sign-in kind and brand kept; a Re-check keeps the last line | `plugins/accounts/ui/test/agents-pane.test.ts` `settings_agents_shows_each_harness_checking_until_its_detect_lands` |
| A wisp before this topic | the picker uses `runtime.agents`' answer, Settings → Agents wisp.accounts' rows | both UI tests above |
| A harness plugin switched off | its row is deleted on the next round | not tested here (the round deletes rows not registered; covered by the checking write) |

## Places it reaches (ADR-0040)

Inside wisp: the runner picker and Settings → Agents. From a harness: `wisp_available` and `wisp_agents` still ask
`runtime.agents` (each call also refreshes the topic). Any client that watches topics (the phone apps, another wisp
through wisp.machines) gets the rows the same way.

## Consequences

- One more table and topic in wisp.runtime; a round costs one write for the checking rows and one per harness
  (about a dozen small rows; detection runs at start and when asked, never per event).
- Detection runs once at start, before anyone asks: the probes it spawns are the ones wisp.accounts' start already
  asked for, so the memo shares them.
- A row says only what detection found; sign-ins, accounts and keys stay wisp.accounts' (its own topic).
