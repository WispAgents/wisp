//! A button's `callback_data`: what a tap does, signed by the hub (HMAC over session, kind, ref and
//! index), in Telegram's 64 bytes. v1 sent `k:session:ref:i` unsigned and trusted the chat id alone;
//! here the signature always goes, and the ids go as far as they fit. What doesn't fit is read back
//! when the tap comes: the session from the message the button was on, the ref from that task's
//! open ask or last turn. Either way the signature is checked over all four, so a stale or forged
//! button does nothing.
use wisp_channels_api::ActionKind;

/// Telegram's limit on `callback_data`, in bytes.
pub const DATA_MAX: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tap {
    pub kind: ActionKind,
    pub index: u32,
    pub sig: String,
    /// absent: the session of the message it was on
    pub session: Option<String>,
    /// absent: the session's open ask (an answer) or last turn (a reply, done)
    pub reference: Option<String>,
}

/// `k:i:sig`, then `:session` and `:ref` while they fit.
pub fn encode_tap(kind: ActionKind, index: u32, sig: &str, session: &str, reference: &str) -> String {
    let mut out = format!("{}:{index}:{sig}", kind.code());
    // A ':' in a session id would shift every field after it; such an id is read back instead.
    if session.contains(':') || out.len() + 1 + session.len() > DATA_MAX {
        return out;
    }
    out.push(':');
    out.push_str(session);
    if out.len() + 1 + reference.len() <= DATA_MAX {
        out.push(':');
        out.push_str(reference);
    }
    out
}

/// The ref a tapped button names when it didn't fit in it, read back from its task (audit H HS13: it wasn't, every tap
/// verified against "" and said "That button is out of date."): an answer's is the open ask, a reply's, Mark done's
/// and Stop's the newest turn. None: there's nothing to read it from, and the signature can't match.
pub fn read_back(kind: ActionKind, s: &wisp_channels_api::views::SessionView) -> Option<String> {
    match kind {
        ActionKind::Answer => s.ask.as_ref().map(|a| a.id.clone()),
        ActionKind::Reply | ActionKind::Done | ActionKind::Stop => s.turns.last().map(|t| t.id.clone()),
    }
    .filter(|r| !r.is_empty())
}

/// The tap a button's data says, or `None` (an unknown kind, no signature, a v1 button).
pub fn parse_tap(data: &str) -> Option<Tap> {
    if data.len() > DATA_MAX {
        return None;
    }
    let mut parts = data.splitn(5, ':');
    let kind = ActionKind::from_code(parts.next()?)?;
    let index_s = parts.next()?;
    // v1's unsigned `k:session:ref:i` has a session id here, never a number.
    if index_s.is_empty() || !index_s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let index = index_s.parse().ok()?;
    let sig = parts.next().filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))?;
    let session = parts.next().filter(|s| !s.is_empty()).map(str::to_owned);
    let reference = parts.next().map(str::to_owned).filter(|_| session.is_some());
    Some(Tap { kind, index, sig: sig.to_owned(), session, reference })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::assert_is_empty)]
mod tests {
    use super::*;

    const SIG: &str = "AbCdEfGhIjKlMnOpQrStUv"; // 22, as the hub's

    /// Audit H HS13: a 32-character task id leaves no room for the ask id (`a:1:<sig>:<session>`, 59 bytes): the ref
    /// is read back from the task, the open ask for an answer and the newest turn for the rest.
    #[test]
    fn a_ref_that_did_not_fit_is_read_back_from_the_task_audit_h_hs13() {
        let session = "0123456789abcdef0123456789abcdef";
        let ask = "0198a3c1-7f00-7000-8000-0123456789ab";
        let d = encode_tap(ActionKind::Answer, 1, SIG, session, ask);
        let t = parse_tap(&d).unwrap();
        assert_eq!((t.session.as_deref(), t.reference.as_deref()), (Some(session), None), "{d}");
        let s: wisp_channels_api::views::SessionView = serde_json::from_value(serde_json::json!({
            "id": session, "title": "t", "state": "waiting", "agent": "claude",
            "ask": { "id": ask, "question": "red or blue?", "options": ["red", "blue"] },
            "turns": [{ "id": "turn-1", "status": "running" }]
        }))
        .unwrap();
        assert_eq!(read_back(ActionKind::Answer, &s).as_deref(), Some(ask));
        for k in [ActionKind::Reply, ActionKind::Done, ActionKind::Stop] {
            assert_eq!(read_back(k, &s).as_deref(), Some("turn-1"), "{k:?}");
        }
        let bare = wisp_channels_api::views::SessionView::default();
        assert_eq!(read_back(ActionKind::Answer, &bare), None);
    }

    #[test]
    fn short_ids_go_whole() {
        let d = encode_tap(ActionKind::Answer, 1, SIG, "s1", "q1");
        assert_eq!(d, format!("a:1:{SIG}:s1:q1"));
        let t = parse_tap(&d).unwrap();
        assert_eq!((t.kind, t.index, t.session.as_deref(), t.reference.as_deref()), (ActionKind::Answer, 1, Some("s1"), Some("q1")));
    }

    #[test]
    fn long_ids_are_left_to_the_message() {
        let sid = "0193a2b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
        let d = encode_tap(ActionKind::Reply, 0, SIG, sid, "0193a2b4-5c6d-7e8f-9a0b-1c2d3e4f5a6c");
        assert!(d.len() <= DATA_MAX, "{}", d.len());
        let t = parse_tap(&d).unwrap();
        assert_eq!(t.session.as_deref(), Some(sid));
        assert_eq!(t.reference, None);
        let huge = "x".repeat(60);
        let d = encode_tap(ActionKind::Done, 2, SIG, &huge, "r");
        assert_eq!(d, format!("d:2:{SIG}"));
        assert_eq!(parse_tap(&d).unwrap().session, None);
    }

    #[test]
    fn a_ref_may_hold_colons() {
        let t = parse_tap(&format!("a:0:{SIG}:s:q:1")).unwrap();
        assert_eq!(t.reference.as_deref(), Some("q:1"));
    }

    #[test]
    fn unsigned_and_unknown_buttons_do_nothing() {
        assert_eq!(parse_tap("a:s1:q1:0"), None, "a v1 button carries no signature");
        assert_eq!(parse_tap("x:0:sig"), None);
        assert_eq!(parse_tap("a:0:"), None);
        assert_eq!(parse_tap("a:0"), None);
        assert_eq!(parse_tap(""), None);
        assert_eq!(parse_tap("a:0:bad sig"), None);
        assert_eq!(parse_tap(&format!("a:0:{SIG}:{}", "s".repeat(70))), None);
    }

    /// Property: whatever the ids, the data fits, and reads back as what was encoded or less.
    #[test]
    fn property_round_trip_within_64_bytes() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let alphabet = b"abcdef0123456789-@_:.";
        for _ in 0..5000 {
            let mut word = |n: u64| {
                (0..next() % n).map(|_| alphabet[usize::try_from(next() % alphabet.len() as u64).unwrap()] as char).collect::<String>()
            };
            let (sid, rf) = (word(50), word(50));
            let kind = [ActionKind::Answer, ActionKind::Reply, ActionKind::Done][usize::try_from(next() % 3).unwrap()];
            let index = u32::try_from(next() % 12).unwrap();
            let d = encode_tap(kind, index, SIG, &sid, &rf);
            assert!(d.len() <= DATA_MAX);
            let t = parse_tap(&d).unwrap();
            assert_eq!((t.kind, t.index, t.sig.as_str()), (kind, index, SIG));
            if let Some(s) = &t.session {
                assert_eq!(s, &sid);
            }
            if let Some(r) = &t.reference {
                assert_eq!(r, &rf);
            }
        }
    }
}
