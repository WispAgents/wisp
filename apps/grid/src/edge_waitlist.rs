//! `POST /api/waitlist {email, source}` on the grid's own address (v1 grid/server.js): public, for trywisp.dev's
//! "get the next seat" form and an in-app browser's "email me the link" (`source: "in-app"`). With
//! `kind: "phone-app"` (and `platform: ios|android|both`) it's trywisp.dev's *Get the app* instead: the phone apps' list. Other origins post it,
//! so it answers with v1's CORS headers (and its preflight); at most 10 a minute from one address.
use http::HeaderMap;
use serde_json::{Value, json};

use super::Grid;
use crate::error::Result;
use crate::http::{Resp, client_ip, json_with, parse_body, respond};

/// v1's CORS for the grid's public API: any origin may call it, nothing rides on cookies.
const CORS: [(&str, &str); 4] = [
    ("access-control-allow-origin", "*"),
    ("access-control-allow-headers", "authorization, content-type, x-wisp-passkey"),
    ("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS"),
    ("access-control-max-age", "600"),
];

fn cors() -> Vec<(&'static str, String)> {
    CORS.iter().map(|(k, v)| (*k, (*v).to_owned())).collect()
}

impl Grid {
    /// The waitlist's preflight and post. Every answer, refusals included, carries CORS so the site can read it.
    pub(super) fn waitlist_in(&self, method: &str, h: &HeaderMap, peer: Option<std::net::IpAddr>, bytes: &[u8]) -> Resp {
        if method == "OPTIONS" {
            return respond(204, "", crate::http::empty(), &cors());
        }
        match self.wait_for(h, peer, bytes) {
            Ok(v) => json_with(200, &v, &cors()),
            Err(e) => json_with(e.status(), &json!({ "error": e.public_message() }), &cors()),
        }
    }

    fn wait_for(&self, h: &HeaderMap, peer: Option<std::net::IpAddr>, bytes: &[u8]) -> Result<Value> {
        self.limits.hit(&format!("wait|{}", client_ip(peer, h)), 10, self.clock.now_ms())?;
        let b = parse_body(h, bytes)?;
        let field = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default();
        // an app's list (the phone apps), apart from the seats; no kind, or `seat`, is the seat list as before
        if !matches!(field("kind"), "" | "seat") {
            let w = self.accounts.wait_for_app(field("email"), field("kind"), field("platform"), field("source"))?;
            // the same answer whether or not the address was already there: the list can't be probed for an address
            return Ok(json!({ "ok": true, "kind": w.kind, "platform": w.platform }));
        }
        let w = self.accounts.wait_for(field("email"), field("source"))?;
        Ok(json!({ "ok": true, "joined": w.joined, "invited": w.invited, "position": w.position }))
    }
}
