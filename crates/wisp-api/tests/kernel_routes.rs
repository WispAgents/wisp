//! A plugin's route served through the edge's table (`<Kernel as Backend>::routes`) runs as the kernel's own
//! `serve` does: the caller's task scope is checked before any handler code runs, and the handler speaks within
//! its plugin's permissions, not with the whole of the caller's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use wisp_api::backend::Backend;
use wisp_kernel::http::{Request, Response, Route};
use wisp_kernel::scope::{TASKS, TaskTree};
use wisp_kernel::{BoxFuture, Caller, Code, Command, Kernel, Manifest, Plugin, Result, Setup};
use wisp_testkit::TestApp;

static RAN_X: AtomicBool = AtomicBool::new(false);
static ENTERED: AtomicBool = AtomicBool::new(false);

fn manifest(id: &str) -> Manifest {
    Manifest::from_value(json!({ "wispPlugin": 1, "id": id, "version": "1.0.0", "api": "^1" })).unwrap()
}

/// No task is under another.
struct Tree;
impl TaskTree for Tree {
    fn within(&self, _task: &str, _root: &str) -> BoxFuture<'static, bool> {
        Box::pin(async { false })
    }
}
struct Tasks;
impl Plugin for Tasks {
    fn manifest(&self) -> Manifest {
        manifest("acme.tasks")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.contribute(&TASKS, Arc::new(Tree) as Arc<dyn TaskTree>);
        Ok(())
    }
}

/// X: a command the owner may run and the route's plugin was never granted.
struct X;
impl Command for X {
    const ID: &'static str = "vault.open";
    const PERMISSION: Option<&'static str> = Some("vault:open");
    type Args = Value;
    type Out = Value;
}
struct Vault;
impl Plugin for Vault {
    fn manifest(&self) -> Manifest {
        manifest("acme.vault")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.command::<X, _, _>(|_, _, _| async move {
            RAN_X.store(true, Ordering::SeqCst);
            Ok(json!({ "opened": true }))
        });
        Ok(())
    }
}

/// Its manifest grants nothing; one route runs X, one names a task.
struct Pages;
impl Plugin for Pages {
    fn manifest(&self) -> Manifest {
        manifest("acme.pages")
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.route(Route {
            method: "GET",
            pattern: "/pages/open",
            permission: "settings:read",
            doc: false,
            summary: "runs X",
            raw: false,
            max_body: None,
            handler: Arc::new(|ctx, call, _req: Request| {
                Box::pin(async move { Response::ok(ctx.run_json(&call, "vault.open", json!({})).await?) })
            }),
        });
        s.route(Route {
            method: "GET",
            pattern: "/pages/guarded",
            permission: "settings:read",
            doc: false,
            summary: "X's work inline, guarded by the call's own check",
            raw: false,
            max_body: None,
            handler: Arc::new(|_, call, _req: Request| {
                Box::pin(async move {
                    call.require("vault:open")?;
                    RAN_X.store(true, Ordering::SeqCst);
                    Response::ok(json!({ "via": call.via() }))
                })
            }),
        });
        s.route(Route {
            method: "GET",
            pattern: "/pages/:sid",
            permission: "agent:read",
            doc: false,
            summary: "a task's page",
            raw: false,
            max_body: None,
            handler: Arc::new(|_, _, req: Request| {
                // the handler's synchronous prelude: it must not run for a request the scope refuses
                ENTERED.store(true, Ordering::SeqCst);
                Box::pin(async move { Response::ok(json!({ "of": req.param("sid") })) })
            }),
        });
        Ok(())
    }
}

async fn edge(k: &Kernel, caller: Caller, pattern: &str, params: &[(&str, &str)]) -> Result<Response> {
    let b = <Kernel as Backend>::routes(k).into_iter().find(|b| b.spec.pattern == pattern).expect("the route is in the table");
    let req = Request {
        method: "GET".into(),
        path: format!("/api{pattern}"),
        params: params.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect(),
        ..Request::default()
    };
    (b.invoke)(caller, req).await
}

#[tokio::test]
async fn edge_route_is_bounded_by_plugin_permissions() {
    let k = TestApp::new().with(Tasks).with(Vault).with(Pages).start().await;
    assert!(k.call(Caller::user(), "vault.open", json!({})).await.is_ok(), "the owner may run X itself");
    RAN_X.store(false, Ordering::SeqCst);
    let e = edge(&k, Caller::user(), "/pages/open", &[]).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden, "{}", e.message);
    // the handler's own call speaks within acme.pages' grant too, so a check it makes on it refuses
    let e = edge(&k, Caller::user(), "/pages/guarded", &[]).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden, "{}", e.message);
    assert!(!RAN_X.load(Ordering::SeqCst), "X's body never ran");
}

#[tokio::test]
async fn edge_route_scope_checked_before_handler() {
    let k = TestApp::new().with(Tasks).with(Vault).with(Pages).start().await;
    let agent = Caller { set: "agent:session".into(), session: Some("A".into()), plugin: None };
    let e = edge(&k, agent.clone(), "/pages/:sid", &[("sid", "B")]).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden, "{}", e.message);
    assert!(!ENTERED.load(Ordering::SeqCst), "no handler code ran for a refused scope");
    // its own task still goes through
    assert!(edge(&k, agent, "/pages/:sid", &[("sid", "A")]).await.is_ok());
    assert!(ENTERED.load(Ordering::SeqCst));
}
