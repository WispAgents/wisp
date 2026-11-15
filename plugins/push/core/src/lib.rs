//! wisp-push-core: what the `push` transport decides, with no IO (ADR-0065 §3). The devices live in secrets
//! (`devices`), each notification is a [`Note`](wisp_push_api::Note) sealed to one phone's key (`seal`), an answer
//! from a notification becomes one runtime command (`answer`), and the relay's status says what to do next
//! (`relay`). Randomness and time come in from the host.

pub mod answer;
pub mod devices;
pub mod note;
pub mod relay;
pub mod seal;

use wisp_kernel::WireError;

pub use answer::{Act, decide};
pub use devices::{check_pair, pair, unpair, views};
pub use note::note;
pub use relay::{Relayed, classify};
pub use seal::{Keys, open, seal};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PushError {
    /// a field the caller got wrong: `path: why`
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    /// an answer that can't land (already answered, a choice that isn't there)
    #[error("{0}")]
    Conflict(String),
    /// the seal or the open failed (a bad key, a note for someone else)
    #[error("{0}")]
    Crypto(String),
}

impl From<PushError> for WireError {
    fn from(e: PushError) -> Self {
        match e {
            PushError::Invalid(m) | PushError::Crypto(m) => WireError::invalid(m),
            PushError::NotFound(m) => WireError::not_found(m),
            PushError::Conflict(m) => WireError::conflict(m),
        }
    }
}

pub type Result<T> = std::result::Result<T, PushError>;

/// At most `max` characters, with `…` when cut. Its own copy: a transport depends on `wisp-channels-api` only.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_map_to_the_wire() {
        assert_eq!(WireError::from(PushError::Invalid("x".into())).status(), 400);
        assert_eq!(WireError::from(PushError::NotFound("x".into())).status(), 404);
        assert_eq!(WireError::from(PushError::Conflict("x".into())).status(), 409);
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
