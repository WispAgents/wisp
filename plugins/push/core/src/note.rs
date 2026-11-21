//! The note a phone opens for one message (`channels`' finished, asks, waiting for approval): the message's title
//! and text, the task it opens, and its buttons, each a tap signed by wisp's channel key so only this wisp's own
//! notifications can answer. An open question's options are the choices (at most 3, as ntfy's buttons) and *Reply*
//! answers it in words; otherwise the message's own actions (Done) are the choices and *Reply* is a follow-up.

use wisp_channels_api::views::{ActionKind, AskView, Msg, SessionView, Signed};
use wisp_push_api::{Choice, MAX_CHOICES, Note};

use crate::truncate;

/// `sign(task, kind, ref, index)`: the hub's signature over a tap.
pub type Sign<'a> = &'a dyn Fn(&str, ActionKind, &str, u32) -> String;

/// A reply's index: to the open question (its answer), or a follow-up to the task.
pub const REPLY_TO_ASK: u32 = 1;
pub const REPLY_FOLLOW_UP: u32 = 0;

const TITLE_MAX: usize = 120;
const BODY_MAX: usize = 400;

fn tap(sign: Sign<'_>, s: &str, kind: ActionKind, r: &str, i: u32) -> Signed {
    Signed { s: s.to_owned(), k: kind.code().to_owned(), r: r.to_owned(), i, sig: sign(s, kind, r, i) }
}

/// The most a note's JSON may be, in bytes: sealed and in base64url it stays under the relay's 3000 characters
/// (FCM's and APNs' 4 KB), whatever script the text is in.
pub const PLAIN_MAX: usize = 2000;

fn json_len(n: &Note) -> usize {
    serde_json::to_vec(n).map_or(usize::MAX, |v| v.len())
}

/// `n` cut until its JSON fits [`PLAIN_MAX`]: the body first, then the title, then the choices' labels. The taps stay.
pub fn fit(mut n: Note) -> Note {
    while json_len(&n) > PLAIN_MAX {
        let half = |s: &str| truncate(s, (s.chars().count() / 2).max(1));
        if n.body.chars().count() > 1 {
            n.body = half(&n.body);
        } else if n.title.chars().count() > 12 {
            n.title = half(&n.title);
        } else if let Some(c) = n.choices.iter_mut().find(|c| c.label.chars().count() > 6) {
            c.label = half(&c.label);
        } else {
            n.choices.pop();
            if n.choices.is_empty() && json_len(&n) > PLAIN_MAX {
                n.reply = None;
                break;
            }
        }
    }
    n
}

/// The note for `msg` about `s` (with its open ask, if any), cut to fit ([`fit`]).
pub fn note(msg: &Msg, s: Option<&SessionView>, ask: Option<&AskView>, sign: Sign<'_>) -> Note {
    fit(built(msg, s, ask, sign))
}

fn built(msg: &Msg, s: Option<&SessionView>, ask: Option<&AskView>, sign: Sign<'_>) -> Note {
    let mut n = Note {
        title: truncate(&msg.title, TITLE_MAX),
        body: truncate(&msg.text, BODY_MAX),
        target: s.map_or(serde_json::Value::Null, |s| serde_json::json!({ "task": s.id })),
        choices: Vec::new(),
        reply: None,
    };
    let Some(s) = s else { return n };
    if let Some(a) = ask.filter(|a| !a.id.is_empty()) {
        if n.body.is_empty() {
            n.body = truncate(a.question.as_deref().unwrap_or_default(), BODY_MAX);
        }
        for (i, o) in a.options.iter().take(MAX_CHOICES).enumerate() {
            let i = u32::try_from(i).unwrap_or(u32::MAX);
            n.choices.push(Choice { label: truncate(o, 40), tap: tap(sign, &s.id, ActionKind::Answer, &a.id, i) });
        }
        n.reply = Some(tap(sign, &s.id, ActionKind::Reply, &a.id, REPLY_TO_ASK));
        return n;
    }
    for a in msg.actions.iter().take(MAX_CHOICES) {
        n.choices.push(Choice { label: truncate(&a.label, 40), tap: tap(sign, &s.id, a.kind, &a.reference, a.index) });
    }
    let turn = msg.actions.first().map(|a| a.reference.clone()).unwrap_or_default();
    n.reply = Some(tap(sign, &s.id, ActionKind::Reply, &turn, REPLY_FOLLOW_UP));
    n
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use wisp_channels_api::views::Action;

    use super::*;

    fn sign(s: &str, k: ActionKind, r: &str, i: u32) -> String {
        format!("{s}|{}|{r}|{i}", k.code())
    }

    fn task() -> SessionView {
        SessionView { id: "s1".into(), title: "Fix".into(), ..SessionView::default() }
    }

    #[test]
    fn a_question_offers_its_first_three_options_and_a_reply_to_it() {
        let ask = AskView {
            id: "a1".into(),
            question: Some("Which?".into()),
            options: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            ..AskView::default()
        };
        let n = note(&Msg { title: "? Fix".into(), ..Msg::default() }, Some(&task()), Some(&ask), &sign);
        assert_eq!(n.body, "Which?");
        assert_eq!(n.choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
        assert_eq!(n.choices[2].tap.sig, "s1|a|a1|2");
        assert_eq!(n.reply.unwrap().sig, "s1|r|a1|1");
        assert_eq!(n.target, serde_json::json!({ "task": "s1" }));
    }

    #[test]
    fn a_long_note_in_any_script_is_cut_to_fit_the_relay() {
        let long = "答".repeat(400); // 1200 bytes of body, plus a 4-byte-per-char title and labels
        let ask = AskView { id: "a1".into(), options: vec!["🙂".repeat(40); 3], ..AskView::default() };
        let msg = Msg { title: "🙂".repeat(120), text: long, ..Msg::default() };
        let n = note(&msg, Some(&task()), Some(&ask), &sign);
        assert!(serde_json::to_vec(&n).unwrap().len() <= PLAIN_MAX);
        assert_eq!(n.choices.len(), 3, "the choices stay, shorter");
        assert!(n.body.ends_with('…'));
    }

    #[test]
    fn a_finished_task_offers_its_actions_and_a_follow_up() {
        let msg = Msg {
            title: "✓ Fix".into(),
            text: "It's done.".into(),
            actions: vec![Action { label: "Done".into(), kind: ActionKind::Done, reference: "t9".into(), index: 0 }],
        };
        let n = note(&msg, Some(&task()), None, &sign);
        assert_eq!((n.choices.len(), n.choices[0].tap.k.as_str()), (1, "d"));
        assert_eq!(n.reply.unwrap().sig, "s1|r|t9|0");
        let plain = note(&msg, None, None, &sign);
        assert!(plain.choices.is_empty() && plain.reply.is_none() && plain.target.is_null(), "a notice about no task: nothing to answer");
    }
}
