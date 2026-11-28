//! Pull requests as a trigger source (ADR-0043: `pr.review_requested`), sans-IO: the `gh pr list` arguments, and
//! its JSON read against what the last poll saw. The cursor keeps each open PR's requested reviewers; the first
//! poll only fills it, so a new trigger doesn't fire on every review already waiting.
//!
//! Each event is one reviewer newly asked (`reviewer`, a login, or a team's `org/slug`), with who opened the PR
//! (`author`), everyone asked now (`reviewers`) and who this computer's gh is (`me`): `match: { reviewer: me }`
//! wakes only the teammate who was asked, so a trigger a team shares in `.wisp/` runs one review, not one each.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use wisp_automations_api::TriggerEvent;

/// The PRs a poll reads (the newest; more open PRs than this only miss the oldest's requests).
pub const LIMIT: usize = 50;

/// What a poll asks of each PR: enough for `reviewer`, `author`, `bot`, `fork`, `draft`, `paths`, `size`,
/// `milestone` and `base` filters.
pub const FIELDS: &str = "number,title,url,author,reviewRequests,labels,baseRefName,headRefName,isDraft,updatedAt,isCrossRepository,additions,deletions,changedFiles,files,milestone";

/// A GitHub author is a bot: gh says so, or its login is an app's (`app/dependabot`, `renovate[bot]`).
pub fn is_bot(author: &Value) -> bool {
    let login = author.get("login").and_then(Value::as_str).unwrap_or_default();
    author.get("is_bot").and_then(Value::as_bool).unwrap_or(false) || login.starts_with("app/") || login.ends_with("[bot]")
}

/// A collaborator permission (`gh api repos/{owner}/{repo}/collaborators/<login>/permission`) that may push: such an
/// author is `trusted`, so `match: { trusted: true }` keeps outsiders' text away from an agent on a public repo.
pub fn may_push(permission: &str) -> bool {
    matches!(permission.trim(), "admin" | "maintain" | "write")
}

/// Whether a trigger's spec filters on `key` (only then is it worth a gh call per author).
pub fn asks_for(spec: &Value, key: &str) -> bool {
    spec.get("match").and_then(|m| m.get(key)).is_some()
}

/// `gh pr list` for the open PRs of the repo gh finds in the trigger's project.
pub fn pr_list_args() -> Vec<String> {
    let mut a: Vec<String> = ["pr", "list", "--state", "open", "--json", FIELDS].iter().map(|s| (*s).to_owned()).collect();
    a.extend(["--limit".to_owned(), LIMIT.to_string()]);
    a
}

/// A review request's name: a user's login, a team's `org/slug` (or its slug, or name).
fn requested(r: &Value) -> Option<String> {
    let s = |k: &str| r.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
    if r.get("__typename").and_then(Value::as_str) == Some("Team") {
        let slug = s("slug").or_else(|| s("name"))?;
        return Some(s("organization").map_or_else(|| slug.to_owned(), |o| format!("{o}/{slug}")));
    }
    s("login").map(str::to_owned)
}

/// The reviewers asked since `cursor` (`{ "<number>": [reviewers] }`), one event each, and the new cursor (the PRs
/// open now). A reviewer asked again after reviewing (gh drops them once they review) fires again: the PR's
/// `updatedAt` is in the key, so the engine's dedupe doesn't hold back a new request.
pub fn review_events(on: &str, stdout: &str, cursor: &Value, at: i64, me: &str) -> Result<(Vec<TriggerEvent>, Value), String> {
    let prs: Vec<Value> = serde_json::from_str(if stdout.trim().is_empty() { "[]" } else { stdout })
        .map_err(|e| format!("gh pr list said something unexpected: {e}"))?;
    let before: Option<BTreeMap<String, Vec<String>>> = cursor.is_object().then(|| serde_json::from_value(cursor.clone()).ok()).flatten();
    let mut next = BTreeMap::new();
    let mut out = Vec::new();
    for p in &prs {
        let n = p.get("number").and_then(Value::as_u64).unwrap_or(0);
        if n == 0 {
            continue;
        }
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let now: Vec<String> =
            p.get("reviewRequests").and_then(Value::as_array).map(|a| a.iter().filter_map(requested).collect()).unwrap_or_default();
        if let Some(b) = &before {
            let had = b.get(&n.to_string()).cloned().unwrap_or_default();
            let labels: Vec<String> = p
                .get("labels")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|l| l.get("name").and_then(Value::as_str)).map(str::to_owned).collect())
                .unwrap_or_default();
            let author = p.pointer("/author/login").and_then(Value::as_str).unwrap_or_default();
            let bot = p.get("author").is_some_and(is_bot);
            let num = |k: &str| p.get(k).and_then(Value::as_u64).unwrap_or(0);
            let paths: Vec<String> = p
                .get("files")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|f| f.get("path").and_then(Value::as_str)).map(str::to_owned).collect())
                .unwrap_or_default();
            let milestone = p.pointer("/milestone/title").and_then(Value::as_str).unwrap_or_default();
            for who in now.iter().filter(|w| !had.contains(w)) {
                let mut payload = json!({
                    "number": n, "ref": format!("#{n}"), "title": s("title"), "url": s("url"), "author": author,
                    "reviewer": who, "reviewers": now, "label": labels, "labels": labels,
                    "base": s("baseRefName"), "branch": s("headRefName"), "draft": p.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                    "bot": bot, "fork": p.get("isCrossRepository").and_then(Value::as_bool).unwrap_or(false),
                    "paths": paths, "additions": num("additions"), "deletions": num("deletions"),
                    "size": num("additions") + num("deletions"), "changedFiles": num("changedFiles"), "milestone": milestone,
                });
                if !me.is_empty() {
                    payload["me"] = json!(me);
                }
                out.push(TriggerEvent {
                    on: on.to_owned(),
                    key: format!("pr:{n}:review:{who}:{}", s("updatedAt")),
                    at,
                    summary: format!("#{n} review requested from {who}: {}", s("title")),
                    payload,
                    ..TriggerEvent::default()
                });
            }
        }
        next.insert(n.to_string(), now);
    }
    Ok((out, serde_json::to_value(next).unwrap_or_default()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::assert_is_empty)]
mod tests {
    use super::*;

    fn pr(n: u64, reviewers: &[&str], updated: &str) -> Value {
        let rr: Vec<Value> = reviewers
            .iter()
            .map(|r| match r.split_once('/') {
                Some((org, slug)) => json!({ "__typename": "Team", "organization": org, "slug": slug, "name": slug }),
                None => json!({ "__typename": "User", "login": r }),
            })
            .collect();
        json!({ "number": n, "title": format!("pr {n}"), "url": "u", "author": { "login": "sam" }, "reviewRequests": rr,
                "labels": [{ "name": "agent" }], "baseRefName": "main", "headRefName": "feat", "isDraft": false, "updatedAt": updated,
                "isCrossRepository": n == 2, "additions": 30, "deletions": 12, "changedFiles": 2,
                "files": [{ "path": "src/a.rs" }, { "path": "README.md" }], "milestone": { "title": "v2.1" } })
    }
    fn list(prs: &[Value]) -> String {
        Value::Array(prs.to_vec()).to_string()
    }

    #[test]
    fn a_reviewer_newly_asked_fires_once_and_the_first_poll_only_fills_the_cursor() {
        let (first, c) = review_events("pr.review_requested", &list(&[pr(1, &["lena"], "t1")]), &Value::Null, 0, "omar").unwrap();
        assert!(first.is_empty(), "lena was asked before the trigger existed");
        let (evs, c) =
            review_events("pr.review_requested", &list(&[pr(1, &["lena", "omar"], "t2"), pr(2, &["acme/web"], "t2")]), &c, 1, "omar")
                .unwrap();
        assert_eq!(evs.iter().map(|e| e.payload["reviewer"].as_str().unwrap()).collect::<Vec<_>>(), ["omar", "acme/web"]);
        let p = &evs[0].payload;
        assert_eq!((p["author"].as_str(), p["me"].as_str(), p["base"].as_str()), (Some("sam"), Some("omar"), Some("main")));
        assert_eq!(p["reviewers"], json!(["lena", "omar"]));
        assert_eq!(p["label"], json!(["agent"]));
        assert_eq!(evs[0].key, "pr:1:review:omar:t2");
        assert_eq!((p["size"].as_u64(), p["changedFiles"].as_u64(), p["milestone"].as_str()), (Some(42), Some(2), Some("v2.1")));
        assert_eq!(p["paths"], json!(["src/a.rs", "README.md"]));
        assert_eq!((p["bot"].as_bool(), p["fork"].as_bool(), evs[1].payload["fork"].as_bool()), (Some(false), Some(false), Some(true)));
        let (again, c) = review_events("pr.review_requested", &list(&[pr(1, &["lena", "omar"], "t2")]), &c, 2, "omar").unwrap();
        assert!(again.is_empty(), "still asked: nothing new");
        // omar reviews (gh drops him), then is asked again: a new request, a new key
        let (_, c) = review_events("pr.review_requested", &list(&[pr(1, &["lena"], "t3")]), &c, 3, "omar").unwrap();
        let (re, _) = review_events("pr.review_requested", &list(&[pr(1, &["lena", "omar"], "t4")]), &c, 4, "omar").unwrap();
        assert_eq!(re.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["pr:1:review:omar:t4"]);
    }

    #[test]
    fn the_gh_call_lists_open_prs_with_their_review_requests() {
        let a = pr_list_args();
        assert_eq!(a[..4], ["pr", "list", "--state", "open"]);
        assert!(a.iter().any(|x| x.contains("reviewRequests") && x.contains("updatedAt")));
        assert_eq!(a[a.len() - 2..], ["--limit", "50"]);
    }

    #[test]
    fn only_people_who_may_push_are_trusted_and_only_a_trigger_that_asks_pays_for_it() {
        assert!(may_push("write\n") && may_push("admin") && may_push("maintain"));
        assert!(!may_push("read") && !may_push("triage") && !may_push("none") && !may_push(""));
        assert!(asks_for(&json!({ "match": { "trusted": true } }), "trusted"));
        assert!(!asks_for(&json!({ "match": { "bot": false } }), "trusted"));
    }

    #[test]
    fn bots_are_what_gh_says_or_an_apps_login() {
        assert!(is_bot(&json!({ "login": "app/dependabot" })));
        assert!(is_bot(&json!({ "login": "renovate[bot]" })));
        assert!(is_bot(&json!({ "login": "x", "is_bot": true })));
        assert!(!is_bot(&json!({ "login": "lena", "is_bot": false })));
    }

    #[test]
    fn signed_out_there_is_no_me_and_gh_saying_nonsense_is_an_error() {
        let (_, c) = review_events("pr.review_requested", "[]", &Value::Null, 0, "").unwrap();
        let (evs, _) = review_events("pr.review_requested", &list(&[pr(3, &["lena"], "t")]), &c, 1, "").unwrap();
        assert!(evs[0].payload.get("me").is_none(), "`me` then matches nothing");
        assert!(review_events("pr.review_requested", "not json", &c, 1, "").is_err());
        assert!(review_events("pr.review_requested", "", &c, 1, "").unwrap().0.is_empty());
    }
}
