//! Where an apply came from, and what a plan checks of everything it declares:
//! - AAC-1: each baseline row records its formula's source; `--prune` removes only what this source applied before,
//!   `prune_all` whatever is undeclared, and without prune the plan names what this source no longer declares.
//! - AAC-2: an unchanged resource's references are checked too; a rename renames what names it in stored resources.
//! - AAC-4: a plaintext token in a spec is a problem, and apply refuses it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::needless_pass_by_value)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Caller, Kernel, Manifest, Plugin, Result, Setup};
use wisp_resources_api::{KindInfo, Names, Op, OpKind, RESOURCE_KIND, Resource, ResourceKind};
use wisp_store::{Location, Query, Store, SystemClock};
use wisp_testkit::TestApp;

struct Mem {
    info: KindInfo,
    have: Mutex<Vec<Resource>>,
}

impl ResourceKind for Mem {
    fn info(&self) -> KindInfo {
        self.info.clone()
    }
    fn read<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        Box::pin(async move { Ok(self.have.lock().unwrap().clone()) })
    }
    fn apply<'a>(&'a self, _call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let mut have = self.have.lock().unwrap();
            have.retain(|r| r.name != op.name);
            if op.op != OpKind::Delete {
                have.push(Resource::new(op.kind.clone(), op.name.clone(), op.spec.clone()));
            }
            Ok(())
        })
    }
}

/// An agent kind, a connector kind (secrets in headers) and a workflow kind whose steps name agents; with `.0` the
/// workflow kind starts with one that names `builder`, which no agent is (made in the app before the agent went).
struct Kinds(bool);
impl Plugin for Kinds {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.kinds", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let mk = |info: KindInfo, have: Vec<Resource>| -> Arc<dyn ResourceKind> { Arc::new(Mem { info, have: Mutex::new(have) }) };
        s.contribute(&RESOURCE_KIND, mk(KindInfo::new("agent", "agents", "acme.kinds", json!({})).order(1), Vec::new()));
        s.contribute(
            &RESOURCE_KIND,
            mk(
                KindInfo::new("connector", "connectors", "acme.kinds", json!({})).order(1).secret_field("headers").secret_field("env"),
                Vec::new(),
            ),
        );
        s.contribute(
            &RESOURCE_KIND,
            mk(
                KindInfo::new("workflow", "workflows", "acme.kinds", json!({})).order(2).names(Names::new("steps[].agent", "agent")),
                if self.0 { vec![Resource::new("workflow", "ship", json!({ "steps": [{ "agent": "builder" }] }))] } else { Vec::new() },
            ),
        );
        Ok(())
    }
}

async fn start(seeded: bool) -> Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new().with(wisp_resources::ResourcesPlugin).with(Kinds(seeded)).service(store).start().await
}

fn agent(name: &str) -> Value {
    json!({ "kind": "agent", "name": name, "spec": { "harness": "codex" } })
}

async fn plan(k: &Kernel, args: Value) -> Value {
    k.call(Caller::user(), "resources.plan", args).await.unwrap()
}

fn list(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or_default().to_owned()).collect()).unwrap_or_default()
}

/// The baseline rows' sources, by `kind/name`.
async fn sources(k: &Kernel) -> Vec<(String, Value)> {
    let store = k.ctx("wisp.resources").unwrap().service::<Store>().unwrap();
    let rows = store.read_async(|r| r.query("resources_applied", &Query::new())).await.unwrap();
    let mut out: Vec<(String, Value)> =
        rows.into_iter().map(|r| (r["id"].as_str().unwrap().to_owned(), r.get("source").cloned().unwrap_or(Value::Null))).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[tokio::test]
async fn prune_removes_only_what_this_source_applied_aac_1() {
    let k = start(false).await;
    let apply = |resources: Value, source: Option<&str>| {
        let mut args = json!({ "resources": resources });
        if let Some(s) = source {
            args["source"] = json!(s);
        }
        let k = &k;
        async move { k.call(Caller::user(), "resources.apply", args).await.unwrap() }
    };
    apply(json!([agent("critic")]), Some("repo-a")).await;
    apply(json!([agent("helper")]), Some("repo-b")).await;
    apply(json!([agent("legacy")]), None).await; // an API call naming no source: unknown, like a row from before
    // each row says where it came from; the second apply left the first row's alone
    assert_eq!(
        sources(&k).await,
        [
            ("agent/critic".to_owned(), json!("repo-a")),
            ("agent/helper".to_owned(), json!("repo-b")),
            ("agent/legacy".to_owned(), Value::Null)
        ]
    );

    // repo A's formula declares no agent now: --prune removes critic only
    let p = plan(&k, json!({ "resources": [], "kinds": ["agent"], "prune": true, "source": "repo-a" })).await;
    let ops: Vec<(String, String)> = p["ops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| (o["op"].as_str().unwrap().to_owned(), o["name"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(ops, [("delete".to_owned(), "critic".to_owned())], "{p}");
    assert_eq!(list(&p["kept"]), ["agent/helper", "agent/legacy"], "{p}");

    // --prune-all: every undeclared one, whoever applied it
    let p = plan(&k, json!({ "resources": [], "kinds": ["agent"], "prune": true, "prune_all": true, "source": "repo-a" })).await;
    assert_eq!(p["ops"].as_array().unwrap().len(), 3, "{p}");

    // without --prune: what repo A applied and no longer declares is named, nothing goes
    let p = plan(&k, json!({ "resources": [], "kinds": ["agent"], "source": "repo-a" })).await;
    assert!(p["ops"].as_array().unwrap().is_empty(), "{p}");
    assert_eq!(list(&p["stale"]), ["agent/critic"], "{p}");

    // the apply does what the plan said
    let done = k
        .call(Caller::user(), "resources.apply", json!({ "resources": [], "kinds": ["agent"], "prune": true, "source": "repo-a" }))
        .await
        .unwrap();
    assert_eq!(done["applied"], 1, "{done}");
    assert_eq!(sources(&k).await.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["agent/helper", "agent/legacy"]);
}

#[tokio::test]
async fn an_unchanged_resource_naming_nothing_is_a_problem_aac_2() {
    let k = start(true).await;
    // the stored workflow, declared as it is: no op, but its step still names an agent there isn't
    let p = plan(&k, json!({ "resources": [{ "kind": "workflow", "name": "ship", "spec": { "steps": [{ "agent": "builder" }] } }] })).await;
    assert!(p["ops"].as_array().unwrap().is_empty(), "{p}");
    let problems = list(&p["problems"]);
    assert!(problems.iter().any(|x| x.starts_with("workflows.ship: no agent named builder")), "{problems:?}");
}

#[tokio::test]
async fn a_rename_renames_what_names_it_in_wisp_aac_2() {
    let k = start(true).await;
    k.call(Caller::user(), "resources.apply", json!({ "resources": [agent("builder")] })).await.unwrap();
    let out = k.call(Caller::user(), "resources.rename", json!({ "kind": "agent", "from": "builder", "to": "maker" })).await.unwrap();
    assert_eq!(out["references"], 1, "{out}");
    let wf = k.call(Caller::user(), "resources.export", json!({ "kinds": ["workflow"] })).await.unwrap();
    assert_eq!(wf[0]["spec"], json!({ "steps": [{ "agent": "maker" }] }), "{wf}");
}

#[tokio::test]
async fn a_plaintext_token_is_a_problem_and_apply_refuses_it_aac_4() {
    let k = start(false).await;
    let gh = format!("Bearer ghp_{}", "a".repeat(36));
    let openai = format!("sk-proj-{}", "b".repeat(40));
    let bad = json!({ "resources": [{ "kind": "connector", "name": "gh", "spec": { "headers": { "Authorization": gh }, "env": { "KEY": openai } } }] });
    let problems = list(&plan(&k, bad.clone()).await["problems"]);
    assert!(
        problems.contains(
            &"connectors.gh.headers.Authorization looks like a GitHub token: wisp secret set GH_TOKEN and write ${secret:GH_TOKEN}"
                .to_owned()
        ),
        "{problems:?}"
    );
    assert!(problems.iter().any(|p| p.starts_with("connectors.gh.env.KEY looks like an OpenAI API key")), "{problems:?}");
    assert!(k.call(Caller::user(), "resources.apply", bad).await.is_err(), "apply refuses it");
    // `${secret:GH_TOKEN}` is no problem (the core's test); a header with no token isn't either
    let ok = json!({ "resources": [{ "kind": "connector", "name": "gh", "spec": { "headers": { "Accept": "application/json" } } }] });
    assert_eq!(list(&plan(&k, ok).await["problems"]), Vec::<String>::new());
}
