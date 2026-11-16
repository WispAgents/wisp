//! Whether a port is free for a task's app (v1 runs.js `listenable` + `answers`): wisp binds it on loopback for a
//! moment (a port the system reserves, Windows' excluded ranges, or one held on 127.0.0.1 / 0.0.0.0 is refused), and
//! nothing answers on it (a dev server on `[::1]` or one bound with `SO_REUSEADDR` still lets the bind through).

use std::time::Duration;

/// How long a connect may take before nothing is taken to be listening (v1 `answers(p, 150)`).
const ANSWER_MS: u64 = 150;

/// Something listens on `port` on loopback, v4 or v6.
async fn answers(port: u16) -> bool {
    for host in ["127.0.0.1", "::1"] {
        let c = tokio::net::TcpStream::connect((host, port));
        if matches!(tokio::time::timeout(Duration::from_millis(ANSWER_MS), c).await, Ok(Ok(_))) {
            return true;
        }
    }
    false
}

/// `port` can be given to an app: loopback binds it and nothing answers on it.
pub async fn free(port: u16) -> bool {
    let bound = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.is_ok();
    bound && !answers(port).await
}

/// A port the system says is free (none of the task's own were).
pub async fn any_free() -> Option<u16> {
    let l = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    l.local_addr().ok().map(|a| a.port())
}
