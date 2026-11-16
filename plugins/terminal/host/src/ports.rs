//! Whether a port is free for a task's app (v1 runs.js `listenable` + `answers`): wisp binds it on loopback for a
//! moment (a port the system reserves, Windows' excluded ranges, or one held on 127.0.0.1 / 0.0.0.0 is refused), and
//! nothing answers on it (a dev server on `[::1]` or one bound with `SO_REUSEADDR` still lets the bind through).
//!
//! The probe binds the way dev servers do (ADR-0086): with `SO_REUSEADDR` on Unix (node, vite, python's http.server),
//! so the `TIME_WAIT` the old app's closed connections leave on its port doesn't pass it over on a restart; without it
//! on Windows, where it would bind over a live listener and `TIME_WAIT` never blocks a listener's bind. Linux still
//! refuses a `TIME_WAIT` left by a socket that had no `SO_REUSEADDR`: then no app could bind it either, so it's not free.

use std::time::Duration;

/// How long a connect may take before nothing is taken to be listening (v1 `answers(p, 150)`).
const ANSWER_MS: u64 = 150;

/// Something listens on `port` on loopback, v4 or v6.
async fn answers(port: u16) -> bool {
    for host in ["127.0.0.1", "::1"] {
        let c = tokio::net::TcpStream::connect((host, port));
        // a connect to nothing may be given `port` itself as its source (app ports sit in Linux's ephemeral range):
        // TCP's simultaneous open then joins the socket to itself, which is no app answering
        if let Ok(Ok(s)) = tokio::time::timeout(Duration::from_millis(ANSWER_MS), c).await
            && s.local_addr().ok() != s.peer_addr().ok()
        {
            return true;
        }
    }
    false
}

/// `port` can be given to an app: loopback binds it and nothing answers on it.
pub async fn free(port: u16) -> bool {
    binds(port) && !answers(port).await
}

/// A dev server could listen on `port` on 127.0.0.1 now (the socket is dropped at once, never listening).
fn binds(port: u16) -> bool {
    let Ok(s) = tokio::net::TcpSocket::new_v4() else { return false };
    if cfg!(unix) && s.set_reuseaddr(true).is_err() {
        return false;
    }
    s.bind((std::net::Ipv4Addr::LOCALHOST, port).into()).is_ok()
}

/// A port the system says is free (none of the task's own were).
pub async fn any_free() -> Option<u16> {
    let l = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    l.local_addr().ok().map(|a| a.port())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    /// A listener on a port the system picks, with `SO_REUSEADDR` as dev servers set it (`reuse`) or without it.
    fn listener(reuse: bool) -> tokio::net::TcpListener {
        let s = tokio::net::TcpSocket::new_v4().unwrap();
        s.set_reuseaddr(reuse).unwrap();
        s.bind((std::net::Ipv4Addr::LOCALHOST, 0).into()).unwrap();
        s.listen(16).unwrap()
    }

    /// An app on `l` that took one connection and closed its side first, then ended: its port is left in `TIME_WAIT`.
    async fn time_wait_on(l: tokio::net::TcpListener) -> u16 {
        let port = l.local_addr().unwrap().port();
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (server, _) = l.accept().await.unwrap();
        drop(server); // the app's side goes first: the TIME_WAIT is on its port, not the client's
        let mut buf = [0_u8; 1];
        assert_eq!(client.read(&mut buf).await.unwrap(), 0, "the app hung up");
        drop(client);
        drop(l);
        tokio::time::sleep(Duration::from_millis(50)).await; // the last ACK lands: TIME_WAIT
        port
    }

    /// ADR-0086 edge case: a restarted app's old connections (the browser's, the proxy's) leave `TIME_WAIT` on its port;
    /// an app that set `SO_REUSEADDR` (node, vite, python's http.server) rebinds it at once, so it's free and the
    /// restart keeps its own port (and origin), never the next one.
    #[tokio::test]
    async fn a_port_in_time_wait_from_a_dev_server_is_free() {
        let port = time_wait_on(listener(cfg!(unix))).await;
        assert!(free(port).await, "TIME_WAIT on {port} from a SO_REUSEADDR app");
    }

    /// The same left by a socket without `SO_REUSEADDR`: Linux refuses every bind of it until the `TIME_WAIT` ends
    /// (macOS and Windows don't), so free says exactly what a dev server's own bind would meet: never a port it can't
    /// bind, never passing over one it can.
    #[tokio::test]
    async fn a_port_in_time_wait_from_a_plain_socket_is_free_exactly_when_a_dev_server_could_bind_it() {
        let port = time_wait_on(listener(false)).await;
        let dev_server = tokio::net::TcpSocket::new_v4().unwrap();
        if cfg!(unix) {
            dev_server.set_reuseaddr(true).unwrap();
        }
        let could = dev_server.bind((std::net::Ipv4Addr::LOCALHOST, port).into()).is_ok();
        drop(dev_server);
        assert_eq!(free(port).await, could, "free({port}) agrees with a dev server's bind");
        if cfg!(target_os = "linux") {
            assert!(!could, "Linux: a TIME_WAIT without SO_REUSEADDR blocks every bind");
        }
    }

    /// A port something listens on isn't free (bound there, or only answering).
    #[tokio::test]
    async fn a_listened_on_port_is_not_free() {
        let l = listener(cfg!(unix));
        let port = l.local_addr().unwrap().port();
        assert!(!free(port).await);
        drop(l);
        assert!(free(port).await, "let go: free again");
    }
}
