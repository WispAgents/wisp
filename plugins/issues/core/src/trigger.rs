//! Issues as a trigger source (ADR-0043: `issue.opened`, `issue.labeled`; v1 `issues`), sans-IO: a poll's list
//! against what the last one saw. The cursor keeps each issue's labels; the first poll only fills it, so a new
//! trigger doesn't fire on every open issue. Each event says who opened the issue (`author`), every assignee
//! (`assignees`) and who this computer is signed in as (`me`), so a trigger a team shares can take only its own
//! (`match: { assignee: me }`) and two teammates' wisps don't both start on one issue.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use wisp_automations_api::TriggerEvent;
use wisp_issues_api::Issue;

/// One issue's event: its payload (`me` when known, plus `extra`) under `key`.
fn event_of(on: &str, i: &Issue, key: String, summary: String, extra: Value, at: i64, me: &str) -> TriggerEvent {
    let mut payload = json!({ "id": i.id, "ref": i.reference, "number": i.number, "title": i.title, "url": i.url, "labels": i.labels, "assignee": i.assignee, "assignees": i.assignees, "author": i.author, "bot": i.author_bot, "milestone": i.milestone, "text": i.body });
    if !me.is_empty()
        && let Some(p) = payload.as_object_mut()
    {
        p.insert("me".into(), Value::String(me.to_owned()));
    }
    if let (Some(p), Value::Object(e)) = (payload.as_object_mut(), extra) {
        p.extend(e);
    }
    TriggerEvent { on: on.to_owned(), key, at, payload, summary, ..TriggerEvent::default() }
}

/// GitHub's `issues` webhook (ADR-0148 §8e: a hub's App sends it) as the events a poll would have made of it: `opened`
/// is `issue.opened`, `labeled` is `issue.labeled` for that label, each with the poll's payload and key, so a webhook
/// and the poll after it fire a trigger once. `repo` (the repository's `owner/name`) is in each payload, for the hub
/// to find the project.
pub fn webhook_events(body: &Value, at: i64) -> Vec<TriggerEvent> {
    let Some(j) = body.get("issue").filter(|_| body.get("pull_request").is_none()) else { return Vec::new() };
    let user = j.get("user").cloned().unwrap_or(Value::Null);
    // as `gh issue list --json` says it, so the poll's own reader makes the issue
    let gh = json!({
        "number": j.get("number").cloned().unwrap_or(Value::Null), "title": j.get("title").cloned().unwrap_or(Value::Null),
        "url": j.get("html_url").cloned().unwrap_or(Value::Null), "labels": j.get("labels").cloned().unwrap_or_else(|| json!([])),
        "assignees": j.get("assignees").cloned().unwrap_or_else(|| json!([])), "body": j.get("body").cloned().unwrap_or(Value::Null),
        "updatedAt": j.get("updated_at").cloned().unwrap_or(Value::Null), "milestone": j.get("milestone").cloned().unwrap_or(Value::Null),
        "author": { "login": user.get("login").cloned().unwrap_or(Value::Null), "is_bot": user.get("type").and_then(Value::as_str) == Some("Bot") },
    });
    let i = crate::rows::gh_issue(&gh);
    let mut out = match body.get("action").and_then(Value::as_str) {
        Some("opened") => vec![event_of(
            "issue.opened",
            &i,
            format!("issue:{}:opened", i.id),
            format!("{} opened: {}", i.reference, i.title),
            json!({}),
            at,
            "",
        )],
        Some("labeled") => body
            .pointer("/label/name")
            .and_then(Value::as_str)
            .map(|l| {
                let (key, summary) = (format!("issue:{}:label:{l}", i.id), format!("{} labelled {l}: {}", i.reference, i.title));
                vec![event_of("issue.labeled", &i, key, summary, json!({ "label": l }), at, "")]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let repo = body.pointer("/repository/full_name").and_then(Value::as_str).unwrap_or_default();
    for e in &mut out {
        e.payload["repo"] = json!(repo);
    }
    out
}

/// The events since `cursor` (`{ "<id>": [labels] }`), and the new cursor (the issues listed now, capped at 500).
/// `me`: the GitHub login this computer's gh is signed in as, empty when unknown (then `me` matches nothing).
pub fn issue_events(on: &str, issues: &[Issue], cursor: &Value, at: i64, me: &str) -> (Vec<TriggerEvent>, Value) {
    let before: Option<BTreeMap<String, Vec<String>>> = serde_json::from_value(cursor.clone()).ok().filter(|_| cursor.is_object());
    let mut next: BTreeMap<String, Vec<String>> = before.clone().unwrap_or_default();
    let mut out = Vec::new();
    for i in issues {
        let was = before.as_ref().map(|b| b.get(&i.id));
        let ev = |key: String, summary: String, extra: Value| event_of(on, i, key, summary, extra, at, me);
        match (on, was) {
            ("issue.opened", Some(None)) => {
                out.push(ev(format!("issue:{}:opened", i.id), format!("{} opened: {}", i.reference, i.title), json!({})));
            }
            ("issue.labeled", Some(prev)) => {
                let had = prev.cloned().unwrap_or_default();
                for l in i.labels.iter().filter(|l| !had.contains(l)) {
                    out.push(ev(
                        format!("issue:{}:label:{l}", i.id),
                        format!("{} labelled {l}: {}", i.reference, i.title),
                        json!({ "label": l }),
                    ));
                }
            }
            _ => {}
        }
        next.insert(i.id.clone(), i.labels.clone());
    }
    while next.len() > 500 {
        let Some(k) = next.keys().next().cloned() else { break };
        next.remove(&k);
    }
    (out, serde_json::to_value(next).unwrap_or_default())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::assert_is_empty)]
mod tests {
    use super::*;

    fn issue(id: &str, labels: &[&str]) -> Issue {
        Issue {
            id: id.into(),
            reference: format!("#{id}"),
            number: wisp_issues_api::IssueNumber::Int(id.parse().unwrap()),
            title: format!("bug {id}"),
            url: String::new(),
            labels: labels.iter().map(|s| (*s).to_owned()).collect(),
            assignee: String::new(),
            body: String::new(),
            updated_at: None,
            author: String::new(),
            assignees: Vec::new(),
            author_bot: false,
            milestone: String::new(),
        }
    }

    #[test]
    fn new_issues_and_new_labels_fire_once() {
        let (first, c) = issue_events("issue.opened", &[issue("1", &[])], &Value::Null, 0, "");
        assert!(first.is_empty(), "the first poll only fills the cursor");
        let (opened, c) = issue_events("issue.opened", &[issue("1", &[]), issue("2", &["bug"])], &c, 1, "");
        assert_eq!(opened.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["issue:2:opened"]);
        let (labeled, c) = issue_events("issue.labeled", &[issue("1", &["bug"]), issue("2", &["bug"])], &c, 2, "");
        assert_eq!(labeled.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["issue:1:label:bug"], "2 had bug already");
        assert_eq!(labeled[0].payload["label"], "bug");
        let (again, _) = issue_events("issue.labeled", &[issue("1", &["bug"])], &c, 3, "");
        assert!(again.is_empty());
    }

    #[test]
    fn each_event_says_who_opened_it_who_has_it_and_who_this_computer_is() {
        let mut i = issue("7", &["agent"]);
        i.author = "sam".into();
        i.assignees = vec!["lena".into(), "omar".into()];
        i.assignee = "lena".into();
        i.author_bot = true;
        i.milestone = "v2.1".into();
        let (_, c) = issue_events("issue.labeled", &[issue("7", &[])], &Value::Null, 0, "omar");
        let (ev, _) = issue_events("issue.labeled", &[i.clone()], &c, 1, "omar");
        let p = &ev[0].payload;
        assert_eq!((p["author"].as_str(), p["assignee"].as_str(), p["me"].as_str()), (Some("sam"), Some("lena"), Some("omar")));
        assert_eq!(p["assignees"], json!(["lena", "omar"]));
        assert_eq!((p["bot"].as_bool(), p["milestone"].as_str()), (Some(true), Some("v2.1")));
        let (_, c) = issue_events("issue.labeled", &[issue("7", &[])], &Value::Null, 0, "");
        let (ev, _) = issue_events("issue.labeled", &[i], &c, 1, "");
        assert!(ev[0].payload.get("me").is_none(), "not signed in: no me, so `me` matches nothing");
    }

    /// A hub's App: GitHub's `issues` webhook is the poll's own event, same key, so the poll after it adds nothing.
    #[test]
    fn an_issues_webhook_is_the_polls_event() {
        let body = json!({
            "action": "opened", "repository": { "full_name": "acme/app" },
            "issue": { "number": 3, "title": "slug keeps a dash", "html_url": "https://github.com/acme/app/issues/3",
                "user": { "login": "ada", "type": "User" }, "labels": [{ "name": "bug" }], "assignees": [], "body": "x" },
        });
        let evs = webhook_events(&body, 9);
        assert_eq!(evs.len(), 1);
        assert_eq!((evs[0].on.as_str(), evs[0].key.as_str()), ("issue.opened", "issue:3:opened"));
        assert_eq!((evs[0].payload["author"].clone(), evs[0].payload["repo"].clone()), (json!("ada"), json!("acme/app")));
        let (polled, _) = issue_events("issue.opened", &[issue("3", &["bug"])], &json!({}), 10, "");
        assert_eq!(polled[0].key, evs[0].key, "the poll's key");
        let labeled =
            json!({ "action": "labeled", "label": { "name": "agent" }, "issue": { "number": 3, "labels": [{ "name": "agent" }] } });
        assert_eq!(webhook_events(&labeled, 1)[0].key, "issue:3:label:agent");
        // a pull request's `issues`-like event, or another action: none
        assert!(webhook_events(&json!({ "action": "closed", "issue": { "number": 3 } }), 1).is_empty());
        assert!(webhook_events(&json!({ "action": "opened", "issue": { "number": 3 }, "pull_request": {} }), 1).is_empty());
    }
}
