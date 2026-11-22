//! The `grid` and `grid-tunnel` transports (`machines.transport@1`, plan §7.2.2): HTTPS (HTTP/2 where the
//! grid speaks it) to the peer's own address on the grid, with this device's token.
//!
//! - `grid`: a cloud box (`<handle>.<domain>`). Opening wisp here never wakes it (`AutoStart::Follow`); a
//!   watch is passive (`x-wisp-passive: 1`) and never counts as use; `probe` reads `/_grid/state` without
//!   waking it; a real request does wake it (`wakes_on_request`), through `direct`.
//! - `grid-tunnel`: another of your computers (`<name>--<handle>.<domain>`), through the tunnel it holds
//!   to the grid. Only its owner turns it on: nothing here wakes it.
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use serde_json::Value;
use tokio::sync::mpsc;
use wisp_kernel::{BoxFuture, CancellationToken, Result, WireError};
use wisp_machines_api::{
    AddArgs, AutoStart, Body, Channel, ChannelEnd, ChannelRequest, ChannelResponse, Cleaned, ConnectIo, EndReason, FailKind, Failing, Hint,
    PeerRow, Probe, StepStatus, Stopped, Transport, TransportFailure,
};

use crate::net;

pub const PASSIVE: &str = "x-wisp-passive";
/// How long `connect` waits for a sleeping cloud computer to wake (`503` while starting).
const WAKE_TIMEOUT: Duration = Duration::from_secs(90);
const WAKE_GAP: Duration = Duration::from_secs(1);
/// Hard errors (502, network failure) retry briefly for restarts, then surface the grid's error.
const HARD_TRIES: u32 = 3;
const WAKE_STEP: &str = "wake";

fn signed_out_hint() -> Hint {
    Hint::titled("Signed out", "The device was revoked or expired. Settings → Cloud → Sign in again.")
}

fn no_token_hint() -> Hint {
    Hint::titled("Sign in again", "Settings → Cloud → Sign in, with the same grid.")
}

fn grid_error(status: u16, body: &Value) -> String {
    body.get("error")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map_or_else(|| format!("The grid answered HTTP {status}."), str::to_owned)
}

pub struct GridChannel {
    /// `<origin>/api`
    base: String,
    origin: String,
    token: Option<String>,
    client: reqwest::Client,
    scope: CancellationToken,
    end: Arc<Mutex<Option<ChannelEnd>>>,
}

impl std::fmt::Debug for GridChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridChannel").field("origin", &self.origin).finish_non_exhaustive()
    }
}

impl GridChannel {
    pub fn new(origin: &str, token: Option<String>, client: reqwest::Client, scope: CancellationToken) -> Arc<Self> {
        Arc::new(Self { base: format!("{origin}/api"), origin: origin.to_owned(), token, client, scope, end: Arc::new(Mutex::new(None)) })
    }
    fn end_with(&self, e: ChannelEnd) {
        let mut g = self.end.lock().unwrap_or_else(PoisonError::into_inner);
        if g.is_none() {
            *g = Some(e);
        }
        drop(g);
        self.scope.cancel();
    }
}

fn body_of(b: Body) -> Option<reqwest::Body> {
    match b {
        Body::Empty => None,
        Body::Bytes(b) => Some(reqwest::Body::from(b)),
        Body::Stream(rx) => {
            let s = futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|c| (Ok::<_, std::io::Error>(c), rx)) });
            Some(reqwest::Body::wrap_stream(s))
        }
    }
}

impl Channel for GridChannel {
    fn request(&self, req: ChannelRequest) -> BoxFuture<'static, Result<ChannelResponse>> {
        let url = format!("{}{}", self.base, req.path);
        let (client, scope, token, label, end) =
            (self.client.clone(), self.scope.clone(), self.token.clone(), self.origin.clone(), self.end.clone());
        Box::pin(async move {
            let method =
                reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes()).map_err(|e| WireError::invalid(e.to_string()))?;
            let mut b = client.request(method, &url);
            if let Some(t) = &token {
                b = b.bearer_auth(t);
            }
            if req.passive {
                b = b.header(PASSIVE, "1");
            }
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            if let Some(body) = body_of(req.body) {
                b = b.body(body);
            }
            if let Some(ms) = req.timeout_ms {
                b = b.timeout(Duration::from_millis(ms));
            }
            let resp = tokio::select! {
                () = scope.cancelled() => return Err(WireError::unavailable(format!("{label}: the connection is gone"))),
                r = b.send() => r.map_err(|e| WireError::unavailable(format!("{}: {e}", net::host_of(&label))))?,
            };
            let status = resp.status().as_u16();
            // signed out, or this device revoked: retrying won't change it
            if status == 401 {
                let e = ChannelEnd {
                    reason: EndReason::Gone,
                    error: Some("The grid doesn't know this device any more: sign in again.".into()),
                    hint: Some(signed_out_hint()),
                    give_up: true,
                };
                let mut g = end.lock().unwrap_or_else(PoisonError::into_inner);
                if g.is_none() {
                    *g = Some(e);
                }
                drop(g);
                scope.cancel();
            }
            let headers = resp.headers().iter().filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned()))).collect();
            let (tx, body) = mpsc::channel(64);
            let cancel = CancellationToken::new();
            let (c2, s2) = (cancel.clone(), scope.clone());
            tokio::spawn(async move {
                let mut stream = resp.bytes_stream();
                loop {
                    tokio::select! {
                        () = c2.cancelled() => return,
                        () = s2.cancelled() => return,
                        c = stream.next() => match c {
                            Some(Ok(c)) => if tx.send(c.to_vec()).await.is_err() { return },
                            _ => return,
                        },
                    }
                }
            });
            Ok(ChannelResponse { status, headers, body, cancel })
        })
    }
    fn closed(&self) -> BoxFuture<'static, ChannelEnd> {
        let (scope, end) = (self.scope.clone(), self.end.clone());
        Box::pin(async move {
            scope.cancelled().await;
            end.lock().unwrap_or_else(PoisonError::into_inner).clone().unwrap_or(ChannelEnd {
                reason: EndReason::Closed,
                error: None,
                hint: None,
                give_up: false,
            })
        })
    }
    fn close(&self) {
        self.end_with(ChannelEnd { reason: EndReason::Closed, error: None, hint: None, give_up: false });
    }
    fn label(&self) -> String {
        net::host_of(&self.origin).to_owned()
    }
}

fn origin(p: &PeerRow) -> Option<String> {
    p.data_str("url").map(|u| u.trim_end_matches('/').to_owned())
}

fn clean(input: &AddArgs, prev: Option<&PeerRow>) -> Failing<Cleaned> {
    let url = input.data.get("url").and_then(|v| v.as_str()).map(str::to_owned).or_else(|| prev.and_then(origin));
    let Some(url) = url else {
        return Err(TransportFailure {
            status: Some(400),
            ..TransportFailure::new("A grid machine comes from signing in: wisp cloud signin")
        });
    };
    let (host, port) = crate::peers::host_port(&url);
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| n.chars().take(40).collect())
        .or_else(|| prev.map(|p| p.name.clone()))
        .unwrap_or_else(|| host.clone());
    Ok(Cleaned {
        name: Some(name),
        address: Some(host),
        port: Some(port),
        command: Some(String::new()),
        auto_connect: Some(input.auto_connect.or(prev.map(|p| p.auto_connect)).unwrap_or(true)),
    })
}

#[derive(Debug)]
pub struct GridTransport {
    pub tunnel: bool,
    pub client: reqwest::Client,
}

async fn wake_and_check(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    peer_name: &str,
    tunnel: bool,
    io: &dyn ConnectIo,
    cancel: &CancellationToken,
) -> Failing<()> {
    let url = format!("{origin}/api/health");
    let deadline = Instant::now() + WAKE_TIMEOUT;
    let mut hard_fails = 0u32;
    let mut last_err = String::new();
    loop {
        if cancel.is_cancelled() {
            return Err(TransportFailure::new("Disconnected.").final_());
        }
        // Non-passive GET /api/health: wakes a sleeping cloud computer on connect, and checks a computer tunnel.
        let res = tokio::select! {
            () = cancel.cancelled() => return Err(TransportFailure::new("Disconnected.").final_()),
            r = net::call(client, "GET", &url, Some(token), None, &[]) => r,
        };
        match res {
            Ok((200, v)) if v.get("name").and_then(Value::as_str) == Some("wisp") => {
                io.step(WAKE_STEP, StepStatus::Done, Some(net::host_of(origin)));
                return Ok(());
            }
            Ok((401 | 403, _)) => {
                return Err(TransportFailure {
                    kind: Some(FailKind::Gone),
                    status: Some(401),
                    ..TransportFailure::new("The grid didn’t accept this computer’s sign-in.").with_hint(signed_out_hint()).final_()
                });
            }
            Ok((status @ (404 | 410), v)) => {
                return Err(TransportFailure {
                    kind: Some(FailKind::Gone),
                    status: Some(status),
                    ..TransportFailure::new(grid_error(status, &v)).final_()
                });
            }
            Ok((503, _)) if tunnel => {
                return Err(TransportFailure {
                    kind: Some(FailKind::Asleep),
                    status: Some(503),
                    ..TransportFailure::new(format!("{peer_name} is offline."))
                });
            }
            Ok((503, v)) => {
                let detail = v
                    .get("error")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty() && !v.get("asleep").and_then(Value::as_bool).unwrap_or(false))
                    .unwrap_or("Starting…");
                detail.clone_into(&mut last_err);
                io.step(WAKE_STEP, StepStatus::Run, Some(detail));
                if Instant::now() >= deadline {
                    return Err(TransportFailure { status: Some(503), ..TransportFailure::new("It didn’t wake up in time.") });
                }
            }
            Ok((status, v)) => {
                let msg = if status == 200 { format!("Unexpected answer from {}.", net::host_of(origin)) } else { grid_error(status, &v) };
                io.log(&format!("HTTP {status}: {msg}"));
                io.step(WAKE_STEP, StepStatus::Run, Some(&msg));
                hard_fails += 1;
                if hard_fails >= HARD_TRIES || Instant::now() >= deadline {
                    return Err(TransportFailure { status: Some(status), ..TransportFailure::new(msg) });
                }
            }
            Err(e) => {
                io.log(&e.message);
                io.step(WAKE_STEP, StepStatus::Run, Some(&e.message));
                hard_fails += 1;
                if hard_fails >= HARD_TRIES || Instant::now() >= deadline {
                    return Err(TransportFailure::new(e.message));
                }
            }
        }
        tokio::select! {
            () = cancel.cancelled() => return Err(TransportFailure::new("Disconnected.").final_()),
            () = tokio::time::sleep(WAKE_GAP) => {}
        }
        let _ = &last_err;
    }
}

impl Transport for GridTransport {
    fn kind(&self) -> &str {
        if self.tunnel { wisp_machines_api::GRID_TUNNEL } else { wisp_machines_api::GRID }
    }
    fn steps(&self) -> &'static [(&'static str, &'static str)] {
        if self.tunnel {
            &[(WAKE_STEP, "Reach it through the grid"), ("agents", "Find coding agents")]
        } else {
            &[(WAKE_STEP, "Wake it up"), ("agents", "Find coding agents")]
        }
    }
    fn clean(&self, input: &AddArgs, prev: Option<&PeerRow>) -> Failing<Cleaned> {
        clean(input, prev)
    }
    fn connect(&self, peer: PeerRow, io: Arc<dyn ConnectIo>, cancel: CancellationToken) -> BoxFuture<'static, Failing<Arc<dyn Channel>>> {
        let (client, tunnel) = (self.client.clone(), self.tunnel);
        Box::pin(async move {
            let o =
                origin(&peer).ok_or_else(|| TransportFailure::new("This machine has no address on the grid: sign in again.").final_())?;
            let token = io.secret().filter(|s| !s.is_empty()).ok_or_else(|| TransportFailure {
                kind: Some(FailKind::Gone),
                status: Some(401),
                ..TransportFailure::new("Signed out of this cloud computer.").with_hint(no_token_hint()).final_()
            })?;
            wake_and_check(&client, &o, &token, &peer.name, tunnel, io.as_ref(), &cancel).await?;
            let ch: Arc<dyn Channel> = GridChannel::new(&o, Some(token), client, cancel);
            Ok(ch)
        })
    }
    fn probe(&self, peer: PeerRow, io: Arc<dyn ConnectIo>) -> BoxFuture<'static, Probe> {
        let client = self.client.clone();
        Box::pin(async move {
            let Some(o) = origin(&peer) else { return Probe::Gone };
            match net::call(&client, "GET", &format!("{o}/_grid/state"), io.secret().as_deref(), None, &[]).await {
                Ok((200, v)) if v.get("state").and_then(serde_json::Value::as_str) == Some("running") => Probe::Running,
                Ok((401 | 404, _)) => Probe::Gone,
                _ => Probe::Asleep,
            }
        })
    }
    fn wakes_on_request(&self, _peer: &PeerRow) -> bool {
        !self.tunnel
    }
    fn direct(&self, peer: &PeerRow, io: Arc<dyn ConnectIo>) -> Option<Arc<dyn Channel>> {
        if self.tunnel {
            return None;
        }
        let o = origin(peer)?;
        let ch: Arc<dyn Channel> = GridChannel::new(&o, io.secret(), self.client.clone(), CancellationToken::new());
        Some(ch)
    }
    fn stop_remote(&self, peer: PeerRow, _pid: Option<u32>, io: Arc<dyn ConnectIo>) -> BoxFuture<'static, Failing<Stopped>> {
        let (client, tunnel) = (self.client.clone(), self.tunnel);
        Box::pin(async move {
            if tunnel {
                return Err(TransportFailure {
                    status: Some(400),
                    ..TransportFailure::new(format!("{} is turned off there, by its owner.", peer.name))
                });
            }
            let o = origin(&peer).ok_or_else(|| TransportFailure::new("no address"))?;
            match net::call(&client, "POST", &format!("{o}/_grid/stop"), io.secret().as_deref(), Some(&serde_json::json!({})), &[]).await {
                Ok((200, _)) => Ok(Stopped::Asleep),
                Ok((s, v)) => Err(TransportFailure {
                    status: Some(s),
                    ..TransportFailure::new(v.get("error").and_then(serde_json::Value::as_str).unwrap_or("the grid refused"))
                }),
                Err(e) => Err(TransportFailure::new(e.message)),
            }
        })
    }
    fn auto_start(&self, _peer: &PeerRow) -> AutoStart {
        if self.tunnel { AutoStart::Connect } else { AutoStart::Follow }
    }
}
