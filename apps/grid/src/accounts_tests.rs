//! `accounts`'s tests.
#![allow(clippy::unwrap_used)]
use super::*;
use crate::clock::ManualClock;

fn accounts() -> (Accounts, Arc<ManualClock>) {
    let clock = ManualClock::new(1_000_000);
    (Accounts { db: Arc::new(Db::memory().unwrap()), clock: clock.clone() }, clock)
}

#[test]
fn device_sign_in_follows_rfc_8628() {
    let (a, clock) = accounts();
    let ada = a.dev_user("ada").unwrap();
    let c = a.device_code("studio (wisp)").unwrap();
    let (code, user) = (c["device_code"].as_str().unwrap().to_owned(), c["user_code"].as_str().unwrap().to_owned());
    assert_eq!((c["interval"].clone(), c["expires_in"].clone()), (json!(5), json!(600)));
    assert_eq!(a.poll(&code).unwrap(), Poll::Pending);
    // faster than the interval
    clock.advance(1000);
    assert_eq!(a.poll(&code).unwrap(), Poll::SlowDown);
    clock.advance(5000);
    assert_eq!(a.poll(&code).unwrap(), Poll::Pending);
    assert_eq!(a.pending(&user.to_lowercase().replace('-', " ")).unwrap().as_deref(), Some("studio (wisp)"));
    a.decide(&user, &ada.id, true).unwrap();
    let Poll::Token { token, user: who } = a.poll(&code).unwrap() else { panic!("no token") };
    assert!(token.starts_with("wgd_") && who.handle == "ada");
    // once: the code is used
    assert_eq!(a.poll(&code).unwrap(), Poll::Invalid);
    assert!(a.decide(&user, &ada.id, true).is_err());
    let (u, d) = a.device(&format!("Bearer {token}")).unwrap().unwrap();
    assert_eq!((u.handle.as_str(), d.name.as_str()), ("ada", "studio (wisp)"));
    // kept hashed only
    assert_ne!(d.token_hash, token);
    // expired
    let late = a.device_code("x").unwrap();
    clock.advance(CODE_TTL_MS + 1);
    assert_eq!(a.poll(late["device_code"].as_str().unwrap()).unwrap(), Poll::Expired);
    assert_eq!(a.poll("nope").unwrap().error(), Some("invalid_grant"));
    // a revoked device is nobody
    a.revoke_device(&ada.id, &d.id).unwrap();
    assert!(a.device(&token).unwrap().is_none());
}

/// v1's grid kept every token as SHA-256 base64url (grid/secrets.js `sha256`); the hashes below are node's
/// `createHash('sha256').update(t).digest('base64url')`, so the rows are what a live v1 grid.db holds.
#[test]
fn v1_hashed_sessions_devices_and_codes_still_sign_in_and_move_to_v2s_hash() {
    let (a, _) = accounts();
    let ada = a.dev_user("ada").unwrap();
    let (device, cookie, code) = (
        "wgd_v1devicetokenfromthev1grid0000000000000000",
        "v1sessioncookie0000000000000000000000000000",
        "v1devicecode000000000000000000000000000000",
    );
    let v1 = |t: &str| sha256_v1(t);
    assert_eq!(v1(device), "-X9qkF5vSGsraB6kiW0cjnpcXR4TT80vJMm2RxBp_io");
    assert_eq!(v1(cookie), "Y-4h-LBszo01aXF2CK8lhnPutHntITHqu68OtufZJRg");
    assert_eq!(v1(code), "I_fQZcVV6zFXRZotl0_PlV0WCXshyiQazBdOGk8vpF8");
    let now = a.now();
    a.db.with(|c| {
        c.execute("INSERT INTO devices (id, user_id, name, token_hash, created_at) VALUES ('d1', ?1, 'laptop', ?2, ?3)", params![ada.id, v1(device), now])?;
        c.execute("INSERT INTO sessions (id_hash, user_id, scope, created_at, expires_at, last_seen_at) VALUES (?1, ?2, 'grid', ?3, ?4, ?3)", params![v1(cookie), ada.id, now, now + DAY])?;
        c.execute("INSERT INTO device_codes (code_hash, user_code, name, user_id, created_at, expires_at, approved_at) VALUES (?1, 'BCDF-GHJK', 'studio', ?2, ?3, ?4, ?3)", params![v1(code), ada.id, now, now + CODE_TTL_MS])
    })
    .unwrap();
    let hash_of = |sql: &str, id: &str| a.db.with(|c| c.query_row(sql, [id], |r| r.get::<_, String>(0))).unwrap();

    // a device signed in under v1 is still signed in, and its row now holds v2's hash (never the token)
    let (u, d) = a.device(&format!("Bearer {device}")).unwrap().unwrap();
    assert_eq!((u.handle.as_str(), d.id.as_str()), ("ada", "d1"));
    assert_eq!(hash_of("SELECT token_hash FROM devices WHERE id = ?1", "d1"), sha256_hex(device));
    assert!(a.device(device).unwrap().is_some(), "still works once rewritten");

    // a v1 browser session too
    assert_eq!(a.session(cookie).unwrap().map(|u| u.handle).as_deref(), Some("ada"));
    assert_eq!(hash_of("SELECT id_hash FROM sessions WHERE user_id = ?1", &ada.id), sha256_hex(cookie));
    assert!(a.session(cookie).unwrap().is_some());

    // a device code v1 handed out just before cutover still finishes
    let Poll::Token { token, user } = a.poll(code).unwrap() else { panic!("no token") };
    assert_eq!(user.handle, "ada");
    assert_eq!(hash_of("SELECT code_hash FROM device_codes WHERE user_code = ?1", "BCDF-GHJK"), sha256_hex(code));
    assert_eq!(a.poll(code).unwrap(), Poll::Invalid);
    assert!(a.device(&token).unwrap().is_some());

    // a revoked v1 device stays revoked; a token that only shares a prefix is nobody
    a.revoke_device(&ada.id, "d1").unwrap();
    assert!(a.device(device).unwrap().is_none());
    assert!(a.device("wgd_v1devicetokenfromthev1grid").unwrap().is_none());
}

#[test]
fn a_denied_sign_in_stops() {
    let (a, _) = accounts();
    let ada = a.dev_user("ada").unwrap();
    let c = a.device_code("laptop").unwrap();
    a.decide(c["user_code"].as_str().unwrap(), &ada.id, false).unwrap();
    assert_eq!(a.poll(c["device_code"].as_str().unwrap()).unwrap(), Poll::Denied);
}

#[test]
fn computers_get_labels_that_never_read_as_another_address() {
    let (a, _) = accounts();
    assert_eq!(computer_slug("Ada's MacBook Pro"), "ada-s-macbook-pro");
    assert_eq!(computer_slug("p5173"), "p5173-pc");
    assert_eq!(computer_slug("sb-0123abcd"), "sb-0123abcd-pc");
    assert_eq!(computer_slug("!!!"), "computer");
    let ada = a.dev_user("ada").unwrap();
    assert!(a.dev_user("a--b").is_err());
    let mk = |name: &str| {
        let c = a.device_code(name).unwrap();
        a.decide(c["user_code"].as_str().unwrap(), &ada.id, true).unwrap();
        let Poll::Token { token, .. } = a.poll(c["device_code"].as_str().unwrap()).unwrap() else { panic!() };
        a.device(&token).unwrap().unwrap().1
    };
    let (d1, d2) = (mk("one"), mk("two"));
    let c1 = a.add_computer(&ada, &d1, "Studio").unwrap();
    let c2 = a.add_computer(&ada, &d2, "Studio").unwrap();
    assert_eq!((c1.label.as_str(), c2.label.as_str()), ("studio--ada", "studio-2--ada"));
    // the same device again: renamed, same label
    assert_eq!(a.add_computer(&ada, &d1, "Studio 2").unwrap().label, "studio--ada");
}

/// One address could hold any number of waiting codes (nothing counted them): it's 20 at once now, and the codes keep
/// where they were asked from, and by what, for the approve page.
#[test]
fn live_device_codes_are_capped_per_ip() {
    let (a, clock) = accounts();
    let agent = "x".repeat(300);
    for _ in 0..LIVE_CODES_PER_IP {
        a.device_code_from("laptop", "203.0.113.9", &agent).unwrap();
    }
    let e = a.device_code_from("laptop", "203.0.113.9", "").unwrap_err();
    assert_eq!(
        (e.status(), e.public_message().as_str()),
        (429, "Too many sign-ins waiting from here. Finish or let one expire, then try again.")
    );
    // another address still signs in
    let c = a.device_code_from("phone", "198.51.100.7", "wisp-android/1").unwrap();
    let uc = c["user_code"].as_str().unwrap();
    let p = a.pending_ask(uc).unwrap().unwrap();
    assert_eq!((p.name.as_str(), p.ip.as_str()), ("phone", "198.51.100.7"));
    let kept: i64 = a
        .db
        .with(|c| c.query_row("SELECT max(length(requester_agent)) FROM device_codes WHERE requester_ip = '203.0.113.9'", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(kept, 200);
    // once they expire, there's room again
    clock.advance(CODE_TTL_MS + 1);
    a.device_code_from("laptop", "203.0.113.9", "").unwrap();
}

/// Rows stayed a day past their expiry, so a flood of codes stayed in the table: an expired code goes at the next insert.
#[test]
fn expired_device_codes_are_swept() {
    let (a, clock) = accounts();
    let old = a.device_code("old").unwrap();
    let rows = || -> i64 { a.db.with(|c| c.query_row("SELECT count(*) FROM device_codes", [], |r| r.get(0))).unwrap() };
    clock.advance(CODE_TTL_MS + 1);
    a.device_code("new").unwrap();
    assert_eq!(rows(), 1);
    // the device polling the swept code is told to start again
    assert_eq!(a.poll(old["device_code"].as_str().unwrap()).unwrap().error(), Some("invalid_grant"));
}
