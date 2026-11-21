//! Home's lower half: your links (#5: published apps and shared tasks, each with Copy link and its ⋯), the
//! Getting-started checklist (#2: three steps that tick themselves, gone once done or dismissed), and what else the
//! grid does, folded (every idea a link to the exact place, #3).
use super::super::esc;
use super::super::icons::{ARROW, CHECK, CLOUD, CODE, GLOBE, KEY, LAPTOP, MOON, PHONE, SHARE, STACK, TRASH};
use super::super::time::ago;
use super::{HomePage, SiteView, bare, more_menu, wisp_at, wisp_settings};

/// One shared task (or setup, doc…) that still opens, as Home lists it.
#[derive(Debug, Clone, Default)]
pub struct ShareView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub url: String,
    /// `anyone` | `people` | `teams` | `password`
    pub access: String,
    pub views: i64,
    pub updated_at: Option<i64>,
    pub expires_at: Option<i64>,
}

/// The Getting-started steps you took, as the page's own cookie (`wisp_start`: `a` agent, `t` task, `x` dismissed)
/// says. Opening the cloud wisp ticks itself from the instance row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Start {
    pub agent: bool,
    pub task: bool,
    pub dismissed: bool,
}

impl Start {
    pub fn from_cookie(v: &str) -> Self {
        let has = |k: &str| v.split(',').any(|x| x.trim() == k);
        Self { agent: has("a"), task: has("t"), dismissed: has("x") }
    }
}

fn access_label(s: &ShareView) -> &'static str {
    match s.access.as_str() {
        "people" => "the people named",
        "teams" => "your teams",
        "password" => "with its passkey",
        _ => "anyone with the link",
    }
}

fn site_row(x: &SiteView, hidden: &str, now: i64) -> String {
    let items = format!(
        r#"<a class="mi" href="{url}" target="_blank" rel="noopener">{GLOBE}<div><span>Open</span><small>In a new tab</small></div></a><form method="post" action="/sites/{key}/remove" data-confirm="Unpublish it?">{hidden}<button class="mi danger" title="Take it offline: the link stops working. It stays in My Stuff.">{TRASH}<div><span>Unpublish</span><small>The link stops working; it stays in My Stuff</small></div></button></form>"#,
        url = esc(&x.url),
        key = esc(&x.key),
    );
    format!(
        r#"<div class="mrow"><span class="w-ic">{GLOBE}</span><div class="m-tx"><b title="{title}">{title}</b><small>{host} · updated {ago} · {n} file{s} · public</small></div><div class="m-acts">{copy}{more}</div></div>"#,
        title = esc(&x.title),
        host = esc(bare(&x.url).trim_end_matches('/')),
        ago = ago(x.updated_at, now),
        n = x.files,
        s = if x.files == 1 { "" } else { "s" },
        copy = copy_link(&x.url),
        more = more_menu(&x.title, &items),
    )
}

fn share_row(s: &ShareView, hidden: &str, now: i64) -> String {
    let expired = s.expires_at.is_some_and(|t| t != 0 && t <= now);
    let views = if s.views == 1 { "1 view".to_owned() } else { format!("{} views", s.views) };
    let state = if expired { " · expired".to_owned() } else { String::new() };
    let items = format!(
        r#"<a class="mi" href="{url}" target="_blank" rel="noopener">{SHARE}<div><span>Open</span><small>As others see it</small></div></a><form method="post" action="/shares/{id}/revoke" data-confirm="Stop sharing?">{hidden}<button class="mi danger">{TRASH}<div><span>Stop sharing</span><small>The link stops working; sharing again brings it back</small></div></button></form>"#,
        url = esc(&s.url),
        id = esc(&s.id),
    );
    format!(
        r#"<div class="mrow"><span class="w-ic">{SHARE}</span><div class="m-tx"><b title="{title}">{title}</b><small>{kind} · {who} · {views} · updated {ago}{state}</small></div><div class="m-acts">{copy}{more}</div></div>"#,
        title = esc(&s.title),
        kind = esc(if s.kind.is_empty() { "task" } else { &s.kind }),
        who = access_label(s),
        ago = ago(s.updated_at, now),
        copy = copy_link(&s.url),
        more = more_menu(&s.title, &items),
    )
}

fn copy_link(url: &str) -> String {
    format!(r#"<button class="ghost line hide-sm" type="button" data-copy="{}"><span>Copy link</span></button>"#, esc(url))
}

/// Your links, when there are some: one list, or two tabs when there are both apps and shared tasks.
pub(super) fn links(p: &HomePage, hidden: &str) -> String {
    let apps: String = p.sites.iter().map(|x| site_row(x, hidden, p.now_ms)).collect();
    let shares: String = p.shares.iter().map(|s| share_row(s, hidden, p.now_ms)).collect();
    let hint = r#"<p class="hint">Anyone with the link can open them, while your wisp sleeps too; search engines don’t list them.</p>"#;
    match (p.sites.len(), p.shares.len()) {
        (0, 0) => String::new(),
        (n, 0) => format!(
            r#"<section class="block" id="apps"><h2 class="sec">Published apps <span class="n">{n}</span></h2>{hint}<div class="ml">{apps}</div></section>"#
        ),
        (0, n) => format!(
            r#"<section class="block" id="shares"><h2 class="sec">Shared tasks <span class="n">{n}</span></h2><div class="ml">{shares}</div></section>"#
        ),
        (a, s) => format!(
            r#"<section class="block links" id="apps"><input type="radio" name="links" id="lnApps" checked><input type="radio" name="links" id="lnShares">
        <div class="sec-row"><h2 class="sec">Links</h2><div class="seg" role="presentation"><label for="lnApps">Apps <span class="n">{a}</span></label><label for="lnShares">Shared tasks <span class="n">{s}</span></label></div></div>
        <div class="ml p-apps">{apps}</div><div class="ml p-shares">{shares}</div></section>"#
        ),
    }
}

struct Step {
    key: &'static str,
    title: &'static str,
    href: String,
    done: bool,
}

fn steps(p: &HomePage) -> [Step; 3] {
    let opened =
        p.instance.state == "running" || p.instance.started_at.is_some_and(|t| t != 0) || p.instance.stopped_at.is_some_and(|t| t != 0);
    [
        Step { key: "o", title: "Open your cloud wisp", href: "/go".into(), done: opened },
        Step { key: "a", title: "Sign in to an agent", href: wisp_settings("agents"), done: p.start.agent },
        Step { key: "t", title: "Send a task, close the laptop", href: "/go".into(), done: p.start.task },
    ]
}

/// The Getting-started row (#2): how many of three, the next one with Continue, and × to dismiss it for good.
pub(super) fn checklist(p: &HomePage) -> String {
    let steps = steps(p);
    let done = steps.iter().filter(|s| s.done).count();
    let Some(next) = steps.iter().find(|s| !s.done).filter(|_| !p.start.dismissed) else { return String::new() };
    let marks: String = steps
        .iter()
        .map(|s| format!(r#"<li class="{}">{}{}</li>"#, if s.done { "done" } else { "" }, if s.done { CHECK } else { "" }, s.title))
        .collect();
    format!(
        r#"<section class="start" aria-label="Getting started"><div class="s-head"><b>Getting started · {done} of 3 done</b><div class="bar" aria-hidden="true"><i style="--p:{pct}%"></i></div><span class="s-next hide-sm">Next: {next_title}</span><a class="ghost line" href="{href}" data-step="{key}">Continue{ARROW}</a><button class="m-x" type="button" data-step="x" aria-label="Hide Getting started" title="Hide">{X}</button></div><ol class="s-steps">{marks}</ol></section>"#,
        pct = done * 100 / 3,
        next_title = next.title.to_lowercase(),
        href = esc(&next.href),
        key = next.key,
        X = super::super::icons::X,
    )
}

struct Idea<'a> {
    icon: &'a str,
    title: &'a str,
    text: String,
    go: &'a str,
    href: String,
    on: String,
}

fn idea(i: &Idea<'_>) -> String {
    let on = if i.on.is_empty() { String::new() } else { format!(r#"<span class="t-on">{CHECK}{}</span>"#, esc(&i.on)) };
    let go = if i.href.is_empty() {
        format!(r#"<button class="t-go" type="button" data-add>{}{ARROW}</button>"#, i.go)
    } else {
        let ext = if i.href.starts_with("http") { r#" target="_blank" rel="noopener""# } else { "" };
        format!(r#"<a class="t-go" href="{}"{ext}>{}{ARROW}</a>"#, esc(&i.href), i.go)
    };
    format!(
        r#"<li><span class="w-ic">{}</span><div><b>{}</b><p>{}</p></div><div class="i-go">{on}{go}</div></li>"#,
        i.icon, i.title, i.text
    )
}

/// What else the grid does (#2): folded at the end, every idea still one click from where it happens.
pub(super) fn ideas(p: &HomePage, host: &str) -> String {
    let rows: String = idea_list(p, host).iter().map(idea).collect();
    format!(r#"<details class="ideas" id="ideas"><summary>{CLOUD}What else can I do?</summary><ul>{rows}</ul></details>"#)
}

fn idea_list(p: &HomePage, host: &str) -> Vec<Idea<'static>> {
    let s = p.sites.len();
    let scheduled = p.instance.next_wake_at.is_some_and(|t| t != 0);
    let mut list = vec![
        Idea {
            icon: MOON,
            title: "Keeps working with your laptop closed",
            text: "Leave a task running or schedule one. Your cloud wisp wakes for it, does the work and goes back to sleep.".into(),
            go: "Schedule a task",
            href: wisp_settings("automations"),
            on: if scheduled { "Scheduled".into() } else { String::new() },
        },
        Idea {
            icon: KEY,
            title: "Bring your own agents",
            text: "Sign in to Claude Code, Codex and GitHub in Settings → Agents. A link and a code, no terminal.".into(),
            go: "Sign in to an agent",
            href: wisp_settings("agents"),
            on: String::new(),
        },
    ];
    if p.offers.sites {
        list.push(Idea {
            icon: GLOBE,
            title: "Publish an app",
            text: "From My Stuff, an app an agent built goes up at a public link, and stays up while your wisp sleeps.".into(),
            go: "Open My Stuff",
            href: wisp_at("/view/stuff.page"),
            on: if s > 0 { format!("{s} published") } else { String::new() },
        });
    }
    if p.offers.sandboxes {
        list.push(Idea {
            icon: STACK,
            title: "A fresh box for one task",
            text: "Start a task with /sandbox and it gets a box of its own, deleted with the task.".into(),
            go: "Try /sandbox",
            href: "/go".into(),
            on: String::new(),
        });
    }
    list.extend(reach_ideas(p, host));
    list
}

/// Reaching it all: the phone, shared tasks, your own computers, scripts.
fn reach_ideas(p: &HomePage, host: &str) -> [Idea<'static>; 4] {
    let n = p.computers.len();
    let phone = p.devices.iter().any(|d| d.name.to_lowercase().contains("phone"));
    [
        Idea {
            icon: PHONE,
            title: "On your phone",
            text: format!("Sign in with {} in the wisp app, or add your cloud wisp to an iPhone’s Home Screen.", esc(host)),
            go: "How to set it up",
            href: "https://trywisp.dev/guides/use-wisp-cloud".into(),
            on: if phone { "Phone connected".into() } else { String::new() },
        },
        Idea {
            icon: SHARE,
            title: "Share a task",
            text: "Share… on a task makes a link to what it did, with a passkey if you want one.".into(),
            go: "Pick a task",
            href: "/go".into(),
            on: String::new(),
        },
        Idea {
            icon: LAPTOP,
            title: "Reach your own computers",
            text: "Your phone and other computers open them through the grid. No SSH, no open ports.".into(),
            go: "Add a computer",
            href: String::new(),
            on: if n > 0 { format!("{n} added") } else { String::new() },
        },
        Idea {
            icon: CODE,
            title: "API keys for scripts",
            text: "A key lets a script start tasks in your cloud wisp, and nothing else.".into(),
            go: "Make a key",
            href: "/account#keys".into(),
            on: String::new(),
        },
    ]
}
