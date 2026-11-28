//! wisp-ship's one error enum, mapped to the wire's `{code, message, retry}` at the edge.

use wisp_kernel::WireError;
use wisp_ship_core::Refusal;

#[derive(Debug, thiserror::Error)]
pub enum ShipError {
    /// a request Ship it won't do, in v1's words (400 or 409)
    #[error("{}", .0.message())]
    Refused(Refusal),
    /// gh failed: its own `error:`/`fatal:` line, or its last lines
    #[error("{0}")]
    Gh(String),
    #[error("{0}")]
    NotFound(String),
    /// the base branch's rules hold a merge (required checks or reviews): the UI asks to bypass or wait
    #[error("{0}")]
    Blocked(String),
    #[error(transparent)]
    Wire(#[from] WireError),
}

impl From<Refusal> for ShipError {
    fn from(r: Refusal) -> Self {
        Self::Refused(r)
    }
}

impl ShipError {
    /// `name: message`, for one repo of a task across several.
    #[must_use]
    pub fn in_repo(self, name: &str, of: usize) -> Self {
        if of <= 1 {
            return self;
        }
        match self {
            Self::Refused(r) => Self::Refused(r.in_repo(name, of)),
            Self::Gh(m) => Self::Gh(format!("{name}: {m}")),
            Self::NotFound(m) => Self::NotFound(format!("{name}: {m}")),
            Self::Blocked(m) => Self::Blocked(format!("{name}: {m}")),
            Self::Wire(mut w) => {
                w.message = format!("{name}: {}", w.message);
                Self::Wire(w)
            }
        }
    }
}

impl From<ShipError> for WireError {
    fn from(e: ShipError) -> Self {
        match e {
            ShipError::Refused(Refusal::Invalid(m)) | ShipError::Gh(m) => WireError::invalid(m),
            ShipError::Refused(Refusal::Conflict(m)) => WireError::conflict(m),
            ShipError::NotFound(m) => WireError::not_found(m),
            ShipError::Blocked(m) => WireError::forbidden(m).with_details(serde_json::json!({ "reason": wisp_ship_api::MERGE_BLOCKED })),
            ShipError::Wire(w) => w,
        }
    }
}

pub type Result<T, E = ShipError> = std::result::Result<T, E>;

impl ShipError {
    /// A task Ship it doesn't follow (`Ship::task`'s not found) that runtime does have: it runs in its project's own
    /// folder, with no worktree to commit, push and open a PR from. v1 said so (routes/ship.js `NO_TREE`); v2 said
    /// "no such session: <id>" (audit B #2). Anything else, or a task that's really gone, is left as it was.
    #[must_use]
    pub fn untracked(self, task_exists: bool) -> Self {
        match self {
            Self::NotFound(_) if task_exists => Self::Refused(Refusal::Invalid(wisp_ship_core::decide::NO_TREE.into())),
            e => e,
        }
    }
}

#[cfg(test)]
mod untracked_tests {
    use super::*;

    #[test]
    fn a_task_in_its_projects_own_folder_gets_v1s_no_worktree_reason_not_no_such_session() {
        let w = WireError::from(ShipError::NotFound("no such session: s1".into()).untracked(true));
        assert_eq!(w.code, wisp_kernel::Code::Invalid);
        assert_eq!(w.message, "Ship it works on worktree sessions: launch with a worktree to get one.");
        // a task that's really gone stays not found; another failure stays itself
        let gone = WireError::from(ShipError::NotFound("no such session: s2".into()).untracked(false));
        assert_eq!((gone.code, gone.message.as_str()), (wisp_kernel::Code::NotFound, "no such session: s2"));
        assert_eq!(WireError::from(ShipError::Gh("boom".into()).untracked(true)).message, "boom");
    }
}
