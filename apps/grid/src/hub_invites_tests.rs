//! Invite links (ADR-0147 §3) and the hub an account is (§1).
#![allow(clippy::unwrap_used)]
use std::sync::Arc;

use super::*;
use crate::accounts::Accounts;
use crate::clock::ManualClock;
use crate::db::Db;

struct World {
    a: Accounts,
    t: Teams,
    clock: Arc<ManualClock>,
}

fn world() -> World {
    let db = Arc::new(Db::memory().unwrap());
    let clock = ManualClock::new(1_000);
    World { a: Accounts { db: db.clone(), clock: clock.clone(), telemetry: None }, t: Teams { db, clock: clock.clone() }, clock }
}

const WEEK: Ask = Ask { role: Role::Member, ttl_ms: TTL_MS, uses: 2 };

#[test]
fn making_a_team_makes_its_hub_and_a_taken_slug_makes_neither() {
    let w = world();
    let ada = w.a.dev_user("ada").unwrap();
    w.t.create(&ada, "acme", "Acme").unwrap();
    let hub = w.t.hub("acme").unwrap().unwrap();
    assert_eq!((hub.slug.as_str(), hub.name.as_str()), ("acme", "Acme"));
    // the hub is an account with the team's slug, so it gets a box at acme.<domain>
    let acct = w.a.user_by_handle("acme").unwrap().unwrap();
    assert_eq!(acct.id, hub.user_id);
    assert!(w.t.is_hub(&acct.id).unwrap() && !w.t.is_hub(&ada.id).unwrap());
    // a person's handle, or a reserved word, is no team's: refused, and no team row is left behind
    assert_eq!(w.t.create(&ada, "ada", "").unwrap_err().status(), 409);
    assert_eq!(w.t.create(&ada, "admin", "").unwrap_err().status(), 409);
    assert!(w.t.of(&ada.id).unwrap().iter().all(|t| t["slug"] == "acme"));
    // nobody signs in as a hub, and a hub is never anyone's member
    assert_eq!(w.a.dev_user("acme").unwrap_err().status(), 403);
    w.t.create(&ada, "beta", "").unwrap();
    assert_eq!(w.t.add(&ada, "beta", "acme", "member").unwrap_err().status(), 400);
    let hubs = w.t.hubs_of(&ada.id, |s| format!("https://{s}.grid.test")).unwrap();
    assert_eq!(hubs[0]["url"], "https://acme.grid.test");
    assert_eq!(hubs[0]["role"], "owner");
}

#[test]
fn an_invite_adds_once_per_person_until_spent_expired_or_revoked() {
    let w = world();
    let (ada, bo, cy, di) =
        (w.a.dev_user("ada").unwrap(), w.a.dev_user("bo").unwrap(), w.a.dev_user("cy").unwrap(), w.a.dev_user("di").unwrap());
    w.t.create(&ada, "acme", "").unwrap();
    let (v, code) = w.t.invite(&ada, "acme", WEEK).unwrap();
    assert!(code.starts_with(PREFIX) && v.uses_left == 2);
    assert_eq!(w.t.join(&bo, &code).unwrap()["role"], "member");
    // joining again uses nothing up
    w.t.join(&bo, &code).unwrap();
    assert_eq!(w.t.invites(&ada, "acme").unwrap()[0].uses_left, 1);
    w.t.join(&cy, &code).unwrap();
    assert_eq!(w.t.join(&di, &code).unwrap_err().status(), 410, "spent");
    assert!(w.t.invites(&ada, "acme").unwrap().is_empty(), "a spent invite isn't open");
    let (_, code) = w.t.invite(&ada, "acme", Ask { uses: 5, ..WEEK }).unwrap();
    w.clock.advance(TTL_MS + 1);
    assert_eq!(w.t.join(&di, &code).unwrap_err().status(), 410, "expired");
    let (v, code) = w.t.invite(&ada, "acme", WEEK).unwrap();
    w.t.revoke_invite(&ada, "acme", &v.id).unwrap();
    assert_eq!(w.t.join(&di, &code).unwrap_err().status(), 410, "revoked");
    assert_eq!(w.t.revoke_invite(&ada, "acme", &v.id).unwrap_err().status(), 404);
    assert_eq!(w.t.join(&di, "wgi_nope").unwrap_err().status(), 404);
}

#[test]
fn an_invite_never_makes_an_owner_and_an_admins_never_an_admin() {
    let w = world();
    let (ada, bo, cy) = (w.a.dev_user("ada").unwrap(), w.a.dev_user("bo").unwrap(), w.a.dev_user("cy").unwrap());
    w.t.create(&ada, "acme", "").unwrap();
    w.t.add(&ada, "acme", "bo", "admin").unwrap();
    w.t.add(&ada, "acme", "cy", "member").unwrap();
    assert_eq!(w.t.invite(&ada, "acme", Ask { role: Role::Owner, ..WEEK }).unwrap_err().status(), 403);
    assert!(w.t.invite(&ada, "acme", Ask { role: Role::Admin, ..WEEK }).is_ok());
    assert_eq!(w.t.invite(&bo, "acme", Ask { role: Role::Admin, ..WEEK }).unwrap_err().status(), 403);
    assert!(w.t.invite(&bo, "acme", WEEK).is_ok());
    assert_eq!(w.t.invite(&cy, "acme", WEEK).unwrap_err().status(), 403, "a member doesn't invite");
    assert_eq!(w.t.invite(&ada, "acme", Ask { ttl_ms: MAX_TTL_MS + 1, ..WEEK }).unwrap_err().status(), 400);
    assert_eq!(w.t.invite(&ada, "acme", Ask { uses: 0, ..WEEK }).unwrap_err().status(), 400);
    // a viewer's invite as member raises them; an admin taking a viewer's invite stays admin
    let (_, code) = w.t.invite(&ada, "acme", Ask { role: Role::Viewer, ..WEEK }).unwrap();
    assert_eq!(w.t.join(&bo, &code).unwrap()["role"], "admin");
    // a hub never joins anything
    let hub = w.a.user_by_handle("acme").unwrap().unwrap();
    assert_eq!(w.t.join(&hub, &code).unwrap_err().status(), 403);
}

#[test]
fn open_invites_are_capped() {
    let w = world();
    let ada = w.a.dev_user("ada").unwrap();
    w.t.create(&ada, "acme", "").unwrap();
    for _ in 0..MAX_OPEN {
        w.t.invite(&ada, "acme", WEEK).unwrap();
    }
    assert_eq!(w.t.invite(&ada, "acme", WEEK).unwrap_err().status(), 409);
}

#[test]
fn a_sole_owner_is_named_and_deleting_a_team_hands_back_its_hub() {
    let w = world();
    let (ada, bo) = (w.a.dev_user("ada").unwrap(), w.a.dev_user("bo").unwrap());
    w.t.create(&ada, "acme", "").unwrap();
    w.t.add(&ada, "acme", "bo", "admin").unwrap();
    assert_eq!(w.t.sole_owner_of(&ada.id).unwrap(), ["acme"]);
    assert_eq!(w.t.sole_owner_of(&bo.id).unwrap(), Vec::<String>::new());
    assert_eq!(w.t.delete(&bo, "acme", "acme").unwrap_err().status(), 403, "an admin doesn't delete");
    assert_eq!(w.t.delete(&ada, "acme", "acm").unwrap_err().status(), 400, "typed wrong");
    let hub = w.t.delete(&ada, "acme", "acme").unwrap().unwrap();
    assert_eq!(hub.handle, "acme");
    assert!(w.t.hub("acme").unwrap().is_none() && w.t.of(&bo.id).unwrap().is_empty());
}
