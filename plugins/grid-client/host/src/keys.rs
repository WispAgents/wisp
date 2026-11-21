//! API keys for scripting the cloud wisp (ADR-0058, v1 server/services/api-keys.js): `Authorization: Bearer wgk_…`
//! at `https://<handle>.<domain>/api/…`. They live on the grid, hashed; this asks the grid's `/api/keys` with this
//! computer's device token and keeps nothing: a new key is in `keysCreate`'s answer and nowhere else (no row, no
//! topic, no log line). A box never reaches these (the grid refuses its secret: `trust`), and an API key can't
//! either (the grid refuses it; the box's wisp gives a key's caller the `apikey` set, which names none of them).

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_grid_client_api as api;
use wisp_kernel::{Result, WireError};

use crate::net;
use crate::state::St;

/// The grid's id for a key (hex): anything else names no key, and never reaches the grid's path.
fn key_id(id: &str) -> Result<&str> {
    let id = id.trim();
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(WireError::not_found("No such key."));
    }
    Ok(id)
}

/// A name as the grid takes it: 1–60 characters, one line.
fn key_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() {
        return Err(WireError::invalid("name: give the key a name"));
    }
    if n.chars().count() > 60 {
        return Err(WireError::invalid("name: keep a key’s name to 60 characters"));
    }
    if n.chars().any(char::is_control) {
        return Err(WireError::invalid("name: one line of text"));
    }
    Ok(n)
}

/// One call to `<grid>/api/keys…`. A 404 on a list or a create is a grid from before keys.
async fn ask(http: &reqwest::Client, grid: &str, token: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let (status, v) = net::call(http, method, &format!("{grid}/api/keys{path}"), Some(token), body, &[]).await?;
    if status == 404 && method != "DELETE" {
        return Err(WireError::unavailable(format!("{} doesn’t make API keys yet (its wisp Grid is older).", net::host_of(grid))));
    }
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

/// `handle`: the account's, for the cloud wisp's address when the grid doesn't say it (v1's grid sends `{ keys }` only).
pub async fn list_at(http: &reqwest::Client, grid: &str, token: &str, handle: &str) -> Result<api::Keys> {
    let v = ask(http, grid, token, "GET", "", None).await?;
    keys_of(v, grid, handle)
}

/// The grid's answer, typed: a field the grid adds (or a key it should never send) doesn't come through. With no
/// `address`, the cloud wisp's (`<scheme>://<handle>.<grid's host>`, as both grids make it). A shape wisp can't read
/// says so in plain words, never serde's (QA r1 #4: "the grid's keys: missing field `address`").
fn keys_of(v: Value, grid: &str, handle: &str) -> Result<api::Keys> {
    #[derive(serde::Deserialize)]
    struct Answer {
        keys: Vec<api::ApiKey>,
        #[serde(default)]
        address: String,
    }
    let a: Answer = serde_json::from_value(v).map_err(|e| {
        tracing::warn!(error = %e, "grid-client: the grid's keys list");
        WireError::internal(format!("{} answered with a list of keys wisp can’t read.", net::host_of(grid)))
    })?;
    let address = if a.address.is_empty() && !handle.is_empty() {
        wisp_grid_client_core::label_host(grid, handle)
            .map(|h| format!("{}://{h}", grid.split("://").next().unwrap_or("https")))
            .unwrap_or_default()
    } else {
        a.address
    };
    Ok(api::Keys { keys: a.keys, address })
}

pub async fn create_at(http: &reqwest::Client, grid: &str, token: &str, name: &str) -> Result<api::NewKey> {
    let name = key_name(name)?;
    let v = ask(http, grid, token, "POST", "", Some(&json!({ "name": name }))).await?;
    let made: api::NewKey = serde_json::from_value(v).map_err(|_| WireError::internal("the grid made a key but didn't say it"))?;
    if !made.key.starts_with("wgk_") || made.id.is_empty() {
        return Err(WireError::internal("the grid made a key but didn't say it"));
    }
    Ok(made)
}

pub async fn revoke_at(http: &reqwest::Client, grid: &str, token: &str, id: &str) -> Result<Value> {
    let id = key_id(id)?;
    ask(http, grid, token, "DELETE", &format!("/{id}"), None).await
}

pub async fn list(st: &Arc<St>) -> Result<api::Keys> {
    let (a, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    list_at(&st.http, &a.grid, &token, &a.handle).await
}

pub async fn create(st: &Arc<St>, name: &str) -> Result<api::NewKey> {
    let (a, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    create_at(&st.http, &a.grid, &token, name).await
}

pub async fn revoke(st: &Arc<St>, id: &str) -> Result<Value> {
    let (a, token) = st.signed_in().await.map_err(WireError::unavailable)?;
    revoke_at(&st.http, &a.grid, &token, id).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::many_single_char_names)]
mod tests {
    use std::sync::Mutex;

    use bytes::Bytes;
    use http_body_util::{BodyExt as _, Full};
    use hyper_util::rt::TokioIo;
    use wisp_kernel::Code;

    use super::*;

    type Seen = Arc<Mutex<Vec<String>>>;

    /// A grid answering `/api/keys` as apps/grid does (`old`: one from before keys, 404 everywhere; `v1`: v1's
    /// grid/server.js, whose list is `{ keys }` with no `address`).
    async fn fake_grid(old: bool) -> (String, Seen) {
        fake_grid_as(old, false).await
    }

    async fn fake_grid_as(old: bool, v1: bool) -> (String, Seen) {
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
                            let (status, v) = match (old, m.as_str(), p.as_str()) {
                                (true, ..) => (404, json!({ "error": "not found" })),
                                (_, "GET", "/api/keys") if v1 => (
                                    200,
                                    json!({ "keys": [{ "id": "a1b2c3d4e5f6", "name": "ci", "hint": "Zq9x", "createdAt": 5, "lastUsedAt": null }] }),
                                ),
                                (_, "GET", "/api/keys") => (
                                    200,
                                    json!({ "keys": [{ "id": "a1b2c3d4e5f6", "name": "ci", "hint": "Zq9x", "createdAt": 5, "lastUsedAt": null, "key": "wgk_never" }], "address": "https://ada.example.dev" }),
                                ),
                                (_, "POST", "/api/keys") => (
                                    201,
                                    json!({ "id": "0f0f0f0f0f0f", "name": "deploy", "hint": "AbCd", "createdAt": 9, "lastUsedAt": null, "key": "wgk_secretAbCd" }),
                                ),
                                (_, "DELETE", "/api/keys/a1b2c3d4e5f6") => (200, json!({ "ok": true })),
                                _ => (404, json!({ "error": "No such key." })),
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
    async fn keys_list_create_and_revoke_ask_the_grid_with_the_device_token() {
        let (grid, seen) = fake_grid(false).await;
        let http = net::client(false);
        let t = "wgd_device";

        let got = list_at(&http, &grid, t, "ada").await.unwrap();
        assert_eq!(got.address, "https://ada.example.dev");
        assert_eq!(
            got.keys,
            vec![api::ApiKey { id: "a1b2c3d4e5f6".into(), name: "ci".into(), hint: "Zq9x".into(), created_at: 5, last_used_at: None }]
        );
        assert!(!serde_json::to_string(&got).unwrap().contains("wgk_"), "a list never carries a key, even one the grid sent");

        let made = create_at(&http, &grid, t, "  deploy ").await.unwrap();
        assert_eq!((made.key.as_str(), made.hint.as_str()), ("wgk_secretAbCd", "AbCd"));

        assert_eq!(revoke_at(&http, &grid, t, "a1b2c3d4e5f6").await.unwrap(), json!({ "ok": true }));
        let gone = revoke_at(&http, &grid, t, "ffffffffffff").await.unwrap_err();
        assert_eq!((gone.code, gone.message.as_str()), (Code::NotFound, "No such key."));

        let seen = seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            [
                "GET /api/keys Bearer wgd_device ",
                "POST /api/keys Bearer wgd_device {\"name\":\"deploy\"}",
                "DELETE /api/keys/a1b2c3d4e5f6 Bearer wgd_device ",
                "DELETE /api/keys/ffffffffffff Bearer wgd_device ",
            ]
        );
    }

    /// QA r1 #4: grid.trywisp.dev (v1's grid) answers the list with no `address`; Settings → Cloud read "the grid's
    /// keys: missing field `address`". Its keys list, and the address is the cloud wisp's (`<handle>.<grid's domain>`,
    /// as both grids make it); a shape wisp can't read says so in plain words.
    #[tokio::test]
    async fn a_v1_grids_keys_list_with_no_address_reads_with_the_cloud_wisps_address() {
        let (grid, _) = fake_grid_as(false, true).await;
        let http = net::client(false);
        let got = list_at(&http, &grid, "wgd_d", "ada").await.unwrap();
        assert_eq!(got.keys.len(), 1);
        assert_eq!(got.address, format!("http://ada.{}", grid.trim_start_matches("http://")));
        let e = keys_of(json!({ "keys": "nope" }), &grid, "ada").unwrap_err();
        assert!(!e.message.contains("missing field") && !e.message.contains("invalid type"), "{}", e.message);
    }

    #[tokio::test]
    async fn a_bad_name_or_id_never_reaches_the_grid_and_an_older_grid_says_so() {
        let (grid, seen) = fake_grid(false).await;
        let http = net::client(false);
        for name in ["", "   ", &"x".repeat(61), "a\nb"] {
            assert_eq!(create_at(&http, &grid, "wgd_d", name).await.unwrap_err().code, Code::Invalid, "{name:?}");
        }
        for id in ["", "../me", "a1b2/../../x", &"a".repeat(33)] {
            assert_eq!(revoke_at(&http, &grid, "wgd_d", id).await.unwrap_err().code, Code::NotFound, "{id:?}");
        }
        assert!(seen.lock().unwrap().is_empty(), "{:?}", seen.lock().unwrap());

        let (old, _) = fake_grid(true).await;
        for e in
            [list_at(&http, &old, "wgd_d", "ada").await.unwrap_err(), create_at(&http, &old, "wgd_d", "ci").await.map(|_| ()).unwrap_err()]
        {
            assert_eq!(e.code, Code::Unavailable);
            assert!(e.message.contains("doesn’t make API keys yet"), "{}", e.message);
        }
    }
}
