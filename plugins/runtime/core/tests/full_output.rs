//! Audit F8: a step keeps a long output's start and end (`clip_output`), and what the clip cut was gone for good. Now
//! a cut output's whole text is named on its step (`full`, its BLAKE3 hash as the store's blobs name it) and handed to
//! the host to keep; a short one names none (nothing more written), nor one that read or quotes a .env value.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value, json};
use wisp_runtime_api::harness::HarnessEvent;
use wisp_runtime_api::model::Activity;
use wisp_runtime_core::activity::{self, Effect, EnvValues, Head};

fn run(h: &mut Head, v: Value) -> Activity {
    match activity::apply(h, &HarnessEvent::from_v1(v).unwrap(), false, &|_| false) {
        Effect::Append(a) => a,
        e => panic!("a step, not {e:?}"),
    }
}

fn long() -> String {
    format!("HEAD_LINE\n{}assertion failed: TAIL_MARKER", "a line of the build …\n".repeat(250))
}

/// The store's `hash_of`, spelled out: the step names the blob the host keeps.
fn hash(s: &str) -> String {
    format!("blake3:{}", blake3::hash(s.as_bytes()).to_hex())
}

fn keeping() -> Head {
    Head { keep_full: true, ..Head::default() }
}

#[test]
fn a_cut_output_names_its_whole_text_and_hands_it_to_the_host() {
    let mut h = keeping();
    let a = run(&mut h, json!({"type":"tool_output","id":"c1","text":long()}));
    let Activity::ToolOutput { text, full: Some(full), .. } = a else { panic!("a kept output: {a:?}") };
    assert!(text.contains(" lines cut …") && text.ends_with("TAIL_MARKER"));
    assert_eq!(full.blob, hash(&long()));
    assert_eq!(full.lines, u32::try_from(long().lines().count()).unwrap());
    assert_eq!(full.bytes, long().len() as u64);
    assert_eq!(h.take_full(), vec![long()]);
    assert_eq!(h.take_full(), Vec::<String>::new(), "taken once");
}

#[test]
fn the_hash_is_blake3_of_the_whole_text() {
    let f = activity::full_of(&long()).unwrap();
    assert_eq!(f.blob.len(), "blake3:".len() + 64);
    assert_ne!(f.blob, activity::full_of(&format!("{}x", long())).unwrap().blob);
}

#[test]
fn a_short_output_names_none() {
    let mut h = keeping();
    let a = run(&mut h, json!({"type":"tool_output","id":"c1","text":"x".repeat(100)}));
    assert!(matches!(a, Activity::ToolOutput { full: None, .. }));
    assert_eq!(h.take_full(), Vec::<String>::new());
}

#[test]
fn a_tool_error_and_an_inline_output_name_theirs_too() {
    let mut h = keeping();
    let e = run(&mut h, json!({"type":"tool_error","name":"Bash","text":long(),"id":"c9"}));
    assert!(matches!(&e, Activity::ToolError { full: Some(f), .. } if f.blob == hash(&long())), "{e:?}");
    let t = run(&mut h, json!({"type":"tool","kind":"run","label":"npm test","id":"c2","output":long()}));
    assert!(matches!(&t, Activity::Tool { full: Some(f), .. } if f.blob == hash(&long())), "{t:?}");
    assert_eq!(h.take_full().len(), 2);
}

#[test]
fn an_env_read_or_a_quote_of_a_value_names_none() {
    // the .env file's own output: masked whole, never kept whole
    let mut h = keeping();
    run(&mut h, json!({"type":"tool","kind":"read","label":"Read .env","file":".env","id":"c1"}));
    let a = run(&mut h, json!({"type":"tool_output","id":"c1","text":format!("API_KEY=sk-live-one\n{}", long())}));
    assert!(matches!(a, Activity::ToolOutput { env: true, full: None, .. }), "{a:?}");
    // another output quoting a value the task read, in the part the clip cuts
    let mut h = Head { keep_full: true, ..Head::seeded(EnvValues(vec!["sk-live-one".into()])) };
    let quoted = format!("{}echo sk-live-one\n{}", "x\n".repeat(400), "y\n".repeat(900));
    let a = run(&mut h, json!({"type":"tool_output","id":"c2","text":quoted}));
    assert!(matches!(a, Activity::ToolOutput { full: None, .. }), "{a:?}");
    assert_eq!(h.take_full(), Vec::<String>::new());
}

#[test]
fn a_side_run_whose_steps_are_kept_nowhere_names_none() {
    let mut h = Head::default();
    let a = run(&mut h, json!({"type":"tool_output","id":"c1","text":long()}));
    assert!(matches!(a, Activity::ToolOutput { full: None, .. }));
}
