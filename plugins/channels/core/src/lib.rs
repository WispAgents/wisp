//! wisp-channels-core: the pure logic of `wisp.channels`, shared by the hub and every transport. Who
//! may speak and what may leave (policy, scrub, secrets), what a message asks for (grammar, mentions,
//! lists), the conversation flow, link codes, signed taps, box rules and routed retries. Sans-IO: no
//! tokio, fs, net or clock; time comes in as an argument.

pub mod conversation;
pub mod default_project;
pub mod grammar;
pub mod link;
pub mod lists;
pub mod mentions;
pub mod report;
pub mod setup;
pub use wisp_channels_api::{policy, scrub, secrets};
pub mod sign;
pub mod ws;

/// A setup card (ADR-0113) tapped where the channel can't tell it was the linked person (ntfy: anyone with the topic;
/// a group chat): answered in wisp instead.
pub const SETUP_ELSEWHERE: &str =
    "That's a change to your setup: answer it in wisp (or from the chat you linked, yourself). Nothing changed.";

/// Whether a tap on a channel's button can be the linked person's (ADR-0113): Slack's, Discord's and Telegram's carry
/// who tapped, checked against who linked the chat. Any other's can't be told (ntfy: anyone who knows the topic taps;
/// a channel a plugin adds says nothing of it), so a setup card goes there without buttons: default deny.
pub fn taps_are_personal(transport: &str) -> bool {
    matches!(transport, "telegram" | "slack" | "discord")
}

/// What a setup card's message says on a channel whose taps can't be the person's (audit H HS8: ntfy got Allow and Deny
/// buttons that did nothing and cleared the notification): no buttons, and where to answer instead.
pub const SETUP_IN_WISP: &str = "This changes your setup: answer it in wisp (Allow or Deny on the task's card).";

/// `msg` as `transport` gets it: a setup card (`setup`) where taps can't be the person's goes without its buttons, saying
/// where to answer; anything else as it is.
pub fn for_channel<'a>(
    msg: &'a wisp_channels_api::views::Msg,
    transport: &str,
    setup: bool,
) -> std::borrow::Cow<'a, wisp_channels_api::views::Msg> {
    if !setup || taps_are_personal(transport) || msg.actions.is_empty() {
        return std::borrow::Cow::Borrowed(msg);
    }
    let text = if msg.text.is_empty() { SETUP_IN_WISP.to_owned() } else { format!("{}\n\n{SETUP_IN_WISP}", msg.text) };
    std::borrow::Cow::Owned(wisp_channels_api::views::Msg { title: msg.title.clone(), text, actions: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_channels_api::views::{Action, ActionKind, Msg};

    /// Audit H HS8: a crew card's (or a keyed connector card's) Allow on ntfy did nothing. There it goes without buttons,
    /// saying to answer in wisp; on Telegram, Slack and Discord the linked person's tap answers it, so they stay.
    #[test]
    fn a_setup_card_has_no_buttons_where_a_tap_cant_be_the_person_audit_h_hs8() {
        let allow = |label: &str, index| Action { label: label.into(), kind: ActionKind::Answer, reference: "ask1".into(), index };
        let msg = Msg {
            title: "? Add Ada".into(),
            text: "Claude Code wants to add Ada to the crew".into(),
            actions: vec![allow("Allow", 0), allow("Deny", 1)],
        };
        let ntfy = for_channel(&msg, "ntfy", true);
        assert!(ntfy.actions.is_empty(), "no button that does nothing");
        assert_eq!(ntfy.text, format!("Claude Code wants to add Ada to the crew\n\n{SETUP_IN_WISP}"));
        for t in ["telegram", "slack", "discord"] {
            assert_eq!(for_channel(&msg, t, true).actions.len(), 2, "{t}: the linked person's tap answers it");
        }
        assert_eq!(for_channel(&msg, "ntfy", false).actions.len(), 2, "a plain question's buttons stay on ntfy");
        assert!(for_channel(&msg, "acme-chat", true).actions.is_empty(), "a channel a plugin adds: can't be told, none");
    }
}
