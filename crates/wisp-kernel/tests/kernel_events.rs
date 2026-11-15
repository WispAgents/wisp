//! Events (plan §6.11.2 rule 6): published after the command returns Ok, never for a failed one, in
//! order per subscriber, at least once, lagging past the cap; the store's commit hook.
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

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;
use wisp_kernel::{Caller, Event, PluginGroup, WireError};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Made {
    n: u32,
}
impl Event for Made {
    const TYPE: &'static str = "src.made";
}

cmd!(Make, "src.make", (u32, bool), ());
cmd!(Burst, "src.burst", u32, ());
cmd!(Tx, "src.tx", (), ());

fn source() -> common::P {
    common::plugin("wisp.src", json!({}), |s| {
        s.command::<Make, _, _>(|ctx, call, (n, fail)| async move {
            ctx.emit(&call, &Made { n });
            if fail { Err(WireError::conflict("no")) } else { Ok(()) }
        });
        s.command::<Burst, _, _>(|ctx, call, n| async move {
            for i in 0..n {
                ctx.emit(&call, &Made { n: i });
            }
            Ok(())
        });
        // what the store does around its transactions
        s.command::<Tx, _, _>(|ctx, call, ()| async move {
            let hook = ctx.service::<wisp_kernel::CommitHook>()?;
            ctx.emit(&call, &Made { n: 1 });
            hook.committed(&call, Some(9001));
            let mark = hook.mark(&call);
            ctx.emit(&call, &Made { n: 2 });
            hook.rolled_back(&call, mark);
            ctx.emit(&call, &Made { n: 3 });
            Ok(())
        });
    })
}

fn sink(got: Arc<Mutex<Vec<u32>>>, fails: Arc<AtomicU32>) -> common::P {
    common::plugin("wisp.sink", json!({}), move |s| {
        let (got, fails) = (got.clone(), fails.clone());
        s.on::<Made, _, _>(move |_, e| {
            let (got, fails) = (got.clone(), fails.clone());
            async move {
                assert!(!(e.n == 99 && fails.fetch_add(1, Ordering::SeqCst) < 2), "flaky subscriber");
                got.lock().unwrap().push(e.n);
            }
        });
    })
}

#[tokio::test]
async fn after_ok_only_in_order_and_at_least_once() {
    let (got, fails) = (Arc::<Mutex<Vec<u32>>>::default(), Arc::new(AtomicU32::new(0)));
    let k = common::start(PluginGroup::new().add(source()).add(sink(got.clone(), fails.clone()))).await;
    k.call(Caller::user(), "src.make", json!([1, false])).await.unwrap();
    assert!(k.call(Caller::user(), "src.make", json!([2, true])).await.is_err());
    k.call(Caller::user(), "src.burst", json!(100)).await.unwrap();
    k.idle().await;
    let g = got.lock().unwrap().clone();
    assert_eq!(g[0], 1, "the failed command's event never went out");
    assert_eq!(&g[1..], (0..100).collect::<Vec<_>>().as_slice(), "in order, and 99 delivered after two panics");
    assert_eq!(fails.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn lagging_ephemeral_subscriber_drops_and_records_diag() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let g2 = gate.clone();
    let slow = common::plugin("wisp.slow", json!({}), move |s| {
        let g = g2.clone();
        s.on::<Made, _, _>(move |_, _| {
            let g = g.clone();
            async move { g.notified().await }
        });
    });
    let k = common::start(PluginGroup::new().add(source()).add(slow)).await;
    k.call(Caller::user(), "src.burst", json!(wisp_kernel::QUEUE_CAP + 5)).await.unwrap();
    assert!(k.lagging("wisp.slow").is_some());
    assert_eq!(k.failures("wisp.slow").last().unwrap().code, "lagging");
    k.caught_up("wisp.slow");
    assert!(k.lagging("wisp.slow").is_none());
    k.set_enabled("wisp.slow", false).await.unwrap();
    k.idle().await; // dropping the queue settles the work it owed
}

#[tokio::test]
async fn the_commit_hook_publishes_on_commit_and_drops_on_rollback() {
    let seen: Arc<Mutex<Vec<(u32, Option<u64>)>>> = Arc::default();
    let s2 = seen.clone();
    let raw = common::plugin("wisp.raw", json!({}), move |s| {
        let seen = s2.clone();
        s.on_raw("src.*", move |_, env| {
            let seen = seen.clone();
            async move {
                seen.lock().unwrap().push((env.get::<Made>().map_or(0, |m| m.n), env.cursor));
                Ok(())
            }
        });
    });
    let k = common::start(PluginGroup::new().add(source()).add(raw)).await;
    k.call(Caller::user(), "src.tx", json!(null)).await.unwrap();
    k.idle().await;
    // 1 went out on commit with the store's cursor, 2 was rolled back, 3 went out when the command returned
    assert_eq!(*seen.lock().unwrap(), [(1, Some(9001)), (3, None)]);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Made2 {
    n: u32,
    by: String,
}
impl Event for Made2 {
    const TYPE: &'static str = "src.made@2";
}

cmd!(Both, "src.both", u32, ());

#[tokio::test]
async fn event_v2_type_and_v1_type_both_emitted_for_one_minor() {
    let (v1, v2) = (Arc::<Mutex<Vec<u32>>>::default(), Arc::<Mutex<Vec<String>>>::default());
    let (a, b) = (v1.clone(), v2.clone());
    let src = common::plugin("wisp.src", json!({}), |s| {
        s.command::<Both, _, _>(|ctx, call, n| async move { ctx.emit_versions(&call, &Made2 { n, by: "ada".into() }, &Made { n }) });
    });
    let old = common::plugin("wisp.old", json!({}), move |s| {
        let a = a.clone();
        s.on::<Made, _, _>(move |_, e| {
            let a = a.clone();
            async move { a.lock().unwrap().push(e.n) }
        });
    });
    let new = common::plugin("wisp.new", json!({}), move |s| {
        let b = b.clone();
        s.on::<Made2, _, _>(move |_, e| {
            let b = b.clone();
            async move { b.lock().unwrap().push(e.by) }
        });
    });
    let k = common::start(PluginGroup::new().add(src).add(old).add(new)).await;
    k.call(Caller::user(), "src.both", json!(7)).await.unwrap();
    k.idle().await;
    assert_eq!(
        (v1.lock().unwrap().clone(), v2.lock().unwrap().clone()),
        (vec![7], vec!["ada".to_owned()]),
        "an old subscriber keeps getting the old shape"
    );
}

cmd!(Count, "ctr.count", (), u32);

/// ADR-0073: a command that answers from what its subscription learned waits for the events published before it
/// (`Ctx::events_delivered`), so a client that saw the fact and asks next gets it, however slow the handler; with
/// nothing queued it answers at once; a handler that never finishes holds it no longer than the command's cap. One
/// thread: the command gets every chance to answer early before the handler is let go, so the order is fixed, not
/// timed.
#[tokio::test]
async fn a_command_waits_for_its_own_handlers_to_take_the_events_published_before_it() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let seen = Arc::new(AtomicU32::new(0));
    // how long the command waits, in seconds: an hour (never reached) until the stuck-handler case
    let cap = Arc::new(std::sync::atomic::AtomicU64::new(3600));
    let (g, n, c2) = (gate.clone(), seen.clone(), cap.clone());
    let ctr = common::plugin("wisp.ctr", json!({}), move |s| {
        let (g, took) = (g.clone(), n.clone());
        s.on::<Made, _, _>(move |_, _| {
            let (g, n) = (g.clone(), took.clone());
            async move {
                g.notified().await;
                n.fetch_add(1, Ordering::SeqCst);
            }
        });
        let (n, cap) = (n.clone(), c2.clone());
        s.command::<Count, _, _>(move |ctx, _, ()| {
            let (n, cap) = (n.clone(), cap.clone());
            async move {
                ctx.events_delivered(std::time::Duration::from_secs(cap.load(Ordering::SeqCst))).await;
                Ok(n.load(Ordering::SeqCst))
            }
        });
    });
    let k = common::start(PluginGroup::new().add(source()).add(ctr)).await;
    assert_eq!(k.call(Caller::user(), "ctr.count", json!(null)).await.unwrap(), json!(0), "nothing queued: at once");
    k.call(Caller::user(), "src.make", json!([1, false])).await.unwrap();
    let mut ask = Box::pin(k.call(Caller::user(), "ctr.count", json!(null)));
    for _ in 0..200 {
        tokio::select! {
            biased;
            r = &mut ask => panic!("answered before its handler took the event: {r:?}"),
            () = tokio::task::yield_now() => {}
        }
    }
    gate.notify_one();
    assert_eq!(ask.await.unwrap(), json!(1), "the answer has the event it waited for");
    // a handler that never finishes holds the answer only as long as the command said: then it answers with what's there
    cap.store(0, Ordering::SeqCst);
    k.call(Caller::user(), "src.make", json!([2, false])).await.unwrap();
    assert_eq!(k.call(Caller::user(), "ctr.count", json!(null)).await.unwrap(), json!(1), "a stuck handler: answered without it");
    gate.notify_one();
    k.idle().await;
}

cmd!(Stage, "wr.stage", Vec<u32>, ());

/// A write's events (ADR-0074) are queued before its COMMIT, held until it settles: a reader who saw the write's
/// rows (here: between `stage`, the moment before COMMIT, and `commit`, the moment after it, the window where the
/// old code had published nothing yet) and asks a command that waits for its handlers gets an answer with them
/// in it, never before; their handler doesn't run until the commit; a rolled-back write's events are never
/// delivered and never waited for; and a write's events reach the handler in the order they were emitted. One
/// thread: the command gets 200 chances to answer early, so the order is fixed, not timed.
#[tokio::test]
async fn a_writes_events_are_queued_before_it_commits_so_a_reader_of_its_rows_finds_them() {
    let slot: Arc<Mutex<Option<wisp_kernel::Staged>>> = Arc::default();
    let s2 = slot.clone();
    let writer = common::plugin("wisp.wr", json!({}), move |s| {
        let slot = s2.clone();
        // a runner's write outside any command: its events in an outbox, staged inside the transaction
        s.command::<Stage, _, _>(move |ctx, _, ns: Vec<u32>| {
            let slot = slot.clone();
            async move {
                let ob = ctx.outbox();
                for n in ns {
                    ob.emit(&Made { n });
                }
                *slot.lock().unwrap() = Some(ob.stage(Some(7), &[]));
                Ok(())
            }
        });
    });
    let got = Arc::new(Mutex::new(Vec::<(u32, Option<u64>)>::new()));
    let g2 = got.clone();
    let ctr = common::plugin("wisp.ctr", json!({}), move |s| {
        let got = g2.clone();
        s.on_raw("src.made", move |_, e| {
            let got = got.clone();
            async move {
                got.lock().unwrap().push((e.get::<Made>().unwrap().n, e.cursor));
                Ok(())
            }
        });
        let got = g2.clone();
        s.command::<Count, _, _>(move |ctx, _, ()| {
            let got = got.clone();
            async move {
                ctx.events_delivered(std::time::Duration::from_secs(3600)).await;
                Ok(u32::try_from(got.lock().unwrap().len()).unwrap())
            }
        });
    });
    let k = common::start(PluginGroup::new().add(writer).add(ctr)).await;
    k.call(Caller::user(), "wr.stage", json!([1, 2])).await.unwrap();
    // the rows are visible now (COMMIT ran): the reader asks
    let mut ask = Box::pin(k.call(Caller::user(), "ctr.count", json!(null)));
    for _ in 0..200 {
        tokio::select! {
            biased;
            r = &mut ask => panic!("answered before the write's events were delivered: {r:?}"),
            () = tokio::task::yield_now() => {}
        }
    }
    assert!(got.lock().unwrap().is_empty(), "held: no handler runs on a write that hasn't settled");
    slot.lock().unwrap().take().unwrap().commit();
    assert_eq!(ask.await.unwrap(), json!(2), "the answer has both of the write's events");
    assert_eq!(*got.lock().unwrap(), vec![(1, Some(7)), (2, Some(7))], "in the order emitted, with the write's cursor");
    // rolled back: never delivered, never waited for
    k.call(Caller::user(), "wr.stage", json!([3])).await.unwrap();
    slot.lock().unwrap().take().unwrap().abort();
    assert_eq!(k.call(Caller::user(), "ctr.count", json!(null)).await.unwrap(), json!(2));
    // dropped unsettled (the write's outcome unknown): the same as a rollback
    k.call(Caller::user(), "wr.stage", json!([4])).await.unwrap();
    drop(slot.lock().unwrap().take());
    k.idle().await;
    assert_eq!(k.call(Caller::user(), "ctr.count", json!(null)).await.unwrap(), json!(2));
    assert_eq!(got.lock().unwrap().iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2], "3 and 4 never happened");
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CmdArgs {
    /// emitted before the write
    pub before: Vec<u32>,
    /// emitted after it
    pub after: u32,
    /// the command returns an error after its write
    pub fail: bool,
}
cmd!(WriteCmd, "wr.cmd", CmdArgs, ());

/// ADR-0075: a command's events emitted before its write are queued with that write, before its rows can be read
/// (here, as the store does: `durable::take` before the write, `InTx::stage` the moment before COMMIT, the `Staged`
/// settled the moment after), not when the command returns. A reader who sees the row while the command is still
/// running finds them (and waits for their handler, never answering early); events emitted after its last write go
/// out at its return as before; a write that rolls back puts its events back, and a command that then fails
/// publishes none of them. One thread: the reader gets 200 chances to answer early.
#[tokio::test]
async fn a_commands_events_go_out_with_its_write_so_a_reader_of_its_row_finds_them() {
    let slot: Arc<Mutex<Option<wisp_kernel::Staged>>> = Arc::default();
    let go = Arc::new(tokio::sync::Notify::new());
    let (s2, g2) = (slot.clone(), go.clone());
    let writer = common::plugin("wisp.wr", json!({}), move |s| {
        let (slot, go) = (s2.clone(), g2.clone());
        s.command::<WriteCmd, _, _>(move |ctx, call, a: CmdArgs| {
            let (slot, go) = (slot.clone(), go.clone());
            async move {
                for n in a.before {
                    ctx.emit(&call, &Made { n });
                }
                // the store's write: taken on the command's task, staged inside the transaction
                let intx = wisp_kernel::durable::take().expect("inside a command a write always stages its events");
                *slot.lock().unwrap() = Some(intx.stage(Some(7), &[]).expect("staged"));
                // the write committed (or not); the command goes on
                go.notified().await;
                ctx.emit(&call, &Made { n: a.after });
                if a.fail { Err(WireError::conflict("no")) } else { Ok(()) }
            }
        });
    });
    let got = Arc::new(Mutex::new(Vec::<(u32, Option<u64>)>::new()));
    let g3 = got.clone();
    let ctr = common::plugin("wisp.ctr", json!({}), move |s| {
        let got = g3.clone();
        s.on_raw("src.made", move |_, e| {
            let got = got.clone();
            async move {
                got.lock().unwrap().push((e.get::<Made>().unwrap().n, e.cursor));
                Ok(())
            }
        });
        let got = g3.clone();
        s.command::<Count, _, _>(move |ctx, _, ()| {
            let got = got.clone();
            async move {
                ctx.events_delivered(std::time::Duration::from_secs(3600)).await;
                Ok(u32::try_from(got.lock().unwrap().len()).unwrap())
            }
        });
    });
    let k = common::start(PluginGroup::new().add(writer).add(ctr)).await;
    let staged = || async {
        for _ in 0..10_000 {
            if slot.lock().unwrap().is_some() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the command never wrote");
    };
    let run = |a: serde_json::Value| {
        let k = k.clone();
        tokio::spawn(async move { k.call(Caller::user(), "wr.cmd", a).await })
    };

    // 1. committed, the command still running: the reader of its row finds both events
    let cmd = run(json!({ "before": [1, 2], "after": 3 }));
    staged().await;
    let mut ask = Box::pin(k.call(Caller::user(), "ctr.count", json!(null)));
    for _ in 0..200 {
        tokio::select! {
            biased;
            r = &mut ask => panic!("answered before the write's events were delivered: {r:?}"),
            () = tokio::task::yield_now() => {}
        }
    }
    assert!(got.lock().unwrap().is_empty(), "held until the write settles");
    slot.lock().unwrap().take().unwrap().commit();
    assert_eq!(ask.await.unwrap(), json!(2), "the command hasn't returned, and its write's events are in the answer");
    assert_eq!(*got.lock().unwrap(), vec![(1, Some(7)), (2, Some(7))], "in emit order, with the write's cursor");
    go.notify_one();
    cmd.await.unwrap().unwrap();
    k.idle().await;
    assert_eq!(got.lock().unwrap().last(), Some(&(3, None)), "emitted after its last write: out at its return, as before");

    // 2. the write rolls back and the command fails: none of its events
    let cmd = run(json!({ "before": [4], "after": 5, "fail": true }));
    staged().await;
    slot.lock().unwrap().take().unwrap().abort();
    go.notify_one();
    assert!(cmd.await.unwrap().is_err());
    k.idle().await;
    assert_eq!(k.call(Caller::user(), "ctr.count", json!(null)).await.unwrap(), json!(3), "4 and 5 never happened");

    // 3. the write rolls back and the command goes on to Ok: its events wait for its return (they're back in its buffer)
    let cmd = run(json!({ "before": [6], "after": 7 }));
    staged().await;
    slot.lock().unwrap().take().unwrap().abort();
    go.notify_one();
    cmd.await.unwrap().unwrap();
    k.idle().await;
    assert_eq!(got.lock().unwrap()[3..].iter().map(|x| x.0).collect::<Vec<_>>(), vec![6, 7], "at its return, in order");

    // 4. the write commits, then the command fails: the write's events are facts (its rows are there); the rest isn't
    let cmd = run(json!({ "before": [8], "after": 9, "fail": true }));
    staged().await;
    slot.lock().unwrap().take().unwrap().commit();
    go.notify_one();
    assert!(cmd.await.unwrap().is_err());
    k.idle().await;
    assert_eq!(got.lock().unwrap()[5..].iter().map(|x| x.0).collect::<Vec<_>>(), vec![8], "9 never went out");
}
