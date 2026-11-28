//! Grid URLs for this computer's webhook triggers (ADR-0147): `https://<computer>--<handle>.<domain>/_hook/wgh_…`, made
//! by the grid's `/api/hooks` with this computer's device token, for this computer only. A call to one reaches
//! `POST /api/triggers/<trigger>` here through the tunnel (`computer.rs`, marked `x-wisp-route: hook`). Like an API key
//! (`keys.rs`), a URL is in `hooksCreate`'s answer and nowhere else: no row, no topic, no log line.

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_grid_client_api as api;
use wisp_kernel::{Result, WireError};

use http_body_util::BodyExt as _;
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;

use crate::computer::{Body, answer};
use crate::net;
use crate::state::St;

/// The grid's id for a URL (hex): anything else names none, and never reaches the grid's path.
fn hook_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(WireError::not_found("No such URL."));
    }
    Ok(id)
}

/// A trigger's name as the grid takes it: one path segment, 1–128 bytes.
fn trigger_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() {
        return Err(WireError::invalid("trigger: name the webhook trigger this URL starts"));
    }
    if n.len() > 128 || n.contains('/') || n.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(WireError::invalid("trigger: that isn’t a trigger’s name"));
    }
    Ok(n)
}

/// One call to `<grid>/api/hooks…`. A 404 on a list or a make is a grid from before trigger URLs; a 409 is this
/// computer not on the grid yet.
async fn ask(http: &reqwest::Client, grid: &str, token: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    // a team hub's box asks with its own secret (`Box <hub> <secret>`), sent as it is; anyone else with a device token
    let (bearer, auth) = if token.starts_with("Box ") { (None, vec![("authorization", token)]) } else { (Some(token), Vec::new()) };
    let (status, v) = net::call(http, method, &format!("{grid}/api/hooks{path}"), bearer, body, &auth).await?;
    if status == 404 && method != "DELETE" {
        return Err(WireError::unavailable(format!(
            "{} doesn’t make webhook trigger URLs yet (its wisp Grid is older).",
            net::host_of(grid)
        )));
    }
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

/// This computer's live URLs, newest first; `trigger` non-empty: that trigger's only.
pub async fn list_at(http: &reqwest::Client, grid: &str, token: &str, trigger: &str) -> Result<Vec<api::TriggerHook>> {
    #[derive(serde::Deserialize)]
    struct Answer {
        hooks: Vec<api::TriggerHook>,
    }
    let v = ask(http, grid, token, "GET", "", None).await?;
    let a: Answer = serde_json::from_value(v).map_err(|e| {
        tracing::warn!(error = %e, "grid-client: the grid's trigger URLs");
        WireError::internal(format!("{} answered with a list of trigger URLs wisp can’t read.", net::host_of(grid)))
    })?;
    let t = trigger.trim();
    Ok(a.hooks.into_iter().filter(|h| t.is_empty() || h.trigger == t).collect())
}

pub async fn create_at(http: &reqwest::Client, grid: &str, token: &str, trigger: &str) -> Result<api::NewHook> {
    let trigger = trigger_name(trigger)?;
    let v = ask(http, grid, token, "POST", "", Some(&json!({ "trigger": trigger }))).await?;
    let made: api::NewHook = serde_json::from_value(v).map_err(|_| WireError::internal("the grid made a URL but didn't say it"))?;
    if !made.url.contains("/_hook/wgh_") || made.id.is_empty() {
        return Err(WireError::internal("the grid made a URL but didn't say it"));
    }
    Ok(made)
}

pub async fn revoke_at(http: &reqwest::Client, grid: &str, token: &str, id: &str) -> Result<Value> {
    let id = hook_id(id)?;
    ask(http, grid, token, "DELETE", &format!("/{id}"), None).await
}

/// The trigger URLs are this computer's: it has to be on the grid (`wisp computer on`) for one to reach it.
async fn on_the_grid(st: &Arc<St>) -> Result<(String, String)> {
    // a team hub's box (ADR-0148): its own hub's trigger URLs, with its box secret (the grid takes it from a hub only),
    // whatever sign-in it holds: a hub's URL starts work on the hub, never on a computer
    if std::env::var("WISP_HUB").as_deref() == Ok("1")
        && let Some(b) = crate::sandboxes::box_asker(&|k| std::env::var(k).ok())
    {
        return Ok((b.grid, b.auth));
    }
    let (a, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    if !a.computer_on {
        return Err(WireError::invalid(
            "Put this computer on the grid first (Settings → Cloud, or wisp computer on): a trigger URL reaches it through the grid.",
        ));
    }
    Ok((a.grid, token))
}

pub async fn list(st: &Arc<St>, trigger: &str) -> Result<Vec<api::TriggerHook>> {
    let (grid, token) = on_the_grid(st).await?;
    list_at(&st.http, &grid, &token, trigger).await
}

pub async fn create(st: &Arc<St>, trigger: &str) -> Result<api::NewHook> {
    let (grid, token) = on_the_grid(st).await?;
    create_at(&st.http, &grid, &token, trigger).await
}

pub async fn revoke(st: &Arc<St>, id: &str) -> Result<Value> {
    let (grid, token) = on_the_grid(st).await?;
    revoke_at(&st.http, &grid, &token, id).await
}

/// A webhook trigger's call through the tunnel, marked `x-wisp-route: hook` by the grid alone (its edge drops the mark
/// from everything else): `POST /api/triggers/<name>` here and nothing else, as no one. Without the tunnel's mark, wisp
/// here takes an open route with no token as `public`, and the trigger checks its own secret or signature.
pub async fn forward(
    local: &Client<HttpConnector, Body>,
    port: u16,
    parts: &http::request::Parts,
    from: &[(String, String)],
    body: Incoming,
) -> http::Response<Body> {
    if !wisp_grid_client_core::is_hook_call(parts.method.as_str(), parts.uri.path()) {
        return answer(404, &json!({ "error": "not found" }));
    }
    let pq = parts.uri.path_and_query().map_or("/", |p| p.as_str());
    let mut b = http::Request::builder().method(http::Method::POST).uri(format!("http://127.0.0.1:{port}{pq}"));
    for (k, v) in wisp_grid_client_core::hook_headers(from, port) {
        b = b.header(k, v);
    }
    let Ok(out) = b.body(body.map_err(std::io::Error::other).boxed()) else { return answer(400, &json!({ "error": "bad request" })) };
    match local.request(out).await {
        Ok(r) => {
            let (mut p, b) = r.into_parts();
            for h in wisp_grid_client_core::HOP {
                p.headers.remove(*h);
            }
            p.headers.remove("set-cookie");
            http::Response::from_parts(p, b.map_err(std::io::Error::other).boxed())
        }
        Err(e) => answer(502, &json!({ "error": format!("wisp here didn’t answer: {e}") })),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::many_single_char_names)]
mod tests {
    use std::sync::Mutex;

    use bytes::Bytes;
    use http_body_util::Full;
    use hyper_util::rt::TokioIo;
    use wisp_kernel::Code;

    use super::*;

    type Seen = Arc<Mutex<Vec<String>>>;

    /// A grid answering `/api/hooks` as apps/grid does.
    async fn fake_grid() -> (String, Seen) {
        let seen: Seen = Arc::default();
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let kept = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else { return };
                let seen = kept.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
                        let seen = seen.clone();
                        async move {
                            let (m, p) = (req.method().to_string(), req.uri().path().to_owned());
                            let auth = req.headers().get("authorization").map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
                            let body = String::from_utf8_lossy(&req.into_body().collect().await.unwrap().to_bytes()).into_owned();
                            seen.lock().unwrap().push(format!("{m} {p} {auth} {body}"));
                            let row = |id: &str, t: &str| json!({ "id": id, "trigger": t, "computer": "studio", "hint": "Zq9x", "createdAt": 5, "lastUsedAt": null });
                            let (status, v) = match (m.as_str(), p.as_str()) {
                                ("GET", "/api/hooks") => {
                                    (200, json!({ "hooks": [row("a1b2c3d4e5f6", "deploy"), row("b1b2c3d4e5f6", "ci")] }))
                                }
                                ("POST", "/api/hooks") => (
                                    201,
                                    json!({ "id": "0f0f0f0f0f0f", "trigger": "deploy", "computer": "studio", "hint": "AbCd", "createdAt": 9, "lastUsedAt": null,
                                            "url": "https://studio--ada.example.dev/_hook/wgh_secretAbCd", "token": "wgh_secretAbCd" }),
                                ),
                                ("DELETE", "/api/hooks/a1b2c3d4e5f6") => (200, json!({ "ok": true })),
                                _ => (404, json!({ "error": "No such URL." })),
                            };
                            let mut r = http::Response::new(Full::new(Bytes::from(v.to_string())));
                            *r.status_mut() = http::StatusCode::from_u16(status).unwrap();
                            Ok::<_, std::convert::Infallible>(r)
                        }
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(sock), svc).await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}"), seen)
    }

    #[tokio::test]
    async fn trigger_urls_list_make_and_revoke_ask_the_grid_with_the_device_token() {
        let (grid, seen) = fake_grid().await;
        let http = net::client(false);
        let t = "wgd_device";

        let all = list_at(&http, &grid, t, "").await.unwrap();
        assert_eq!(all.len(), 2);
        let one = list_at(&http, &grid, t, " deploy ").await.unwrap();
        assert_eq!(one.iter().map(|h| h.trigger.as_str()).collect::<Vec<_>>(), ["deploy"]);

        let made = create_at(&http, &grid, t, " deploy ").await.unwrap();
        assert_eq!((made.url.as_str(), made.hint.as_str()), ("https://studio--ada.example.dev/_hook/wgh_secretAbCd", "AbCd"));
        assert!(!serde_json::to_string(&made).unwrap().contains("\"token\""), "the URL is the only place the token is");

        assert_eq!(revoke_at(&http, &grid, t, "a1b2c3d4e5f6").await.unwrap(), json!({ "ok": true }));
        let gone = revoke_at(&http, &grid, t, "ffffffffffff").await.unwrap_err();
        assert_eq!((gone.code, gone.message.as_str()), (Code::NotFound, "No such URL."));
        let never = revoke_at(&http, &grid, t, "../keys").await.unwrap_err();
        assert_eq!(never.code, Code::NotFound, "an id that isn't one never reaches the grid's path");
        let bad = create_at(&http, &grid, t, "a/b").await.unwrap_err();
        assert_eq!(bad.code, Code::Invalid);

        let seen = seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            [
                "GET /api/hooks Bearer wgd_device ",
                "GET /api/hooks Bearer wgd_device ",
                "POST /api/hooks Bearer wgd_device {\"trigger\":\"deploy\"}",
                "DELETE /api/hooks/a1b2c3d4e5f6 Bearer wgd_device ",
                "DELETE /api/hooks/ffffffffffff Bearer wgd_device ",
            ]
        );
    }
}
