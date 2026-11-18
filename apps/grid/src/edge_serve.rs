//! Serving the grid: its accept loop, the inbox's sweep, and a grid for development and tests.
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use super::{Grid, common};
use crate::db::Db;
use crate::error::{Result, bad};
use crate::hostd::ProcessHost;
use crate::http::Body;

/// After a restart, everyone with something held is drained; every minute what's past its hour goes.
fn inbox_loop(grid: &Arc<Grid>, stop: CancellationToken) {
    if !grid.inbox.opts.enabled {
        return;
    }
    for uid in grid.inbox.waiting().unwrap_or_default() {
        if let Ok(Some(u)) = grid.accounts.user_by_id(&uid) {
            grid.drain_later(u);
        }
    }
    let g = grid.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = stop.cancelled() => return,
                () = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            }
            if let Err(e) = g.inbox.sweep() {
                tracing::warn!("grid: inbox sweep: {e}");
            }
            if let Err(e) = g.routes.sweep() {
                tracing::warn!("grid: route sweep: {e}");
            }
        }
    });
}

/// Serves `grid` on `listener` until `stop`.
pub async fn serve(grid: Arc<Grid>, listener: TcpListener, stop: CancellationToken) {
    inbox_loop(&grid, stop.clone());
    tokio::spawn(grid.follow_boxes(stop.clone()));
    grid.restore_boxes();
    loop {
        let (sock, addr): (tokio::net::TcpStream, SocketAddr) = tokio::select! {
            () = stop.cancelled() => return,
            a = listener.accept() => match a { Ok(a) => a, Err(_) => continue },
        };
        let g = grid.clone();
        tokio::spawn(async move {
            let peer = common::Peer(addr.ip());
            let svc = hyper::service::service_fn(move |mut req: http::Request<Incoming>| {
                let g = g.clone();
                req.extensions_mut().insert(peer);
                async move { Ok::<_, std::convert::Infallible>(g.handle(req).await) }
            });
            let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(sock), svc)
                .await;
        });
    }
}

/// A grid on this machine for development and tests: `auth.dev` on, boxes as `wisp serve` processes.
pub async fn dev(
    data: &std::path::Path,
    port: u16,
    wisp_bin: std::path::PathBuf,
    env: Vec<(String, String)>,
    idle_ms: i64,
) -> Result<(Arc<Grid>, u16, CancellationToken)> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| bad(format!("can't listen on {port}: {e}"), 500))?;
    let port = listener.local_addr().map_err(|e| bad(e.to_string(), 500))?.port();
    let cfg = crate::config::for_test(
        data,
        json!({ "domain": "wisp.localhost", "listen": { "host": "127.0.0.1", "port": port }, "auth": { "dev": true } }),
    )?;
    let clock = crate::clock::system();
    let db = Arc::new(Db::open(data, clock.now_ms())?);
    let url_cfg = cfg.clone();
    let boxes = ProcessHost::new(
        crate::hostd::ProcessOptions {
            bin: wisp_bin,
            data: data.to_owned(),
            idle_ms,
            check_ms: 1000,
            start_ms: 30_000,
            max_running: 8,
            env,
            url_of: Arc::new(move |h| url_cfg.instance_url(h)),
            boxd: None,
            previews: false,
        },
        clock.clone(),
    );
    let grid = Grid::new(cfg, db, clock, boxes);
    let stop = CancellationToken::new();
    tokio::spawn(serve(grid.clone(), listener, stop.clone()));
    Ok((grid, port, stop))
}

/// Bytes, for tests that want a request body.
pub fn bytes_body(b: impl Into<Bytes>) -> Body {
    crate::http::full(b)
}
