# ADR-0144: build output goes first, This computer has tabs, and a plugin's own topic reads its table

- Status: accepted
- Date: 2026-11-27
- Source: the owner's disk filled. One left-over worktree held 127 GB, 120 GB of it a cargo `target/`, and the
  This computer page stayed on "Reading…". The owner asked whether worktree cleanup works at all. After seeing the
  mocks, the owner picked direction B (tabs Overview, Space and Limits, with a disk bar on Space) and asked for three
  fixes: the sweep removes build output from left-over and idle folders when the disk is low, reclaimable counts
  left-over folders, and the snapshot no longer stalls (D-0056).

## Context

- **Left-over folders were never cleaned by themselves.** `plan_sweep` picks idle task folders only, the low-disk
  rule included. A left-over folder goes only when someone clicks Remove, and the one that filled the disk listed
  580 changed files (regenerated screenshots), so nobody would click it. Removing a folder is all or nothing, yet
  most of a folder's size is often build output, which the next build makes again.
- **Reclaimable ignored them.** The page's "can go" counted left-over folders only when they held no work of their
  own. It said 1.1 GB while 136 GB sat in left-over folders.
- **The snapshot stalled.** No snapshot source was declared for `plugin:wisp.machine:snapshot`, so a first look
  rebuilt it from the change feed (`changed_since(topic, 0)`). Machine upserts a 30 KB row (its 24 h history) every
  10 s, which left 21k patches, 1.5 GB of the 1.7 GB store. Replaying them took 10–12 s. Once retention pruned the
  feed's start (7 days), the topic read empty for good. Every later patch then failed to apply, the client asked
  again, and the answer was empty again, so the page stayed on "Reading…". About 30 plugin kinds keep a whole table
  on one topic of their own the same way.

## Decision

1. **Build output** is a folder named `target` (only with cargo's `CACHEDIR.TAG`), `node_modules`, `.next` or
   `.turbo`, at most 4 levels down a worktree, and ignored by git there. A nested one counts in its parent. The
   worktrees `du` runs 4 levels deep instead of 1 and records each folder's build output beside its size
   (`wisp-worktrees-core::build`). Rows carry it as `ItemRow.build` and `LeftoverRow.build`. A new folder with no size
   makes a `du` due at once; before, it waited up to 6 h.
2. **`worktrees.clean {build: true}`** removes only the build output of the named tasks' folders and left-over
   folders. The folders, their branches and their changes stay. Right before each removal it checks again that the
   path is still inside the worktree, is named as build output, is ignored by git, has the tag (for `target`), and
   that no task works there. Without `ids` it takes every task's folders, but only when no leftovers are named, the
   same as `pick_clean`.
3. **A low disk takes build output first.** Under 10 GB free, the hourly sweep removes the build output of left-over
   folders, then of idle tasks' folders, oldest first, until 15 GB is free. A folder used in the last hour is
   skipped. Only after that does `plan_sweep` remove whole idle folders, as before.
4. **Reclaimable**: machine's reclaim groups start with `build` (each folder's bytes, biggest first, under `folders`).
   The other groups count what remains of each folder, so the groups never overlap and their sum is what all of them
   free.
5. **A clean-up that ends is announced.** `worktrees.cleaned`, declared but never emitted until now, is emitted, and
   machine samples again when it hears it, so the page updates within a second instead of up to a minute later.
   Machine also writes its 24 h history only when a point was added (once a minute), not every 10 s.
6. **Whole-table topics read their table.** When a kind is defined, the store asks its `topics` fn about three rows
   that differ in every column: one all nulls, then two with different values. If the answer is the same non-empty
   list each time, every row of the kind is on those topics (`Kind::whole`, `Reader::whole_kinds`). A first look at
   such a topic, when no source, reader or paged source covers it, reads the kind's table. A resume still reads the
   feed. `changed_since` also stops reading the patches of non-append changes, which it never needed.
7. **This computer has three tabs**: Overview (the owner's second pick, B: one status line that says the worst first, so
   memory short, disk almost full, automations paused or waiting, processes left running, else *Room to spare*, with
   the facts behind it; memory, CPU and disk as bars with a mark where automations start to wait and the numbers on
   the right, never cut; *Needs you* (what was left running, named by what it runs: `python3 -m http.server`, the whole
   line in its tooltip) before *Running now*; the last 24 hours as two lines over a time axis and one sentence), Space (a
   segmented disk bar of build output, wisp's folders, everything else and free; the build output folder by folder
   with Remove and Remove all; Free up space; harness folders) and Limits (automations and where their limits are).
   A warning above the tabs names a short disk, with one button that frees the build output, or memory under the
   floor. A build-output row says *Removing…* from the click until it leaves the snapshot. Each harness row waits
   alone. The page asks for a reading when it opens. Machine's toasts now go to the shell's toaster; until now they
   went to one that was mounted nowhere and never showed.
8. The kit gains `StackedMeter` (a bar in parts, with a legend), `LoadRow` (name · bar with a mark · numbers),
   `DayLines` (lines over a time axis, with a legend and a caption), and `UsageRow.tip`. The Overview's gauges go: three
   across, they cut their value ("210 GB fr…") because their narrow layout never applied (the grid queries itself as its
   container).

## Consequences

- A build after a trim starts cold. That's the price of not deleting anyone's work. The sweep picks left-over folders
  first, and nothing used in the last hour.
- The detection in 6 is a heuristic. A `topics` fn that returns one topic for some values of a column and another
  for others gives different answers across the probes and is left out, so it keeps the feed path. A fn that put
  only some rows on a fixed topic and decided that without reading any column would be read whole. No kind does that
  today.
- `du` 4 levels deep reads the same files as 1 level deep and prints more lines: about 17k for 30 worktrees, under
  the 32 MB output cap.
- Places it reaches (ADR-0040): `worktrees.clean --build` from the CLI and as an MCP tool, `wisp worktrees sweep`, and
  the page.
- Tests: `wisp-worktrees-core::build` (du parsing, the plan, which paths may go), the worktrees host
  `tests/build_output.rs` (a leftover's Remove takes only its own; tracked and untagged folders stay; a low disk takes
  a leftover's first), machine's `reclaim_tests`, the store's `a_plugins_own_topic_reads_its_table_even_after_retention_pruned_the_feed`,
  and the UI's `space.test.ts` and `overview.test.ts` (the status's order, the bars and marks, short command names).
