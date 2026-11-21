//! A device signs itself out (`POST /api/device/revoke`, v1 grid/server.js, ADR-0078): the phone's and the desktop's
//! Sign out tell the grid, so the token stops working there and then, not only once the person removes the device on
//! the account page. The token that asks is the one revoked, and only it; asking twice is fine; it's audited as the
//! account page's Remove is.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::{Value, json};
use wisp_grid::accounts::Poll;

struct G {
    grid: std::sync::Arc<wisp_grid::edge::Grid>,
    port: u16,
    _stop: tokio_util::sync::CancellationToken,
    _dir: PathBuf,
}

async fn grid(name: &str) -> G {
    let dir = std::env::temp_dir().join(format!("wisp-grid-revoke-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (grid, port, stop) = wisp_grid::edge::dev(&dir, 0, PathBuf::from("/nonexistent/wisp"), Vec::new(), 600_000).await.unwrap();
    G { grid, port, _stop: stop, _dir: dir }
}

impl G {
    fn url(&self, path: &str) -> String {
        format!("http://wisp.localhost:{}{path}", self.port)
    }
    fn web(&self) -> reqwest::Client {
        reqwest::Client::builder().no_proxy().resolve("wisp.localhost", ([127, 0, 0, 1], self.port).into()).build().unwrap()
    }
    /// A device token for `handle` named `name` (the device flow, approved at once).
    fn device(&self, handle: &str, name: &str) -> String {
        let a = &self.grid.accounts;
        let u = a.dev_user(handle).unwrap();
        let c = a.device_code(name).unwrap();
        a.decide(c["user_code"].as_str().unwrap(), &u.id, true).unwrap();
        match a.poll(c["device_code"].as_str().unwrap()).unwrap() {
            Poll::Token { token, .. } => token,
            other => panic!("{other:?}"),
        }
    }
    fn audit(&self) -> Vec<String> {
        self.grid
            .db
            .with(|c| {
                c.prepare("SELECT action || ' ' || meta FROM audit WHERE action = 'device.revoke'")?.query_map([], |r| r.get(0))?.collect()
            })
            .unwrap()
    }
    async fn me(&self, token: &str) -> u16 {
        self.web().get(self.url("/api/me")).bearer_auth(token).send().await.unwrap().status().as_u16()
    }
    async fn revoke(&self, token: &str, body: Value) -> (u16, Value) {
        let r = self.web().post(self.url("/api/device/revoke")).bearer_auth(token).json(&body).send().await.unwrap();
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_revoked_token_is_refused_on_the_next_call() {
    let g = grid("next").await;
    let phone = g.device("kai", "Phone (wisp app)");
    assert_eq!(g.me(&phone).await, 200);
    // its computer goes with it
    let made = g.web().post(g.url("/api/computers")).bearer_auth(&phone).json(&json!({ "name": "phone" })).send().await.unwrap();
    assert_eq!(made.status().as_u16(), 200);
    let laptop = g.device("kai", "Laptop (wisp)");
    let listed = |v: Value| v.as_array().map_or(0, Vec::len);
    let before = g.web().get(g.url("/api/computers")).bearer_auth(&laptop).send().await.unwrap().json::<Value>().await.unwrap();
    assert_eq!(listed(before), 1);

    assert_eq!(g.revoke(&phone, json!({})).await, (200, json!({ "ok": true, "revoked": true })));
    assert_eq!(g.me(&phone).await, 401, "the next call with it is refused");
    let after = g.web().get(g.url("/api/computers")).bearer_auth(&phone).send().await.unwrap();
    assert_eq!(after.status().as_u16(), 401);
    let left = g.web().get(g.url("/api/computers")).bearer_auth(&laptop).send().await.unwrap().json::<Value>().await.unwrap();
    assert_eq!(listed(left), 0, "its computer is off the grid");

    // audited as the account page's Remove, by its id, never the token
    let lines = g.audit();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"by\":\"device\"") && !lines[0].contains(&phone[4..]), "{lines:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_cant_revoke_another_devices_token() {
    let g = grid("other").await;
    let laptop = g.device("kai", "Laptop (wisp)");
    let phone = g.device("kai", "Phone (wisp app)");
    let bob = g.device("bob", "Laptop (wisp)");
    let phone_id = g.grid.accounts.device(&phone).unwrap().unwrap().1.id;

    // naming another device in the body changes nothing: the token that asks is the one revoked
    assert_eq!(g.revoke(&laptop, json!({ "id": phone_id, "token": bob })).await.0, 200);
    assert_eq!((g.me(&laptop).await, g.me(&phone).await, g.me(&bob).await), (401, 200, 200));

    // a token the grid never handed out, an API-key-shaped one, or none at all revokes nothing
    for t in ["wgd_notatoken", "wgk_notadevice", ""] {
        assert_eq!(g.revoke(t, json!({})).await.0, 401, "{t}");
    }
    assert_eq!((g.me(&phone).await, g.me(&bob).await), (200, 200));
    assert_eq!(g.audit().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn revoking_twice_is_fine() {
    let g = grid("twice").await;
    let phone = g.device("kai", "Phone (wisp app)");
    assert_eq!(g.revoke(&phone, json!({})).await, (200, json!({ "ok": true, "revoked": true })));
    // the first answer lost on a bad network: the phone asks again and hears it's done
    assert_eq!(g.revoke(&phone, json!({})).await, (200, json!({ "ok": true, "revoked": false })));
    assert_eq!(g.me(&phone).await, 401);
    assert_eq!(g.audit().len(), 1, "one revocation, one line");

    // the account page's Remove of a device already signed out says so and adds no line
    let kai = g.grid.accounts.dev_user("kai").unwrap();
    let id = g.grid.db.with(|c| c.query_row("SELECT id FROM devices WHERE user_id = ?1", [&kai.id], |r| r.get::<_, String>(0))).unwrap();
    assert!(!g.grid.accounts.revoke_device(&kai.id, &id).unwrap().gone);
    assert_eq!(g.audit().len(), 1);
}
