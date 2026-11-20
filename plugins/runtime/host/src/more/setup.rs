//! A setup card (ADR-0113): an agent asks to change the user's setup; the card is built from the exact writes that
//! would run (`wisp_runtime_core::setup`), stored with them and a digest of both, and only the user's Allow on that
//! card (named by its id) runs them, after the stored writes are checked to still give the card the user saw.

use std::future::Future;
use std::sync::Arc;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wisp_runtime_api::SessionId;
use wisp_runtime_api::commands::SetupAsked;
use wisp_runtime_api::model::{SessionState, ToolKind};
use wisp_runtime_core::setup::{self, Card, Write};

use super::ask::{NOT_OPEN, open_setup};
use super::{slot, str_at, tool_row, with_slot};
use crate::error::{Result, RuntimeError};
use crate::runtime::Runtime;

/// The permission a plugin that relays the user's own tap (a channel, the phone) declares, word for word, in its
/// manifest to answer a setup card; `runtime:*` doesn't count.
pub const RELAY: &str = "runtime:setup.answer";
/// The API keys' sets (`custom:<name>`, `apikey`) whose owner let them answer setup cards: none unless listed.
pub const KEYS_SETTING: &str = "runtime.setupAnswerSets";

/// Who answers a setup card (ADR-0113), an allow-list: the user's own page and harness (`user:*`), a grid box's own
/// page (`box`: its owner), a plugin that declared [`RELAY`] (`relay`: a channel, the phone), and an API key's set the
/// owner listed in [`KEYS_SETTING`]. Never an agent, a crew member's set, an app pane or a preview.
#[must_use]
pub fn may_answer(set: &str, relay: bool, keys: &[String]) -> bool {
    set.starts_with("user:")
        || set == "box"
        || (set.starts_with("plugin:") && relay)
        || ((set == "apikey" || set.starts_with("custom:")) && keys.iter().any(|k| k == set))
}

/// The words a write's answer says: an owner's `{ text }`, a project's name and folder; Err for its `{ error }`.
fn said(v: &Value) -> std::result::Result<String, String> {
    if let Some(e) = v.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()) {
        return Err(e.to_owned());
    }
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        return Ok(t.to_owned());
    }
    if let (Some(n), Some(p)) = (v.get("name").and_then(Value::as_str), v.get("path").and_then(Value::as_str)) {
        return Ok(format!("The project {n} is {p}."));
    }
    let joined: Vec<&str> =
        v.get("members").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("name").and_then(Value::as_str)).collect();
    Ok(if joined.is_empty() { String::new() } else { format!("Added {} to your crew.", joined.join(", ")) })
}

/// The digest a card is stored with: who asked, the writes, and every word the user was shown.
#[must_use]
pub fn digest(who: &str, writes: &[Write], c: &Card) -> String {
    let all = json!({ "who": who, "writes": writes, "card": c });
    Sha256::digest(all.to_string().as_bytes()).iter().fold(String::with_capacity(64), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// The stored card's writes, when they still give the card the user saw and its digest: the only writes Allow runs.
fn checked(card: &Value) -> std::result::Result<Vec<Write>, String> {
    let changed = || "This card changed after it was shown, so nothing ran. Ask again.".to_owned();
    let who = str_at(card, "who");
    let writes: Vec<Write> = serde_json::from_value(card.get("writes").cloned().unwrap_or(Value::Null)).map_err(|_| changed())?;
    let shown: Card = serde_json::from_value(card.get("card").cloned().unwrap_or(Value::Null)).map_err(|_| changed())?;
    let rebuilt = setup::card(&who, &writes)?;
    if rebuilt != shown || digest(&who, &writes, &shown) != str_at(card, "digest") {
        return Err(changed());
    }
    Ok(writes)
}

impl Runtime {
    /// `runtime.setup.ask`: the card for `writes` (already normalized, `from` already read) goes on task `id` (Allow,
    /// Deny) and the task waits on the user. `who`: the crew member's name, else the harness's.
    pub async fn setup_ask(self: &Arc<Self>, id: &SessionId, who: &str, writes: Vec<Write>) -> Result<SetupAsked> {
        let c = setup::card(who, &writes).map_err(RuntimeError::Invalid)?;
        let kind = writes.first().map(|w| w.command.clone()).unwrap_or_default();
        let stored = json!({
            "who": who,
            "writes": writes,
            "card": c,
            "digest": digest(who, &writes, &c),
            // what the task page, the phone and channels show
            "action": c.action,
            "headline": c.headline,
            "lines": c.lines,
            "commands": c.commands,
            "full": c.full,
        });
        let options = [setup::ALLOW, setup::DENY].map(str::to_owned);
        let ask_id = self.ask_in(id, &c.question(), "", &options, Some(("setup", stored))).await?;
        self.host.track("setup_asked", json!({ "kind": kind }));
        Ok(SetupAsked { text: setup::asked_words(&ask_id), ask_id })
    }

    /// Whether task `id`'s open question is a setup card.
    pub async fn asks_setup(&self, id: &SessionId) -> bool {
        self.repo.session(id.as_str()).await.is_ok_and(|s| open_setup(&s).is_some())
    }

    /// `runtime.answer` on a setup card. `ask`: the card the answer names, required and the open one. Allow runs the
    /// stored writes (`run`: by command id with their arguments, from wisp.runtime's side) once they're checked against
    /// the card; anything else changes nothing and the agent reads that the user said no. Under the task's lock, so a
    /// second Allow finds nothing asked and the writes run once; the queue behind the card goes on after.
    pub async fn answer_setup<F, Fut>(self: &Arc<Self>, id: &SessionId, text: &str, ask: Option<&str>, run: F) -> Result<Value>
    where
        F: Fn(String, Value) -> Fut,
        Fut: Future<Output = std::result::Result<Value, String>>,
    {
        let out = {
            let _g = self.task_lock(id.as_str()).await;
            let s = self.repo.session(id.as_str()).await?;
            let open = slot(&s).and_then(|m| m.get("ask")).cloned();
            let Some(card) = open.as_ref().and_then(|a| a.get("setup")).cloned() else {
                return Err(RuntimeError::Conflict(NOT_OPEN.into()));
            };
            let open_id = open.as_ref().map(|a| str_at(a, "id")).unwrap_or_default();
            match ask {
                None => {
                    return Err(RuntimeError::Invalid(
                        "A change to the user's setup is answered from its own card, which names it.".into(),
                    ));
                }
                Some(want) if want != open_id => return Err(RuntimeError::Conflict(NOT_OPEN.into())),
                Some(_) => {}
            }
            let action = str_at(&card, "action");
            let (allow, message) = setup::allowed(text);
            let (words, label) = if allow {
                let writes = checked(&card).map_err(RuntimeError::Conflict)?;
                let mut done = Ok(Vec::new());
                for w in writes {
                    done = match done {
                        Ok(mut said_so) => run(w.command, w.args).await.and_then(|v| said(&v)).map(|t| {
                            said_so.push(t);
                            said_so
                        }),
                        e => e,
                    };
                }
                match done {
                    Ok(t) => (setup::allowed_words(&t.join(" ")), format!("You allowed: {action}")),
                    Err(e) => (setup::failed_words(&e), format!("You allowed: {action} (it didn't go through: {e})")),
                }
            } else {
                let m = message.as_deref().map(|m| format!(" (“{m}”)")).unwrap_or_default();
                (setup::denied_words(message.as_deref()), format!("You denied: {action}{m}"))
            };
            self.finish_setup(id, &open_id, &words, label).await?
        };
        // the follow-ups that waited behind the card (ADR-0113)
        let s = self.repo.session(id.as_str()).await?;
        if !s.queue.is_empty() && !s.queue_held {
            Box::pin(self.kick(s)).await?;
        }
        Ok(out)
    }

    /// The card taken off the task, the agent's answer kept for `pollAnswer`, the row on its turn.
    async fn finish_setup(self: &Arc<Self>, id: &SessionId, ask_id: &str, words: &str, label: String) -> Result<Value> {
        let mut s = self.repo.session(id.as_str()).await?;
        with_slot(&mut s.ext, |m| m.remove("ask"));
        self.asks().remove(ask_id);
        self.answers().insert(ask_id.to_owned(), words.to_owned());
        s.state =
            if self.run_state(id.as_str()) == wisp_runtime_core::queue::Run::Idle { SessionState::Review } else { SessionState::Running };
        self.persist(&mut s, Vec::new()).await?;
        if let Some(t) = self.last_turn(id.as_str()).await? {
            self.add_row(id.as_str(), t.id.as_str(), tool_row(ToolKind::Answer, label)).await?;
        }
        Ok(json!({ "answered": true, "askId": ask_id }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Review r194 row 66 #5, #6: an allow-list. A grid box's own page answers its owner's cards; a plugin only when it
    /// declared the relay permission; an API key only when the owner listed its set; agents never.
    #[test]
    fn who_answers_a_setup_card_is_an_allow_list_setup_answerers() {
        let keys = vec!["custom:ci".to_owned()];
        for set in ["user:full", "box"] {
            assert!(may_answer(set, false, &[]), "{set}");
        }
        assert!(may_answer("plugin:wisp.channels", true, &[]));
        assert!(!may_answer("plugin:wisp.channels", false, &[]), "a plugin that didn't declare it");
        assert!(may_answer("custom:ci", false, &keys));
        for set in [
            "apikey",
            "custom:other",
            "agent:session",
            "agent:session:auto",
            "agent:capped",
            "crew:ops",
            "pane:abc",
            "preview",
            "public",
            "connector",
            "kernel",
        ] {
            assert!(!may_answer(set, false, &keys), "{set}");
        }
    }

    /// Review r194 row 66 #1: Allow runs only writes that still give the card the user saw.
    #[test]
    fn a_stored_card_whose_writes_changed_runs_nothing_setup_card_digest() {
        let w = vec![Write {
            command: setup::CONNECTOR_ADD.into(),
            args: json!({ "name": "x", "url": "https://ok.example/mcp", "transport": "remote", "sid": "s" }),
        }];
        let c = setup::card("Ada", &w).unwrap();
        let stored = json!({ "who": "Ada", "writes": w, "card": c, "digest": digest("Ada", &w, &c) });
        assert_eq!(checked(&stored).unwrap(), w);
        // the stored write swapped for a local command, the card left as it was shown
        let mut evil = stored.clone();
        evil["writes"][0]["args"] = json!({ "name": "x", "command": "sh", "args": ["-c", "id"], "transport": "local", "sid": "s" });
        assert!(checked(&evil).is_err());
        // the card rewritten to match, but the digest is the one stored with what was shown
        let mut both = evil.clone();
        both["card"] =
            serde_json::to_value(setup::card("Ada", &serde_json::from_value::<Vec<Write>>(evil["writes"].clone()).unwrap()).unwrap())
                .unwrap();
        assert!(checked(&both).is_err());
    }
}
