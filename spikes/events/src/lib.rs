//! The durable append rate (ADR-0044's budget, plan §8: ≥ 20 000 events/s), shared by this bench and
//! spikes/server-budgets, which gates it in PR CI (ADR-0082): one measurement, two callers.
use serde_json::json;
use wisp_kernel::durable::{EventLog, NewEvent};
use wisp_store::Store;

/// Events appended by [`append_rate`], 1 000 a transaction.
pub const APPENDED: u64 = 100_000;

fn row(n: u64) -> NewEvent {
    NewEvent {
        ty: "bench.fact".into(),
        plugin: "wisp.bench".into(),
        subject: Some(format!("s{}", n % 100)),
        key: None,
        cause: None,
        root: "r".into(),
        at: 0,
        data: json!({ "n": n }),
    }
}

/// Durable events a second: [`APPENDED`] events into the store's log, 1 000 a transaction.
///
/// # Panics
/// When the store refuses an append (a bench: there is nothing to measure then).
pub async fn append_rate(st: &Store) -> f64 {
    let t = std::time::Instant::now();
    for chunk in 0..(APPENDED / 1000) {
        let rows: Vec<NewEvent> = (0..1000).map(|i| row(chunk * 1000 + i)).collect();
        st.append(rows).await.unwrap_or_else(|e| panic!("{e:?}"));
    }
    #[allow(clippy::cast_precision_loss)]
    let r = APPENDED as f64 / t.elapsed().as_secs_f64();
    r
}
