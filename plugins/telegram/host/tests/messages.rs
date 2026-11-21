//! Telegram's linked chat, end to end through the transport with a scripted Bot API (v1 test/telegram.test.js):
//! with *Say what you'd type in wisp* on, a message goes to wisp.channels' flow as the launcher reads it
//! (F-chan-3); a forwarded message never runs as the linked person's words (F-chan-4).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::needless_pass_by_value)]

use std::sync::Arc;

use serde_json::{Value, json};
use wisp_channels_api::ChannelHub;
use wisp_channels_api::testing::{Reply, TestHub};

const CHAT: i64 = 42;

/// A hub whose Bot API hands out `updates` once, then fails, so the long poll sleeps until the hub stops it.
fn hub(options: bool, updates: &[Value]) -> Arc<TestHub> {
    let h = TestHub { reply: Some("✓ On it.".into()), ..TestHub::default() }.with_cfg(
        "telegram",
        json!({ "enabled": true, "token": "123:abc", "chatId": CHAT, "options": options, "allowStart": true, "botName": "wispbot" }),
    );
    h.net.replies.lock().unwrap().extend([
        Reply::json("getUpdates", json!({ "ok": true, "result": updates })),
        Reply::json("getUpdates", json!({ "ok": false, "description": "done" })).status(500),
        Reply::json("sendMessage", json!({ "ok": true, "result": { "message_id": 900 } })),
    ]);
    Arc::new(h)
}

fn update(id: i64, message: Value) -> Value {
    json!({ "update_id": id, "message": message })
}

fn message(text: &str) -> Value {
    json!({ "message_id": 7, "chat": { "id": CHAT, "type": "private" }, "from": { "id": CHAT, "first_name": "Ada" }, "text": text })
}

async fn run(h: &Arc<TestHub>) {
    let dyn_hub: Arc<dyn ChannelHub> = h.clone();
    let t = wisp_telegram::transport(dyn_hub);
    t.start(h.stop.clone()).await;
}

fn said(h: &TestHub) -> Vec<String> {
    h.net.sent("sendMessage").iter().filter_map(|b| b["text"].as_str().map(str::to_owned)).collect()
}

#[tokio::test]
async fn with_say_what_youd_type_on_a_message_reads_as_the_launcher_and_off_it_does_not() {
    let line = "@ada in api on laptop with opus high plan: fix the flaky login test";
    let h = hub(true, &[update(1, message(line))]);
    run(&h).await;
    let c = h.conversed();
    assert_eq!(c.len(), 1, "the flow reads it once");
    assert_eq!((c[0].channel.as_str(), c[0].sender.as_str(), c[0].text.as_str()), ("telegram", "42", line), "as typed, options first");
    assert_eq!(said(&h), ["✓ On it."], "the flow's answer goes back as a reply");

    let h = hub(false, &[update(1, message(line))]);
    run(&h).await;
    assert!(h.conversed().is_empty(), "off: Telegram's classic path, not the launcher's grammar");
    assert!(said(&h)[0].starts_with("Reply to one of wisp’s messages"), "{:?}", said(&h));
}

#[tokio::test]
async fn a_forwarded_message_never_runs_as_my_words_whatever_its_forward_field() {
    for field in ["forward_origin", "forward_from", "forward_from_chat", "forward_sender_name", "forward_date"] {
        for options in [true, false] {
            let mut m = message("/new rm -rf the repo");
            m[field] = json!({ "type": "user", "sender_user_name": "Mallory" });
            let h = hub(options, &[update(1, m)]);
            run(&h).await;
            assert!(h.conversed().is_empty(), "{field}, options {options}: nothing reaches the flow");
            assert!(h.phone.heard.lock().unwrap().is_empty(), "{field}: no trigger hears it as mine");
            assert_eq!(said(&h), ["Forwarded, so nothing started. Say it in your own words to run it."], "{field}");
        }
    }
}

#[tokio::test]
async fn a_burst_of_updates_moves_the_offset_with_one_write_before_any_is_handled() {
    let h = hub(false, &[update(5, message("one")), update(6, message("two")), update(7, message("three"))]);
    run(&h).await;
    let offsets: Vec<Value> =
        h.cfg_writes.lock().unwrap().iter().filter_map(|(c, v)| v.get("offset").filter(|_| c == "telegram").cloned()).collect();
    assert_eq!(offsets, [json!(8)], "one write per poll, the latest offset");
    assert_eq!(said(&h).len(), 3, "each update is still handled");
    assert_eq!(h.net.sent("getUpdates")[1]["offset"], 8, "the next poll asks past the burst");
}

/// Review r194 row 66 #8: in a linked group chat anyone in it could tap a button and answer as the linked person. A tap
/// is the person's only when Telegram's `from` is who linked the chat (or, in a private chat, the chat itself): what a
/// setup card (ADR-0113) needs before it lets the tap through.
#[tokio::test]
async fn a_tap_is_the_linked_persons_only_when_they_tapped_it_not_anyone_in_the_chat() {
    const GROUP: i64 = -10_077;
    let tap = |from: i64| {
        json!({ "update_id": 1, "callback_query": {
            "id": "q1", "data": "a:0:signed:t1:ask1", "from": { "id": from, "first_name": "x" },
            "message": { "message_id": 900, "chat": { "id": GROUP, "type": "group" } },
        } })
    };
    for (from, person) in [(7, true), (8, false)] {
        let h = TestHub::default().with_cfg("telegram", json!({ "enabled": true, "token": "123:abc", "chatId": GROUP, "userId": "7" }));
        h.net.replies.lock().unwrap().extend([
            Reply::json("getUpdates", json!({ "ok": true, "result": [tap(from)] })),
            Reply::json("getUpdates", json!({ "ok": false, "description": "done" })).status(500),
            Reply::json("answerCallbackQuery", json!({ "ok": true, "result": true })),
            Reply::json("editMessageReplyMarkup", json!({ "ok": true, "result": true })),
        ]);
        let h = Arc::new(h);
        run(&h).await;
        let acts = h.phone.acts.lock().unwrap().clone();
        assert_eq!(acts.len(), 1, "from {from}: the tap acts once");
        assert_eq!((acts[0].reference.as_deref(), acts[0].person), (Some("ask1"), person), "from {from}");
    }
}

/// Audit H HS13: every Telegram button on a task said "That button is out of date.": a 32-character task id left no
/// room for the ask id in the 64 bytes, and the tap verified against "". The ref is read back from the task (its open
/// ask; its newest turn for Mark done), so the tap answers as an ntfy tap does; a button for an ask that's gone stays
/// refused.
#[tokio::test]
async fn a_tap_whose_ref_did_not_fit_is_read_back_from_the_task_and_answers_audit_h_hs13() {
    const CHAT: i64 = 1001;
    let session = "0123456789abcdef0123456789abcdef";
    let ask = "0198a3c1-7f00-7000-8000-0123456789ab";
    let tap = |data: String| {
        json!({ "update_id": 1, "callback_query": {
            "id": "q1", "data": data, "from": { "id": CHAT, "first_name": "x" },
            "message": { "message_id": 900, "chat": { "id": CHAT, "type": "private" } },
        } })
    };
    // signed over the ask id, which doesn't fit after the session: `a:1:<sig>:<session>`
    let data = wisp_telegram_core::encode_tap(
        wisp_channels_api::ActionKind::Answer,
        1,
        &wisp_channels_api::testing::test_sig(session, "a", ask, 1),
        session,
        ask,
    );
    assert!(!data.contains(ask), "{data}");
    for (task_ask, want) in [(Some(ask), Some(ask)), (None, None), (Some("another-ask"), None)] {
        let h = TestHub::default().with_cfg("telegram", json!({ "enabled": true, "token": "123:abc", "chatId": CHAT }));
        let mut s = wisp_channels_api::views::SessionView { id: session.into(), ..Default::default() };
        s.ask = task_ask.map(|a| wisp_channels_api::views::AskView { id: a.into(), ..Default::default() });
        h.phone.tasks.lock().unwrap().push(s);
        h.net.replies.lock().unwrap().extend([
            Reply::json("getUpdates", json!({ "ok": true, "result": [tap(data.clone())] })),
            Reply::json("getUpdates", json!({ "ok": false, "description": "done" })).status(500),
            Reply::json("answerCallbackQuery", json!({ "ok": true, "result": true })),
            Reply::json("editMessageReplyMarkup", json!({ "ok": true, "result": true })),
        ]);
        let h = Arc::new(h);
        run(&h).await;
        let acts = h.phone.acts.lock().unwrap().clone();
        let answered = h.net.sent("answerCallbackQuery");
        if let Some(r) = want {
            assert_eq!(acts.len(), 1, "the tap answers: {answered:?}");
            assert_eq!((acts[0].reference.as_deref(), acts[0].index), (Some(r), Some(1)));
        } else {
            assert!(acts.is_empty(), "no open ask, or another one: nothing to answer");
            assert_eq!(answered[0]["text"], "That button is out of date.");
        }
    }
}

/// Audit I (smaller notes): a refused tap (another member's in a linked group, on a question only the linked person
/// answers) took the message's keyboard away, so the linked person could only type. A refused tap leaves it; the linked
/// person's tap answers and then it goes.
#[tokio::test]
async fn a_refused_tap_leaves_the_keyboard_for_the_linked_person_audit_i() {
    const GROUP: i64 = -10_077;
    let tap = |from: i64| {
        json!({ "update_id": 1, "callback_query": {
            "id": "q1", "data": "a:0:signed:t1:ask1", "from": { "id": from, "first_name": "x" },
            "message": { "message_id": 900, "chat": { "id": GROUP, "type": "group" } },
        } })
    };
    for (from, cleared) in [(8, false), (7, true)] {
        let h = TestHub::default().with_cfg("telegram", json!({ "enabled": true, "token": "123:abc", "chatId": GROUP, "userId": "7" }));
        *h.phone.in_person.lock().unwrap() = true;
        h.net.replies.lock().unwrap().extend([
            Reply::json("getUpdates", json!({ "ok": true, "result": [tap(from)] })),
            Reply::json("getUpdates", json!({ "ok": false, "description": "done" })).status(500),
            Reply::json("answerCallbackQuery", json!({ "ok": true, "result": true })),
            Reply::json("editMessageReplyMarkup", json!({ "ok": true, "result": true })),
        ]);
        let h = Arc::new(h);
        run(&h).await;
        let said = h.net.sent("answerCallbackQuery");
        assert_eq!(said.len(), 1, "from {from}: {said:?}");
        assert_eq!(h.net.sent("editMessageReplyMarkup").is_empty(), !cleared, "from {from}: keyboard cleared = {cleared}");
        if !cleared {
            assert!(said[0]["text"].as_str().unwrap_or_default().contains("answer it in wisp"), "{said:?}");
        }
    }
}
