//! How the edge answers what didn't work: a page in the grid's look for a browser, `{ "error": … }` for anything
//! else (v1 grid/server.js `failed` and its `messagePage`s). Which one by the request, never by the path: a page a
//! browser opened (`Accept: text/html`, not a fetch) or a form it posted gets the page.
use http::{HeaderMap, Method};
use serde_json::json;

use super::Grid;
use crate::error::GridError;
use crate::http::{Resp, header_str, html, json, json_with, wants_html};
use crate::pages::{self, MessagePage, Tone};
use crate::rows::ComputerRow;

/// What of a request decides how its failure reads (taken before the request is spent).
#[derive(Debug, Clone, Copy)]
pub(super) struct Asker {
    pub page: bool,
}

impl Asker {
    pub fn of(method: &Method, h: &HeaderMap) -> Self {
        let form = header_str(h, "content-type").starts_with("application/x-www-form-urlencoded");
        Self { page: wants_html(method, h) || form }
    }
}

impl Grid {
    /// A page that isn't there: the grid's 404 page for a browser, JSON for anything else.
    pub(super) fn not_found<B>(&self, req: &http::Request<B>, title: &str, text: &str) -> Resp {
        self.not_found_for(Asker::of(req.method(), req.headers()), title, text)
    }

    pub(super) fn not_found_for(&self, who: Asker, title: &str, text: &str) -> Resp {
        if !who.page {
            return json(404, &json!({ "error": text }));
        }
        let m = MessagePage {
            title: title.into(),
            text: text.into(),
            link: Some((self.cfg.public_url.clone(), "Home".into())),
            tone: Tone::Plain,
        };
        html(404, pages::message_page(&self.page, &m), &[])
    }

    /// An error, answered: its message (a generic one past 500, the details in the log).
    pub(super) fn failed(&self, who: Asker, e: &GridError) -> Resp {
        let status = e.status();
        if status >= 500 {
            tracing::warn!("grid: {e}");
        }
        if !who.page {
            return super::err(e);
        }
        if status == 404 {
            return self.not_found_for(who, "Nothing here", &e.public_message());
        }
        let title = if status == 429 { "Slow down" } else { "That didn’t work" };
        let m = MessagePage { title: title.into(), text: e.public_message(), link: Some(("/".into(), "Back".into())), tone: Tone::Bad };
        html(status, pages::message_page(&self.page, &m), &[])
    }

    /// A computer that isn't connected (v1 grid/server.js `computerEdge`): a page saying so for a browser.
    pub(super) fn offline(&self, who: Asker, c: &ComputerRow) -> Resp {
        let retry = [("retry-after", "10".to_owned())];
        if !who.page {
            return json_with(503, &json!({ "error": "This computer isn’t online.", "offline": true }), &retry);
        }
        let p = pages::OfflinePage {
            name: c.name.clone(),
            last_seen_at: c.last_seen_at,
            now_ms: self.clock.now_ms(),
            cloud: format!("{}/go", self.cfg.public_url),
            home: self.cfg.public_url.clone(),
        };
        html(503, pages::offline_page(&self.page, &p), &retry)
    }
}
