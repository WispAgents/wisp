//! v1 kept `telemetryId` and `defaults.telemetry` in its config: the install's one id and the off switch survive a
//! restart (they were lost on every start while `telemetry_meta` rows were refused for being on no topic).
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
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.sink", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
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

async fn sent(k: &wisp_kernel::Kernel, got: &Got) -> Vec<Value> {
    k.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    for _ in 0..50 {
        if !got.0.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    std::mem::take(&mut *got.0.lock().unwrap())
}

fn opened() -> Value {
    json!({ "event": "getapp_opened", "props": { "from": "button" } })
}

#[tokio::test(flavor = "multi_thread")]
async fn the_install_id_and_the_off_switch_survive_a_restart() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let (got1, got2, got3) = (Got::default(), Got::default(), Got::default());
    let k1 = kernel(&store, &got1).await;
    k1.call(Caller::user(), "telemetry.track", opened()).await.unwrap();
    let first = sent(&k1, &got1).await;
    let id = first[0]["properties"]["distinct_id"].as_str().unwrap().to_owned();

    let k2 = kernel(&store, &got2).await;
    k2.call(Caller::user(), "telemetry.track", opened()).await.unwrap();
    let again = sent(&k2, &got2).await;
    assert_eq!(again[0]["properties"]["distinct_id"].as_str(), Some(id.as_str()), "one id per install, not per start");
    k2.call(Caller::user(), "telemetry.set", json!({ "on": false })).await.unwrap();

    let k3 = kernel(&store, &got3).await;
    assert_eq!(k3.call(Caller::user(), "telemetry.track", opened()).await.unwrap()["queued"], false, "still off after a restart");
    drop((k1, k2, k3));
}

/// The saved body holds feedback you wrote that waits to be sent and a crash's queue: only you read its topic, never a
/// task's agent (capped or not), an API key or a preview link, whose sets read every other topic.
#[test]
fn only_the_user_reads_the_saved_body() {
    use wisp_kernel::caps::allowed;
    let meta = "topic:read{topic:plugin:wisp.telemetry:meta}";
    assert!(allowed(&Caller::user(), meta));
    for c in
        [Caller::set("agent:session"), Caller::agent("s1", false), Caller::agent("s1", true), Caller::set("apikey"), Caller::set("preview")]
    {
        assert!(!allowed(&c, meta), "{} reads it", c.set);
        assert!(allowed(&c, "topic:read{topic:plugin:wisp.telemetry:state}"), "{} still reads the Privacy row", c.set);
    }
}
