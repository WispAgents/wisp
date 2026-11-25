//! Live usage reads via `accounts.limits` (no model turn, nothing spent):
//! - While work waits for a Codex or Antigravity plan (`budget.liveLimits`, off by default), wisp asks every 10 minutes
//!   so an early/banked reset frees waiting work through `record` (backing off up to hourly on failure).
//! - For Antigravity (whose `stream-json` turns don't include quota windows), wisp also reads `accounts.limits`
//!   asynchronously after each Antigravity turn ends and when `budget.view` sees a missing or stale reading.
//! - Cursor (its tier, `cursor-agent about`), opencode (`opencode stats --days 7`) and Grok (`grok usage` over its
//!   sessions of the last 7 days) report no windows, only a tier or what the CLI totals as spent: `budget.view` reads
//!   them the same way, in the background, at most every [`STALE_MS`] (ADR-0123), and the view carries `plan`/`spent`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::Value;
use wisp_budget_api::LimitsReport;
use wisp_budget_core::limits::{codex_banked, codex_live, report_from_value};
use wisp_kernel::Ctx;

use crate::{State, account_of, lock, now};

pub const SETTING: &str = "budget.liveLimits";
/// How often held work looks whether its account is free, and reads when due (v1 tick).
const TICK_MS: u64 = 60_000;
/// Minimum interval between passive (view-triggered) refreshes and retry backoff for Antigravity.
pub const STALE_MS: i64 = 5 * 60_000;
const ACCOUNTS: &str = "wisp.accounts";

/// The look's interval; tests look sooner (`WISP_BUDGET_TICK_MS`, at least 200 ms).
pub fn tick_ms() -> u64 {
    std::env::var("WISP_BUDGET_TICK_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(TICK_MS).max(200)
}
pub const EVERY_MS: i64 = 10 * 60_000;
pub const MAX_BACKOFF_MS: i64 = 60 * 60_000;

/// When each account's next read is due, and how long the last failure waits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Due {
    pub at: i64,
    pub backoff: i64,
}

/// Deduplicates background `accounts.limits` refreshes and backs off stale-view reads.
#[derive(Debug, Default)]
pub struct Refresh {
    inflight: BTreeSet<(String, String)>,
    next_at: BTreeMap<(String, String), i64>,
}

/// The harnesses `budget.view` reads in the background (their turns carry no quota windows): Antigravity's windows,
/// Cursor's tier, and what opencode and Grok total as spent.
pub fn read_on_view(agent: &str) -> bool {
    matches!(agent, "antigravity" | "cursor" | "opencode" | "grok")
}

/// Whether `wisp.accounts` is active or resolved so `accounts.limits` can be called.
pub fn accounts_on(ctx: &Ctx) -> bool {
    ctx.plugin_status().iter().any(|p| p.id == ACCOUNTS && (p.state == "active" || p.state == "resolved"))
}

/// After a read: on time again in ten minutes, or later and later while it fails (v1: half of ten, doubled).
pub fn next(d: Option<Due>, ok: bool, now: i64) -> Due {
    if ok {
        return Due { at: now + EVERY_MS, backoff: 0 };
    }
    let backoff = (d.map_or(EVERY_MS / 2, |d| if d.backoff == 0 { EVERY_MS / 2 } else { d.backoff }) * 2).min(MAX_BACKOFF_MS);
    Due { at: now + backoff, backoff }
}

fn parse_live(agent: &str, v: &Value, t: i64, read: bool) -> Option<LimitsReport> {
    if agent == "codex" {
        let banked = codex_banked(v, t);
        codex_live(v, t).map(|mut r| {
            r.read = read;
            r.banked = banked;
            r
        })
    } else {
        report_from_value(v).map(|mut r| {
            r.read = read;
            r
        })
    }
}

impl State {
    /// Reads one account's live limits through `accounts.limits` and records them when present.
    pub(crate) async fn read_limits(&self, ctx: &Ctx, agent: &str, account: &str, read: bool) -> bool {
        let a = wisp_accounts_api::commands::AccountArgs { agent: agent.into(), account: account.into() };
        let got = match ctx.run::<wisp_accounts_api::commands::Limits>(&ctx.root_call(), a).await {
            Ok(v) => Some(v).filter(|v| !v.is_null()),
            Err(err) => {
                tracing::warn!(error = %err.message, agent, account, "budget: accounts.limits");
                None
            }
        };
        let t = now(ctx);
        let report = got.as_ref().and_then(|v| parse_live(agent, v, t, read));
        if let Some(r) = report {
            self.record(ctx, agent, account, &r).await;
            true
        } else {
            tracing::debug!(agent, account, "live usage: no reading");
            false
        }
    }

    /// Spawns a non-blocking `accounts.limits` refresh for a [`read_on_view`] harness. `force` refreshes after a turn ends;
    /// `!force` refreshes on view only when missing or older than [`STALE_MS`].
    pub(crate) fn spawn_refresh(self: &Arc<Self>, ctx: &Ctx, agent: &str, account: &str, read: bool, force: bool) {
        if !read_on_view(agent) || !accounts_on(ctx) {
            return;
        }
        let account = account_of(account);
        let key = (agent.to_owned(), account.clone());
        let t = now(ctx);
        {
            let mut r = lock(&self.refresh);
            if r.inflight.contains(&key) {
                return;
            }
            if !force {
                if r.next_at.get(&key).is_some_and(|at| *at > t) {
                    return;
                }
                if lock(&self.budget).reading_at(agent, &account).is_some_and(|at| t - at < STALE_MS) {
                    return;
                }
            }
            r.inflight.insert(key.clone());
            r.next_at.insert(key.clone(), t + STALE_MS);
        }
        let (st, c, agent) = (Arc::clone(self), ctx.clone(), agent.to_owned());
        ctx.spawn(async move {
            let _ = st.read_limits(&c, &agent, &account, read).await;
            lock(&st.refresh).inflight.remove(&(agent, account));
        });
    }

    /// One pass: each Codex or Antigravity account with work held on it, when its read is due.
    pub(crate) async fn live(&self, ctx: &Ctx, due: &mut BTreeMap<(String, String), Due>) -> u32 {
        if ctx.setting(SETTING).and_then(|v| v.as_bool()) != Some(true) {
            return 0;
        }
        let mut n = 0;
        for (agent, account) in self.waiting(now(ctx)) {
            let key = (agent.clone(), account.clone());
            let t = now(ctx);
            if due.get(&key).is_some_and(|d| d.at > t) {
                continue;
            }
            let ok = self.read_limits(ctx, &agent, &account, true).await;
            if ok {
                n += 1;
            }
            let was = due.get(&key).copied();
            due.insert(key, next(was, ok, t));
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_comes_every_ten_minutes_and_less_often_while_it_fails() {
        assert_eq!(next(None, true, 0), Due { at: EVERY_MS, backoff: 0 });
        let f1 = next(None, false, 0);
        assert_eq!(f1, Due { at: EVERY_MS, backoff: EVERY_MS }, "half of ten, doubled");
        let f2 = next(Some(f1), false, 0);
        assert_eq!(f2.backoff, 2 * EVERY_MS);
        let mut d = f2;
        for _ in 0..5 {
            d = next(Some(d), false, 0);
        }
        assert_eq!(d.backoff, MAX_BACKOFF_MS, "up to hourly");
        assert_eq!(next(Some(d), true, 0).backoff, 0, "a good read starts over");
    }
}
