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

/// GitHub's `pull_request` webhook (ADR-0148 §8e: a hub's App sends it) as the events a poll would have made of it:
/// `review_requested` is `pr.review_requested` for the one just asked, with the poll's payload and key (the same PR at
/// the same moment), so a webhook and the poll after it fire a trigger once. Any other action: none. `repo` (the
/// repository's `owner/name`) is in each payload, for the hub to find the project.
pub fn webhook_events(body: &Value, at: i64) -> Vec<TriggerEvent> {
    if body.get("action").and_then(Value::as_str) != Some("review_requested") {
        return Vec::new();
    }
    let Some(pr) = body.get("pull_request") else { return Vec::new() };
    let str_at = |v: &Value, p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or_default().to_owned();
    let org = str_at(body, "/organization/login");
    let team = |t: &Value| json!({ "__typename": "Team", "slug": t.get("slug").cloned().unwrap_or(Value::Null), "organization": org });
    let user = |u: &Value| json!({ "__typename": "User", "login": u.get("login").cloned().unwrap_or(Value::Null) });
    let mut asked: Vec<Value> =
        pr.get("requested_reviewers").and_then(Value::as_array).map(|a| a.iter().map(user).collect()).unwrap_or_default();
    asked.extend(pr.get("requested_teams").and_then(Value::as_array).map(|a| a.iter().map(team).collect::<Vec<_>>()).unwrap_or_default());
    let just = body.get("requested_team").map(team).or_else(|| body.get("requested_reviewer").map(user));
    let Some(just) = just.as_ref().and_then(requested) else { return Vec::new() };
    let author = pr.get("user").cloned().unwrap_or(Value::Null);
    let gh = json!({
        "number": pr.get("number").cloned().unwrap_or(Value::Null),
        "title": str_at(pr, "/title"), "url": str_at(pr, "/html_url"),
        "author": { "login": author.get("login").cloned().unwrap_or(Value::Null), "is_bot": author.get("type").and_then(Value::as_str) == Some("Bot") },
        "reviewRequests": asked,
        "labels": pr.get("labels").cloned().unwrap_or_else(|| json!([])),
        "baseRefName": str_at(pr, "/base/ref"), "headRefName": str_at(pr, "/head/ref"),
        "isDraft": pr.get("draft").and_then(Value::as_bool).unwrap_or(false),
        "updatedAt": str_at(pr, "/updated_at"),
        "isCrossRepository": str_at(pr, "/head/repo/full_name") != str_at(pr, "/base/repo/full_name"),
        "additions": pr.get("additions").cloned().unwrap_or(json!(0)), "deletions": pr.get("deletions").cloned().unwrap_or(json!(0)),
        "changedFiles": pr.get("changed_files").cloned().unwrap_or(json!(0)),
        "files": [], "milestone": { "title": pr.pointer("/milestone/title").cloned().unwrap_or(Value::Null) },
    });
    let n = pr.get("number").and_then(Value::as_u64).unwrap_or(0).to_string();
    // everyone asked before this one: the poll's cursor as it was just before
    let had: Vec<String> = asked.iter().filter_map(requested).filter(|w| *w != just).collect();
    let (mut out, _) = review_events("pr.review_requested", &json!([gh]).to_string(), &json!({ n: had }), at, "").unwrap_or_default();
    let repo = str_at(body, "/repository/full_name");
    for e in &mut out {
        e.payload["repo"] = json!(repo);
    }
    out
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

    /// A hub's App: GitHub's `review_requested` webhook is the poll's own event for that reviewer, same key.
    #[test]
    fn a_review_request_webhook_is_the_polls_event() {
        let body = json!({
            "action": "review_requested",
            "requested_reviewer": { "login": "bo" },
            "organization": { "login": "acme" },
            "repository": { "full_name": "acme/app" },
            "pull_request": {
                "number": 12, "title": "Fix slug", "html_url": "https://github.com/acme/app/pull/12", "draft": false,
                "user": { "login": "ada", "type": "User" }, "updated_at": "2026-11-28T09:00:00Z",
                "requested_reviewers": [{ "login": "cy" }, { "login": "bo" }], "requested_teams": [{ "slug": "core" }],
                "labels": [{ "name": "bug" }], "base": { "ref": "main", "repo": { "full_name": "acme/app" } },
                "head": { "ref": "fix", "repo": { "full_name": "acme/app" } }, "additions": 3, "deletions": 1, "changed_files": 1,
            },
        });
        let evs = webhook_events(&body, 5);
        assert_eq!(evs.len(), 1, "{evs:?}");
        let e = &evs[0];
        assert_eq!((e.on.as_str(), e.key.as_str()), ("pr.review_requested", "pr:12:review:bo:2026-11-28T09:00:00Z"));
        assert_eq!(e.payload["reviewer"], "bo");
        assert_eq!(e.payload["reviewers"], json!(["cy", "bo", "acme/core"]));
        assert_eq!(
            (e.payload["author"].clone(), e.payload["size"].clone(), e.payload["repo"].clone()),
            (json!("ada"), json!(4), json!("acme/app"))
        );
        // the poll seeing the same PR after it, with bo already in its cursor, makes nothing new
        let polled = json!([{ "number": 12, "title": "Fix slug", "updatedAt": "2026-11-28T09:00:00Z",
            "reviewRequests": [{ "login": "cy" }, { "login": "bo" }] }]);
        let (again, _) = review_events("pr.review_requested", &polled.to_string(), &json!({ "12": ["cy", "bo"] }), 6, "").unwrap();
        assert!(again.is_empty());
        // a team asked
        let team = json!({ "action": "review_requested", "requested_team": { "slug": "core" }, "organization": { "login": "acme" },
            "pull_request": { "number": 3, "updated_at": "t", "requested_teams": [{ "slug": "core" }] } });
        assert_eq!(webhook_events(&team, 1)[0].payload["reviewer"], "acme/core");
        assert!(webhook_events(&json!({ "action": "opened", "pull_request": {} }), 1).is_empty());
    }
}
