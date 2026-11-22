//! Manager handling of grid transport failures (`FailKind::Asleep` enters watch mode instead of
//! `Error`, non-tunnel errors preserve their message rather than SSH's `NO_TUNNEL`, and a grid
//! stream dropping puts the peer to `Asleep` and watches it cleanly).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;
use wisp_kernel::{BoxFuture, Caller, CancellationToken, Plugin, Result, Setup};
use wisp_machines::MachinesPlugin;
use wisp_machines_api::{
    AddArgs, Channel, ChannelEnd, ChannelRequest, ChannelResponse, Cleaned, ConnectIo, EndReason, FailKind, Failing, PeerRow, Probe,
    TRANSPORT, Transport, TransportFailure,
};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

struct FakeGrid {
    mode: Arc<AtomicUsize>,
    probes: Arc<AtomicUsize>,
}

struct FakeChannel {
    scope: CancellationToken,
    health_status: u16,
    health_body: String,
}

impl Channel for FakeChannel {
    fn request(&self, req: ChannelRequest) -> BoxFuture<'static, Result<ChannelResponse>> {
        let status = if req.path == "/health" { self.health_status } else { 503 };
        let body = if req.path == "/health" { self.health_body.clone() } else { "{}".into() };
        Box::pin(async move {
            let (tx, rx) = mpsc::channel(4);
            let _ = tx.send(body.into_bytes()).await;
            Ok(ChannelResponse { status, headers: Vec::new(), body: rx, cancel: CancellationToken::new() })
        })
    }
    fn closed(&self) -> BoxFuture<'static, ChannelEnd> {
        let scope = self.scope.clone();
        Box::pin(async move {
            scope.cancelled().await;
            ChannelEnd { reason: EndReason::Closed, error: None, hint: None, give_up: false }
        })
    }
    fn close(&self) {
        self.scope.cancel();
    }
    fn label(&self) -> String {
        "ada.grid.test".into()
    }
}

impl Transport for FakeGrid {
    fn kind(&self) -> &'static str {
        "grid"
    }
    fn steps(&self) -> &'static [(&'static str, &'static str)] {
        &[("wake", "Wake it up")]
    }
    fn clean(&self, input: &AddArgs, _prev: Option<&PeerRow>) -> Failing<Cleaned> {
        Ok(Cleaned {
            name: input.name.clone(),
            address: Some("ada.grid.test".into()),
            port: Some(443),
            command: Some(String::new()),
            auto_connect: Some(false),
        })
    }
    fn connect(&self, _peer: PeerRow, _io: Arc<dyn ConnectIo>, cancel: CancellationToken) -> BoxFuture<'static, Failing<Arc<dyn Channel>>> {
        let m = self.mode.load(Ordering::SeqCst);
        Box::pin(async move {
            match m {
                0 => Err(TransportFailure { kind: Some(FailKind::Asleep), status: Some(503), ..TransportFailure::new("offline") }),
                1 => Ok(Arc::new(FakeChannel {
                    scope: cancel,
                    health_status: 502,
                    health_body: r#"{"error":"Your wisp didn't answer: client error (Connect)"}"#.into(),
                }) as Arc<dyn Channel>),
                _ => Ok(Arc::new(FakeChannel {
                    scope: cancel,
                    health_status: 200,
                    health_body: r#"{"name":"wisp","version":"1.0.0"}"#.into(),
                }) as Arc<dyn Channel>),
            }
        })
    }
    fn probe(&self, _peer: PeerRow, _io: Arc<dyn ConnectIo>) -> BoxFuture<'static, Probe> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Probe::Asleep })
    }
}

struct FakeGridPlugin(Arc<FakeGrid>);

impl Plugin for FakeGridPlugin {
    fn manifest(&self) -> wisp_kernel::Manifest {
        wisp_kernel::Manifest::from_value(json!({
            "wispPlugin": 1,
            "id": "test.fake-grid",
            "version": "1.0.0-alpha.1",
            "api": "^1"
        }))
        .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.contribute(&TRANSPORT, self.0.clone());
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn manager_handles_grid_asleep_502_and_stream_lost() {
    let mode = Arc::new(AtomicUsize::new(0));
    let probes = Arc::new(AtomicUsize::new(0));
    let fg = Arc::new(FakeGrid { mode: mode.clone(), probes });
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().service(store).with(MachinesPlugin).with(FakeGridPlugin(fg)).start().await;

    let added = k.call(Caller::user(), "machines.add", json!({ "name": "Cloud", "transport": "grid", "connect": false })).await.unwrap();
    let id = added["id"].as_str().unwrap().to_owned();

    // 1. FailKind::Asleep transitions to asleep (not error).
    mode.store(0, Ordering::SeqCst);
    k.call(Caller::user(), "machines.connect", json!({ "id": id })).await.unwrap();
    for _ in 0..50 {
        let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
        if g["state"] == "asleep" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
    assert_eq!(g["state"], "asleep", "{g}");
    assert_eq!(g["error"], "", "{g}");

    // 2. Non-tunnel health 502 surfaces the real error, not SSH's NO_TUNNEL.
    mode.store(1, Ordering::SeqCst);
    k.call(Caller::user(), "machines.connect", json!({ "id": id })).await.unwrap();
    for _ in 0..50 {
        let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
        if g["state"] == "error" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
    assert_eq!(g["state"], "error", "{g}");
    assert_eq!(g["error"], "Your wisp didn't answer: client error (Connect)", "{g}");

    // 3. Health 200 followed by SSE stream ending (503) transitions a grid peer cleanly to asleep (no hang).
    mode.store(2, Ordering::SeqCst);
    k.call(Caller::user(), "machines.connect", json!({ "id": id })).await.unwrap();
    for _ in 0..50 {
        let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
        if g["state"] == "asleep" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let g = k.call(Caller::user(), "machines.get", json!({ "id": id })).await.unwrap();
    assert_eq!(g["state"], "asleep", "{g}");
}
