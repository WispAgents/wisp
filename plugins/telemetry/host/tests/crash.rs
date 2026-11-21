//! v1 services/telemetry.js `crashed`: a panic goes as an unhandled `$exception` (its message and where), and the
//! queue is kept in the store, since a release build aborts right after; the next start sends it. Its own test
//! binary: the panic hook is the process's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Caller, Manifest, Plugin, Result, Setup};
use wisp_store::{Location, Store, SystemClock};
use wisp_telemetry_api::{SINK, Sink};
use wisp_testkit::TestApp;

#[derive(Default, Clone)]
struct Got(Arc<Mutex<Vec<Value>>>);

struct FileSink(Got);
impl Sink for FileSink {
    fn id(&self) -> &'static str {
        "file"
    }
    fn send(&self, batch: Vec<Value>) -> BoxFuture<'static, bool> {
        self.0.0.lock().unwrap().extend(batch);
        Box::pin(async { true })
    }
}

struct Acme(Got);
impl Plugin for Acme {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.crash", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.contribute(&SINK, Arc::new(FileSink(self.0.clone())) as Arc<dyn Sink>);
        Ok(())
    }
}

async fn kernel(store: &Store, got: &Got) -> wisp_kernel::Kernel {
    TestApp::new()
        .with(wisp_telemetry::TelemetryPlugin::with_env(wisp_telemetry_core::Env::new()))
        .with(Acme(got.clone()))
        .service(store.clone())
        .start()
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_panic_is_kept_with_the_queue_and_sent_by_the_next_start() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let (got1, got2) = (Got::default(), Got::default());
    let k1 = kernel(&store, &got1).await;
    let ok = k1.call(Caller::user(), "telemetry.track", json!({ "event": "getapp_opened", "props": { "from": "button" } })).await.unwrap();
    assert_eq!(ok["queued"], true);
    // a panic on a thread of its own (the hook runs before the unwind, as it would before an abort)
    let joined = std::thread::spawn(|| panic!("index out of range at {}/src/x.rs", std::env::var("HOME").unwrap_or_default())).join();
    assert!(joined.is_err());
    // a second one in a build that unwinds: kept after the first, not over it (review: the first batch was lost)
    assert!(std::thread::spawn(|| panic!("second")).join().is_err());
    k1.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(got1.0.lock().unwrap().is_empty(), "the crash took the queue for the next start");

    let k2 = kernel(&store, &got2).await;
    k2.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    for _ in 0..50 {
        if got2.0.lock().unwrap().len() >= 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let batch = got2.0.lock().unwrap().clone();
    let names: Vec<&str> = batch.iter().filter_map(|e| e["event"].as_str()).collect();
    assert_eq!(names, vec!["getapp_opened", "$exception", "$exception"], "the queue and both panics, in order");
    let ex = batch.iter().find(|e| e["event"] == "$exception").unwrap();
    let e = &ex["properties"]["$exception_list"][0];
    assert_eq!((e["type"].as_str(), e["mechanism"]["handled"].as_bool()), (Some("Panic"), Some(false)));
    let value = e["value"].as_str().unwrap();
    assert!(value.starts_with("index out of range") && !value.contains("/src/x.rs"), "scrubbed: {value}");
    assert!(e["stacktrace"]["frames"].as_array().is_some_and(|f| !f.is_empty()), "where it happened: {e}");
    // sent once: a third start finds nothing left
    let got3 = Got::default();
    let k3 = kernel(&store, &got3).await;
    k3.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(got3.0.lock().unwrap().iter().all(|e| e["event"] != "$exception"), "the kept queue goes once");
    drop((k1, k2, k3));
}
