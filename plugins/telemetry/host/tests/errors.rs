//! v1 server/index.js `captureError`: a server error at the edge (a root command's `internal`, a route's 5xx) goes
//! as `$exception` through `kernel.failures@1` (ADR-0062), scrubbed, once per call; a refusal is an answer, not an
//! error; with stats off nothing goes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::http::{OPEN, Request, Route};
use wisp_kernel::{BoxFuture, Caller, Command, Manifest, Plugin, Result, Setup, WireError};
use wisp_store::{Location, Store, SystemClock};
use wisp_telemetry_api::{SINK, Sink};
use wisp_testkit::TestApp;

#[derive(Default, Clone)]
struct Got(Arc<Mutex<Vec<Value>>>);

struct FileSink(Got);
impl Sink for FileSink {
    fn id(&self) -> &'static str {
        "file"
    }
    fn send(&self, batch: Vec<Value>) -> BoxFuture<'static, bool> {
        self.0.0.lock().unwrap().extend(batch);
        Box::pin(async { true })
    }
}

struct Boom;
impl Command for Boom {
    const ID: &'static str = "errors.boom";
    type Args = Value;
    type Out = Value;
}
struct Outer;
impl Command for Outer {
    const ID: &'static str = "errors.outer";
    type Args = Value;
    type Out = Value;
}
struct Refuse;
impl Command for Refuse {
    const ID: &'static str = "errors.refuse";
    type Args = Value;
    type Out = Value;
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

struct Acme(Got);
impl Plugin for Acme {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.errors", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        s.contribute(&SINK, Arc::new(FileSink(self.0.clone())) as Arc<dyn Sink>);
        s.command::<Boom, _, _>(
            |_ctx, _call, _a| async move { Err(WireError::internal(format!("can't open {}/notes/secret.md", home()))) },
        );
        s.command::<Outer, _, _>(|ctx, call, a| async move { ctx.run::<Boom>(&call, a).await });
        s.command::<Refuse, _, _>(|_ctx, _call, _a| async move { Err(WireError::invalid("say what to do")) });
        s.route(Route {
            method: "GET",
            pattern: "/errors/:id",
            permission: OPEN,
            doc: false,
            summary: "a route that fails",
            raw: false,
            max_body: None,
            handler: Arc::new(|_ctx, _call, _req: Request| Box::pin(async move { Err(WireError::internal("the disk is full")) })),
        });
        // a route whose command fails: one error, the route's (review: it was told by the command and the route)
        s.route(Route {
            method: "POST",
            pattern: "/errors/run",
            permission: OPEN,
            doc: false,
            summary: "a route that runs a failing command",
            raw: false,
            max_body: None,
            handler: Arc::new(|ctx, call, _req: Request| {
                Box::pin(async move { ctx.run::<Boom>(&call, Value::Null).await.and_then(wisp_kernel::http::Response::ok) })
            }),
        });
        s.route(Route {
            method: "GET",
            pattern: "/errors/later",
            permission: OPEN,
            doc: false,
            summary: "a route that answers 503",
            raw: false,
            max_body: None,
            handler: Arc::new(|_ctx, _call, _req: Request| Box::pin(async move { Err(WireError::unavailable("starting")) })),
        });
        Ok(())
    }
}

async fn kernel(got: &Got) -> wisp_kernel::Kernel {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    TestApp::new()
        .with(wisp_telemetry::TelemetryPlugin::with_env(wisp_telemetry_core::Env::new()))
        .with(Acme(got.clone()))
        .service(store)
        .start()
        .await
}

/// What the sinks got once the queue is flushed (waits for the sink tasks).
async fn flushed(k: &wisp_kernel::Kernel, got: &Got) -> Vec<Value> {
    k.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    for _ in 0..50 {
        if !got.0.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    std::mem::take(&mut *got.0.lock().unwrap())
}

fn exceptions(batch: &[Value]) -> Vec<&Value> {
    batch.iter().filter(|e| e["event"] == "$exception").map(|e| &e["properties"]).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_error_at_the_edge_goes_as_one_scrubbed_exception() {
    let got = Got::default();
    let k = kernel(&got).await;
    let err = k.call(Caller::user(), "errors.boom", Value::Null).await.unwrap_err();
    assert_eq!(err.code, wisp_kernel::Code::Internal, "{err:?}");
    let batch = flushed(&k, &got).await;
    let ex = exceptions(&batch);
    assert_eq!(ex.len(), 1, "{batch:?}");
    let e = &ex[0]["$exception_list"][0];
    assert_eq!((ex[0]["route"].as_str(), e["type"].as_str()), (Some("errors.boom"), Some("InternalError")));
    assert_eq!(e["mechanism"]["handled"], true);
    let value = e["value"].as_str().unwrap();
    assert!(!home().is_empty() && !value.contains(&home()), "the home folder is scrubbed: {value}");
    assert!(!value.contains("notes/") && value.ends_with("…/secret.md"), "a path is cut to its name (v1's scrub): {value}");
    // a nested failure is told once, by the call nobody above answers for
    assert!(k.call(Caller::user(), "errors.outer", Value::Null).await.is_err());
    let batch = flushed(&k, &got).await;
    let ex = exceptions(&batch);
    assert_eq!(ex.iter().map(|p| p["route"].as_str().unwrap()).collect::<Vec<_>>(), vec!["errors.outer"], "{batch:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_route_that_answers_5xx_goes_with_its_pattern_never_its_values() {
    let got = Got::default();
    let k = kernel(&got).await;
    let req = Request { method: "GET".into(), path: "/api/errors/task-1234".into(), ..Request::default() };
    assert!(k.serve(Caller::user(), req).await.is_err());
    let batch = flushed(&k, &got).await;
    let ex = exceptions(&batch);
    assert_eq!(ex.len(), 1, "{batch:?}");
    assert_eq!(ex[0]["route"], "GET /errors/:id");
    assert!(!batch[0].to_string().contains("task-1234"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_is_an_answer_and_stats_off_send_no_exception() {
    let got = Got::default();
    let k = kernel(&got).await;
    assert!(k.call(Caller::user(), "errors.refuse", Value::Null).await.is_err());
    assert!(exceptions(&flushed(&k, &got).await).is_empty(), "invalid input isn't a server error");
    k.call(Caller::user(), "telemetry.set", json!({ "on": false })).await.unwrap();
    assert!(k.call(Caller::user(), "errors.boom", Value::Null).await.is_err());
    k.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(got.0.lock().unwrap().is_empty(), "the off switch: no exception either");
}

#[tokio::test(flavor = "multi_thread")]
async fn do_not_track_sends_no_exception() {
    let got = Got::default();
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let env: wisp_telemetry_core::Env = [("DO_NOT_TRACK".to_owned(), "1".to_owned())].into_iter().collect();
    let k = TestApp::new().with(wisp_telemetry::TelemetryPlugin::with_env(env)).with(Acme(got.clone())).service(store).start().await;
    assert!(k.call(Caller::user(), "errors.boom", Value::Null).await.is_err());
    k.call(Caller::user(), "telemetry.flush", Value::Null).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(got.0.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_route_that_runs_a_failing_command_is_one_error_and_a_503_is_none() {
    let got = Got::default();
    let k = kernel(&got).await;
    let run = Request { method: "POST".into(), path: "/api/errors/run".into(), ..Request::default() };
    assert!(k.serve(Caller::user(), run).await.is_err());
    let later = Request { method: "GET".into(), path: "/api/errors/later".into(), ..Request::default() };
    assert!(k.serve(Caller::user(), later).await.is_err());
    let batch = flushed(&k, &got).await;
    let routes: Vec<&str> = exceptions(&batch).iter().map(|p| p["route"].as_str().unwrap()).collect();
    assert_eq!(routes, vec!["POST /errors/run"], "the route's 5xx once, nothing for a plugin that isn't ready: {batch:?}");
}
