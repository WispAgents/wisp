//! A link code's life in a transport's config (Slack's, Discord's, Telegram's `link <code>`): it links one person,
//! then it's used up (`linkCode` empty: Settings' Relink issues another), and wrong guesses change it. The wrong ones
//! are counted for everyone in `linkGuess` (`{"wrong": n}`), not per sender, so many strangers share the same five
//! tries; the count lives in the config, as a cloud box forgets the rest each time it sleeps. Pure on the hub.

use serde_json::{Value, json};

use crate::hub::ChannelHub;

/// Wrong codes, from everyone, before the code changes.
pub const LINK_TRIES: u64 = 5;

/// A fresh code as Settings issues one (`newLinkCode`: six digits, never starting with 0).
pub fn fresh_code(hub: &dyn ChannelHub) -> String {
    hub.random_int(100_000, 1_000_000).to_string()
}

/// A config after its code linked someone: the code used up, the count gone.
pub fn use_up(cfg: &mut Value) {
    if let Some(o) = cfg.as_object_mut() {
        o.insert("linkCode".into(), json!(""));
        o.insert("linkGuess".into(), Value::Null);
    }
}

/// What one wrong code writes over `cfg` (a partial config, for `set_cfg`): the count one higher, or at
/// [`LINK_TRIES`] a `fresh` code and the count reset; and whether the code changed.
pub fn missed(cfg: &Value, fresh: impl FnOnce() -> String) -> (Value, bool) {
    let wrong = cfg.pointer("/linkGuess/wrong").and_then(Value::as_u64).unwrap_or(0) + 1;
    if wrong >= LINK_TRIES {
        (json!({ "linkCode": fresh(), "linkGuess": Value::Null }), true)
    } else {
        (json!({ "linkGuess": { "wrong": wrong } }), false)
    }
}

/// One wrong code on `channel`, counted; Settings is told when the code changed (it shows the new one). True then.
pub fn miss(hub: &dyn ChannelHub, channel: &str) -> bool {
    let (patch, changed) = missed(&hub.cfg(channel), || fresh_code(hub));
    hub.set_cfg(channel, patch);
    if changed {
        hub.changed();
    }
    changed
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn wrong_codes_from_everyone_change_the_code_at_five_and_a_link_uses_it_up() {
        let mut cfg = json!({ "linkCode": "424242" });
        for n in 1..LINK_TRIES {
            let (patch, changed) = missed(&cfg, || unreachable!("no new code yet"));
            assert!(!changed);
            assert_eq!(patch["linkGuess"]["wrong"], n);
            cfg.as_object_mut().unwrap().extend(patch.as_object().unwrap().clone());
        }
        let (patch, changed) = missed(&cfg, || "777777".into());
        assert!(changed);
        assert_eq!(patch, json!({ "linkCode": "777777", "linkGuess": null }));
        let mut c = json!({ "linkCode": "424242", "linkGuess": { "wrong": 2 }, "senders": ["U1"] });
        use_up(&mut c);
        assert_eq!(c, json!({ "linkCode": "", "linkGuess": null, "senders": ["U1"] }));
    }
}
