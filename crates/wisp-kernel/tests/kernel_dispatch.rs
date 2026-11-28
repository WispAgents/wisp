//! The command bus (plan §6.5, §6.11.2): default deny, a call narrows through plugins, depth ≤ 8,
//! no re-entry, interceptors only read, lazy activation, plugin off.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::many_single_char_names
)]
mod common;

use serde_json::{Value, json};
use wisp_kernel::{Caller, Code, PluginGroup};

cmd!(Echo, "alpha.echo", Value, Value);
cmd!(Deep, "alpha.deep", u32, u32);
cmd!(Loop, "alpha.loop", (), ());
cmd!(Peek, "alpha.peek", (), u32, read_only);
cmd!(Write, "alpha.write", (), ());
cmd!(BetaUse, "beta.use", (), Value);
cmd!(Lazy, "lazy.hello", (), String);
cmd!(Mode, "alpha.mode", String, ());

fn alpha() -> common::P {
    common::plugin("wisp.alpha", json!({ "permissions": ["runtime:start"] }), |s| {
        s.command::<Echo, _, _>(|_, _, a| async move { Ok(a) });
        s.command::<Deep, _, _>(|ctx, call, n| async move { if n == 0 { Ok(call.depth()) } else { ctx.run::<Deep>(&call, n - 1).await } });
        s.command::<Loop, _, _>(|ctx, call, ()| async move { ctx.run::<Loop>(&call, ()).await });
        s.command::<Peek, _, _>(|_, call, ()| async move { Ok(call.depth()) });
        s.command::<Write, _, _>(|_, _, ()| async move { Ok(()) });
        s.command::<Mode, _, _>(|_, call, m| async move { call.require(&format!("runtime:start{{mode:{m}}}")) });
    })
}

fn beta(perms: &[&str]) -> common::P {
    common::plugin("wisp.beta", json!({ "permissions": perms }), |s| {
        s.command::<BetaUse, _, _>(|ctx, call, ()| async move { ctx.run::<Echo>(&call, json!("via beta")).await });
    })
}

#[tokio::test]
async fn edge_calls_run_typed_and_json() {
    let k = common::start(PluginGroup::new().add(alpha())).await;
    assert_eq!(k.call(Caller::user(), "alpha.echo", json!({ "a": 1 })).await.unwrap(), json!({ "a": 1 }));
    assert_eq!(k.call(Caller::user(), "alpha.deep", json!(3)).await.unwrap(), json!(3));
    let e = k.call(Caller::user(), "alpha.deep", json!("x")).await.unwrap_err();
    assert_eq!(e.code, Code::Invalid);
    let e = k.call(Caller::user(), "nobody.here", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::NotFound);
    assert!(e.message.contains("unknown command nobody.here"));
}

#[tokio::test]
async fn default_deny_and_capability_sets() {
    let k = common::start(PluginGroup::new().add(alpha())).await;
    // agent:session holds runtime:* … but not alpha:*
    let e = k.call(Caller::agent("s1", false), "alpha.echo", json!(1)).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden);
    assert!(e.message.contains("agent:session may not alpha:echo"), "{}", e.message);
    assert_eq!(k.call(Caller::set("pane:x"), "alpha.write", json!(null)).await.unwrap_err().code, Code::Forbidden);
    k.define_set(&wisp_kernel::caps::pane_set("x", vec!["alpha:write".into()]));
    assert!(k.call(Caller::set("pane:x"), "alpha.write", json!(null)).await.is_ok());
    // scoped checks inside a command: the mode cap, in v1's words for capped sets
    assert!(
        k.call(Caller::agent_at("s1", "edits", false), "alpha.mode", json!("edits")).await.is_err(),
        "alpha:mode not granted to agents"
    );
    k.define_set(&wisp_kernel::CapabilitySet {
        id: "agent:session:edits".into(),
        allow: vec!["alpha:*".into(), "runtime:*".into()],
        deny: vec!["runtime:start{mode:auto}".into()],
        ..Default::default()
    });
    assert!(k.call(Caller::agent_at("s1", "edits", false), "alpha.mode", json!("edits")).await.is_ok());
    let e = k.call(Caller::agent_at("s1", "edits", false), "alpha.mode", json!("auto")).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden);
    // refusals are kept with their trace
    assert!(k.refusals().iter().any(|r| r.code == "forbidden" && r.stack.iter().any(|f| f == "wisp.alpha:alpha.echo")));
}

/// ADR-0147: a read-only set (a hub's viewer) starts reads and is refused a write, whatever its allow list says;
/// the same allow list without `readOnly` writes.
#[tokio::test]
async fn a_read_only_set_starts_reads_only() {
    let k = common::start(PluginGroup::new().add(alpha())).await;
    k.define_set(&wisp_kernel::CapabilitySet { id: "test:viewer".into(), allow: vec!["*".into()], read_only: true, ..Default::default() });
    k.define_set(&wisp_kernel::CapabilitySet { id: "test:member".into(), allow: vec!["*".into()], ..Default::default() });
    assert_eq!(k.call(Caller::set("test:viewer"), "alpha.peek", json!(null)).await.unwrap(), json!(0));
    let e = k.call(Caller::set("test:viewer"), "alpha.write", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden);
    assert!(e.message.contains("only reads"), "{}", e.message);
    assert!(k.call(Caller::set("test:member"), "alpha.write", json!(null)).await.is_ok());
    // the data file's viewer is read-only too
    let e = k.call(Caller::set("hub:viewer"), "alpha.write", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden);
    assert!(k.call(Caller::set("hub:viewer"), "alpha.peek", json!(null)).await.is_ok());
}

#[tokio::test]
async fn a_call_narrows_through_the_plugin_it_passes() {
    let k = common::start(PluginGroup::new().add(alpha()).add(beta(&[]))).await;
    let e = k.call(Caller::user(), "beta.use", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::Forbidden);
    assert!(e.message.contains("wisp.beta doesn't declare alpha:echo"), "{}", e.message);
    let k = common::start(PluginGroup::new().add(alpha()).add(beta(&["alpha:echo"]))).await;
    assert_eq!(k.call(Caller::user(), "beta.use", json!(null)).await.unwrap(), json!("via beta"));
    // the plugin's grant doesn't widen the caller: a preview link still can't
    assert_eq!(k.call(Caller::set("preview"), "beta.use", json!(null)).await.unwrap_err().code, Code::Forbidden);
}

#[tokio::test]
async fn depth_and_reentry() {
    let k = common::start(PluginGroup::new().add(alpha())).await;
    assert_eq!(k.call(Caller::user(), "alpha.deep", json!(7)).await.unwrap(), json!(7));
    let e = k.call(Caller::user(), "alpha.deep", json!(8)).await.unwrap_err();
    assert_eq!(e.code, Code::DepthExceeded);
    assert!(e.message.contains("at depth 8"));
    // a command may call itself (re-entry is refused for hooks, rule 3); depth bounds it, with the trace
    let e = k.call(Caller::user(), "alpha.loop", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::DepthExceeded);
    assert!(e.message.contains("wisp.alpha:alpha.loop → wisp.alpha:alpha.loop"), "{}", e.message);
}

#[tokio::test]
async fn interceptors_only_read() {
    struct H;
    impl wisp_kernel::Hook for H {
        const ID: &'static str = "before:thing";
        type Ctx = Value;
    }
    cmd!(Go, "gamma.go", (), Value);
    let g = common::plugin("wisp.gamma", json!({ "permissions": ["alpha:*"] }), |s| {
        s.command::<Go, _, _>(|ctx, call, ()| async move { ctx.run_hook::<H>(&call, json!({})).await });
        s.intercept::<H, _, _>(
            wisp_kernel::InterceptOpts { failure: wisp_kernel::Failure::Closed, ..Default::default() },
            |ctx, call, c| async move {
                assert!(call.read_only());
                let peek = ctx.run::<Peek>(&call, ()).await;
                let write = ctx.run::<Write>(&call, ()).await;
                match (peek, write) {
                    (Ok(d), Err(e)) if e.code == Code::Forbidden && e.message.contains("an interceptor may only read") => {
                        wisp_kernel::Verdict::Modify { ctx: json!({ "peek": d, "write": "refused", "c": c }) }
                    }
                    other => wisp_kernel::Verdict::Veto { code: "bad".into(), message: format!("{other:?}"), then: Vec::new() },
                }
            },
        );
    });
    let k = common::start(PluginGroup::new().add(alpha()).add(g)).await;
    let out = k.call(Caller::user(), "gamma.go", json!(null)).await.unwrap();
    assert_eq!(out["write"], "refused");
    assert_eq!(out["peek"], 2, "edge 0 → interceptor 1 → peek 2");
    assert!(k.call(Caller::user(), "alpha.write", json!(null)).await.is_ok(), "outside an interceptor it writes");
}

#[tokio::test]
async fn a_lazy_plugin_activates_on_its_command_and_its_routes_wait() {
    let lazy = common::plugin(
        "wisp.lazy",
        json!({ "activation": ["onCommand:lazy.*"], "contributes": { "commands": [{ "id": "lazy.hello", "title": "Hello" }] } }),
        |s| {
            s.command::<Lazy, _, _>(|_, _, ()| async move { Ok("hi".to_owned()) });
        },
    );
    let k = common::start(PluginGroup::new().add(lazy)).await;
    assert_eq!(common::state(&k, "wisp.lazy").0, "resolved");
    assert_eq!(k.call(Caller::user(), "lazy.hello", json!(null)).await.unwrap(), json!("hi"));
    assert_eq!(common::state(&k, "wisp.lazy").0, "active");
    k.set_enabled("wisp.lazy", false).await.unwrap();
    let e = k.call(Caller::user(), "lazy.hello", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::PluginOff);
    assert!(e.message.contains("wisp.lazy is off (wisp.lazy was switched off)"), "{}", e.message);
}

#[tokio::test]
async fn a_lazy_plugins_declared_commands_are_in_the_catalogue_before_it_wakes() {
    cmd!(Cat, "cat.list", (), Value, read_only);
    let lazy = common::plugin(
        "wisp.lazy",
        json!({ "activation": ["onCommand:lazy.*"], "contributes": { "commands": [{ "id": "lazy.hello", "title": "Hello" }] } }),
        |s| {
            s.command::<Lazy, _, _>(|_, _, ()| async move { Ok("hi".to_owned()) });
        },
    );
    let cat = common::plugin("wisp.cat", json!({}), |s| {
        s.command::<Cat, _, _>(|ctx, call, ()| {
            let out = ctx.catalogue(call.caller());
            async move { Ok(serde_json::to_value(out).unwrap()) }
        });
    });
    let k = common::start(PluginGroup::new().add(lazy).add(cat)).await;
    let ids = |v: &Value| v.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    let before = k.call(Caller::user(), "cat.list", json!(null)).await.unwrap();
    assert_eq!(common::state(&k, "wisp.lazy").0, "resolved", "reading the catalogue doesn't wake it");
    assert!(ids(&before).contains(&"lazy.hello".to_owned()), "{before}");
    let row = before.as_array().unwrap().iter().find(|c| c["id"] == "lazy.hello").unwrap();
    assert_eq!((row["plugin"].as_str(), row["title"].as_str()), (Some("wisp.lazy"), Some("Hello")));
    assert_eq!(k.call(Caller::user(), "lazy.hello", json!(null)).await.unwrap(), json!("hi"));
    let after = k.call(Caller::user(), "cat.list", json!(null)).await.unwrap();
    assert_eq!(ids(&after).iter().filter(|i| *i == "lazy.hello").count(), 1, "listed once once it's awake");
    k.set_enabled("wisp.lazy", false).await.unwrap();
    let off = k.call(Caller::user(), "cat.list", json!(null)).await.unwrap();
    assert!(!ids(&off).contains(&"lazy.hello".to_owned()), "a plugin switched off isn't listed: {off}");
}

#[tokio::test]
async fn a_command_outside_the_namespace_refuses_the_plugin() {
    let bad = common::plugin("wisp.bad", json!({}), |s| {
        s.command::<Echo, _, _>(|_, _, a| async move { Ok(a) });
    });
    let k = common::start(PluginGroup::new().add(bad)).await;
    let (state, why) = common::state(&k, "wisp.bad");
    assert_eq!(state, "refused");
    assert!(why.unwrap().contains("registers alpha.echo outside its namespace"));
}

#[tokio::test]
async fn a_panicking_command_is_an_internal_error_and_kept_in_diagnostics() {
    cmd!(Boom, "boom.go", (), ());
    let p = common::plugin("wisp.boom", json!({}), |s| {
        s.command::<Boom, _, _>(|_, _, ()| async move { panic!("kaboom") });
    });
    let k = common::start(PluginGroup::new().add(p)).await;
    let e = k.call(Caller::user(), "boom.go", json!(null)).await.unwrap_err();
    assert_eq!(e.code, Code::Internal);
    assert_eq!(k.failures("wisp.boom").last().unwrap().code, "internal");
}
