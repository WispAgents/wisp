//! The scheduler's look at one member (v1 crew.js `tick`, every [`wisp_crew_api::TICK_MS`]): whether
//! a check-in is due, and the bookkeeping that goes with firing it. Pure: the host starts the job.

use wisp_crew_api::Member;

use crate::cron::Zone;
use crate::schedule::{due_now, minute_of};

/// A check-in the tick fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fire {
    /// a wake-up the member asked for (not its cron)
    pub wake: bool,
}

/// If a check-in is due now, mark the minute as run (`lastMinute`) and say what to fire. The check-in is unattended:
/// the budget may hold it. A wake-up that came is kept: the host clears it ([`clear_wake`]) once its check-in started
/// or queued, and one refused (already checking in, a full queue) is asked again the next minute instead of lost.
pub fn tick(m: &mut Member, now: i64, zone: &dyn Zone) -> Option<Fire> {
    if !due_now(m, now, zone) {
        return None;
    }
    let wake = m.wake_at.is_some_and(|w| w <= now);
    m.last_minute = Some(minute_of(now));
    Some(Fire { wake })
}

/// The wake-up that came by `now` is used up (its check-in started or queued); one set for later stays.
pub fn clear_wake(m: &mut Member, now: i64) {
    if m.wake_at.is_some_and(|w| w <= now) {
        m.wake_at = None;
    }
}

/// The soonest check-in across the crew (v1 `nextWake`, for a host that wakes this wisp for it).
pub fn next_wake(crew: &[Member], now: i64, zone: &dyn Zone) -> Option<i64> {
    crew.iter().filter_map(|m| crate::schedule::next_checkin(m, now, zone)).min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cron::Utc;

    fn member(cron: &str) -> Member {
        Member {
            id: "m".into(),
            name: "Ada".into(),
            agent: "claude".into(),
            mode: "auto".into(),
            cron: cron.into(),
            parallel: 2,
            ..Member::default()
        }
    }

    /// Audit E12: a new every-minute member printed "next 1m ago": this minute's check-in (due, not run yet) is now.
    #[test]
    fn the_next_check_in_is_never_in_the_past_audit_e12() {
        let m = member("* * * * *");
        let now = 1_700_000_030_000; // 30 s into a minute
        assert_eq!(crate::schedule::next_checkin(&m, now, &Utc), Some(now));
        assert_eq!(next_wake(std::slice::from_ref(&m), now, &Utc), Some(now));
    }

    #[test]
    fn fires_once_per_cron_minute() {
        let mut m = member("* * * * *");
        let now = 1_700_000_000_000;
        assert_eq!(tick(&mut m, now, &Utc), Some(Fire { wake: false }));
        assert_eq!(tick(&mut m, now + 5_000, &Utc), None);
        assert!(tick(&mut m, now + 60_000, &Utc).is_some());
    }

    /// A wake-up fires once a minute until its check-in starts: the tick keeps it (the check-in may be refused) and
    /// the host clears it once it started; one set later than now stays.
    #[test]
    fn a_wake_up_fires_once_a_minute_until_its_check_in_clears_it() {
        let mut m = member("");
        let now = 1_700_000_000_000;
        m.wake_at = Some(now - 1);
        assert_eq!(tick(&mut m, now, &Utc), Some(Fire { wake: true }));
        assert_eq!(m.wake_at, Some(now - 1), "kept until its check-in starts");
        assert_eq!(tick(&mut m, now + 20_000, &Utc), None, "once a minute");
        assert_eq!(tick(&mut m, now + 60_000, &Utc), Some(Fire { wake: true }), "refused: asked again the next minute");
        clear_wake(&mut m, now + 60_000);
        assert_eq!(m.wake_at, None);
        assert_eq!(tick(&mut m, now + 120_000, &Utc), None);
        m.wake_at = Some(now + 600_000);
        clear_wake(&mut m, now + 120_000);
        assert_eq!(m.wake_at, Some(now + 600_000), "a wake-up still to come stays");
    }

    #[test]
    fn paused_never_fires() {
        let mut m = member("* * * * *");
        m.paused = true;
        assert_eq!(tick(&mut m, 1_700_000_000_000, &Utc), None);
        assert_eq!(next_wake(&[m], 1_700_000_000_000, &Utc), None);
    }
}
