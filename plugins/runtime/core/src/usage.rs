//! Settings → Usage's line (v1 GET /api/usage summary): spend, turns, how many worked, and the questions
//! agents asked, over the turns that started since a time; and v1's Charts and breakdowns (usage.js): per day,
//! per project and per harness · model. Pure: the host reads the turns and says where each task ran.
//!
//! size-ok: the summary, its day/week/harness buckets and their tests share one set of fixtures; under half is code.

use std::collections::BTreeMap;

use wisp_runtime_api::commands::{UsageDay, UsageHarnessDay, UsageRow, UsageSummary, UsageWeek};
use wisp_runtime_api::model::{Turn, TurnStatus};

use crate::prices::turn_cost;

/// A turn that ended by asking (a `question` block in its answer, or held for an answer).
fn asked(t: &Turn) -> bool {
    t.presentation.as_ref().is_some_and(|p| p.blocks.iter().any(|b| b.get("type").and_then(serde_json::Value::as_str) == Some("question")))
}

fn n(v: Option<&serde_json::Value>) -> u64 {
    v.and_then(serde_json::Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).map_or(0, |x| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = x.round() as u64;
        count
    })
}

/// What one turn read (cache included), read from cache, and wrote (`(input, cached, output)`).
fn tokens_of(t: &Turn) -> (u64, u64, u64) {
    let Some(st) = t.stats.as_ref() else { return (0, 0, 0) };
    let cached = st.cache_tokens.unwrap_or(0);
    let read = st.input_tokens.unwrap_or(0).saturating_add(cached);
    let out = st.output_tokens.unwrap_or(0);
    if let Some(m) = st.models.as_ref() {
        let (mr, mc, mo) = m.values().fold((0u64, 0u64, 0u64), |(r, c, o), x| {
            let cr = n(x.get("cacheRead"));
            (r + n(x.get("input")) + cr + n(x.get("cacheWrite")), c + cr, o + n(x.get("output")))
        });
        if mr + mo > read + out {
            return (mr, mc.max(cached), mo);
        }
    }
    (read, cached, out)
}

/// The summary of `turns` that started at or after `from`. `worked`: the share (0–100) of the ended ones that
/// didn't fail; None when none ended.
pub fn summarize(turns: &[Turn], from: i64) -> UsageSummary {
    let mine: Vec<&Turn> = turns.iter().filter(|t| t.started_at >= from).collect();
    let ended: Vec<&&Turn> =
        mine.iter().filter(|t| matches!(t.status, TurnStatus::Done | TurnStatus::Error | TurnStatus::Stopped)).collect();
    let ok = ended.iter().filter(|t| t.status == TurnStatus::Done && !t.is_error).count();
    #[allow(clippy::cast_precision_loss)] // counts of turns
    let worked = (!ended.is_empty()).then(|| (ok as f64 * 100.0 / ended.len() as f64).round());
    let (input_tokens, cache_tokens, output_tokens) = mine.iter().fold((0u64, 0u64, 0u64), |(i, c, o), t| {
        let (r, ch, out) = tokens_of(t);
        (i.saturating_add(r), c.saturating_add(ch), o.saturating_add(out))
    });
    UsageSummary {
        from,
        spend: mine.iter().filter_map(|t| turn_cost(t)).map(|(c, _)| c).sum(),
        turns: u32::try_from(mine.len()).unwrap_or(u32::MAX),
        worked,
        questions: u32::try_from(mine.iter().filter(|t| asked(t)).count()).unwrap_or(u32::MAX),
        tokens: input_tokens.saturating_add(output_tokens),
        input_tokens,
        output_tokens,
        cache_tokens,
        ..UsageSummary::default()
    }
}

const DAY: i64 = 86_400_000;

/// `YYYY-MM-DD` of a UTC day number (days since 1970-01-01), Howard Hinnant's `civil_from_days`.
pub(crate) fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The day number of a civil date (Howard Hinnant's `days_from_civil`), for the ISO week's year.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The Monday of a day's ISO week (1970-01-01 was a Thursday) and its `YYYY-Www`.
fn iso_week(day: i64) -> (i64, String) {
    let monday = day - (day + 3).rem_euclid(7);
    // the ISO year is the Thursday's
    let thursday = monday + 3;
    let year: i64 = civil(thursday)[..4].parse().unwrap_or(1970);
    let week = (thursday - days_from_civil(year, 1, 1)) / 7 + 1;
    (monday, format!("{year:04}-W{week:02}"))
}

/// Where a `days`-long window starts: the viewer's local midnight `days - 1` days before today, so `days` is
/// exactly that many bars and the last one is the viewer's today (it was `now - days × 24h`, one UTC bar too many
/// and cut at the current hour). `tz_offset_min` is east of UTC (`-Date#getTimezoneOffset()`).
pub fn window_start(now: i64, days: u32, tz_offset_min: i32) -> i64 {
    let off = i64::from(tz_offset_min) * 60_000;
    let today = (now + off).div_euclid(DAY);
    (today - i64::from(days.max(1)) + 1) * DAY - off
}

fn ended(t: &Turn) -> bool {
    matches!(t.status, TurnStatus::Done | TurnStatus::Error | TurnStatus::Stopped)
}

/// One row over a group of turns.
fn row(key: String, name: String, agent: String, turns: &[&Turn]) -> UsageRow {
    let done: Vec<&&Turn> = turns.iter().filter(|t| ended(t)).collect();
    let ok = done.iter().filter(|t| t.status == TurnStatus::Done && !t.is_error).count();
    let lens: Vec<i64> = done.iter().filter_map(|t| t.ended_at.map(|e| e - t.started_at)).filter(|ms| *ms >= 0).collect();
    #[allow(clippy::cast_precision_loss)] // counts and lengths
    let (worked, avg_ms) = (
        (!done.is_empty()).then(|| (ok as f64 * 100.0 / done.len() as f64).round()),
        (!lens.is_empty()).then(|| lens.iter().sum::<i64>() as f64 / lens.len() as f64),
    );
    let costs: Vec<(f64, bool)> = turns.iter().filter_map(|t| turn_cost(t)).collect();
    let estimated = costs.iter().filter(|(_, e)| *e).count();
    let mut sids: Vec<&str> = turns.iter().map(|t| t.session_id.as_str()).filter(|s| !s.is_empty()).collect();
    sids.sort_unstable();
    sids.dedup();
    let (input_tokens, cache_tokens, output_tokens) = turns.iter().fold((0u64, 0u64, 0u64), |(i, c, o), t| {
        let (r, ch, out) = tokens_of(t);
        (i.saturating_add(r), c.saturating_add(ch), o.saturating_add(out))
    });
    UsageRow {
        key,
        name,
        agent,
        cost: costs.iter().map(|(c, _)| c).sum(),
        priced: u32::try_from(costs.len()).unwrap_or(u32::MAX),
        estimated: u32::try_from(estimated).unwrap_or(u32::MAX),
        cost_estimated: estimated > 0,
        premium_requests: turns.iter().filter_map(|t| t.stats.as_ref().and_then(|s| s.premium_requests)).sum(),
        turns: u32::try_from(turns.len()).unwrap_or(u32::MAX),
        sessions: u32::try_from(sids.len()).unwrap_or(u32::MAX),
        tokens: input_tokens.saturating_add(output_tokens),
        input_tokens,
        output_tokens,
        cache_tokens,
        worked,
        questions: u32::try_from(turns.iter().filter(|t| asked(t)).count()).unwrap_or(u32::MAX),
        avg_ms,
    }
}

/// The folder's last part, as v1 named a project it didn't know (places.js projectName).
pub(crate) fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(cwd).to_owned()
}

/// Most spent first, then most turns, then most tokens, then by key: the same input lists the same way.
fn ranked(mut rows: Vec<UsageRow>) -> Vec<UsageRow> {
    rows.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(b.turns.cmp(&a.turns)).then(b.tokens.cmp(&a.tokens)).then(a.key.cmp(&b.key)));
    rows
}

/// The summary with v1's breakdowns: a bar per local day from `from` to `now` (empty days too; `tz_offset_min` east
/// of UTC places a turn on the viewer's day), the days split by harness and rolled up by ISO week, and the turns by
/// harness, by project (`cwd_of` says where a task ran; a task it can't place goes under its id) and by harness · model.
pub fn breakdown(turns: &[Turn], from: i64, now: i64, tz_offset_min: i32, cwd_of: &dyn Fn(&str) -> Option<String>) -> UsageSummary {
    let off = i64::from(tz_offset_min) * 60_000;
    let mut out = summarize(turns, from);
    let mine: Vec<&Turn> = turns.iter().filter(|t| t.started_at >= from).collect();
    let costs: Vec<(f64, bool)> = mine.iter().filter_map(|t| turn_cost(t)).collect();
    let estimated = costs.iter().filter(|(_, e)| *e).count();
    out.priced = u32::try_from(costs.len()).unwrap_or(u32::MAX);
    out.estimated = u32::try_from(estimated).unwrap_or(u32::MAX);
    out.cost_estimated = estimated > 0;
    let mut ids: Vec<&str> = mine.iter().map(|t| t.session_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    out.sessions = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    let (first, last) = ((from + off).div_euclid(DAY), (now.max(from) + off).div_euclid(DAY));
    let mut days: BTreeMap<i64, UsageDay> = (first..=last).map(|d| (d, UsageDay { date: civil(d), ..UsageDay::default() })).collect();
    let mut by_harness: BTreeMap<String, Vec<&Turn>> = BTreeMap::new();
    let mut by_project: BTreeMap<String, Vec<&Turn>> = BTreeMap::new();
    let mut by_runner: BTreeMap<(String, String), Vec<&Turn>> = BTreeMap::new();
    let mut by_day_harness: BTreeMap<(i64, String), UsageHarnessDay> = BTreeMap::new();
    for t in &mine {
        let (r, c, o) = tokens_of(t);
        let day = (t.started_at + off).div_euclid(DAY);
        let (cost, guess) = turn_cost(t).unwrap_or((0.0, false));
        let agent = if t.agent.trim().is_empty() { "claude".to_owned() } else { t.agent.trim().to_owned() };
        if let Some(d) = days.get_mut(&day) {
            d.turns += 1;
            d.cost += cost;
            d.cost_estimated |= guess;
            d.tokens = d.tokens.saturating_add(r.saturating_add(o));
            let h = by_day_harness.entry((day, agent.clone())).or_insert_with(|| UsageHarnessDay {
                date: d.date.clone(),
                agent: agent.clone(),
                ..UsageHarnessDay::default()
            });
            h.turns += 1;
            h.cost += cost;
            h.cost_estimated |= guess;
            h.tokens = h.tokens.saturating_add(r.saturating_add(o));
            h.cache_read = h.cache_read.saturating_add(c);
        }
        by_harness.entry(agent.clone()).or_default().push(t);
        by_project.entry(cwd_of(t.session_id.as_str()).unwrap_or_else(|| t.session_id.as_str().to_owned())).or_default().push(t);
        by_runner.entry((agent, t.model.clone())).or_default().push(t);
    }
    let mut weeks: BTreeMap<i64, UsageWeek> = BTreeMap::new();
    for (day, d) in &days {
        let (monday, week) = iso_week(*day);
        let w = weeks.entry(monday).or_insert_with(|| UsageWeek { week, start: civil(monday), ..UsageWeek::default() });
        w.cost += d.cost;
        w.turns += d.turns;
        w.tokens = w.tokens.saturating_add(d.tokens);
        w.cost_estimated |= d.cost_estimated;
    }
    out.weekly = weeks.into_values().collect();
    out.daily = days.into_values().collect();
    out.daily_by_harness = by_day_harness.into_values().collect();
    out.harnesses = ranked(by_harness.into_iter().map(|(agent, ts)| row(agent.clone(), agent.clone(), agent, &ts)).collect());
    out.projects = ranked(by_project.into_iter().map(|(cwd, ts)| row(cwd.clone(), project_name(&cwd), String::new(), &ts)).collect());
    out.runners = ranked(by_runner.into_iter().map(|((agent, model), ts)| row(format!("{agent} {model}"), model, agent, &ts)).collect());
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use serde_json::json;
    use wisp_runtime_api::model::{Presentation, TurnStats};

    fn turn(at: i64, status: TurnStatus, cost: f64, question: bool) -> Turn {
        let blocks = if question { vec![json!({ "type": "question", "question": "which?" })] } else { vec![] };
        Turn {
            started_at: at,
            status,
            stats: Some(TurnStats { cost_usd: Some(cost), ..TurnStats::default() }),
            presentation: Some(Presentation { blocks, v: 1, ..Presentation::default() }),
            ..Turn::default()
        }
    }

    #[test]
    fn usage_sums_spend_counts_turns_and_says_how_many_worked() {
        let turns = vec![
            turn(10, TurnStatus::Done, 0.5, false),
            turn(20, TurnStatus::Error, 0.25, false),
            turn(30, TurnStatus::Done, 0.25, true),
            turn(40, TurnStatus::Running, 0.0, false),
            turn(1, TurnStatus::Done, 9.0, false),
        ];
        let u = summarize(&turns, 5);
        assert_eq!(u.turns, 4, "the one before the window is left out");
        assert_eq!(u.spend, 1.0);
        assert_eq!(u.worked, Some(67.0), "2 of 3 ended ones worked; a running one isn't counted");
        assert_eq!(u.questions, 1);
        assert_eq!(summarize(&[], 0).worked, None);
    }

    #[test]
    fn usage_breaks_down_by_day_project_and_harness_model_with_empty_days_kept() {
        const D: i64 = 86_400_000;
        let mut a = turn(D + 10, TurnStatus::Done, 0.5, false);
        a.session_id = "s1".into();
        a.agent = "claude".into();
        a.model = "opus".into();
        a.ended_at = Some(D + 2010);
        let mut b = turn(3 * D + 5, TurnStatus::Error, 0.25, true);
        b.session_id = "s2".into();
        b.agent = "codex".into();
        b.model = "gpt".into();
        b.stats = None; // Codex reports no price
        let cwd = |id: &str| (id == "s1").then(|| "/home/me/code/wisp/".to_owned());
        let u = breakdown(&[a, b], D, 3 * D + 100, 0, &cwd);
        assert_eq!(
            u.daily.iter().map(|d| (d.date.as_str(), d.turns)).collect::<Vec<_>>(),
            vec![("1970-01-02", 1), ("1970-01-03", 0), ("1970-01-04", 1)],
            "a day with nothing is still a bar"
        );
        assert_eq!((u.priced, u.sessions), (1, 2));
        assert_eq!(u.projects[0].name, "wisp", "the folder's last part, most spent first");
        assert_eq!(u.projects[1].key, "s2", "a task it can't place goes under its id");
        assert_eq!((u.runners[0].agent.as_str(), u.runners[0].name.as_str(), u.runners[0].avg_ms), ("claude", "opus", Some(2000.0)));
        assert_eq!((u.runners[1].priced, u.runners[1].worked, u.runners[1].questions), (0, Some(0.0), 1));
        assert_eq!(civil(19_673), "2023-11-12");
    }

    #[test]
    fn usage_aggregates_tokens_and_per_harness_breakdown_across_all_harnesses_try_32() {
        const D: i64 = 86_400_000;
        let mut claude = turn(D + 10, TurnStatus::Done, 0.12, false);
        claude.session_id = "s1".into();
        claude.agent = "claude".into();
        claude.model = "opus".into();
        claude.stats = Some(TurnStats {
            cost_usd: Some(0.12),
            input_tokens: Some(15_000),
            cache_tokens: Some(30_000),
            output_tokens: Some(4_000),
            ..TurnStats::default()
        });
        let mut codex = turn(D + 20, TurnStatus::Done, 0.0, false);
        codex.session_id = "s2".into();
        codex.agent = "codex".into();
        codex.model = "gpt-5.5-codex".into();
        codex.stats = Some(TurnStats {
            cost_usd: None,
            input_tokens: Some(5_000),
            cache_tokens: Some(15_000),
            output_tokens: Some(3_000),
            ..TurnStats::default()
        });
        let mut agy = turn(D + 30, TurnStatus::Done, 0.0, false);
        agy.session_id = "s2".into();
        agy.agent = "antigravity".into();
        agy.model = String::new();
        agy.stats = Some(TurnStats {
            cost_usd: None,
            input_tokens: Some(8_000),
            cache_tokens: Some(2_000),
            output_tokens: Some(1_000),
            ..TurnStats::default()
        });
        let u = breakdown(&[claude, codex, agy], D, D + 100, 0, &|_| Some("/repo".into()));
        // Claude: read = 15k + 30k = 45k, cache = 30k, out = 4k -> 49k
        // Codex: read = 5k + 15k = 20k, cache = 15k, out = 3k -> 23k
        // Antigravity: read = 8k + 2k = 10k, cache = 2k, out = 1k -> 11k
        assert_eq!((u.input_tokens, u.cache_tokens, u.output_tokens, u.tokens), (75_000, 47_000, 8_000, 83_000));
        assert_eq!(u.daily[0].tokens, 83_000);
        assert_eq!(
            u.harnesses.iter().map(|h| (h.agent.as_str(), h.turns, h.sessions, h.tokens, h.cache_tokens)).collect::<Vec<_>>(),
            vec![("claude", 1, 1, 49_000, 30_000), ("codex", 1, 1, 23_000, 15_000), ("antigravity", 1, 1, 11_000, 2_000),]
        );
    }

    #[test]
    fn usage_buckets_by_the_viewers_local_day_with_exactly_days_bars_and_iso_weeks() {
        const D: i64 = 86_400_000;
        const H: i64 = 3_600_000;
        // 2026-11-25 is day 20_782 (a Wednesday); now is 10:00 UTC, 02:00 on the 25th in UTC-8
        let today = 20_782 * D;
        assert_eq!(civil(20_782), "2026-11-25");
        let now = today + 10 * H;
        let off = -480;
        let from = window_start(now, 7, off);
        assert_eq!(from, (20_782 - 6) * D + 8 * H, "local midnight six days back, in UTC");
        // 23:30 local on Nov 24 is 07:30 UTC on Nov 25: it belongs to Nov 24, the viewer's day
        let mut late = turn(today + 7 * H + 30 * 60_000, TurnStatus::Done, 0.5, false);
        late.agent = "claude".into();
        late.stats = Some(TurnStats {
            cost_usd: Some(0.5),
            input_tokens: Some(100),
            cache_tokens: Some(300),
            output_tokens: Some(50),
            ..TurnStats::default()
        });
        let mut other = turn(today + 6 * H + 30 * 60_000, TurnStatus::Done, 0.0, false);
        other.agent = "codex".into();
        other.stats = Some(TurnStats { cost_usd: None, input_tokens: Some(10), output_tokens: Some(5), ..TurnStats::default() });
        let mut same_day = turn(today + 6 * H, TurnStatus::Done, 0.25, false);
        same_day.agent = "claude".into();
        let u = breakdown(&[late, other, same_day], from, now, off, &|_| None);
        assert_eq!(u.daily.len(), 7, "days=7 is seven bars");
        assert_eq!(u.daily.last().map(|d| d.date.as_str()), Some("2026-11-25"), "the last bar is the viewer's today (02:00 local)");
        assert_eq!(u.daily.first().map(|d| d.date.as_str()), Some("2026-11-19"));
        let last = &u.daily[5];
        assert_eq!(last.date, "2026-11-24");
        assert_eq!((last.turns, last.cost), (3, 0.75), "23:30 local stays on its local day, not the next");
        assert_eq!(
            u.daily_by_harness.iter().map(|h| (h.date.as_str(), h.agent.as_str(), h.turns, h.tokens, h.cache_read)).collect::<Vec<_>>(),
            vec![("2026-11-24", "claude", 2, 450, 300), ("2026-11-24", "codex", 1, 15, 0)],
            "two agents split on their day"
        );
        // Nov 19 (Thu) .. Nov 22 (Sun) is week 47, Nov 23 (Mon) .. Nov 25 is week 48
        assert_eq!(
            u.weekly.iter().map(|w| (w.week.as_str(), w.start.as_str())).collect::<Vec<_>>(),
            vec![("2026-W47", "2026-11-16"), ("2026-W48", "2026-11-23")]
        );
        assert_eq!(u.weekly.iter().map(|w| w.turns).sum::<u32>(), u.daily.iter().map(|d| d.turns).sum::<u32>(), "the weeks sum the days");
        assert_eq!(u.weekly[1].cost, 0.75);
        assert!(!u.weekly[1].cost_estimated, "reported costs only: not an estimate");
        assert_eq!(iso_week(days_from_civil(2027, 1, 1)).1, "2026-W53", "Jan 1 2027 is a Friday: the last week of 2026");
        // UTC without an offset: the same turns land on Nov 25
        assert_eq!(
            breakdown(&[turn(today + 7 * H, TurnStatus::Done, 0.1, false)], window_start(now, 7, 0), now, 0, &|_| None).daily.len(),
            7
        );
    }

    #[test]
    fn usage_estimates_a_turn_with_tokens_and_no_cost_and_flags_it_cost_estimated() {
        let mut reported = turn(10, TurnStatus::Done, 0.5, false);
        reported.agent = "claude".into();
        let mut codex = turn(20, TurnStatus::Done, 0.0, false);
        codex.agent = "codex".into();
        codex.model = "gpt-5".into();
        codex.stats = Some(TurnStats { input_tokens: Some(1_000_000), output_tokens: Some(100_000), ..TurnStats::default() });
        let mut unknown = turn(30, TurnStatus::Done, 0.0, false);
        unknown.agent = "codex".into();
        unknown.model = "gpt-9".into();
        unknown.stats = Some(TurnStats { input_tokens: Some(10), ..TurnStats::default() });
        let mut copilot = turn(40, TurnStatus::Done, 0.0, false);
        copilot.agent = "copilot".into();
        copilot.stats = Some(TurnStats { premium_requests: Some(2), ..TurnStats::default() });
        let u = breakdown(&[reported, codex, unknown, copilot], 0, 100, 0, &|_| None);
        assert_eq!((u.priced, u.estimated, u.cost_estimated), (2, 1, true), "the gpt-9 turn stays unpriced");
        assert!((u.spend - (0.5 + 1.25 + 1.0)).abs() < 1e-9);
        let cx = u.harnesses.iter().find(|h| h.agent == "codex").unwrap();
        assert_eq!((cx.turns, cx.priced, cx.estimated, cx.cost_estimated), (2, 1, 1, true), "priced < turns: a partial row");
        let cl = u.harnesses.iter().find(|h| h.agent == "claude").unwrap();
        assert!(!cl.cost_estimated, "a reported cost is never an estimate");
        assert_eq!(u.harnesses.iter().find(|h| h.agent == "copilot").unwrap().premium_requests, 2);
        assert!(u.daily[0].cost_estimated && u.daily_by_harness.iter().any(|d| d.agent == "codex" && d.cost_estimated));
        assert!(u.weekly.len() == 1 && u.weekly[0].cost_estimated, "a week holding an estimated day says so");
    }
}
