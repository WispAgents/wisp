//! Caddy's on-demand TLS check (deploy/Caddyfile: `ask http://127.0.0.1:<port>/_grid/tls-ask`). Caddy sends it with
//! the loopback address as its Host, not the grid's: the v2 grid answered that with a 421, so Caddy got no certificate
//! for any account made after the cutover and their box never opened (2026-11-23). A request Caddy passed on (it
//! carries X-Forwarded-For) is never answered as the check, so nobody outside can ask which names exist.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread")]
async fn caddys_ask_on_the_loopback_host_gets_a_certificate_for_a_new_account() {
    let dir = std::env::temp_dir().join(format!("wisp-grid-tls-ask-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (grid, port, _stop) = wisp_grid::edge::dev(&dir, 0, PathBuf::from("/nonexistent/wisp"), Vec::new(), 600_000).await.unwrap();
    let idn = wisp_grid::auth::Identity {
        provider: "github".into(),
        subject: "77".into(),
        email_verified: true,
        login: "sabi9-epss".into(),
        ..wisp_grid::auth::Identity::default()
    };
    let user = grid.accounts.sign_in(&grid.cfg.auth, &idn, None).unwrap();
    let domain = grid.cfg.domain.clone();
    let c = reqwest::Client::builder().no_proxy().build().unwrap();
    let ask = |name: String, host: String, forwarded: bool| {
        let mut r = c.get(format!("http://127.0.0.1:{port}/_grid/tls-ask?domain={name}")).header("host", host);
        if forwarded {
            r = r.header("x-forwarded-for", "203.0.113.9");
        }
        r.send()
    };
    let caddy = format!("127.0.0.1:{port}");

    // Caddy's own ask, as the Caddyfile sends it
    let r = ask(format!("{}.{domain}", user.handle), caddy.clone(), false).await.unwrap();
    assert_eq!(r.status().as_u16(), 200, "{:?}", r.text().await);
    let r = ask(domain.clone(), caddy.clone(), false).await.unwrap();
    assert_eq!(r.status().as_u16(), 200, "the grid itself");
    let r = ask(format!("nosuchuser.{domain}"), caddy.clone(), false).await.unwrap();
    assert_eq!(r.status().as_u16(), 404, "a name no one has");
    let r = ask(format!("{}.{domain}", user.handle), "localhost".into(), false).await.unwrap();
    assert_eq!(r.status().as_u16(), 200, "localhost is loopback too");

    // a request from outside that Caddy passed on: not the check, on any Host
    for host in [caddy, domain.clone()] {
        let r = ask(format!("{}.{domain}", user.handle), host.clone(), true).await.unwrap();
        let status = r.status().as_u16();
        let body = r.text().await.unwrap();
        assert!(!body.contains("\"ok\""), "{host}: {status} {body}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
