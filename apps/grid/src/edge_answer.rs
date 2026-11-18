//! How the edge answers what didn't work: a page in the grid's look for a browser, `{ "error": … }` for anything
//! else (v1 grid/server.js `failed` and its `messagePage`s). Which one by the request, never by the path: a page a
//! browser opened (`Accept: text/html`, not a fetch) or a form it posted gets the page.
use http::{HeaderMap, Method};
use serde_json::json;

use super::Grid;
use crate::error::GridError;
use crate::http::{Resp, cookies, header_str, html, json, json_with, wants_html};
use crate::pages::{self, MessagePage, PageUser, Tone};
use crate::rows::ComputerRow;

/// What of a request decides how its failure reads (taken before the request is spent): a page or JSON, and who's
/// signed in to the grid there (its header on the page, QA round 6 #4).
#[derive(Debug, Clone, Default)]
pub(super) struct Asker {
    pub page: bool,
    pub viewer: Option<PageUser>,
}

impl Asker {
    pub fn of(method: &Method, h: &HeaderMap) -> Self {
        let form = header_str(h, "content-type").starts_with("application/x-www-form-urlencoded");
        Self { page: wants_html(method, h) || form, viewer: None }
    }
}

impl Grid {
    /// Who's signed in to the grid on this request, as a page shows them: the grid's own cookie, so only on its address.
    pub(super) fn viewer(&self, h: &HeaderMap) -> Option<PageUser> {
        let c = cookies(h).get(super::COOKIE).cloned()?;
        let u = self.accounts.session(&c).ok().flatten()?;
        Some(PageUser { handle: u.handle, name: u.name, email: u.email, avatar: u.avatar })
    }

    /// `Asker::of` with the viewer, for an answer that may be a page.
    pub(super) fn asker(&self, method: &Method, h: &HeaderMap) -> Asker {
        let a = Asker::of(method, h);
        let viewer = if a.page { self.viewer(h) } else { None };
        Asker { viewer, ..a }
    }

    /// A page that isn't there: the grid's 404 page for a browser, JSON for anything else.
    pub(super) fn not_found<B>(&self, req: &http::Request<B>, title: &str, text: &str) -> Resp {
        self.not_found_for(self.asker(req.method(), req.headers()), title, text)
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
            user: who.viewer,
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
        let m = MessagePage {
            title: title.into(),
            text: e.public_message(),
            link: Some(("/".into(), "Back".into())),
            tone: Tone::Bad,
            user: who.viewer,
        };
        html(status, pages::message_page(&self.page, &m), &[])
    }

    /// A computer that isn't connected (v1 grid/server.js `computerEdge`): a page saying so for a browser.
    pub(super) fn offline(&self, who: &Asker, c: &ComputerRow) -> Resp {
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
            // only its owner gets this far (computer_reach): the grid's header, for them
            user: self.accounts.user_by_id(&c.user_id).ok().flatten().map(|u| PageUser {
                handle: u.handle,
                name: u.name,
                email: u.email,
                avatar: u.avatar,
            }),
        };
        html(503, pages::offline_page(&self.page, &p), &retry)
    }
}
