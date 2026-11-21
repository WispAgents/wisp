//! `GET <domain>/events` (v1 grid/server.js `watchers`): each signed-in grid page holds one open, and hears
//! `event: change` when something on it changed (a computer came online or went away, the cloud wisp woke or fell
//! asleep), so it fetches itself again (`pages/scripts.rs` LIVE). A ping every 25 s keeps proxies from dropping it.
//! Nothing is said about what changed: the page asks with its own sign-in.
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt as _, StreamBody};
use hyper::body::Frame;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;

use super::Grid;
use crate::boxhost::BoxHost as _;
use crate::http::{Resp, json};
use crate::rows::UserRow;

const PING: Duration = Duration::from_secs(25);

impl Grid {
    /// Something on `user_id`'s pages changed.
    pub fn notify(&self, user_id: &str) {
        // no page open is no one to tell
        let _ = self.notes.send(user_id.to_owned());
    }

    /// The event stream for `user`'s open pages (None: not signed in at the grid).
    pub(super) fn events(&self, user: Option<&UserRow>) -> Resp {
        let Some(user) = user else { return json(401, &json!({ "error": "Sign in again." })) };
        let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(8);
        let mut notes = self.notes.subscribe();
        let uid = user.id.clone();
        tokio::spawn(async move {
            if tx.send(Bytes::from_static(b"retry: 3000\n\n")).await.is_err() {
                return;
            }
            let mut ping = tokio::time::interval(PING);
            ping.tick().await;
            loop {
                let frame: &'static [u8] = tokio::select! {
                    n = notes.recv() => match n {
                        Ok(who) if who == uid => b"event: change\ndata: {}\n\n",
                        Ok(_) => continue,
                        // missed some: say changed, the page asks again anyway
                        Err(RecvError::Lagged(_)) => b"event: change\ndata: {}\n\n",
                        Err(RecvError::Closed) => return,
                    },
                    _ = ping.tick() => b": ping\n\n",
                    // the page went away
                    () = tx.closed() => return,
                };
                if tx.send(Bytes::from_static(frame)).await.is_err() {
                    return;
                }
            }
        });
        let stream = futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|b| (Ok(Frame::data(b)), rx)) });
        crate::http::respond(
            200,
            "text/event-stream",
            StreamBody::new(stream).boxed(),
            &[("x-accel-buffering", "no".into()), ("x-content-type-options", "nosniff".into())],
        )
    }

    /// Each box change is kept (`instances`, what Home reads) and reaches its owner's open pages (a sandbox's, its
    /// owner's).
    pub(super) fn follow_boxes(&self, stop: CancellationToken) -> impl Future<Output = ()> + Send + 'static {
        let mut changes = self.boxes.changes();
        let (notes, db, clock) = (self.notes.clone(), self.db.clone(), self.clock.clone());
        async move {
            loop {
                tokio::select! {
                    () = stop.cancelled() => return,
                    c = changes.recv() => match c {
                        Ok(c) => {
                            if let Err(e) = crate::instances::record(&db, &c, clock.now_ms()) {
                                tracing::warn!("grid: {} {}: {e}", c.user.handle, c.event);
                            }
                            let _ = notes.send(c.user.owner.unwrap_or(c.user.id));
                        }
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => return,
                    },
                }
            }
        }
    }

    /// The grid stopped every box and is going: their rows say so (`instances::grid_stopped`).
    pub fn boxes_stopped(&self) {
        if let Err(e) = crate::instances::grid_stopped(&self.db, self.clock.now_ms()) {
            tracing::warn!("grid: the boxes' stops weren't kept: {e}");
        }
    }

    /// At start: nothing runs yet, and boxes with a task scheduled wake for it (v1 grid/server.js).
    pub(super) fn restore_boxes(&self) {
        match crate::instances::on_start(&self.db) {
            Ok(wakes) => {
                for (u, at) in wakes {
                    self.boxes.restore(&u, Some(at));
                }
            }
            Err(e) => tracing::warn!("grid: the boxes' schedules weren't read back: {e}"),
        }
    }
}
