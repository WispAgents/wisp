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
    assert_eq!((v["version"].clone(), v["source"].clone()), (json!(1), json!("grid://ada/review")));
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
