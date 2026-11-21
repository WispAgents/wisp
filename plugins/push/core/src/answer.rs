//! What a verified tap from a notification does (the signature is the hub's to check): one runtime command, as the
//! app's own button would run it, or a refusal that says why. It lands once: a choice or a reply to a question needs
//! that question still open (the first answer closes it); a Done or a follow-up is kept off a second time by the
//! host's list of taps it already ran.

use wisp_channels_api::views::{ActionKind, AskView};

use crate::note::REPLY_TO_ASK;
use crate::{PushError, Result};

/// The most a reply may say.
pub const TEXT_MAX: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// `runtime.answer` with this text
    Answer(String),
    /// `runtime.followUp`
    FollowUp(String),
    /// `runtime.setState done`
    Done,
    /// `runtime.stop` (a silent run's Stop, TRY-23)
    Stop,
}

fn text_of(text: Option<&str>) -> Result<String> {
    let t = text.unwrap_or_default().trim();
    if t.is_empty() {
        return Err(PushError::Invalid("text: a reply says something".into()));
    }
    if t.chars().count() > TEXT_MAX {
        return Err(PushError::Invalid(format!("text: at most {TEXT_MAX} characters")));
    }
    Ok(t.to_owned())
}

/// The open ask, if it's the one the tap was for.
fn open_ask<'a>(ask: Option<&'a AskView>, r: &str) -> Result<&'a AskView> {
    ask.filter(|a| a.id == r).ok_or_else(|| PushError::Conflict("That question was already answered.".into()))
}

/// What tap `(k, r, i)` with `text` does to a task whose open ask is `ask`.
#[allow(clippy::many_single_char_names)] // the tap's own field names, `{s, k, r, i, sig}`
pub fn decide(k: &str, r: &str, i: u32, text: Option<&str>, ask: Option<&AskView>) -> Result<Act> {
    match ActionKind::from_code(k) {
        Some(ActionKind::Answer) => {
            let a = open_ask(ask, r)?;
            let o = usize::try_from(i)
                .ok()
                .and_then(|i| a.options.get(i))
                .ok_or_else(|| PushError::Conflict("That choice isn't there any more.".into()))?;
            Ok(Act::Answer(o.clone()))
        }
        Some(ActionKind::Reply) if i == REPLY_TO_ASK => {
            let text = text_of(text)?;
            open_ask(ask, r)?;
            Ok(Act::Answer(text))
        }
        Some(ActionKind::Reply) => Ok(Act::FollowUp(text_of(text)?)),
        Some(ActionKind::Done) => Ok(Act::Done),
        Some(ActionKind::Stop) => Ok(Act::Stop),
        None => Err(PushError::Invalid(format!("k: not a tap kind: {k}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ask() -> AskView {
        AskView { id: "a1".into(), options: vec!["Yes".into(), "No".into()], ..AskView::default() }
    }

    #[test]
    fn a_choice_answers_the_open_question_with_its_label_and_only_that_question() {
        assert_eq!(decide("a", "a1", 1, None, Some(&ask())).unwrap(), Act::Answer("No".into()));
        assert!(matches!(decide("a", "a1", 0, None, None), Err(PushError::Conflict(_))), "answered already: closed");
        assert!(matches!(decide("a", "a0", 0, None, Some(&ask())), Err(PushError::Conflict(_))), "an older question");
        assert!(matches!(decide("a", "a1", 5, None, Some(&ask())), Err(PushError::Conflict(_))));
    }

    #[test]
    fn a_reply_answers_in_words_or_follows_up() {
        assert_eq!(decide("r", "a1", REPLY_TO_ASK, Some(" main "), Some(&ask())).unwrap(), Act::Answer("main".into()));
        assert!(matches!(decide("r", "a1", REPLY_TO_ASK, Some("x"), None), Err(PushError::Conflict(_))));
        assert_eq!(decide("r", "t1", 0, Some("and the tests"), None).unwrap(), Act::FollowUp("and the tests".into()));
        assert!(matches!(decide("r", "t1", 0, Some("  "), None), Err(PushError::Invalid(_))));
        assert_eq!(decide("d", "t1", 0, None, None).unwrap(), Act::Done);
        assert!(decide("z", "t1", 0, None, None).is_err());
    }
}
