//! wisp-push-api: the contract of `wisp.push` (ADR-0065 §3), the `push` transport for `wisp.channels`. A phone
//! pairs itself (`push.pair`: the relay's opaque handle and the phone's X25519 public key); wisp seals each
//! notification to that key (HPKE, RFC 9180) and posts only `{handle, sealed, collapse}` to the grid relay, which
//! forwards it to FCM or APNs. The phone opens it; a choice or a reply comes back as `push.answer`, signed by
//! wisp's channel key and run once, as the app's own button would.
//!
//! The ids are the permissions' verbs: `push.pair` → `push:pair`. `push.pair`, `push.unpair` and `push.answer`
//! are the phone's own (ui-only: no harness has a device to pair); `push.devices` is a read an agent may make.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wisp_channels_api::views::Signed;
use wisp_kernel::Command;

pub const PLUGIN: &str = "wisp.push";
/// The transport's name in `channels.transport@1`, its `channel` resource's name and its notification's target.
pub const TRANSPORT: &str = "push";
/// Where the paired devices are kept: `~/.wisp/secrets.json`, never the store (a handle reaches the phone).
pub const SECRET_KEY: &str = "push/devices";
/// The relay a phone registered with when `push.pair` names none.
pub const DEFAULT_RELAY: &str = "https://grid.trywisp.dev";
/// What the relay shows when the phone can't open the note in time (APNs' visible alert; FCM shows nothing).
pub const FALLBACK: &str = "A task needs you";
/// At most this many choices in a note (as ntfy's buttons).
pub const MAX_CHOICES: usize = 3;
/// At most this many phones on one wisp.
pub const MAX_DEVICES: usize = 10;
/// The HPKE `info` every note is sealed with: a note sealed for another purpose never opens as one.
pub const HPKE_INFO: &[u8] = b"wisp push v1";

pub const PERM_PAIR: &str = "push:pair";
pub const PERM_UNPAIR: &str = "push:unpair";
pub const PERM_READ: &str = "push:read";
pub const PERM_ANSWER: &str = "push:answer";

/// Which push service reaches the phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// FCM: a data message the app's messaging service opens
    Android,
    /// APNs: a `mutable-content` alert the Notification Service Extension opens
    Ios,
}

/// One paired phone, as kept in secrets. The handle and the key never leave this wisp but to the relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// wisp's id for it (`push.unpair`'s)
    pub id: String,
    /// the relay's opaque id for the phone's FCM/APNs token
    pub handle: String,
    /// the phone's X25519 public key, base64url (32 bytes)
    pub public_key: String,
    pub platform: Platform,
    /// what the person calls it ("Pixel 9")
    pub device: String,
    /// the relay that holds the handle
    pub relay: String,
    pub paired_at: i64,
}

/// What `secrets.json` holds under [`SECRET_KEY`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    #[serde(default)]
    pub devices: Vec<Device>,
    /// why the last phone was unpaired by wisp itself (its token gone): what `push.devices` says
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped: Option<String>,
}

/// A paired phone as anyone may see it: no handle, no key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub platform: Platform,
    pub device: String,
    pub paired_at: i64,
}

/// `push.pair`: this phone gets the notifications. Pairing the same handle again replaces its entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PairArgs {
    /// the handle the relay gave the phone when it registered
    pub handle: String,
    /// the phone's X25519 public key, base64url
    pub public_key: String,
    pub platform: Platform,
    /// the phone's name
    pub device: String,
    /// the relay that gave the handle (default `https://grid.trywisp.dev`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relay: Option<String>,
}

/// `push.unpair`: this phone gets nothing more. The last one switches the `push` channel off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnpairArgs {
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NoArgs {}

/// `push.devices`: the phones that get alerts, and why wisp unpaired one itself if it did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Devices {
    pub devices: Vec<DeviceView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dropped: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Unpaired {
    pub removed: bool,
}

/// `push.answer`: a choice or a reply from a notification, as the phone sends it: the signed tap the note carried
/// (`{s, k, r, i, sig}`) and, for a reply, its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AnswerArgs {
    /// the task
    pub s: String,
    /// `a` (a choice), `r` (a reply), `d` (Done)
    pub k: String,
    /// the ask id (a choice, a reply to an ask) or the turn id (Done, a reply to a finished task)
    pub r: String,
    pub i: u32,
    pub sig: String,
    /// a reply's text (`RemoteInput` / `UNTextInputNotificationAction`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
}

impl AnswerArgs {
    pub fn signed(&self) -> Signed {
        Signed { s: self.s.clone(), k: self.k.clone(), r: self.r.clone(), i: self.i, sig: self.sig.clone() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Answered {
    pub line: String,
}

// ---------- what the phone opens ----------

/// One button on the notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    /// what `push.answer` sends back for it
    pub tap: Signed,
}

/// What wisp seals to a phone: the plaintext only the phone reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub title: String,
    pub body: String,
    /// what a tap opens: `{ "task": "<id>" }` (the app's `onOpen` target); null: the app
    pub target: serde_json::Value,
    /// at most [`MAX_CHOICES`]
    #[serde(default)]
    pub choices: Vec<Choice>,
    /// *Reply*'s tap (its text goes in `push.answer`'s `text`); none: no reply box
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<Signed>,
}

// ---------- the relay's wire (apps/grid `/api/push/*`) ----------

/// `POST <relay>/api/push/send`: all the relay ever sees. Any other key is refused there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendBody {
    pub handle: String,
    /// base64url of HPKE's encapsulated key (32 bytes) followed by the ciphertext
    pub sealed: String,
    /// the task's id: a newer note for the same task replaces the older one
    pub collapse: String,
}

// ---------- commands ----------

macro_rules! command {
    ($name:ident = $id:literal, $perm:expr, $ro:expr, $args:ty => $out:ty) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
            const READ_ONLY: bool = $ro;
            type Args = $args;
            fn args_ts() -> Option<String> {
                Some(format!(
                    "{}{}",
                    <$args as ::ts_rs::TS>::docs().unwrap_or_default(),
                    <$args as ::ts_rs::TS>::inline(&::ts_rs::Config::default())
                ))
            }
            type Out = $out;
        }
    };
}

command!(Pair = "push.pair", PERM_PAIR, false, PairArgs => DeviceView);
command!(Unpair = "push.unpair", PERM_UNPAIR, false, UnpairArgs => Unpaired);
command!(List = "push.devices", PERM_READ, true, NoArgs => Devices);
command!(Answer = "push.answer", PERM_ANSWER, false, AnswerArgs => Answered);
