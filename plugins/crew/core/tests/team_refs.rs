//! Audit B #8: `crew.update --team` documents `[{ crew, why? }]`, but `{crew:"bob", why:"reviews"}` was refused
//! with "unknown subagent harness: ". A row naming a member, by id or name, is that member, with why as its use.
#![allow(clippy::unwrap_used)]

use serde_json::json;
use wisp_crew_api::Member;
use wisp_crew_core::member::{crew_refs, normalize_team};

fn crew() -> Vec<Member> {
    let m = |id: &str, name: &str| Member { id: id.into(), name: name.into(), ..Member::default() };
    vec![m("a1b2c3d4e5f6a7b8", "Ada"), m("0f1e2d3c4b5a6978", "Bob Lee")]
}

#[test]
fn a_team_row_names_a_member_by_id_or_name_with_why_as_its_use() {
    let rows = crew_refs(&json!([{ "crew": "bob lee", "why": "reviews" }, { "crew": "a1b2c3d4e5f6a7b8" }, { "agent": "codex" }]), &crew())
        .unwrap();
    let team = normalize_team(&rows).unwrap();
    assert_eq!(team[0].crew.as_deref(), Some("0f1e2d3c4b5a6978"));
    assert_eq!(team[0].use_, "reviews");
    assert_eq!(team[0].label.as_deref(), Some("Bob Lee"), "their name labels it");
    assert_eq!(team[1].crew.as_deref(), Some("a1b2c3d4e5f6a7b8"));
    assert_eq!((team[2].agent.as_str(), team[2].crew.as_deref()), ("codex", None), "a harness row is still one");
    assert_eq!(crew_refs(&json!([{ "crew": "bob-lee" }]), &crew()).unwrap()[0]["crew"], "0f1e2d3c4b5a6978", "by handle");
}

#[test]
fn a_team_row_naming_no_member_says_who_it_named() {
    let e = crew_refs(&json!([{ "crew": "zed", "why": "x" }]), &crew()).unwrap_err();
    assert!(e.to_string().contains("No one on your crew is called zed"), "{e}");
}

/// Audit E13: `crew update ada --team '[{"crew":"bob"}]'` showed `bob  Bob  auto` though Bob runs on codex: a row naming a
/// member takes that member's harness (their job runs on it), unless the row names one.
#[test]
fn a_team_row_naming_a_member_shows_their_harness_audit_e13() {
    let mut c = crew();
    c[1].agent = "codex".into();
    let rows = crew_refs(&json!([{ "crew": "bob lee" }, { "crew": "ada", "agent": "claude" }]), &c).unwrap();
    let team = normalize_team(&rows).unwrap();
    assert_eq!(team[0].agent, "codex");
    assert_eq!(team[1].agent, "claude", "said on the row: kept");
}
