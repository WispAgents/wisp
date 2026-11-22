//! Grid and grid-tunnel transport connection behaviour: waking a sleeping cloud computer with a
//! non-passive `/api/health`, surfacing real 502/401/404 errors instead of SSH's `NO_TUNNEL`,
//! and marking an offline `grid-tunnel` computer as `FailKind::Asleep`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use wisp_grid_client::GridTransport;
use wisp_kernel::CancellationToken;
use wisp_machines_api::{ConnectIo, FailKind, PeerRow, StepStatus, Transport as _};

#[derive(Default)]
struct SpyIo {
    secret: Option<String>,
    steps: Mutex<Vec<(String, StepStatus, Option<String>)>>,
    logs: Mutex<Vec<String>>,
}

impl ConnectIo for SpyIo {
    fn step(&self, id: &str, status: StepStatus, detail: Option<&str>) {
        self.steps.lock().unwrap().push((id.to_owned(), status, detail.map(str::to_owned)));
    }
    fn log(&self, line: &str) {
        self.logs.lock().unwrap().push(line.to_owned());
    }
    fn stale(&self, _pid: Option<u32>) {}
    fn secret(&self) -> Option<String> {
        self.secret.clone()
    }
}

async fn mock_grid<F>(handler: F) -> String
where
    F: Fn(http::Request<Incoming>) -> (StatusCode, Value) + Send + Sync + 'static,
{
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            let handler = handler.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req| {
                    let handler = handler.clone();
                    async move {
                        let (status, body) = handler(req);
                        Ok::<_, std::convert::Infallible>(
                            Response::builder()
                                .status(status)
                                .header("content-type", "application/json")
                                .body(Full::new(Bytes::from(body.to_string())))
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn peer(url: &str, tunnel: bool) -> PeerRow {
    let mut data = BTreeMap::new();
    data.insert("url".into(), json!(url));
    PeerRow {
        id: "p1".into(),
        peer: None,
        name: if tunnel { "Laptop".into() } else { "Cloud".into() },
        transport: if tunnel { "grid-tunnel".into() } else { "grid".into() },
        address: "127.0.0.1".into(),
        port: 80,
        command: String::new(),
        auto_connect: true,
        added_at: 0,
        data,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn connecting_to_a_cloud_computer_wakes_it_without_passive_header() {
    let seen_passive = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let sp = seen_passive.clone();
    let cl = calls.clone();
    let origin = mock_grid(move |req| {
        if req.headers().get("x-wisp-passive").is_some() {
            sp.fetch_add(1, Ordering::SeqCst);
            return (StatusCode::SERVICE_UNAVAILABLE, json!({ "error": "This box is asleep.", "asleep": true }));
        }
        let n = cl.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            (StatusCode::SERVICE_UNAVAILABLE, json!({ "error": "This box is asleep.", "asleep": true }))
        } else {
            (StatusCode::OK, json!({ "name": "wisp", "version": "1.0.0" }))
        }
    })
    .await;

    let t = GridTransport { tunnel: false, client: reqwest::Client::builder().no_proxy().build().unwrap() };
    let io = Arc::new(SpyIo { secret: Some("wgd_tok".into()), ..SpyIo::default() });
    let ch = t.connect(peer(&origin, false), io.clone(), CancellationToken::new()).await.unwrap();
    ch.close();

    assert_eq!(seen_passive.load(Ordering::SeqCst), 0, "connect must never send x-wisp-passive");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "retried after 503 and connected");
    let steps = io.steps.lock().unwrap();
    assert!(steps.iter().any(|(id, st, d)| id == "wake" && *st == StepStatus::Run && d.as_deref() == Some("Starting…")), "{steps:?}");
    assert!(steps.iter().any(|(id, st, _)| id == "wake" && *st == StepStatus::Done), "{steps:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn grid_connect_surfaces_real_502_401_404_errors_and_marks_offline_tunnel_asleep() {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let grid = GridTransport { tunnel: false, client: client.clone() };
    let tunnel = GridTransport { tunnel: true, client };

    // 502 from a dead box surfaces the grid's actual error, never SSH's NO_TUNNEL.
    let origin_502 = mock_grid(|_| (StatusCode::BAD_GATEWAY, json!({ "error": "Your wisp didn't answer: client error (Connect)" }))).await;
    let io = Arc::new(SpyIo { secret: Some("wgd_tok".into()), ..SpyIo::default() });
    let err = grid.connect(peer(&origin_502, false), io, CancellationToken::new()).await.err().unwrap();
    assert_eq!(err.message, "Your wisp didn't answer: client error (Connect)");
    assert_eq!(err.status, Some(502));

    // 401 gives up immediately with the Signed out hint.
    let origin_401 = mock_grid(|_| (StatusCode::UNAUTHORIZED, json!({ "error": "Sign in again." }))).await;
    let io = Arc::new(SpyIo { secret: Some("wgd_tok".into()), ..SpyIo::default() });
    let err = grid.connect(peer(&origin_401, false), io, CancellationToken::new()).await.err().unwrap();
    assert!(err.give_up);
    assert_eq!(err.kind, Some(FailKind::Gone));
    assert_eq!(err.hint.as_ref().and_then(|h| h.title.as_deref()), Some("Signed out"));

    // 404 gives up immediately with the grid's message.
    let origin_404 = mock_grid(|_| (StatusCode::NOT_FOUND, json!({ "error": "There's nobody with that handle here." }))).await;
    let io = Arc::new(SpyIo { secret: Some("wgd_tok".into()), ..SpyIo::default() });
    let err = tunnel.connect(peer(&origin_404, true), io, CancellationToken::new()).await.err().unwrap();
    assert!(err.give_up);
    assert_eq!(err.kind, Some(FailKind::Gone));
    assert_eq!(err.message, "There's nobody with that handle here.");

    // 503 on grid-tunnel means the other computer is offline: Asleep, so manager watches it.
    let origin_503 = mock_grid(|_| (StatusCode::SERVICE_UNAVAILABLE, json!({ "error": "Offline" }))).await;
    let io = Arc::new(SpyIo { secret: Some("wgd_tok".into()), ..SpyIo::default() });
    let err = tunnel.connect(peer(&origin_503, true), io, CancellationToken::new()).await.err().unwrap();
    assert_eq!(err.kind, Some(FailKind::Asleep));
    assert!(!err.give_up);

    // Missing token fails immediately with sign-in hint.
    let io = Arc::new(SpyIo { secret: None, ..SpyIo::default() });
    let err = grid.connect(peer(&origin_502, false), io, CancellationToken::new()).await.err().unwrap();
    assert!(err.give_up);
    assert_eq!(err.kind, Some(FailKind::Gone));
    assert!(err.hint.is_some());
}
