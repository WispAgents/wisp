//! Silent runs (TRY-23): a run with no sign of life for 30 minutes while its plan still moves is told to the phone
//! once ("Silent 30 min, spent 4% of weekly", with Stop); with *Stop a run silent for* on (`budget.silentStop`, off by
//! default), one silent that long is stopped. Codex once showed "Thinking" for 14 hours and spent 15% of a week.
//!
//! A sign of life is any step of the turn. A subagent call that is still open (a `Tool` of kind `Agent` with no
//! output yet) works out of sight: its run isn't silent while it's open (as the task page's *No output* line).
//! Pure: the host feeds each step as a [`Sign`], the plan's reading and `now`.

use std::collections::BTreeSet;

/// What a step of the turn says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sign {
    /// a subagent call opened (it works out of sight until its answer)
    Opened(String),
    /// a call's answer came
    Closed(String),
    /// anything else it did
    Other,
}

/// How long without a sign of life a run is silent.
pub const SILENT_MS: i64 = 30 * 60_000;

/// A running turn as the watch sees it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Quiet {
    pub agent: String,
    pub account: String,
    pub turn_id: String,
    /// its last sign of life
    pub life: i64,
    /// the plan's longest window (`used`, 0..1) at its last sign of life; None until a reading came
    pub base: Option<f64>,
    /// subagent calls still open
    pub open: BTreeSet<String>,
    /// the phone was told of this silent stretch
    pub told: bool,
    /// it was stopped for its silence
    pub stopped: bool,
}

impl Quiet {
    pub fn new(agent: &str, account: &str, turn_id: &str, now: i64, used: Option<f64>) -> Self {
        Self { agent: agent.into(), account: account.into(), turn_id: turn_id.into(), life: now, base: used, ..Self::default() }
    }

    /// Its steps came: a sign of life (the silent stretch starts again, from the plan as it is now).
    pub fn steps(&mut self, signs: &[Sign], now: i64, used: Option<f64>) {
        for s in signs {
            match s {
                Sign::Opened(id) => {
                    self.open.insert(id.clone());
                }
                Sign::Closed(id) => {
                    self.open.remove(id);
                }
                Sign::Other => {}
            }
        }
        self.life = now;
        self.base = used;
        self.told = false;
    }

    /// How long it has been silent at `now` (0 while a subagent of it works).
    pub fn silent_for(&self, now: i64) -> i64 {
        if self.open.is_empty() { (now - self.life).max(0) } else { 0 }
    }
}

/// What the watch does about one run at `now`.
#[derive(Debug, Clone, PartialEq)]
pub enum Due {
    Nothing,
    /// tell the phone: silent this many minutes, and how much of the window it spent meanwhile (0..1)
    Tell {
        minutes: i64,
        spent: f64,
    },
    /// stop it: silent this many minutes, past *Stop a run silent for*
    Stop {
        minutes: i64,
    },
}

/// `used`: the plan's longest window now; `stop_after_ms`: *Stop a run silent for*, None when off.
pub fn due(q: &Quiet, now: i64, used: Option<f64>, stop_after_ms: Option<i64>) -> Due {
    let silent = q.silent_for(now);
    if silent < SILENT_MS {
        return Due::Nothing;
    }
    let minutes = silent / 60_000;
    if let Some(after) = stop_after_ms
        && silent >= after
        && !q.stopped
    {
        return Due::Stop { minutes };
    }
    match (q.base, used) {
        // only while its plan still moves: a run that waits on something and spends nothing isn't worth a ping
        (Some(base), Some(now_used)) if !q.told && now_used > base => Due::Tell { minutes, spent: now_used - base },
        _ => Due::Nothing,
    }
}

/// "Silent 34 min, spent 4% of weekly" (the window's label as Settings → Usage names it).
pub fn told_text(minutes: i64, spent: f64, window: &str) -> String {
    // at least 1%: it was told because the plan moved
    let pct = format!("{:.0}", (spent * 100.0).max(1.0));
    format!("Silent {}, spent {pct}% of {}", span(minutes), window.to_lowercase())
}

/// "Stopped after 2 h silent"
pub fn stopped_text(minutes: i64) -> String {
    format!("Stopped after {} silent", span(minutes))
}

fn span(minutes: i64) -> String {
    if minutes < 60 {
        format!("{minutes} min")
    } else if minutes % 60 == 0 {
        format!("{} h", minutes / 60)
    } else {
        format!("{} h {} min", minutes / 60, minutes % 60)
    }
}

/// *Stop a run silent for*: "1", "2", "4", "8" hours, or off.
pub fn stop_after_ms(v: Option<&str>) -> Option<i64> {
    v.and_then(|s| s.trim().parse::<i64>().ok()).filter(|h| *h > 0).map(|h| h * 3_600_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn a_run_silent_30_min_while_its_plan_moves_is_told_once() {
        let mut q = Quiet::new("codex", "default", "t1", 0, Some(0.10));
        assert_eq!(due(&q, 29 * MIN, Some(0.14), None), Due::Nothing, "not silent long enough");
        let d = due(&q, 31 * MIN, Some(0.14), None);
        assert!(matches!(d, Due::Tell { minutes: 31, spent } if (spent - 0.04).abs() < 1e-9), "{d:?}");
        q.told = true;
        assert_eq!(due(&q, 40 * MIN, Some(0.15), None), Due::Nothing, "once per silent stretch");
        // a sign of life starts a new stretch, from the plan as it is then
        q.steps(&[Sign::Other], 41 * MIN, Some(0.15));
        assert_eq!(due(&q, 70 * MIN, Some(0.15), None), Due::Nothing, "silent but spending nothing: no ping");
        assert!(matches!(due(&q, 72 * MIN, Some(0.16), None), Due::Tell { minutes: 31, .. }));
    }

    #[test]
    fn no_reading_of_the_plan_never_pings() {
        let q = Quiet::new("cursor", "default", "t1", 0, None);
        assert_eq!(due(&q, 5 * 60 * MIN, None, None), Due::Nothing);
    }

    #[test]
    fn a_working_subagent_isnt_silence() {
        let mut q = Quiet::new("claude", "default", "t1", 0, Some(0.1));
        q.steps(&[Sign::Opened("c1".into())], 0, Some(0.1));
        assert_eq!(due(&q, 3 * 60 * MIN, Some(0.5), Some(60 * MIN)), Due::Nothing);
        q.steps(&[Sign::Closed("c1".into())], 3 * 60 * MIN, Some(0.5));
        assert_eq!(q.silent_for(3 * 60 * MIN + MIN), MIN, "its answer is a sign of life");
    }

    #[test]
    fn stop_after_n_hours_only_when_set_and_once() {
        let mut q = Quiet::new("codex", "default", "t1", 0, Some(0.1));
        assert!(matches!(due(&q, 2 * 60 * MIN, Some(0.1), None), Due::Nothing), "off by default");
        assert_eq!(due(&q, 2 * 60 * MIN, Some(0.1), stop_after_ms(Some("2"))), Due::Stop { minutes: 120 });
        q.stopped = true;
        assert_eq!(due(&q, 3 * 60 * MIN, Some(0.1), stop_after_ms(Some("2"))), Due::Nothing);
        assert_eq!(stop_after_ms(Some("off")), None);
        assert_eq!(stop_after_ms(Some("0")), None);
    }

    #[test]
    fn the_words() {
        assert_eq!(told_text(34, 0.04, "Weekly"), "Silent 34 min, spent 4% of weekly");
        assert_eq!(told_text(90, 0.001, "5-hour"), "Silent 1 h 30 min, spent 1% of 5-hour");
        assert_eq!(stopped_text(120), "Stopped after 2 h silent");
    }
}
