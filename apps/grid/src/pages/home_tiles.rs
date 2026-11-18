//! Home's "What your grid can do": what to try next, each tile saying so when the grid knows it's already done.
use super::super::esc;
use super::super::icons::{ARROW, CHECK, CODE, GLOBE, KEY, LAPTOP, MOON, PHONE, SHARE, STACK};
use super::super::site::shot;
use super::HomePage;

struct Tile<'a> {
    icon: &'a str,
    title: &'a str,
    text: String,
    on: String,
    go: &'a str,
    href: &'a str,
    add: bool,
}

fn tile(t: &Tile<'_>) -> String {
    let step = if t.add {
        format!(r#"<button class="t-go" type="button" data-add>{}{ARROW}</button>"#, t.go)
    } else {
        let ext = if t.href.starts_with("http:") || t.href.starts_with("https:") { r#" target="_blank" rel="noopener""# } else { "" };
        format!(r#"<a class="t-go" href="{}"{ext}>{}{ARROW}</a>"#, esc(t.href), t.go)
    };
    let on = if t.on.is_empty() { String::new() } else { format!(r#"<span class="t-on">{CHECK}{}</span>"#, esc(&t.on)) };
    format!(
        r#"<div class="t"><span class="w-ic">{}</span><b>{}</b><p>{}</p><div class="t-foot">{step}{on}</div></div>"#,
        t.icon, t.title, t.text
    )
}

pub(super) fn tiles(p: &HomePage, host: &str) -> String {
    let mut list = offered_tiles(p);
    list.extend(reach_tiles(p, host));
    format!("{}{}", big_tile(p), list.join(""))
}

/// The wide one: a cloud wisp keeps working with the laptop closed.
fn big_tile(p: &HomePage) -> String {
    let scheduled = if p.instance.next_wake_at.is_some_and(|t| t != 0) {
        format!(r#"<span class="t-on">{CHECK}Scheduled</span>"#)
    } else {
        String::new()
    };
    format!(
        r#"<div class="t big"><div class="t-tx"><span class="w-ic cloud">{MOON}</span><b>Keeps working with your laptop closed</b><p>Leave a task running or schedule one. Your cloud wisp wakes for it, does the work and goes back to sleep. The report waits for you.</p><div class="t-foot"><a class="t-go" href="/go">Schedule a task{ARROW}</a>{scheduled}</div></div><div class="t-shot" aria-hidden="true">{}</div></div>"#,
        shot("report", "")
    )
}

/// What the cloud wisp offers: agents, and publishing and sandboxes where this grid has them.
fn offered_tiles(p: &HomePage) -> Vec<String> {
    let s = p.sites.len();
    let mut list = vec![tile(&Tile {
        icon: KEY,
        title: "Bring your own agents",
        text: "Sign in to Claude Code, Codex and GitHub in Settings → Agents. A link and a code, no terminal.".into(),
        on: String::new(),
        go: "Sign in to an agent",
        href: "/go",
        add: false,
    })];
    if p.offers.sites {
        list.push(tile(&Tile {
            icon: GLOBE,
            title: "Publish an app",
            text: "From My Stuff, an app an agent built goes up at a public link, and stays up while your wisp sleeps.".into(),
            on: if s > 0 { format!("{s} published") } else { String::new() },
            go: if s > 0 { "See your apps" } else { "Publish from My Stuff" },
            href: if s > 0 { "#apps" } else { "/go" },
            add: false,
        }));
    }
    if p.offers.sandboxes {
        list.push(tile(&Tile {
            icon: STACK,
            title: "A fresh box for one task",
            text: "Start a task with /sandbox and it gets a box of its own, deleted with the task.".into(),
            on: String::new(),
            go: "Try /sandbox",
            href: "/go",
            add: false,
        }));
    }
    list
}

/// Reaching it all: the phone, shared tasks, your own computers, scripts.
fn reach_tiles(p: &HomePage, host: &str) -> Vec<String> {
    let phone = p.devices.iter().any(|d| d.name.to_lowercase().contains("phone"));
    let n = p.computers.len();
    let mut list = Vec::new();
    list.push(tile(&Tile {
        icon: PHONE,
        title: "On your phone",
        text: format!("Sign in with {} in the wisp app, or add your cloud wisp to an iPhone’s Home Screen.", esc(host)),
        on: if phone { "Phone connected".into() } else { String::new() },
        go: "How to set it up",
        href: "https://trywisp.dev/guides/use-wisp-cloud",
        add: false,
    }));
    list.push(tile(&Tile {
        icon: SHARE,
        title: "Share a task",
        text: "Share… on a task makes a link to what it did, with a passkey if you want one.".into(),
        on: String::new(),
        go: "Pick a task",
        href: "/go",
        add: false,
    }));
    list.push(tile(&Tile {
        icon: LAPTOP,
        title: "Reach your own computers",
        text: "Your phone and other computers open them through the grid. No SSH, no open ports.".into(),
        on: if n > 0 { format!("{n} added") } else { String::new() },
        go: "Add a computer",
        href: "",
        add: true,
    }));
    list.push(tile(&Tile {
        icon: CODE,
        title: "API keys for scripts",
        text: "Settings → Cloud → API keys makes a key, so a script can start tasks in your cloud wisp.".into(),
        on: String::new(),
        go: "Make a key",
        href: "/go",
        add: false,
    }));
    list
}
