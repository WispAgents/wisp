# server budgets (ADR-0011, ADR-0080)

```sh
cargo build --profile budgets -p wisp
cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml   # [--wisp=<bin>] [--summary=<file>] [--keep]
```

Seeds a home with 5 000 tasks through wisp.runtime's writer (`schema::install`, `repo::save_session`), starts the real
`wisp serve` on it (fake harness CLIs, a cleared environment, no network) and gates it on `tools/budgets.ts`
`SERVER_BUDGETS`; exits 1 on a miss, 2 when it can't run:

- cold start: process start → the first `200` from `GET /api/status`, median of 7 restarts ≤ 150 ms
- idle RSS: the server's resident set once settled (2 s within 1 MB), after a client read the list ≤ 120 MB
- the task list: `GET /api/events?fresh=list` (the UI's first read) → the last byte of the `list` snapshot's first page
  (what the first screen draws, ADR-0081), median of 30 ≤ 5 ms; the time to its last page (every task), the pages and
  the bytes are reported. A task in two pages, or pages at two seqs, fails the run.

It also prints where the list's time goes (the first page's query, every task in one query, and the change-feed replay a
first look made before ADR-0081), measured in process. PR CI runs it on Linux (`.github/workflows/suite.yml`, *server budgets*) and puts the table in the job
summary. `--keep` keeps the bench's folder (the home and each server's log).

Not here yet: p99 patch latency, append rate and write amplification in CI (plan §4.3).
