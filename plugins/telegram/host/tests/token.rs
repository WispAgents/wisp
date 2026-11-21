//! Setup audit B12: with a bad token Telegram's row said only "Telegram: Unauthorized" (Slack and Discord say where to
//! fix theirs), and the long poll asked again every 5 s. It says where to check the token, and stops asking.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::json;
use wisp_channels_api::ChannelHub;
use wisp_channels_api::testing::{Reply, TestHub};

#[tokio::test]
async fn a_refused_token_says_where_to_check_it_and_the_poll_stops() {
    for (status, description) in [(401, "Unauthorized"), (404, "Not Found")] {
        let h = TestHub::default().with_cfg("telegram", json!({ "enabled": true, "token": "123:bad", "chatId": 42 }));
        h.net.replies.lock().unwrap().extend([Reply::json(
            "getUpdates",
            json!({ "ok": false, "error_code": status, "description": description }),
        )
        .status(status)]);
        let h = Arc::new(h);
        let dyn_hub: Arc<dyn ChannelHub> = h.clone();
        wisp_telegram::transport(dyn_hub).start(h.stop.clone()).await;
        let err = h.last_mark("telegram").unwrap().error.unwrap();
        assert_eq!(err, format!("Telegram: {description}. Check the token in wisp → Settings → Phone → Telegram."));
        assert_eq!(h.net.sent("getUpdates").len(), 1, "{status}: asked once, not every 5 s");
    }
}
