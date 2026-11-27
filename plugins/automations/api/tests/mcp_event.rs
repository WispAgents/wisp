//! An `mcp.event` trigger (ADR-0145): checked when it's applied (its connector and event named, a known delivery),
//! and its `project` is where the task runs, never a filter (an MCP server's event names no project).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use serde_json::Value;
use wisp_automations_api::TriggerSpec;
use wisp_automations_api::spec::check;

#[test]
fn an_mcp_event_names_its_connector_and_event_and_runs_in_its_project_unfiltered() {
    let mut m = BTreeMap::new();
    m.insert("connector".to_owned(), Value::String("pd".into()));
    let half = TriggerSpec { on: "mcp.event".into(), prompt: "x".into(), filters: m.clone(), ..TriggerSpec::default() };
    assert!(check(&half).iter().any(|e| e.contains("match.event")), "{:?}", check(&half));
    m.insert("event".to_owned(), Value::String("incident.created".into()));
    m.insert("delivery".to_owned(), Value::String("carrier pigeon".into()));
    let odd = TriggerSpec { on: "mcp.event".into(), prompt: "x".into(), filters: m.clone(), ..TriggerSpec::default() };
    assert!(check(&odd).iter().any(|e| e.contains("match.delivery")), "{:?}", check(&odd));
    m.insert("delivery".to_owned(), Value::String("webhook".into()));
    let ok = TriggerSpec { on: "mcp.event".into(), prompt: "x".into(), project: "/repo".into(), filters: m, ..TriggerSpec::default() };
    assert!(check(&ok).is_empty(), "{:?}", check(&ok));
    assert!(!ok.all_filters().contains_key("project"));
}
