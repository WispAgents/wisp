# ADR-0119: One price table for the runtime, and an estimated cost is always marked (`cost_estimated`, `~$x.xx`)

- Status: accepted
- Date: 2026-11-25
- Source: the harness audit, round 2 (item `user-usage-every-harness`): only Claude and opencode report a dollar cost,
  so Usage showed Codex, Cursor, Copilot, Antigravity and most Grok turns as `—`, and their CLIs' own reports
  (`cursor-agent about`, `opencode stats`, `grok usage`, Copilot's `premiumRequests`) weren't read at all.

## Context

`wisp-runtime-core::cold` held a private five-id `PRICES` table (Claude ids) for Compact now. Usage summed only
`stats.cost_usd`, which most harnesses never set, so a week of Codex work read as `$0.00` or `—`. A cost wisp works
out is useful (what the same tokens would cost at API list price), but it must never read as what a vendor billed: a
subscription harness bills a plan, not tokens.

Each CLI exposes different things, and some of what the audit asked for can't be verified here: Copilot isn't
installed (its G6 fixtures are a fake CLI), `grok usage <session>` needs a session nobody has captured, and Claude has
no zero-spend usage read wisp can make without its OAuth token.

## Decision

1. **One table.** `plugins/runtime/core/src/prices.rs` holds USD per million tokens, `[input, cache read, output]`,
   for exact model ids of every family wisp runs (Claude, GPT/Codex, Gemini, Grok, Cursor's Composer), dated
   2026-11-25 from the vendors' public list prices. `cold.rs` re-exports `price` and `bare_model` from it; there is no
   second table. An id is normalised (`[1m]`, a `-YYYYMMDD` date and an opencode `provider/` prefix dropped; Claude's
   dotted `4.5` read as `4-5`), then matched exactly: **an id the table doesn't name stays unpriced**, never guessed
   from a family. A price change is a PR to the table.
2. **An estimate is flagged on the wire.** `prices::turn_cost` gives a turn's reported `cost_usd`, else the
   list-price estimate of its tokens (per model when the harness splits them, every one priced, else by the turn's
   model). `runtime.usage`'s `UsageSummary` and `UsageRow` add `estimated` (how many of `priced` are estimates) and
   `cost_estimated`; `UsageDay` and `UsageHarnessDay` add `cost_estimated`. All additive with `#[serde(default)]`:
   an older client ignores them, an older runtime sends none and the UI shows what it reported. `priced` now counts
   reported and estimated turns. Nothing is written back to a turn: the estimate is worked out on each read.
3. **The UI marks both.** A cost with `cost_estimated` renders `~$x.xx`; a row where `priced < turns` adds `+`
   (at least this much: some turns had no price) with a tooltip saying how many were priced. When nothing is priced the
   first tile is Tokens, never Spend.
4. **Each harness reports what its CLI exposes, parsed only from captured output** (fixtures under each core crate's
   `tests/cli/`, account details scrubbed):
   - Cursor: `cursor-agent about --format json` (and its text form) gives the subscription tier; `live_limits` returns
     it with no windows.
   - opencode: `opencode stats --days 7` gives its own cost and token counts; `live_limits` returns them with no
     windows.
   - Copilot: `premium_requests` reads `usage.premiumRequests`, but `PREMIUM_VERIFIED` is false: no turn carries the
     count until a real capture replaces the fake G6 one (`TurnStats.premium_requests` and `UsageRow.premium_requests`
     are on the wire, empty until then).
   - Grok: `USAGE_VERIFIED` is false; a turn without `total_cost_usd` is estimated from its tokens or stays unpriced.
   - Claude: keeps its `rate_limit_event` windows; no zero-spend live read (none wisp can make without its OAuth
     token).
   A harness with no window says "limits not reported by <CLI>" in Usage, its cards and the runner picker, and its
   stop-at control is disabled.

## Consequences

- Usage gives every harness a comparable number, honestly marked; a model released after the table's date shows
  tokens only until it's added.
- Places it reaches (ADR-0040): the app's Usage page and Settings → Usage, `wisp usage`/the MCP usage tools through
  `runtime.usage`, and any peer reading that command.
