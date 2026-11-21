//! ADR-0021's list of what wisp sends (goldens/emitters.json): every catalogue event with its property keys, and
//! the file of the plugin that sends it, or why nothing does. A catalogue event nobody sends can't hide again (F-upd-4
//! was partial for 14 of them), and a key added to or dropped from an event shows up here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value;
use wisp_telemetry_api::catalogue;

fn golden() -> Value {
    serde_json::from_str(include_str!("goldens/emitters.json")).unwrap()
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn source(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn every_catalogue_event_is_listed_with_its_keys() {
    let g = golden();
    let events = g["events"].as_object().unwrap();
    assert_eq!(events.len(), catalogue().len(), "one row per catalogue event");
    for e in catalogue() {
        let row = events.get(&e.name).unwrap_or_else(|| panic!("{} isn't in goldens/emitters.json", e.name));
        let mut want = e.props.clone();
        want.sort();
        let have: Vec<String> = row["props"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_owned()).collect();
        assert_eq!(have, want, "{}'s keys", e.name);
    }
}

#[test]
fn each_event_is_sent_by_the_file_named_or_says_why_not() {
    let g = golden();
    let mut sent = 0;
    for (name, row) in g["events"].as_object().unwrap() {
        match (row["from"].as_str(), row["notSent"].as_str()) {
            (Some(file), None) => {
                assert!(source(file).contains(&format!("\"{name}\"")), "{file} doesn't send {name}");
                sent += 1;
            }
            (None, Some(why)) => assert!(!why.trim().is_empty(), "{name}: say why it isn't sent"),
            _ => panic!("{name}: either `from` (the file that sends it) or `notSent` (why), not both or neither"),
        }
    }
    assert_eq!(sent, 47, "v1's 52 events, 5 with no v2 feature to send them");
}

#[test]
fn server_errors_go_as_exceptions_from_the_files_named() {
    let g = golden();
    let ex = &g["exceptions"];
    assert_eq!(ex["event"], "$exception");
    for (ty, file) in ex["from"].as_object().unwrap() {
        assert!(source(file.as_str().unwrap()).contains(&format!("\"{ty}\"")), "{file} doesn't report {ty}");
    }
}
