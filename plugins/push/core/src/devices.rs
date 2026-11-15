//! The paired phones (kept in secrets as [`Kept`]): one entry per handle, at most [`MAX_DEVICES`]. Pairing the
//! same handle again (the app reinstalled its key, renamed the phone) replaces that entry and keeps its id.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64U;
use wisp_push_api::{DEFAULT_RELAY, Device, DeviceView, Kept, MAX_DEVICES, PairArgs};

use crate::{PushError, Result};

/// A relay's handle: what `/api/push/register` makes (`wph_` and 43 base64url characters), never anything with a
/// character a URL or a log line would need to escape.
pub fn handle_ok(h: &str) -> bool {
    (16..=128).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn invalid(path: &str, why: &str) -> PushError {
    PushError::Invalid(format!("{path}: {why}"))
}

/// Every field checked before anything is kept. → the relay's address, normalised.
pub fn check_pair(a: &PairArgs) -> Result<String> {
    if !handle_ok(&a.handle) {
        return Err(invalid("handle", "not a relay handle (16 to 128 letters, digits, - or _)"));
    }
    match B64U.decode(a.public_key.trim_end_matches('=')) {
        Ok(k) if k.len() == 32 => {}
        _ => return Err(invalid("publicKey", "an X25519 public key is 32 bytes, base64url")),
    }
    let name = a.device.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err(invalid("device", "the phone's name, 1 to 60 characters"));
    }
    let relay = a.relay.as_deref().map_or(DEFAULT_RELAY, str::trim).trim_end_matches('/').to_owned();
    let host = relay.split_once("://").map_or("", |(_, rest)| rest.split(['/', ':']).next().unwrap_or_default());
    let local = host == "localhost" || host == "127.0.0.1" || host.ends_with(".localhost");
    let host_ok = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let scheme_ok = relay.starts_with("https://") || (local && relay.starts_with("http://"));
    if !host_ok || !scheme_ok || relay.contains(['?', '#', ' ', '@']) {
        return Err(invalid("relay", "the grid's https address"));
    }
    Ok(relay)
}

/// `a` paired into `kept`: a known handle is replaced in place (its id stays), a new one gets `id`. → its view.
pub fn pair(kept: &mut Kept, a: &PairArgs, id: &str, now_ms: i64) -> Result<DeviceView> {
    let relay = check_pair(a)?;
    let d = Device {
        id: id.to_owned(),
        handle: a.handle.clone(),
        public_key: a.public_key.trim_end_matches('=').to_owned(),
        platform: a.platform,
        device: a.device.trim().to_owned(),
        relay,
        paired_at: now_ms,
    };
    if let Some(had) = kept.devices.iter_mut().find(|x| x.handle == a.handle) {
        *had = Device { id: had.id.clone(), ..d };
        kept.dropped = None;
        return Ok(view(had));
    }
    if kept.devices.len() >= MAX_DEVICES {
        return Err(PushError::Conflict(format!("At most {MAX_DEVICES} phones get notifications: unpair one first.")));
    }
    let v = view(&d);
    kept.devices.push(d);
    kept.dropped = None;
    Ok(v)
}

/// The device `id` out of `kept`. → whether it was there.
pub fn unpair(kept: &mut Kept, id: &str) -> bool {
    let before = kept.devices.len();
    kept.devices.retain(|d| d.id != id);
    kept.devices.len() != before
}

/// The device whose handle the relay said is gone, out of `kept`, with the reason `push.devices` shows.
pub fn drop_gone(kept: &mut Kept, handle: &str) -> Option<Device> {
    let i = kept.devices.iter().position(|d| d.handle == handle)?;
    let d = kept.devices.remove(i);
    kept.dropped =
        Some(format!("{} stopped getting notifications (the app was removed or its notifications reset): pair it again.", d.device));
    Some(d)
}

pub fn view(d: &Device) -> DeviceView {
    DeviceView { id: d.id.clone(), platform: d.platform, device: d.device.clone(), paired_at: d.paired_at }
}

pub fn views(kept: &Kept) -> Vec<DeviceView> {
    kept.devices.iter().map(view).collect()
}

/// What secrets.json holds, read back; anything unreadable is no devices (never a crash on a hand edit).
pub fn read(text: Option<&str>) -> Kept {
    text.and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use wisp_push_api::Platform;

    use super::*;

    fn args(handle: &str) -> PairArgs {
        PairArgs {
            handle: handle.into(),
            public_key: B64U.encode([7u8; 32]),
            platform: Platform::Android,
            device: " Pixel 9 ".into(),
            relay: None,
        }
    }

    #[test]
    fn pairing_the_same_handle_again_replaces_it_and_keeps_its_id() {
        let mut k = Kept::default();
        let a = pair(&mut k, &args("wph_aaaaaaaaaaaaaaaa"), "d1", 1).unwrap();
        assert_eq!((a.id.as_str(), a.device.as_str()), ("d1", "Pixel 9"));
        let mut again = args("wph_aaaaaaaaaaaaaaaa");
        again.device = "Work phone".into();
        let b = pair(&mut k, &again, "d2", 2).unwrap();
        assert_eq!((b.id.as_str(), b.device.as_str(), k.devices.len()), ("d1", "Work phone", 1));
        assert_eq!(k.devices[0].relay, DEFAULT_RELAY);
    }

    #[test]
    fn a_bad_field_is_refused_by_its_name() {
        let mut a = args("short");
        assert!(check_pair(&a).unwrap_err().to_string().starts_with("handle:"));
        a = args("wph_aaaaaaaaaaaaaaaa");
        a.public_key = B64U.encode([1u8; 31]);
        assert!(check_pair(&a).unwrap_err().to_string().starts_with("publicKey:"));
        a = args("wph_aaaaaaaaaaaaaaaa");
        a.device = "  ".into();
        assert!(check_pair(&a).unwrap_err().to_string().starts_with("device:"));
        a = args("wph_aaaaaaaaaaaaaaaa");
        a.relay = Some("http://evil.example".into());
        assert!(check_pair(&a).unwrap_err().to_string().starts_with("relay:"));
        for evil in ["http://evil.example/x.localhost", "http://localhost.evil.com", "http://a@localhost"] {
            a.relay = Some(evil.into());
            assert!(check_pair(&a).is_err(), "{evil}");
        }
        a.relay = Some("http://wisp.localhost:8080/".into());
        assert_eq!(check_pair(&a).unwrap(), "http://wisp.localhost:8080");
    }

    #[test]
    fn at_most_ten_phones_and_a_gone_one_says_why() {
        let mut k = Kept::default();
        for i in 0..MAX_DEVICES {
            pair(&mut k, &args(&format!("wph_aaaaaaaaaaaaaaa{i}")), &format!("d{i}"), 1).unwrap();
        }
        assert!(matches!(pair(&mut k, &args("wph_bbbbbbbbbbbbbbbb"), "x", 1), Err(PushError::Conflict(_))));
        let gone = drop_gone(&mut k, "wph_aaaaaaaaaaaaaaa3").unwrap();
        assert_eq!(gone.id, "d3");
        assert!(k.dropped.as_deref().unwrap().starts_with("Pixel 9 stopped getting notifications"));
        assert!(unpair(&mut k, "d4") && !unpair(&mut k, "d4"));
        assert_eq!(read(Some("not json")), Kept::default());
    }
}
