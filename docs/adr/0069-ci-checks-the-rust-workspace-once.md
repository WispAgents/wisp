# ADR-0069: CI checks the Rust workspace once, through cargo; monoflow verifies every other package

- Status: accepted
- Date: 2026-11-15
- Source: PR #756's `full / suite` jobs, run 5b84787a (job logs in `target/ci/5b84787a-*.log`). macOS, node 22:
  `cargo test --workspace` ended 22 min in, then `monoflow verify --all --no-cache` ran for 38 min (still compiling
  with rustc) until the 60-minute limit cancelled the job. macOS, node 24: cargo test 11 min, verify --all 27 min,
  43 min in all. Linux: cargo test 10 min, verify --all 23 min. Windows: both jobs hung in one test (below).
- Amends ADR-0001's "PR CI runs `monoflow verify --all`" for the Cargo workspace's crates only; gate G14 and the
  report-only affected set are unchanged.

## Context

- monoflow (2a86d14, `graph/discover_cargo.go`) gives each crate `cargo check -p <crate>`, `cargo clippy -p <crate> --
  -D warnings` and `cargo test -p <crate>`, and `verify --all` runs all three for each of the workspace's 150 crates.
  suite.yml already runs `cargo clippy --workspace --all-targets` (a superset of both check and clippy of each crate's
  lib and bins) and `cargo test --workspace --no-fail-fast` just before. So every Rust test ran twice per job, and
  each `-p` build resolves features for that crate alone, which builds the dependency graph again in other feature
  sets: that is the rustc still running when macOS was cancelled.
- Of the 195 packages monoflow knows, 149 are workspace crates. The other 46: 43 npm workspaces, `sdk/python`, and
  `spikes/events`, `spikes/sync-bench-rs` (their own Cargo workspaces, which `cargo test --workspace` never builds).
- Three packages are named `wisp` (crates/wisp, desktop/src-tauri, sdk/python); monoflow's graph keeps one (sdk/python),
  so crates/wisp, the CLI and server with every `serve_*` test, was never in `verify --all` at all. It is a workspace
  member, so cargo test always ran it.
- Windows: both `full / suite` jobs sat in `crates/wisp/tests/serve_desktop.rs`
  `with_no_wisp_running_the_app_starts_one` for 38 min. `CreateProcess` gives a child every inheritable handle of its
  parent, whatever its stdio is: the detached desktop app (`desktop::launch`) and the server it started kept the
  test's stdout pipe open, so `Command::output` waited for an EOF that came only when the server exited. That is a
  product bug too: on Windows, a script or tool reading `wisp`'s output never saw it end while the app it opened ran. Fixed in wisp-proc (`keep_std_handles`, called before the app or a browser starts).

## Decision

1. **The Cargo workspace is checked once, by its own steps:** `cargo fmt --all --check`, `cargo clippy --workspace
   --all-targets --locked -- -D warnings`, `cargo test --workspace --locked --no-fail-fast` (and cargo deny on Linux).
2. **monoflow verifies every other package** with `node tools/ci-verify.ts --no-cache`: it asks monoflow (`graph
   query`) about every folder with a manifest, skips the workspace members, and runs `monoflow verify
   --package=<name> --no-cache --json` for each package left, half the CPUs at a time. A folder monoflow doesn't know
   as a package of its own is skipped and named, as `verify --all` skipped it. The spikes build into the job's
   target dir (`CARGO_TARGET_DIR`), so they reuse what the workspace built.
3. **One result, one shape:** ci-verify writes `verify-all-<n>.json` in monoflow's `--json` shape (status, counts,
   `failed`, `errors`, and `results` with each target's time), and adds the crates the cargo test step's log names as
   failing (`error: N targets failed`, `could not compile`). The job summary (tools/monoflow-report.ts, now with the
   five slowest targets) and G14 (tools/g14.ts) read it as before, Rust failures included. A failing crate named like
   a monoflow package (crates/wisp, sdk/python) is keyed by its folder (`crates/wisp`), so G14 never credits it to
   the other package. Crates without a cargo test result (an earlier step failed) count as neither passed nor failed.
4. **The verify step runs after a failing Rust step too** (when `npm ci` succeeded), so a red job still says which
   other packages fail. A nightly repeat (`repeat: 3`) runs `cargo test --workspace` again with each monoflow pass.

## Consequences

- Expected per job, from 5b84787a's step times (setup to clippy, clippy, the test build, the tests) plus the monoflow
  pass over the 46 packages (~5-8 min; @wisp/ui is 14 s alone, the spikes 20-30 s on a warm target): Linux 37 → ~21
  min, macOS node 24 43 → ~22-25, macOS node 22 over 60 → ~28-30, Windows (hung) → ~35-40 (its test build alone is 12).
- A crate's checks in CI are the workspace build's (features unified across the workspace), not the crate's own
  feature set. A crate that compiles only with features another member turns on is not caught here; `cargo check -p`
  in a coder's own loop, and monoflow's `verify --affected`, still build it alone.
- ci-verify mirrors two things of monoflow it can't ask for: which manifests it reads and the wrapper's environment.
  tools/test/tools.test.ts checks the wrapper's; a package monoflow finds some other way would be missed, so ci-verify
  prints how many packages it ran next to monoflow's own `graph query` answers.
- G14's "time saved" (`scorePr` sums `results` durations) now covers the monoflow pass only: the Rust crates have no
  per-crate time (the cargo step's is one number), so the share is computed over TS, Python and the spikes.
- Every detached start (the desktop app, a browser, the headless server `Client::connect` starts, wisp-proc's
  `detach`) clears the inherit flag first; a build without the `proc` feature still has the Windows leak.
- Locally nothing changes: `monoflow verify` and `npm test` (`verify --all --test`) still run every crate.
