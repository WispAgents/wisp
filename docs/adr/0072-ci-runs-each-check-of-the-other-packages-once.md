# ADR-0072: CI runs each check of the non-workspace packages once: the typecheck step's programs, one vitest run, monoflow for the rest

- Status: accepted
- Date: 2026-11-15
- Source: PR CI run 9ea69e14 (round 268). Windows, node 22: the step `monoflow verify, every other package` ran from
  13:16:46 until the 60-minute limit cancelled the job at 13:43, 28 of 46 packages done: 60-345 s per small plugin UI,
  2 at a time (@wisp/ui 209 s, crew-ui 345 s, cursor-ui 217 s). Linux node 24: the same step 7.7 min (plugin UIs
  11-27 s each, @wisp/ui 48 s, spikes/events and spikes/sync-bench-rs 130 s each, last); macOS node 24: 16.4 min.
- Amends ADR-0069 decision 2 (how ci-verify checks a package). Its shape (the Rust workspace once through cargo, every
  other package after it, one verify-all.json for the summary and G14) is unchanged.

## Context

- ADR-0069 left one `monoflow verify --package=<p> --no-cache` per package. For an npm workspace monoflow (2a86d14,
  `graph/discover_node.go`) runs the `typecheck` script and the `test` script. Every plugin UI's `typecheck` is
  `tsc -p tsconfig.json`, a program over its own files that imports the kit's sources (`@wisp/ui`, `@wisp/shell`
  export `./src/index.ts`), so each of 43 tsc runs type-checked the kit again; and each `vitest run` started a vitest
  (config bundle, a vite server, a worker pool) to run 1-17 test files. Locally (M-series, 10 cores) goal-ui's verify is
  4.7 s: 3.6 s of tsc, ~1 s of vitest. On a 4-vCPU Windows runner process starts and file reads are several times
  slower, and 2 at a time it took 26+ minutes.
- The job's typecheck step (`npm run typecheck`, tools/typecheck.ts: 4 programs, 1 minute on Windows) already checks
  every one of those files with the same compiler options: every web package's tsconfig extends `tsconfig.web.json`
  with only `include`/`exclude`, the others extend `tsconfig.base.json` like the root program, and the web app's dev
  config is a program of its own.
- Vitest's `projects` runs many packages, each with its own `vitest.config.ts` (root, environment, plugins, includes),
  in one process and one worker pool. Locally the 8 packages tried ran the same 117 files and 366 tests in one run as
  one by one.
- The spikes' own workspaces resolve other features than the Cargo workspace's, so they reuse little of its build
  (cold, locally: events-bench 18 s, sync-bench-rs 10 s to build); they share the target dir's lock, and came last.

## Decision

1. **A package's typecheck that `npm run typecheck` already holds is not run again.** ci-verify (tools/ci-plan.ts)
   reads each `tsc -p <config>` of a package's `typecheck` script with TypeScript's own config parser and calls it
   covered when one of tools/typecheck.ts's `PROGRAMS` has exactly its compiler options and every one of its files.
   The typecheck step tees its output (`$RUNNER_TEMP/typecheck.log`, `--typecheck-log`); its errors are credited to
   the package whose config holds the file (`code: typecheck`). A log without the step's summary line (the step
   didn't run or was cut off) is not a result: ci-verify then runs tools/typecheck.ts itself. A config not covered
   (other options, a file no program holds, a script in another form) is type-checked through monoflow
   (`verify --package --typecheck`).
2. **Every package whose `test` script is `vitest run` (with or without `--passWithNoTests`) and that has a
   `vitest.config.ts` runs its tests in one vitest process** (tools/vitest.ci.config.ts, one project per package;
   `--reporter=json` split by package folder for the result). A package whose script has no `--passWithNoTests` and
   has no test file in the run fails, as its own `vitest run` would. A failure the report places in no package (an
   unhandled error) fails a target named `vitest`. Other test scripts (the `node --test` packages) run through
   monoflow (`verify --package --test`).
3. **Anything else is monoflow's whole verify, as before:** a package with a lint script or under a root eslint config
   (monoflow would run a lint), one without a `typecheck` or `test` script, sdk/python and the spikes. The spikes are
   the first monoflow calls, so their builds run beside the vitest run instead of after it.
4. **The result keeps its shape** (`verify-all-<n>.json`: status, counts, `failed`, `errors`, `results`): one result per
   package as before, each `results` row naming what ran it (`via`: `npm run typecheck + vitest (one run)`,
   `monoflow --test`, `monoflow`). tools/monoflow-report.ts and tools/g14.ts read it unchanged.

## Consequences

- Locally, all 46 packages: 128.6 s → 38.5 s (same 45 packages passing; sdk/python fails on this Mac in both for want of
  `ruff`). Expected in CI: the plugin UIs' per-package minutes go to one vitest run (~1-3 min) beside the spikes' builds
  (~2 min on Linux, ~5 on Windows); target under 8 minutes on Windows, so the job ends under 45. Not measured on a
  runner yet: the next `ci:full` run is the proof.
- A package's types are checked in a program shared with the others, not alone. A global declaration in one package
  that another package's code relies on would pass here and fail in that package's own `tsc -p`; the coder's own
  `npm run typecheck` sees the same as CI, and monoflow's `verify --affected` (local) still runs each package's script.
- A package's tests run with the repo as the working directory, not the package folder (every test today resolves
  files from `import.meta.url`). A test that reads a path relative to the cwd fails here first.
- ci-verify mirrors more of monoflow than before (its script names for typecheck, test and lint, and the root eslint
  rule). When monoflow's pin changes, tools/ci-plan.ts's lists are checked against `graph/discover_node.go`; a package
  they don't describe falls back to the whole monoflow verify, so a mismatch costs time, not coverage.
- The spikes still build their own feature sets (no reuse is forced); they are overlapped, not shortened.
