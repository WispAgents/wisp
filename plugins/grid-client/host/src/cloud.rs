//! wisp Cloud as the place shares go (ADR-0038): the `wisp-cloud` destination (`share.destination@1`), my
//! shares (list, access, revoke), and the other way: fetching a link (checking its signature), recording
//! what was taken in, and saying when a share moved on ("update available").
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde_json::{Value, json};
use wisp_grid_client_api::{AccessArgs, Fetched, Update};
use wisp_grid_client_core::{Link, grid_url, link};
use wisp_kernel::{BoxFuture, Result, WireError};
use wisp_share_api::{Outgoing, ShareDestination};

use crate::net;
use crate::state::{Import, St};

pub struct Cloud(pub Arc<St>);

impl std::fmt::Debug for Cloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cloud")
    }
}

impl ShareDestination for Cloud {
    fn id(&self) -> &'static str {
        "wisp-cloud"
    }
    fn label(&self) -> &'static str {
        "wisp Cloud"
    }
    fn ready(&self) -> BoxFuture<'static, std::result::Result<(), String>> {
        let st = self.0.clone();
        Box::pin(async move {
            st.signed_in()
                .await
                .map(|_| ())
                .map_err(|_| "Not signed in to wisp Cloud: run wisp cloud signin (or share to a file: --to file)".to_owned())
        })
    }
    fn send(&self, out: Outgoing) -> BoxFuture<'static, Result<Value>> {
        let st = self.0.clone();
        Box::pin(async move {
            let (a, token) = st.signed_in().await.map_err(WireError::invalid)?;
            let body = json!({
                "kind": out.kind, "key": out.key, "title": out.title, "summary": out.summary, "tags": out.tags,
                "body": B64.encode(&out.body), "signer": out.signer, "sig": out.sig,
                "access": out.access, "expiresAt": out.expires_at, "publish": out.publish,
            });
            send_at(&st.http, &a.grid, &token, &body).await
        })
    }
}

/// A share sent to `grid`. v1's grid (grid.trywisp.dev until v2's is deployed) takes only shared tasks, at
/// `PUT /api/shares/mine/<key>`, and answers v2's `POST /api/shares` with a bare 404 "not found", which the crew's
/// Share link toasted as it was: said here as what it is, and what to do instead.
pub async fn send_at(http: &reqwest::Client, grid: &str, token: &str, body: &Value) -> Result<Value> {
    let (status, v) = net::call(http, "POST", &format!("{grid}/api/shares"), Some(token), Some(body), &[]).await?;
    if status == 404 {
        return Err(WireError::invalid(format!(
            "{} can't hold this link yet: its grid runs an older wisp. Download or Copy the file instead.",
            net::host_of(grid)
        )));
    }
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

/// My shares on the grid: kind, access, views, imports, versions.
pub async fn mine(st: &St) -> Result<Value> {
    let (a, token) = st.signed_in().await.map_err(WireError::invalid)?;
    mine_at(&st.http, &a.grid, &token).await
}

/// Your shares on `grid`: v2's grid lists them at `/api/shares`, v1's (grid.trywisp.dev) at `/api/shares/mine` and
/// answers `/api/shares` with a bare 404 "not found", which Settings → Cloud toasted on every open (QA desktop r2 #5).
pub async fn mine_at(http: &reqwest::Client, grid: &str, token: &str) -> Result<Value> {
    let (mut status, mut v) = net::call(http, "GET", &format!("{grid}/api/shares"), Some(token), None, &[]).await?;
    if status == 404 {
        (status, v) = net::call(http, "GET", &format!("{grid}/api/shares/mine"), Some(token), None, &[]).await?;
    }
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

pub async fn access(st: &St, x: &AccessArgs) -> Result<Value> {
    let (a, token) = st.signed_in().await.map_err(WireError::invalid)?;
    let body = json!({ "access": x.access, "expiresAt": x.expires_at, "publish": x.publish });
    let (status, v) = net::call(&st.http, "PATCH", &format!("{}/api/shares/{}", a.grid, x.id), Some(&token), Some(&body), &[]).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

pub async fn revoke(st: &St, id: &str) -> Result<Value> {
    let (a, token) = st.signed_in().await.map_err(WireError::invalid)?;
    let (status, v) = net::call(&st.http, "DELETE", &format!("{}/api/shares/{id}", a.grid), Some(&token), None, &[]).await?;
    if status >= 400 {
        return Err(net::refused(status, &v));
    }
    Ok(v)
}

/// Whether `sig` is `signer`'s over `body`.
pub fn verify(signer: &str, sig: &str, body: &[u8]) -> bool {
    let (Ok(k), Ok(s)) = (B64.decode(signer.trim()), B64.decode(sig.trim())) else { return false };
    let (Ok(k), Ok(s)) = (<[u8; 32]>::try_from(k.as_slice()), <[u8; 64]>::try_from(s.as_slice())) else { return false };
    VerifyingKey::from_bytes(&k).is_ok_and(|vk| vk.verify(body, &Signature::from_bytes(&s)).is_ok())
}

/// The grid a link names (its own origin), else the one signed in to.
fn grid_of(st: &St, l: &Link) -> Result<String> {
    if let Link::Id { grid: Some(g), .. } = l {
        return Ok(g.clone());
    }
    st.account().map(|a| a.grid).or_else(|| grid_url(wisp_grid_client_api::DEFAULT_GRID)).ok_or_else(|| WireError::invalid("which grid?"))
}

/// The token of an account on `grid` (the current one if it's there, ADR-0061): a link on any grid signed in to
/// opens as that account; on another grid, as nobody.
async fn grid_token(st: &St, grid: &str) -> Option<String> {
    let key = st.accounts().on_grid(grid).map(crate::state::Account::key)?;
    st.token_for(&key).await
}

fn url_of(grid: &str, l: &Link) -> String {
    let q = |v: &Option<String>| v.as_ref().map(|r| format!("?ref={r}")).unwrap_or_default();
    match l {
        Link::Id { id, version, .. } => format!("{grid}/api/shares/{id}{}", q(version)),
        Link::Name { handle, name, version } => format!("{grid}/api/shares/by/{handle}/{name}{}", q(version)),
    }
}

/// A share, fetched and checked. Anyone may fetch an `anyone` link; signed in, people and teams shares too.
pub async fn fetch(st: &St, raw: &str, password: Option<&str>) -> Result<Fetched> {
    let l = link(raw).ok_or_else(|| WireError::invalid(format!("{raw} isn't a share link")))?;
    let grid = grid_of(st, &l)?;
    let token = grid_token(st, &grid).await;
    let headers: Vec<(&str, &str)> = password.map(|p| vec![("x-wisp-share-password", p)]).unwrap_or_default();
    let (status, v) = net::call(&st.http, "GET", &url_of(&grid, &l), token.as_deref(), None, &headers).await?;
    if status >= 400 {
        let mut e = net::refused(status, &v);
        if let Some(how) = v.get("locked").and_then(Value::as_str) {
            e.message = match how {
                "password" => "This share has a password: --password".into(),
                "people" => "This share is for some people only: sign in as one of them (wisp cloud signin)".into(),
                _ => "This share is for a team: sign in as a member (wisp cloud signin)".into(),
            };
            // what locks it, for a GUI to ask in its own words (a password box, Sign in) instead of the CLI's
            e = e.with_details(json!({ "locked": how }));
        }
        return Err(e);
    }
    let body = B64.decode(v.get("body").and_then(Value::as_str).unwrap_or_default()).unwrap_or_default();
    let (sig, signer) =
        (v.get("sig").and_then(Value::as_str).unwrap_or_default(), v.get("signer").and_then(Value::as_str).unwrap_or_default());
    let content: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    Ok(Fetched {
        card: v.get("card").cloned().unwrap_or(Value::Null),
        version: n("version"),
        latest: n("latest"),
        content,
        verified: verify(signer, sig, &body),
        signer: signer.to_owned(),
    })
}

/// Taken in: counted on the grid, and remembered here for "update available".
pub async fn imported(st: &St, raw: &str, version: i64) -> Result<Value> {
    let f = fetch(st, raw, None).await?;
    let id = f.card.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
    let l = link(raw).ok_or_else(|| WireError::invalid("not a link"))?;
    let grid = grid_of(st, &l)?;
    let token = grid_token(st, &grid).await;
    // counted on the grid as taken in; the import stands without it
    crate::calls::warn(
        "the grid's import count",
        net::call(&st.http, "POST", &format!("{grid}/api/shares/{id}/import"), token.as_deref(), Some(&json!({})), &[]).await,
    );
    let s = |k: &str| f.card.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    st.save_import(Import {
        id,
        grid,
        url: s("url"),
        title: s("title"),
        kind: s("kind"),
        version: if version > 0 { version } else { f.version },
    })
    .await?;
    Ok(json!({ "ok": true }))
}

/// The imports whose share has a newer version than the one taken.
pub async fn updates(st: &St) -> Result<Vec<Update>> {
    let mut out = Vec::new();
    for i in st.imports().await? {
        let Ok(f) = fetch(st, &i.url, None).await else { continue };
        if f.latest > i.version {
            out.push(Update { id: i.id, url: i.url, title: i.title, kind: i.kind, have: i.version, latest: f.latest });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;

    #[test]
    fn a_fetched_share_is_checked_against_its_signer() {
        let k = SigningKey::from_bytes(&[3; 32]);
        let (signer, sig) = (B64.encode(k.verifying_key().to_bytes()), B64.encode(k.sign(b"files").to_bytes()));
        assert!(verify(&signer, &sig, b"files"));
        assert!(!verify(&signer, &sig, b"other files"));
        assert!(!verify("nope", &sig, b"files"));
        let l = link("grid://ada/review?ref=v1").unwrap_or(Link::Id { grid: None, id: String::new(), version: None });
        assert_eq!(url_of("http://g", &l), "http://g/api/shares/by/ada/review?ref=v1");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod v1_shares_tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use http_body_util::Full;
    use hyper_util::rt::TokioIo;
    use serde_json::json;

    /// A grid answering only v1's list (`GET /api/shares/mine`; `/api/shares` itself is a 404 there).
    async fn v1_grid() -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else { return };
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(|req: http::Request<hyper::body::Incoming>| async move {
                        let (status, v) = match req.uri().path() {
                            "/api/shares/mine" => (
                                200,
                                json!({ "enabled": true, "maxBytes": 1, "shares": [{ "id": "s1", "url": "https://g/s/s1", "title": "Plan" }] }),
                            ),
                            _ => (404, json!({ "error": "not found" })),
                        };
                        let mut r = http::Response::new(Full::new(Bytes::from(v.to_string())));
                        *r.status_mut() = http::StatusCode::from_u16(status).unwrap();
                        Ok::<_, std::convert::Infallible>(r)
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(sock), svc).await;
                });
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    /// QA desktop r2 #5: Settings → Cloud toasted a bare "not found" each time it opened: grid.trywisp.dev (v1's grid)
    /// lists your shares at `/api/shares/mine`, and wisp asked `/api/shares`. Asked again there, the list comes back.
    #[tokio::test]
    async fn a_v1_grid_lists_your_shares_at_its_own_route() {
        let grid = v1_grid().await;
        let http = Arc::new(crate::net::client(false));
        let v = super::mine_at(&http, &grid, "wgd_d").await.unwrap();
        assert_eq!(v["shares"][0]["id"], "s1", "{v}");
    }

    /// The crew's Share link toasted "not found": v1's grid has no `POST /api/shares`. The refusal says why and
    /// what still works, and isn't a `not_found` (nothing the user asked for is missing).
    #[tokio::test]
    async fn a_v1_grid_refuses_a_share_in_words() {
        let grid = v1_grid().await;
        let http = Arc::new(crate::net::client(false));
        let e = super::send_at(&http, &grid, "wgd_d", &json!({ "kind": "crew" })).await.unwrap_err();
        assert_eq!(e.code, wisp_kernel::Code::Invalid, "{e:?}");
        assert!(e.message.contains("older wisp") && e.message.contains("Download or Copy"), "{}", e.message);
    }
}
