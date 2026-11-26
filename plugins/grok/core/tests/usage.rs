//! `grok usage <id>` as the real CLI printed it (grok 1.0.70, captured 2026-11-25 from two finished headless sessions;
//! nothing personal in them): a two-turn session with its cost in ticks, and a session whose one turn used nothing.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use wisp_grok_core::{spent, usage};

const USAGE: &str = include_str!("cli/usage.json");
const EMPTY: &str = include_str!("cli/usage-empty.json");

/// 2026-10-19T15:55:32Z and T15:56:04Z, the two turns' `endedAt`, in ms (what the host's RFC 3339 reader gives).
fn ms(s: &str) -> i64 {
    match s {
        "2026-10-19T15:55:32.108484+00:00" => 1_792_425_332_108,
        "2026-10-19T15:56:04.861846+00:00" => 1_792_425_364_861,
        "2026-10-19T20:21:21.136651+00:00" => 1_792_441_281_136,
        _ => 0,
    }
}

#[test]
fn a_real_sessions_usage_parses_with_its_cost_in_ticks() {
    let u = usage(USAGE).unwrap();
    assert_eq!(u.session_id, "01a154df-836c-7cd3-9f61-192d1864227a");
    assert_eq!(u.tokens, 136_102, "totalTokens: input (cache reads included) + output");
    assert!((u.cost_usd.unwrap() - 0.118_592).abs() < 1e-12, "1_185_920_000 ticks at 10^10 a dollar");
    assert_eq!(u.turns.len(), 2);
    let t = &u.turns[0];
    assert_eq!((t.number, t.tokens, t.cached_read, t.model.as_deref()), (1, 117_691, 101_888, Some("grok-4.7-build")));
    assert!((t.cost_usd.unwrap() - 0.084_438).abs() < 1e-12);
    let sum: f64 = u.turns.iter().filter_map(|t| t.cost_usd).sum();
    assert!((sum - u.cost_usd.unwrap()).abs() < 1e-12, "the turns sum to the session");
}

#[test]
fn a_session_that_used_nothing_has_no_cost_not_a_zero_one() {
    let u = usage(EMPTY).unwrap();
    assert_eq!((u.tokens, u.cost_usd, u.turns.len()), (0, None, 1));
    assert_eq!(u.turns[0].cost_usd, None);
}

#[test]
fn what_is_not_usage_json_is_none() {
    assert_eq!(usage("Error: No usage recorded for session '01a154e6-9e97-7570-b897-f36e4040a60d'."), None);
    assert_eq!(usage("{}"), None);
}

#[test]
fn spent_sums_the_turns_that_ended_in_the_window() {
    let all = [usage(USAGE).unwrap(), usage(EMPTY).unwrap()];
    let s = spent(&all, 0, &ms);
    assert_eq!((s.turns, s.tokens, s.unpriced), (3, 136_102, 0), "the empty turn has no tokens: not unpriced");
    assert!((s.cost_usd.unwrap() - 0.118_592).abs() < 1e-12);
    // only the second turn of the first session is at or after 15:56
    let s = spent(&all[..1], 1_792_425_364_000, &ms);
    assert_eq!((s.turns, s.tokens), (1, 18_411));
    assert!((s.cost_usd.unwrap() - 0.034_154).abs() < 1e-12);
    let s = spent(&all[1..], 0, &ms);
    assert_eq!(s.cost_usd, None, "no cost recorded: none, never $0.00");
}
