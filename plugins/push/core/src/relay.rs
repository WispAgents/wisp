//! What the relay's answer to `POST /api/push/send` means for the device it was for.

/// One send's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Relayed {
    Sent,
    /// FCM/APNs said the token is gone (the app removed, its notifications reset): the relay dropped the handle,
    /// so wisp unpairs the device
    Gone,
    /// the relay has no FCM/APNs credentials (or they're down): nothing to do but keep the other channels
    Unavailable(String),
    /// too many to this handle this minute
    Busy,
    Failed(String),
}

/// `status` and the relay's `{error}`.
pub fn classify(status: u16, error: Option<&str>) -> Relayed {
    let why = || error.unwrap_or("no reason given").to_owned();
    match status {
        200..=299 => Relayed::Sent,
        410 => Relayed::Gone,
        429 => Relayed::Busy,
        503 | 504 => Relayed::Unavailable(why()),
        _ => Relayed::Failed(format!("the relay answered {status}: {}", why())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relays_answers_mean_what_they_say() {
        assert_eq!(classify(200, None), Relayed::Sent);
        assert_eq!(classify(410, Some("gone")), Relayed::Gone);
        assert!(matches!(classify(404, None), Relayed::Failed(_)), "a grid without the routes never unpairs a phone");
        assert_eq!(classify(503, Some("unavailable")), Relayed::Unavailable("unavailable".into()));
        assert_eq!(classify(429, None), Relayed::Busy);
        assert!(matches!(classify(400, Some("x")), Relayed::Failed(m) if m.contains("400")));
    }
}
