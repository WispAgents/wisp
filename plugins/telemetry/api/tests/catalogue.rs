//! ADR-0021's golden: the catalogue holds v1's 52 events, each with exactly v1's property keys
//! (fixtures/v1-events.json, extracted from v1's `track()` calls), so none can go missing again, plus the keys v2
//! added on purpose ([`V2_ADDED`]: each one a deliberate change, here and in goldens/emitters.json).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Map, Value, json};
use wisp_telemetry_api::{catalogue, entry, ui_event};

/// Keys v2 sends beyond v1's. `harness_install`: how it was installed (`npm` | `download` | `none`) and, when it
/// failed, why (`no_npm`, `npm_failed`, `npm_timeout`, `download_failed`, `not_detected`, …): v1 only counted the
/// attempts npm got through, so a first run that never got a harness was invisible (2026-11-25).
const V2_ADDED: &[(&str, &[&str])] = &[("harness_install", &["how", "reason"])];

#[test]
fn the_catalogue_is_v1s_52_events_with_their_properties() {
    let v1: Value = serde_json::from_str(include_str!("fixtures/v1-events.json")).unwrap();
    let want = v1["events"].as_object().unwrap();
    assert_eq!(want.len(), 52);
    assert_eq!(catalogue().len(), 52, "one entry per v1 event");
    for (name, props) in want {
        let e = entry(name).unwrap_or_else(|| panic!("{name} is missing from the catalogue"));
        let mut have = e.props.clone();
        have.sort();
        let mut want: Vec<String> = props.as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_owned()).collect();
        want.extend(V2_ADDED.iter().filter(|(n, _)| n == name).flat_map(|(_, k)| k.iter().map(|k| (*k).to_owned())));
        want.sort();
        assert_eq!(have, want, "{name}'s properties");
        assert!(!e.owner.is_empty(), "{name} has an owner");
    }
}

#[test]
fn the_ui_may_send_only_its_events_and_values() {
    let p = |v: Value| v.as_object().cloned().unwrap_or_else(Map::new);
    assert_eq!(ui_event("getapp_opened", &p(json!({ "from": "button", "x": 1 }))), Some(p(json!({ "from": "button" }))));
    assert_eq!(ui_event("getapp_opened", &p(json!({ "from": "free text" }))), Some(Map::new()));
    assert_eq!(ui_event("turn_ended", &Map::new()), None, "a server event");
    assert_eq!(ui_event("nope", &Map::new()), None);
}
