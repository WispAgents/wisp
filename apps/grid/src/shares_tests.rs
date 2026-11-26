//! `shares`'s tests.
#![allow(clippy::unwrap_used)]
use ed25519_dalek::{Signer as _, SigningKey};

use super::*;
use crate::accounts::Accounts;
use crate::clock::ManualClock;

struct T {
    a: Accounts,
    s: Shares,
    clock: Arc<ManualClock>,
}

fn t() -> T {
    let db = Arc::new(Db::memory().unwrap());
    let clock = ManualClock::new(1_000);
    let teams = Teams { db: db.clone(), clock: clock.clone() };
    T {
        a: Accounts { db: db.clone(), clock: clock.clone() },
        s: Shares { db, clock: clock.clone(), teams, public_url: "http://wisp.localhost:8080".into() },
        clock,
    }
}

fn put(key: &SigningKey, k: &str, body: &str) -> Put {
    Put {
        kind: "setup".into(),
        key: k.into(),
        title: "Review team".into(),
        body: B64.encode(body),
        signer: B64.encode(key.verifying_key().to_bytes()),
        sig: B64.encode(key.sign(body.as_bytes()).to_bytes()),
        ..Put::default()
    }
}

#[test]
fn a_share_must_be_signed_by_its_sharer() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let (k1, k2) = (SigningKey::from_bytes(&[1; 32]), SigningKey::from_bytes(&[2; 32]));
    let mut bad_sig = put(&k1, "review", "files: 1");
    bad_sig.body = B64.encode("files: 2");
    assert_eq!(t.s.put(&ada, &bad_sig).unwrap_err().status(), 400);
    let v = t.s.put(&ada, &put(&k1, "review", "files: 1")).unwrap();
    assert_eq!(v["version"], json!(1));
    assert!(v["source"].as_str().unwrap().starts_with("grid://ada/review#"), "unlisted: its name carries the cap");
    // another key can't update it, even signed properly
    assert_eq!(t.s.put(&ada, &put(&k2, "review", "files: 2")).unwrap_err().status(), 403);
    let s = t.s.get(v["id"].as_str().unwrap()).unwrap().unwrap();
    let c = t.s.read(&s, None, None, false).unwrap().unwrap();
    assert!(verify(&c.signer, &c.sig, &c.body));
}

/// Crew's Share link was refused "Bad share key.": the crew keys a share `crew:<id>,<id>` (sorted member ids), and
/// the grid took only `[A-Za-z0-9_.-]`, 64 at most.
#[test]
fn a_crew_share_key_is_taken() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let five = format!("crew:{}", ["62cbf4c243be"; 5].join(","));
    for key in ["crew", "crew:62cbf4c243be", five.as_str()] {
        let p = Put { kind: "crew".into(), ..put(&k, key, "{\"members\":[]}") };
        assert_eq!(t.s.put(&ada, &p).unwrap()["version"], json!(1), "{key}");
    }
    for bad in ["", "a/b", "a b", &"x".repeat(257)] {
        assert!(!key_ok(bad), "{bad:?}");
    }
}

#[test]
fn sharing_again_is_a_new_version_and_ref_pins_one() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let id = t.s.put(&ada, &put(&k, "review", "one")).unwrap()["id"].as_str().unwrap().to_owned();
    // the same content: no new version
    assert_eq!(t.s.put(&ada, &put(&k, "review", "one")).unwrap()["version"], 1);
    t.clock.advance(10);
    let v = t.s.put(&ada, &put(&k, "review", "two")).unwrap();
    assert_eq!((v["id"].as_str().unwrap(), v["version"].clone()), (id.as_str(), json!(2)));
    let s = t.s.get(&id).unwrap().unwrap();
    assert_eq!(t.s.read(&s, None, parse_ref("v1"), false).unwrap().unwrap().body, b"one");
    assert_eq!(t.s.read(&s, None, None, false).unwrap().unwrap().body, b"two");
    assert_eq!(t.s.read(&s, None, parse_ref("v9"), false).unwrap(), Err(Refused::NoVersion));
    assert_eq!(t.s.versions(&s).unwrap().len(), 2);
    assert_eq!(t.s.by_name("ada", "review").unwrap().unwrap().id, id);
}

#[test]
fn share_access_anyone_people_teams_password() {
    let t = t();
    let (ada, bo, cy) = (t.a.dev_user("ada").unwrap(), t.a.dev_user("bo").unwrap(), t.a.dev_user("cy").unwrap());
    let k = SigningKey::from_bytes(&[1; 32]);
    let id = t.s.put(&ada, &put(&k, "x", "x")).unwrap()["id"].as_str().unwrap().to_owned();
    let may = |who: Option<&UserRow>, pass: Option<&str>| t.s.may_read(&t.s.get(&id).unwrap().unwrap(), who, pass).unwrap();
    assert_eq!(may(None, None), Ok(()));
    let set = |a: Access| t.s.patch(&ada, &id, &Patch { access: Some(a), ..Patch::default() });
    set(Access { kind: "people".into(), people: vec!["bo".into()], ..Access::default() }).unwrap();
    assert_eq!(
        (may(Some(&bo), None), may(Some(&cy), None), may(None, None)),
        (Ok(()), Err(Refused::Locked("people".into())), Err(Refused::Locked("people".into())))
    );
    // the sharer always reads it
    assert_eq!(may(Some(&ada), None), Ok(()));
    // teams: only a member (not a viewer) shares to one; any member reads
    t.s.teams.create(&bo, "acme", "").unwrap();
    assert_eq!(set(Access { kind: "teams".into(), teams: vec!["acme".into()], ..Access::default() }).unwrap_err().status(), 403);
    t.s.teams.add(&bo, "acme", "ada", "member").unwrap();
    t.s.teams.add(&bo, "acme", "cy", "viewer").unwrap();
    set(Access { kind: "teams".into(), teams: vec!["acme".into()], ..Access::default() }).unwrap();
    assert_eq!((may(Some(&cy), None), may(None, None)), (Ok(()), Err(Refused::Locked("teams".into()))));
    // a password, kept hashed
    set(Access { kind: "password".into(), password: Some("hunter2".into()), ..Access::default() }).unwrap();
    assert_eq!(
        (may(None, Some("hunter2")), may(None, Some("nope")), may(None, None)),
        (Ok(()), Err(Refused::Locked("password".into())), Err(Refused::Locked("password".into())))
    );
    let kept = t.s.get(&id).unwrap().unwrap().pass.unwrap();
    assert!(!kept.contains("hunter2") && check_password(&kept, "hunter2"));
    assert_eq!(set(Access { kind: "secret".into(), ..Access::default() }).unwrap_err().status(), 400);
}

#[test]
fn an_expired_or_revoked_link_is_gone() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let id = t.s.put(&ada, &Put { expires_at: Some(2_000), ..put(&k, "x", "x") }).unwrap()["id"].as_str().unwrap().to_owned();
    let may = |who: Option<&UserRow>| t.s.may_read(&t.s.get(&id).unwrap().unwrap(), who, None).unwrap();
    assert_eq!(may(None), Ok(()));
    t.clock.set(2_000);
    assert_eq!(may(None), Err(Refused::Gone("expired")));
    t.s.patch(&ada, &id, &Patch { expires_at: Some(0), ..Patch::default() }).unwrap();
    assert_eq!(may(None), Ok(()));
    t.s.revoke(&ada, &id).unwrap();
    assert_eq!(may(None), Err(Refused::Gone("revoked")));
    // sharing again brings it back
    t.s.put(&ada, &put(&k, "x", "y")).unwrap();
    assert_eq!(may(None), Ok(()));
}

#[test]
fn views_and_imports_are_counted() {
    let t = t();
    let (ada, bo) = (t.a.dev_user("ada").unwrap(), t.a.dev_user("bo").unwrap());
    let k = SigningKey::from_bytes(&[1; 32]);
    let id = t.s.put(&ada, &put(&k, "x", "x")).unwrap()["id"].as_str().unwrap().to_owned();
    let s = t.s.get(&id).unwrap().unwrap();
    t.s.read(&s, Some(&ada), None, true).unwrap().unwrap();
    t.s.read(&s, Some(&bo), None, true).unwrap().unwrap();
    t.s.read(&s, None, None, true).unwrap().unwrap();
    t.s.count_import(&s, Some(&bo)).unwrap();
    t.s.count_import(&s, Some(&ada)).unwrap();
    let s = t.s.get(&id).unwrap().unwrap();
    assert_eq!((s.views, s.imports), (2, 1));
}

#[test]
fn the_gallery_lists_only_what_was_published() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let one = t
        .s
        .put(&ada, &Put { publish: Some(true), summary: "reviews every PR".into(), tags: vec!["Review".into()], ..put(&k, "review", "a") })
        .unwrap();
    t.s.put(&ada, &put(&k, "private", "b")).unwrap();
    let found = t.s.gallery("", None).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["card"]["id"], one["id"]);
    assert_eq!(t.s.gallery("every review", None).unwrap().len(), 1);
    assert_eq!(t.s.gallery("deploy", None).unwrap().len(), 0);
    assert_eq!(t.s.gallery("", Some("task")).unwrap().len(), 0);
    // locked or revoked: out of the gallery
    let id = one["id"].as_str().unwrap();
    t.s.patch(
        &ada,
        id,
        &Patch { access: Some(Access { kind: "password".into(), password: Some("p".into()), ..Access::default() }), ..Patch::default() },
    )
    .unwrap();
    assert_eq!(t.s.gallery("", None).unwrap().len(), 0);
}

/// `n` live shares of ada's, keys `k0`…: up to the cap.
fn fill(t: &T, ada: &UserRow, k: &SigningKey, n: i64) {
    for i in 0..n {
        t.s.put(ada, &put(k, &format!("k{i}"), "x")).unwrap();
    }
}

const FULL: &str = "You have 200 live shares, as many as this grid keeps. Revoke or let one expire to make room.";

/// The cap counted every row a user ever shared, revoked and expired included, and no row is ever deleted: at 200 the
/// user was stuck for good, however many they revoked.
#[test]
fn revoking_a_share_frees_a_slot() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    fill(&t, &ada, &k, PER_USER);
    let e = t.s.put(&ada, &put(&k, "one-more", "x")).unwrap_err();
    assert_eq!((e.status(), e.public_message()), (409, FULL.to_owned()));
    let id = t.s.by_name("ada", "k0").unwrap().unwrap().id;
    t.s.revoke(&ada, &id).unwrap();
    assert_eq!(t.s.put(&ada, &put(&k, "one-more", "x")).unwrap()["version"], json!(1));
}

#[test]
fn expired_shares_do_not_count_toward_the_cap() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    fill(&t, &ada, &k, PER_USER - 1);
    t.s.put(&ada, &Put { expires_at: Some(2_000), ..put(&k, "soon", "x") }).unwrap();
    assert_eq!(t.s.put(&ada, &put(&k, "one-more", "x")).unwrap_err().status(), 409);
    t.clock.set(2_000);
    t.s.put(&ada, &put(&k, "one-more", "x")).unwrap();
    // and clearing the expired one's expiry would bring a 201st back: refused
    let soon = t.s.by_name("ada", "soon").unwrap().unwrap().id;
    assert_eq!(t.s.patch(&ada, &soon, &Patch { expires_at: Some(0), ..Patch::default() }).unwrap_err().status(), 409);
}

#[test]
fn reviving_a_revoked_key_respects_the_cap() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    fill(&t, &ada, &k, PER_USER);
    let id = t.s.by_name("ada", "k0").unwrap().unwrap().id;
    t.s.revoke(&ada, &id).unwrap();
    t.s.put(&ada, &put(&k, "new", "x")).unwrap();
    // k0 wasn't counted while revoked: sharing it again would make 201 live
    let e = t.s.put(&ada, &put(&k, "k0", "y")).unwrap_err();
    assert_eq!((e.status(), e.public_message()), (409, FULL.to_owned()));
    assert!(t.s.get(&id).unwrap().unwrap().revoked_at.is_some());
    // a live key shares again freely, at the cap
    assert_eq!(t.s.put(&ada, &put(&k, "k1", "y")).unwrap()["version"], json!(2));
    // with room, the revoked key comes back in place
    t.s.revoke(&ada, &t.s.by_name("ada", "new").unwrap().unwrap().id).unwrap();
    assert_eq!(t.s.put(&ada, &put(&k, "k0", "y")).unwrap()["id"], json!(id));
}

#[test]
fn an_impossible_share_password_is_wrong_without_hashing() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let a = Access { kind: "password".into(), password: Some("hunter2".into()), ..Access::default() };
    let id = t.s.put(&ada, &Put { access: Some(a), ..put(&k, "x", "x") }).unwrap()["id"].as_str().unwrap().to_owned();
    let s = t.s.get(&id).unwrap().unwrap();
    let long = "overlong-in-shares-tests".repeat(8);
    assert!(long.len() > PASSWORD_MAX);
    assert_eq!(t.s.may_read(&s, None, Some(&long)).unwrap(), Err(Refused::Locked("password".into())));
    assert_eq!(derived(&long), 0);
}

/// ADR-0130: `grid://ada/crew` opened anyone's whole crew, unlisted or not: `crew` is every crew's key, so a handle was
/// all it took. A name opens an unlisted share only with its cap, for its sharer, or for the people and teams it's for.
#[test]
fn unlisted_shares_open_by_name_only_with_their_cap() {
    let t = t();
    let (ada, bo, cy) = (t.a.dev_user("ada").unwrap(), t.a.dev_user("bo").unwrap(), t.a.dev_user("cy").unwrap());
    let key = SigningKey::from_bytes(&[1; 32]);
    let made = t.s.put(&ada, &Put { kind: "crew".into(), ..put(&key, "crew", "{}") }).unwrap();
    let cap = t.s.get(made["id"].as_str().unwrap()).unwrap().unwrap().cap;
    assert_eq!(cap.len(), 32, "128 bits, hex");
    assert_eq!(made["source"], json!(format!("grid://ada/crew#{cap}")));
    let named = |who: Option<&UserRow>, cap: Option<&str>| t.s.named("ada", "crew", who, cap).unwrap().map(|s| s.key);
    // no cap or a wrong one: as if there were no such name
    assert_eq!((named(None, None), named(Some(&bo), None), named(None, Some("0".repeat(32).as_str()))), (None, None, None));
    assert_eq!(t.s.named("ada", "nope", None, None).unwrap(), None);
    // the right cap, or its sharer
    assert_eq!((named(None, Some(&cap)), named(Some(&ada), None)), (Some("crew".into()), Some("crew".into())));
    // for people: one of them opens it by name, no cap
    let id = made["id"].as_str().unwrap();
    let people = Access { kind: "people".into(), people: vec!["bo".into()], ..Access::default() };
    t.s.patch(&ada, id, &Patch { access: Some(people), ..Patch::default() }).unwrap();
    assert_eq!((named(Some(&bo), None), named(Some(&cy), None)), (Some("crew".into()), None));
    // a password: the cap first, then the password
    let pw = Access { kind: "password".into(), password: Some("p".into()), ..Access::default() };
    t.s.patch(&ada, id, &Patch { access: Some(pw), ..Patch::default() }).unwrap();
    assert_eq!(named(None, None), None);
    let opened = t.s.named("ada", "crew", None, Some(&cap)).unwrap().unwrap();
    assert_eq!(t.s.may_read(&opened, None, None).unwrap(), Err(Refused::Locked("password".into())));
    // published: its plain name opens it, and its source has no cap
    let published = t.s.patch(&ada, id, &Patch { access: Some(Access::default()), publish: Some(true), ..Patch::default() }).unwrap();
    assert_eq!(published["source"], json!("grid://ada/crew"));
    assert!(!published["source"].as_str().unwrap().contains('#'));
    assert_eq!(named(None, None), Some("crew".into()));
}

/// Updating a password-locked task share without retyping its passkey sent `{kind: anyone}`, and the put made it
/// public. A put with no access keeps the access it has.
#[test]
fn shares_put_without_access_keeps_a_password() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    let pw = Access { kind: "password".into(), password: Some("hunter2".into()), ..Access::default() };
    let id = t.s.put(&ada, &Put { access: Some(pw), ..put(&k, "t1", "one") }).unwrap()["id"].as_str().unwrap().to_owned();
    let v = t.s.put(&ada, &Put { access: None, ..put(&k, "t1", "two") }).unwrap();
    assert_eq!((v["version"].clone(), v["access"]["kind"].clone()), (json!(2), json!("password")));
    let s = t.s.get(&id).unwrap().unwrap();
    assert!(s.pass.as_deref().is_some_and(|p| check_password(p, "hunter2")));
}

/// Re-sharing a crew reset access set with `wisp cloud access teams:acme` to anyone. A put with no access keeps it.
#[test]
fn shares_put_without_access_keeps_teams_set_by_patch() {
    let t = t();
    let ada = t.a.dev_user("ada").unwrap();
    let k = SigningKey::from_bytes(&[1; 32]);
    t.s.teams.create(&ada, "acme", "").unwrap();
    let id = t.s.put(&ada, &put(&k, "crew", "one")).unwrap()["id"].as_str().unwrap().to_owned();
    let teams = Access { kind: "teams".into(), teams: vec!["acme".into()], ..Access::default() };
    t.s.patch(&ada, &id, &Patch { access: Some(teams), ..Patch::default() }).unwrap();
    t.s.put(&ada, &Put { access: None, ..put(&k, "crew", "two") }).unwrap();
    let s = t.s.get(&id).unwrap().unwrap();
    assert_eq!((s.access.as_str(), s.teams), ("teams", vec!["acme".to_owned()]));
}
