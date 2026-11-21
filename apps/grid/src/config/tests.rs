#![allow(clippy::unwrap_used)]

use super::check::topic_arn_ok;
use super::*;

#[test]
fn defaults_and_urls() {
    let c = for_test(Path::new("/tmp/g"), json!({})).unwrap();
    assert_eq!(c.domain, "wisp.localhost");
    assert_eq!(c.public_url, "http://wisp.localhost:8080");
    assert_eq!(c.instance_url("ada"), "http://ada.wisp.localhost:8080");
    let c = for_test(Path::new("/tmp/g"), json!({ "scheme": "https", "domain": "Grid.Example.com." })).unwrap();
    assert_eq!(c.public_url, "https://grid.example.com");
    assert!(c.secure);
}

#[test]
fn refuses_bad_settings() {
    let bad = |v: Value| for_test(Path::new("/tmp/g"), v).unwrap_err().to_string();
    assert!(bad(json!({ "domain": "a b" })).contains("isn’t a host name"));
    assert!(bad(json!({ "auth": { "signup": "maybe" } })).contains("open or closed"));
    assert!(bad(json!({ "hostd": { "container": { "boxd": "rel/box.sock" } } })).contains("boxd’s socket"));
    assert!(bad(json!({ "channels": { "route": true } })).contains("channels.route needs"));
    assert!(
        bad(json!({ "mail": { "domain": "m.x.com", "inbound": { "provider": "ses" }, "outbound": { "provider": "ses" } } }))
            .contains("mail needs channels")
    );
    assert!(bad(json!({ "auth": { "providers": { "github": { "clientId": "x" } } } })).contains("no client secret"));
}

#[test]
fn a_seat_cap_left_in_grid_json_is_ignored_and_providers_come_from_env() {
    // the owner removed maxUsers (2026-10-27): an old grid.json still loads, capping nothing
    for old in [json!(5), json!("auto"), json!(0)] {
        let c = for_test(Path::new("/tmp/g"), json!({ "auth": { "maxUsers": old, "seats": { "min": 0 } } })).unwrap();
        assert!(!serde_json::to_string(&c.auth).unwrap().contains("maxUsers"));
    }
    let mut env = Env::new();
    env.insert("GRID_GITHUB_CLIENT_ID".into(), "cid".into());
    env.insert("GRID_GITHUB_CLIENT_SECRET".into(), "shh".into());
    let merged = merge(&defaults(), &from_env(&env));
    let c = from_value(merged, "/tmp/g".into(), "/tmp/g/grid.json".into(), &env).unwrap();
    assert_eq!(c.auth.providers["github"].client_secret.as_deref(), Some("shh"));
    // A disabled provider is dropped, not checked.
    let c = for_test(Path::new("/tmp/g"), json!({ "auth": { "providers": { "google": { "clientId": "x", "enabled": false } } } })).unwrap();
    assert!(c.auth.providers.is_empty());
}

#[test]
fn topic_arns() {
    assert!(topic_arn_ok("arn:aws:sns:eu-west-1:123456789012:wisp-mail"));
    assert!(!topic_arn_ok("arn:aws:sqs:eu-west-1:1:x"));
}
