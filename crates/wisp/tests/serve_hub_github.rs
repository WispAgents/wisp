//! A team hub's GitHub App webhooks on a real `wisp serve` (ADR-0148 §8e), the grid stood in for by the proxy secret and
//! `x-wisp-proxy-via: github`: an `issues` event starts the trigger of the project that names its repo (by name or
//! folder), once however often it comes, and no other project's; a person, a viewer or a deploy key can't post one.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{Api, start_with, temp};
use serde_json::{Value, json};

const SECRET: &str = "proxy-secret-for-the-hub-github-test-0123";

#[tokio::test(flavor = "multi_thread")]
async fn a_github_issue_webhook_starts_the_trigger_of_the_project_naming_its_repo_once() {
    let dir = temp("serve-hub-github");
    let (_serve, port) = start_with(&dir, &[("WISP_TRUSTED_PROXY_SECRET", SECRET), ("WISP_PUBLIC_URL", "http://acme.grid.test")]);
    let api = Api { http: reqwest::Client::new(), port, n: 0.into() };
    let (app, web) = (dir.join("app"), dir.join("web"));
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(&web).unwrap();
    let r = api
        .try_cmd(
            "resources.apply",
            json!({ "resources": [
                { "kind": "project", "name": "app", "spec": { "path": app, "repo": "https://github.com/acme-org/app" } },
                { "kind": "project", "name": "web", "spec": { "path": web, "repo": "acme-org/web" } },
                { "kind": "trigger", "name": "triage", "spec": { "on": "issue.opened", "project": "app", "prompt": "Triage {event.summary}" } },
                { "kind": "trigger", "name": "web-triage", "spec": { "on": "issue.opened", "project": "web", "prompt": "Web {event.summary}" } }
            ] }),
        )
        .await;
    assert_eq!(r["ok"], true, "{r}");
    let body = json!({ "action": "opened", "repository": { "full_name": "acme-org/app" },
        "issue": { "number": 3, "title": "slug keeps a dash", "user": { "login": "ada", "type": "User" }, "labels": [] } });
    let post = |via: &'static str| {
        let body = body.to_string();
        let http = api.http.clone();
        async move {
            let r = http
                .post(format!("http://127.0.0.1:{port}/api/issues/webhook"))
                .header("host", "acme.grid.test")
                .header("x-wisp-proxy-secret", SECRET)
                .header("x-wisp-proxy-via", via)
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap();
            (r.status().as_u16(), r.json::<Value>().await.unwrap_or(Value::Null))
        }
    };
    // no one but the grid's checked delivery posts one
    for via in ["member", "viewer", "deploy", "plan"] {
        let (s, v) = post(via).await;
        assert_eq!(s, 403, "{via}: {v}");
    }
    let (s, v) = post("github").await;
    assert_eq!((s, v["pushed"].clone()), (200, json!(1)), "{v}");
    // the trigger of the project naming acme-org/app started its task; the other project's didn't
    let mut prompts = Vec::new();
    for _ in 0..100 {
        let list = api.cmd("runtime.list", json!({})).await;
        prompts = list.as_array().unwrap().iter().map(ToString::to_string).collect();
        if !prompts.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(prompts.len(), 1, "{prompts:?}");
    assert!(prompts[0].contains("slug keeps a dash") && !prompts[0].contains("Web "), "{prompts:?}");
    // GitHub delivering it again (it retries) starts nothing new
    post("github").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(api.cmd("runtime.list", json!({})).await.as_array().unwrap().len(), 1);
    // a repo no project names reaches no trigger
    let mut other = body.clone();
    other["repository"]["full_name"] = json!("someone/else");
    other["issue"]["number"] = json!(4);
    let r = api
        .http
        .post(format!("http://127.0.0.1:{port}/api/issues/webhook"))
        .header("host", "acme.grid.test")
        .header("x-wisp-proxy-secret", SECRET)
        .header("x-wisp-proxy-via", "github")
        .header("content-type", "application/json")
        .body(other.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(r.json::<Value>().await.unwrap()["pushed"], 0);
}
