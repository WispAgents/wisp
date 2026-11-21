# ADR-0087: glibc's malloc gets two arenas and maps big blocks on their own, and the gate says where the RSS is

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §7 *Linux idle RSS 150-159 MB vs 120*. Follows ADR-0084 (the trim). Adds one reviewed OS
  call (`mallopt`) to `wisp-proc`'s sys module (plan §9.2: `unsafe` only there). No new dependency, no budget change.

## Context

After ADR-0084's trim, PR CI (ubuntu-latest, df56a408) still missed idle RSS: **150–159 MB** settled after the list
reads (budget 120), **129–135 MB before any client**. The same binary on macOS: 92 MB before, 97 MB after. There is no
Linux machine or container here (no docker), so Linux is reasoned and CI is the measurement.

What macOS shows (budgets profile, 5 000 tasks, `vmmap`/`footprint` on the idle server, `ps -M`):

- **42 threads at the gate's moment** (~3 s after start), 8 once idle for 10 s+: 4 tokio workers, the store's writer,
  a few long-lived ones, and ~33 of tokio's blocking pool, alive for its 10 s keep-alive after the start's reads.
- footprint 52 MB dirty, of which malloc ~48 MB; 29 MB clean text; thread stacks 1.2 MB resident for 50 threads.
  So stacks aren't it, and the binary's text is ~the same on both.

The difference is glibc's malloc. It gives each thread that allocates its own arena, up to 8 × cores (32 on CI's
4 cores): each of those ~33 pool threads read rows, parsed JSON and kept a little of it in its own arena. `malloc_trim`
(ADR-0084) returns only whole free pages; a page holding one live object stays, and what's live (the list cache, the
5 connections' SQLite page caches, hyper's and the stream's buffers) is spread over up to 32 arenas' pages. glibc
also raises its mmap threshold to the biggest block freed so far (up to 32 MB): after the first 400 KB page frame or
the 2.5 MB summaries text is freed, every later block that size comes out of an arena and is kept there.

## Decision

1. **`wisp_proc::tune_heap()` is the first thing `main` does** (every `wisp` process; before tokio's runtime, so no
   thread exists yet). On `linux` + `gnu` it calls `mallopt(M_ARENA_MAX, 2)` (`HEAP_ARENAS`: the cap usually set for
   threaded servers) and `mallopt(M_MMAP_THRESHOLD, 128 KiB)` (`HEAP_MMAP_AT`: glibc's starting threshold, fixed, so
   every block that big is its own mapping and goes back to the OS on free). Elsewhere (macOS, musl, Windows) it does
   nothing and returns false.
2. **The environment wins:** when `GLIBC_TUNABLES`, `MALLOC_ARENA_MAX` or `MALLOC_MMAP_THRESHOLD_` is set, the user
   chose and `tune_heap` changes nothing.
3. **The gate says where the RSS is** (`spikes/server-budgets/src/mem.rs`, reported, not gated): on Linux it sorts
   `/proc/<pid>/smaps` into the binary's pages, other files, `[heap]`, glibc's other arenas (anonymous mappings at a
   64 MB-aligned address), thread stacks (above a one-page guard, and `[stack]`), the rest of the anonymous memory
   (its three biggest) and the thread count, before the list and once settled; on macOS the thread count. Then it
   boots the same home once more with `MALLOC_ARENA_MAX=<8 × cores>` (glibc's own default; rule 2 turns `tune_heap`
   off), before the gated idle server on the same home, and reports its RSS before and after the same 34 list reads. One CI run then shows both what each
   part holds and what this decision saved.

## Results

macOS, where `tune_heap` does nothing (`budgets` profile, 5 000 tasks, one run each):

| | before | after |
|---|---:|---:|
| idle RSS before the list / settled | 92.2 / 97.1 MB | 95.3 / 99.6 MB |
| cold start median | ~50 ms | 56.9 ms |
| list first page median | — | 1.35 ms |
| patch latency p99 | — (run disturbed) | 104.0 ms |

Every budget passes after. Linux: PR CI's next run, the job summary's *Where the idle RSS is* and *On glibc's own
malloc settings* lines.

## Places it reaches (ADR-0040)

Every `wisp` process on glibc Linux: the npm package's and the desktop app's Linux builds, grid boxes. No command, MCP
tool, wire message or setting changes.

## Edge cases and their tests

- A malloc setting in the environment leaves glibc to it; names match whole: `crates/wisp-proc/src/heap.rs`
  `a_malloc_setting_in_the_environment_leaves_glibc_to_it`.
- `tune_heap` is true only on glibc without an environment setting, is safe to call again, and a block past the
  threshold still works: `tuning_the_heap_is_glibcs_alone_and_safe_to_call_twice` (runs as "false" on macOS; Linux CI
  runs the glibc arm).
- smaps sorts into the parts (a path with a space, an arena's PROT_NONE rest counting nothing, a guarded stack vs a
  plain anonymous mapping): `spikes/server-budgets/src/mem.rs` `smaps_sorts_into_the_binary_the_heaps_the_stacks_and_the_rest`;
  mappings newer glibc names (`[anon: glibc: malloc arena]`, `… pthread user stack …`) sort the same:
  `mappings_glibc_names_sort_as_the_unnamed_ones_do`; the report line, and the thread count alone without smaps:
  `the_line_names_every_part_or_only_the_threads_without_smaps`.
- Compiled and clippy-checked for `x86_64-unknown-linux-gnu`, `-musl` and `x86_64-pc-windows-msvc`.

## Consequences

- Two arenas mean threads that allocate at the same moment can wait on an arena's lock; glibc's per-thread cache
  (tcache) serves most small blocks without it. The list's first page and the patch latency are gated in the same
  run, so a cost shows there.
- Blocks of 128 KiB or more cost an `mmap`/`munmap` and fresh zeroed pages each time (a 4.5 MB list read: ~1 000
  page faults, well under a millisecond).
- If CI's breakdown shows the RSS is elsewhere (the binary's pages, the pool's threads, SQLite's caches), the next
  step follows that line, not more allocator settings: e.g. a shorter blocking-pool keep-alive, or a smaller
  `cache_size` on the 4 read connections.
