//! B5 (owner, 2026-11-16: "the 5-hour window shows for a Claude plan that has none", Ultra is weekly only): a window
//! the latest reports leave out is gone once its reset passes (v1 #556), and so is its notice. v1 kept the notice
//! ("Claude Code has a new 5-hour window, resets 2h", its reset long past) until you dismissed it.
#![allow(clippy::unwrap_used)]

use wisp_budget_api::{LimitWindow, LimitsReport};
use wisp_budget_core::Budget;

const H: i64 = 3_600_000;
const NOW: i64 = 1_000 * H;

fn window(id: &str, used: f64, resets_at: i64, ms: i64) -> LimitWindow {
    LimitWindow { id: id.into(), used, resets_at: Some(resets_at), ms: Some(ms), slot: None }
}

fn report(windows: Vec<LimitWindow>) -> LimitsReport {
    LimitsReport { windows, complete: true, read: true, ..LimitsReport::default() }
}

#[test]
fn a_window_gone_after_its_reset_takes_its_new_window_notice_with_it_b5() {
    let mut b = Budget::new();
    let week = || window("seven_day", 0.28, NOW + 100 * H, 168 * H);
    b.record("claude", "default", &report(vec![week()]), NOW);
    // one report also has a 5-hour window: a new one, said once
    b.record("claude", "default", &report(vec![week(), window("five_hour", 0.4, NOW + 3 * H, 5 * H)]), NOW + H);
    let v = b.view("claude", NOW + H);
    assert!(v[0].notices.iter().any(|n| n.window == "five_hour"), "the new window is told while it's there: {:?}", v[0].notices);
    // the plan reports the week only again; past the 5-hour reset the window is gone, and so is what was said of it
    b.record("claude", "default", &report(vec![week()]), NOW + 2 * H);
    b.record("claude", "default", &report(vec![week()]), NOW + 5 * H);
    let v = b.view("claude", NOW + 5 * H);
    assert_eq!(v[0].windows.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), ["seven_day"], "v1 #556");
    assert!(v[0].notices.iter().all(|n| n.window != "five_hour"), "no notice of a window that isn't there: {:?}", v[0].notices);
}
