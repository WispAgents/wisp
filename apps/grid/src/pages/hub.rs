//! The hub's pages (ADR-0038, ADR-0041), readable without wisp: a share of every kind at `/s/<id>`, a
//! locked or gone one, and the gallery at `/gallery`. Every value is escaped (`esc`); nothing a sharer
//! wrote is ever markup.
//!
//! What a share's content is, by kind (the JSON wisp signs and sends):
//! - `task`: a `.wisp-task` bundle (`title`, `spec.prompt`, `report.answers[].text|blocks`, `report.files`);
//! - `setup`, `module`, `crew`, `workflow`, `agent`: `{ resources: [{kind, name, summary}], files: [{name, text}] }`;
//! - `doc`, `report`: `{ markdown }`;
//! - anything else: shown as its JSON.

use serde_json::Value;

use super::PageUser;
use super::layout::{AppPage, app_page};
use super::{PageCfg, esc};

/// What the page knows about a share before its content.
#[derive(Debug, Clone, Default)]
pub struct ShareCard {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub from: String,
    pub version: i64,
    /// `grid://<handle>/<key>`
    pub source: String,
    /// who may open it: `anyone` | `people` | `teams` | `password`
    pub access: String,
    /// the reader is signed in to this grid, so has a cloud wisp to open it in
    pub signed_in: bool,
}

const STYLE: &str = "<style>.hub-meta{color:var(--muted,#666);font-size:13px;margin:4px 0 16px}.hub-list{list-style:none;padding:0;margin:0 0 16px}.hub-list li{padding:6px 0;border-bottom:1px solid rgba(127,127,127,.18)}.hub-pre{white-space:pre-wrap;word-break:break-word;font:13px/1.5 ui-monospace,Menlo,monospace;background:rgba(127,127,127,.08);padding:12px;border-radius:8px;overflow:auto}.hub-k{font:12px ui-monospace,Menlo,monospace;opacity:.7;margin-right:6px}.hub-open{margin:14px 0 24px}.hub-acts{display:flex;flex-wrap:wrap;gap:8px}.hub-alt{margin-top:8px;color:var(--muted,#666);font-size:12.5px}.hub-alt a{color:var(--accent)}.hub-alt code,.hub-open code{font:12px ui-monospace,Menlo,monospace}.hub-prose{white-space:pre-wrap;word-break:break-word;font:14px/1.55 var(--sans,system-ui);background:rgba(127,127,127,.06);border:1px solid rgba(127,127,127,.16);padding:12px 14px;border-radius:10px}.main h2.h2{margin:22px 0 8px;font-size:15px;font-weight:600}.hub-list code{font:13px ui-monospace,Menlo,monospace}.hub-n{font-weight:400;color:var(--muted,#666);font-size:12.5px;margin-left:4px}.hub-src{word-break:keep-all;overflow-wrap:normal}.hub-alt code{overflow-wrap:anywhere}.hub-hero{margin:10px 0 2px;font-weight:600;font-size:14.5px}.hub-sum{margin:2px 0 8px;color:var(--ink-2,#3d3d3d)}.hub-prose+.hub-hero{margin-top:14px}.hub-list li code{margin-right:6px}.hub-answer{background:rgba(127,127,127,.06);border:1px solid rgba(127,127,127,.16);padding:12px 14px;border-radius:10px}.hub-answer+.hub-answer{margin-top:10px}.hub-answer .hub-prose{background:none;border:0;padding:0;border-radius:0}.hub-answer>.hub-hero:first-child{margin-top:0}.hub-answer .hub-list{margin:6px 0 4px}.hub-answer .hub-list li:last-child{border-bottom:0}.hub-answer>:last-child{margin-bottom:0}</style>";

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The way in (#8): signed in to this grid, the cloud wisp, its New task holding this link as text (`?share=`, which
/// the wisp takes only as a share link and never sends: the person presses Send, and the share imports); anywhere, wisp
/// on this computer (`wisp://open`); and for someone with no wisp at all, where to get it and the command.
fn open_in_wisp(c: &ShareCard, public_url: &str) -> String {
    let link = format!("{public_url}/s/{}", c.id);
    let local = format!(
        r#"<a class="{} tall" href="wisp://open/{}">Open in wisp on this computer</a>"#,
        if c.signed_in { "ghost line" } else { "primary" },
        esc(&c.id)
    );
    let (cloud, then) = if c.signed_in {
        let to = super::home::wisp_at(&format!("/?share={}", crate::http::encode_component(&link)));
        (
            format!(r#"<a class="primary tall" href="{}">Open in your cloud wisp</a>"#, esc(&to)),
            "In your cloud wisp it waits in New task: press Send to import it. ",
        )
    } else {
        (String::new(), "")
    };
    format!(
        r#"<div class="hub-open"><div class="hub-acts">{cloud}{local}</div><p class="hub-alt">{then}No wisp? <a href="https://trywisp.dev" target="_blank" rel="noopener">Get wisp</a> · or run <code>wisp open {link}</code></p></div>"#,
        link = esc(&link)
    )
}

/// `grid://<wbr>handle/<wbr>key`: the reference may wrap after `://` and each `/`, and nowhere else (QA round 6 #16).
fn src_breaks(src: &str) -> String {
    match src.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://<wbr>{}", rest.replace('/', "/<wbr>")),
        None => src.replace('/', "/<wbr>"),
    }
}

/// Who opens it, in the reader's words.
fn access_words(a: &str) -> &'static str {
    match a {
        "people" => "the people named",
        "teams" => "its teams",
        "password" => "with its passkey",
        _ => "anyone with the link",
    }
}

fn task_body(v: &Value) -> String {
    let mut out = String::new();
    let prompt = v.pointer("/spec/prompt").and_then(Value::as_str).unwrap_or_default();
    if !prompt.is_empty() {
        out.push_str(&format!(r#"<h2 class="h2">The ask</h2><div class="hub-prose">{}</div>"#, esc(prompt)));
    }
    let answers = v.pointer("/report/answers").and_then(Value::as_array).cloned().unwrap_or_default();
    if !answers.is_empty() {
        out.push_str(r#"<h2 class="h2">What it did</h2>"#);
        for a in &answers {
            out.push_str(&super::answer::answer_html(a));
        }
    }
    let files: Vec<String> = v
        .pointer("/report/files")
        .and_then(Value::as_array)
        .map(|f| f.iter().filter_map(|x| x.as_str().map(esc)).collect())
        .unwrap_or_default();
    if !files.is_empty() {
        out.push_str(&format!(
            r#"<h2 class="h2">Files it changed <span class="hub-n">{}</span></h2><ul class="hub-list">{}</ul>"#,
            files.len(),
            files.iter().map(|f| format!("<li><code>{f}</code></li>")).collect::<String>()
        ));
    }
    if out.is_empty() {
        out.push_str(r#"<p class="sub">The ask and its setup, to run on your own code.</p>"#);
    }
    out
}

fn setup_body(v: &Value) -> String {
    let mut out = String::new();
    let res = v.get("resources").and_then(Value::as_array).cloned().unwrap_or_default();
    if !res.is_empty() {
        out.push_str(&format!(
            r#"<h2 class="h2">What it sets up</h2><ul class="hub-list">{}</ul>"#,
            res.iter()
                .map(|r| format!(
                    r#"<li><span class="hub-k">{}</span><b>{}</b> {}</li>"#,
                    esc(s(r, "kind")),
                    esc(s(r, "name")),
                    esc(s(r, "summary"))
                ))
                .collect::<String>()
        ));
    }
    for f in v.get("files").and_then(Value::as_array).into_iter().flatten() {
        out.push_str(&format!(r#"<h2 class="h2">{}</h2><div class="hub-pre">{}</div>"#, esc(s(f, "name")), esc(s(f, "text"))));
    }
    out
}

/// A share's page: what it is, who shared it, how to open it in wisp, and its content in plain words.
pub fn share_page(cfg: &PageCfg, c: &ShareCard, content: &Value, viewer: Option<&PageUser>) -> String {
    let what = match c.kind.as_str() {
        "task" => task_body(content),
        "setup" | "module" | "crew" | "workflow" | "agent" => setup_body(content),
        "doc" | "report" => format!(r#"<div class="hub-pre">{}</div>"#, esc(s(content, "markdown"))),
        _ => format!(r#"<div class="hub-pre">{}</div>"#, esc(&serde_json::to_string_pretty(content).unwrap_or_default())),
    };
    let summary = if c.summary.is_empty() { String::new() } else { format!(r#"<p class="sub">{}</p>"#, esc(&c.summary)) };
    let body = format!(
        r#"<div class="hub-meta">A {kind} shared by <b>{from}</b> · v{v} · {who} · <code class="hub-src">{src}</code></div><h1 class="h1">{title}</h1>{summary}{open}{what}"#,
        title = esc(&c.title),
        kind = esc(&c.kind),
        from = esc(&c.from),
        v = c.version,
        // wraps only after `://` and each `/` (QA round 6 #16): the handle and the key never break mid-word
        src = src_breaks(&esc(&c.source)),
        who = access_words(&c.access),
        open = open_in_wisp(c, &cfg.public_url),
    );
    app_page(&AppPage { head: STYLE, ..AppPage::new(cfg, &c.title, &body) }.signed(viewer))
}

/// A locked share: only that it's locked, and how it opens. Nothing of what it is.
pub fn locked_page(cfg: &PageCfg, id: &str, access: &str, wrong: bool, viewer: Option<&PageUser>) -> String {
    let body = match access {
        "password" => format!(
            r#"<h1 class="h1">This share has a password</h1><p class="sub">Ask whoever shared it for the password.</p>{}<form method="post" action="/s/{}" style="margin-top:16px;display:flex;gap:10px"><input class="input" type="password" name="password" autocomplete="off" autofocus required><button class="primary tall">Open</button></form>"#,
            if wrong { r#"<div class="flash bad">That password isn’t right.</div>"# } else { "" },
            esc(id)
        ),
        "people" => {
            r#"<h1 class="h1">This share is for some people only</h1><p class="sub">Sign in as one of them to open it.</p>"#.to_owned()
        }
        _ => r#"<h1 class="h1">This share is for a team</h1><p class="sub">Sign in as a member of that team to open it.</p>"#.to_owned(),
    };
    app_page(&AppPage { narrow: true, ..AppPage::new(cfg, "A locked share", &body) }.signed(viewer))
}

/// Revoked or expired.
pub fn gone_page(cfg: &PageCfg, why: &str, viewer: Option<&PageUser>) -> String {
    let text = if why == "expired" { "This link has expired." } else { "Whoever shared this stopped sharing it." };
    let body = format!(r#"<h1 class="h1">This share is gone</h1><p class="sub">{text}</p>"#);
    app_page(&AppPage { narrow: true, ..AppPage::new(cfg, "A share that's gone", &body) }.signed(viewer))
}

/// The gallery: what people published, searchable.
pub fn gallery_page(cfg: &PageCfg, q: &str, found: &[Value], viewer: Option<&PageUser>) -> String {
    let items: String = found
        .iter()
        .map(|f| {
            let c = &f["card"];
            format!(
                r#"<li><span class="hub-k">{kind}</span><a href="/s/{id}"><b>{title}</b></a> <span class="sub">{summary}</span> <span class="hub-meta">by {from} · {imports} imports</span></li>"#,
                kind = esc(s(c, "kind")),
                id = esc(s(c, "id")),
                title = esc(s(c, "title")),
                summary = esc(s(c, "summary")),
                from = esc(c.pointer("/from/handle").and_then(Value::as_str).unwrap_or_default()),
                imports = f["imports"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    let list = if items.is_empty() {
        r#"<p class="sub">Nothing matches yet.</p>"#.to_owned()
    } else {
        format!(r#"<ul class="hub-list">{items}</ul>"#)
    };
    let body = format!(
        r#"<h1 class="h1">What people are building</h1><p class="sub">Formulas, modules, crew, workflows, plugins and apps people chose to publish. Use this opens it in wisp, where you see its plan before anything changes.</p><form method="get" action="/gallery" style="margin:16px 0;display:flex;gap:10px"><input class="input" name="q" value="{}" placeholder="Search"><button class="primary tall">Search</button></form>{list}"#,
        esc(q)
    );
    app_page(&AppPage { head: STYLE, ..AppPage::new(cfg, "Gallery", &body) }.signed(viewer))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn cfg() -> PageCfg {
        PageCfg { name: "wisp Grid".into(), domain: "wisp.localhost".into(), public_url: "http://wisp.localhost:8080".into() }
    }

    #[test]
    fn a_share_page_works_without_wisp() {
        let card = |kind: &str| ShareCard {
            id: "abc".into(),
            kind: kind.into(),
            title: "<b>Review</b>".into(),
            from: "ada".into(),
            version: 2,
            source: "grid://ada/review".into(),
            ..ShareCard::default()
        };
        let task = share_page(
            &cfg(),
            &card("task"),
            &json!({ "spec": { "prompt": "fix <script>x</script>" }, "report": { "answers": [{ "text": "Done" }], "files": ["src/a.ts"] } }),
            None,
        );
        assert!(task.contains("fix &#60;script&#62;") || task.contains("fix &lt;script&gt;"), "escaped");
        assert!(!task.contains("<script>x") && task.contains("src/a.ts") && task.contains("wisp open http://wisp.localhost:8080/s/abc"));
        assert!(!task.contains("<b>Review</b>"));
        let setup = share_page(
            &cfg(),
            &card("setup"),
            &json!({ "resources": [{ "kind": "agent", "name": "reviewer", "summary": "claude, plan mode" }], "files": [{ "name": "agents.wisp.yml", "text": "agent: …" }] }),
            None,
        );
        assert!(setup.contains("reviewer") && setup.contains("agents.wisp.yml") && setup.contains("v2"));
        let doc = share_page(&cfg(), &card("doc"), &json!({ "markdown": "# Notes" }), None);
        assert!(doc.contains("# Notes"));
        let locked = locked_page(&cfg(), "abc", "password", true, None);
        assert!(locked.contains("password isn’t right") && !locked.contains("Review"));
        assert!(gone_page(&cfg(), "expired", None).contains("expired"));
        assert!(gallery_page(&cfg(), "<q>", &[], None).contains("Nothing matches"));
    }

    #[test]
    fn the_way_in_works_with_or_without_wisp_and_says_who_opens_it() {
        let mut c =
            ShareCard { id: "abc".into(), kind: "task".into(), title: "T".into(), access: "password".into(), ..ShareCard::default() };
        let out = share_page(&cfg(), &c, &json!({}), None);
        // signed out: wisp on this computer is the way, with where to get wisp
        assert!(out.contains(r#"<a class="primary tall" href="wisp://open/abc">Open in wisp on this computer</a>"#));
        assert!(out.contains("No wisp? <a href=\"https://trywisp.dev\"") && out.contains("with its passkey"));
        assert!(!out.contains("Open in your cloud wisp"));
        // signed in to the grid: the cloud wisp first, New task there filled with this link (never sent), this computer second
        c.signed_in = true;
        let inn = share_page(&cfg(), &c, &json!({}), None);
        let to = "/go?path=%2F%3Fshare%3Dhttp%253A%252F%252Fwisp.localhost%253A8080%252Fs%252Fabc";
        assert_eq!(
            super::super::home::wisp_at(&format!("/?share={}", crate::http::encode_component("http://wisp.localhost:8080/s/abc"))),
            to
        );
        assert!(inn.contains(&format!(r#"<a class="primary tall" href="{}">Open in your cloud wisp</a>"#, esc(to))), "{inn}");
        assert!(inn.contains(r#"<a class="ghost line tall" href="wisp://open/abc">"#) && inn.contains("press Send to import it"));
        assert!(!inn.contains("data-copy") && !inn.contains("paste the link"));
    }

    #[test]
    fn an_answer_reads_as_the_app_shows_it_never_as_its_wisp_json() {
        // QA round 6 #2: every harness ends its answer with a ```wisp block
        let text = "All done.\n```wisp\n{\"blocks\":[{\"type\":\"hero\",\"title\":\"Fake claude finished\",\"summary\":\"No question asked.\",\"status\":\"done\"},{\"type\":\"markdown\",\"text\":\"Fixed **math**\"},{\"type\":\"files\",\"items\":[{\"path\":\"math.js\",\"change\":\"modified\",\"note\":\"the off-by-one\"}]},{\"type\":\"steps\",\"items\":[{\"title\":\"Ran the tests\",\"detail\":\"12 pass\"}]},{\"type\":\"tests\",\"command\":\"npm test\",\"passed\":12,\"failed\":0},{\"type\":\"callout\",\"title\":\"Heads up\",\"text\":\"<script>x</script>\"},{\"type\":\"image\",\"path\":\"/etc/passwd\"}]}\n```\n";
        let page = share_page(
            &cfg(),
            &ShareCard { id: "abc".into(), kind: "task".into(), title: "T".into(), ..ShareCard::default() },
            &json!({ "report": { "answers": [{ "text": text }] } }),
            None,
        );
        for gone in ["```", "\"blocks\"", "{\"type\"", "/etc/passwd", "<script>x"] {
            assert!(!page.contains(gone), "{gone}: {page}");
        }
        for shown in [
            "All done.",
            "Fake claude finished",
            "No question asked.",
            "Fixed **math**",
            "math.js",
            "the off-by-one",
            "Ran the tests",
            "12 pass",
            "npm test: 12 passed, 0 failed",
            "Heads up",
            "&#60;script&#62;x",
        ] {
            assert!(page.contains(shown), "{shown}: {page}");
        }
        // only blocks, no text: the same; a block that isn't JSON is dropped, the words before it kept
        let task = || ShareCard { kind: "task".into(), ..ShareCard::default() };
        let only = share_page(
            &cfg(),
            &task(),
            &json!({ "report": { "answers": [{ "blocks": [{ "type": "hero", "title": "Done it" }] }] } }),
            None,
        );
        assert!(only.contains("Done it") && !only.contains("\"type\""), "{only}");
        let broken =
            share_page(&cfg(), &task(), &json!({ "report": { "answers": [{ "text": "Half.\n```wisp\n{\"blocks\":[\n" }] } }), None);
        assert!(broken.contains("Half.") && !broken.contains("```") && !broken.contains("blocks"), "{broken}");
    }

    #[test]
    fn the_grid_reference_wraps_only_between_its_parts() {
        // QA round 6 #16: never mid-word
        let c = ShareCard {
            id: "abc".into(),
            kind: "task".into(),
            source: "grid://qa-tester/01a1ed71d7bd751cbe6bb7f04e889d28".into(),
            ..ShareCard::default()
        };
        let page = share_page(&cfg(), &c, &json!({}), None);
        assert!(page.contains(r#"<code class="hub-src">grid://<wbr>qa-tester/<wbr>01a1ed71d7bd751cbe6bb7f04e889d28</code>"#), "{page}");
        assert!(page.contains(".hub-src{") && page.contains("word-break:keep-all"));
    }

    #[test]
    fn a_signed_in_reader_has_the_grid_s_header() {
        let u = PageUser { handle: "ada".into(), ..PageUser::default() };
        let nav = r#"<a href="/">Home</a><a href="/account">Account</a>"#;
        let c = ShareCard { id: "abc".into(), kind: "task".into(), ..ShareCard::default() };
        assert!(share_page(&cfg(), &c, &json!({}), Some(&u)).contains(nav));
        assert!(!share_page(&cfg(), &c, &json!({}), None).contains(nav));
        assert!(gone_page(&cfg(), "expired", Some(&u)).contains(nav));
        assert!(locked_page(&cfg(), "abc", "password", false, Some(&u)).contains(nav));
        assert!(gallery_page(&cfg(), "", &[], Some(&u)).contains(nav));
    }
}
