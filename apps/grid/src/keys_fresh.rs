//! A key just made, held for the one page that says it (QA round 6 #3): `POST /keys` makes the key, keeps it here under
//! a random token and redirects to Account with that token in a short-lived cookie; Account takes it, once. A reload,
//! another browser, or another account with the token gets nothing, and the key itself is never in a cookie or a URL.
//! In memory only: a grid that restarts within the minute loses the one showing, never the key (it's in the list).
use std::collections::HashMap;
use std::sync::Mutex;

/// How long a made key waits for its page.
pub const FRESH_MS: i64 = 60_000;

struct Held {
    user: String,
    name: String,
    key: String,
    until: i64,
}

#[derive(Default)]
pub struct FreshKeys(Mutex<HashMap<String, Held>>);

impl std::fmt::Debug for FreshKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreshKeys")
    }
}

impl FreshKeys {
    /// Holds `key` for `user`'s next Account page; the token to find it with.
    pub fn put(&self, user: &str, name: &str, key: &str, now: i64) -> String {
        let token = crate::secrets::token("", 18);
        let mut m = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        m.retain(|_, h| h.until > now);
        m.insert(token.clone(), Held { user: user.to_owned(), name: name.to_owned(), key: key.to_owned(), until: now + FRESH_MS });
        token
    }

    /// The key a token holds for `user`, once: (its name, the key). Gone after, whoever asked.
    pub fn take(&self, token: &str, user: &str, now: i64) -> Option<(String, String)> {
        let h = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(token)?;
        (h.user == user && h.until > now).then_some((h.name, h.key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_made_key_is_said_once_to_its_own_account_within_the_minute() {
        let f = FreshKeys::default();
        let t = f.put("ada", "deploy", "wgk_x", 0);
        assert_eq!(f.take(&t, "ada", 1), Some(("deploy".into(), "wgk_x".into())));
        assert_eq!(f.take(&t, "ada", 2), None, "once");
        let t = f.put("ada", "deploy", "wgk_y", 0);
        assert_eq!(f.take(&t, "bob", 1), None, "not to another account");
        assert_eq!(f.take(&t, "ada", 1), None, "and spent by that try");
        let t = f.put("ada", "deploy", "wgk_z", 0);
        assert_eq!(f.take(&t, "ada", FRESH_MS), None, "not after the minute");
        assert_eq!(f.take("guess", "ada", 1), None);
    }
}
