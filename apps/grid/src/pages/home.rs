//! Home (`/`), signed in (docs/ux/2026-11-18-grid.md #1–#5): one list of your machines (the cloud wisp first, then
//! your own computers), each a row with Open and the rest in its ⋯; one + Add for a computer or another cloud machine;
//! your links (published apps and shared tasks); and, while it's new, a Getting-started checklist. What else the grid
//! does sits folded at the end, each idea linking to the exact place in the wisp (`wisp_at`).

use super::account::DeviceView;
use super::icons::{ARROW, CLOUD, CODE, COPY, LAPTOP, MOON, PLUS};
use super::layout::{AppPage, Tab, app_page, state_label};
use super::machines::{MachineView, add_machine_row, machines_dialog, machines_js, spec};
use super::scripts::{COPY_JS, HOME_JS, LIVE};
use super::time::{ago, locale_string};
use super::{PageCfg, PageUser, Tone, esc};
use crate::http::encode_component;

#[path = "home_links.rs"]
mod links;
pub use links::{ShareView, Start};

/// The account's cloud wisp, as its instance row has it.
#[derive(Debug, Clone, Default)]
pub struct InstanceView {
    /// `running`, `queued`, `starting`, `stopping`, `stopped`
    pub state: String,
    pub started_at: Option<i64>,
    pub stopped_at: Option<i64>,
    pub next_wake_at: Option<i64>,
}

/// One of your own computers on the grid.
#[derive(Debug, Clone, Default)]
pub struct ComputerView {
    pub id: String,
    pub device_id: String,
    pub name: String,
    pub url: String,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

/// One published app.
#[derive(Debug, Clone, Default)]
pub struct SiteView {
    pub key: String,
    pub title: String,
    pub url: String,
    pub files: u32,
    pub updated_at: Option<i64>,
}

/// What this grid offers (an idea goes when it's off).
#[derive(Debug, Clone, Copy)]
pub struct Offers {
    pub sites: bool,
    pub sandboxes: bool,
}

impl Default for Offers {
    fn default() -> Self {
        Self { sites: true, sandboxes: true }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HomePage {
    pub user: PageUser,
    /// `cfg.instance_url(handle)`
    pub url: String,
    pub instance: InstanceView,
    pub machine: MachineView,
    pub devices: Vec<DeviceView>,
    pub computers: Vec<ComputerView>,
    pub sites: Vec<SiteView>,
    /// the shares that still open (not revoked), newest first
    pub shares: Vec<ShareView>,
    pub offers: Offers,
    /// the Getting-started steps you took (`wisp_start`, set by the page itself)
    pub start: Start,
    pub csrf: String,
    pub flash: String,
    pub tone: Tone,
    pub now_ms: i64,
}

pub(super) fn hidden(csrf: &str) -> String {
    format!(r#"<input type="hidden" name="csrf" value="{}">"#, esc(csrf))
}

pub(super) fn flash(msg: &str, tone: Tone) -> String {
    if msg.is_empty() { String::new() } else { format!(r#"<div class="flash {}">{}</div>"#, tone.class(), esc(msg)) }
}

pub(super) fn bare(u: &str) -> &str {
    u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(u)
}

fn host_of(url: &str) -> &str {
    let b = bare(url);
    b.split('/').next().unwrap_or(b)
}

pub(super) fn copy_btn(v: &str, label: Option<&str>) -> String {
    let v = esc(v);
    format!(
        r#"<button class="copy" type="button" data-copy="{v}" title="Copy {v}" aria-label="Copy {v}"><span>{}</span>{COPY}</button>"#,
        label.map_or_else(|| v.clone(), esc)
    )
}

/// A ⋯ menu's Copy: says Copied for a moment (`COPY_JS`), the address under it.
pub(super) fn copy_item(v: &str, label: &str) -> String {
    format!(
        r#"<button class="mi" type="button" data-copy="{v}">{COPY}<div><span>{label}</span><small>{shown}</small></div></button>"#,
        v = esc(v),
        shown = esc(bare(v).trim_end_matches('/')),
    )
}

/// Where in the wisp a link lands (#3): `/go` hands the browser to the cloud wisp, then on to `path` there.
pub fn wisp_at(path: &str) -> String {
    format!("/go?path={}", encode_component(path))
}

/// A Settings pane of the cloud wisp (its navigation URL: the settings sheet as an overlay, `nav-url.ts`).
pub fn wisp_settings(pane: &str) -> String {
    wisp_at(&format!("/?o={}", encode_component(&format!(r#"viewer:shell-settings.sheet:{{"pane":"{pane}"}}"#))))
}

/// A row's ⋯: a native disclosure, so it works without script; `HOME_JS` closes it on a click elsewhere or Escape.
pub(super) fn more_menu(label: &str, items: &str) -> String {
    format!(
        r#"<details class="pop"><summary class="more" aria-label="More for {label}" title="More">⋯</summary><div class="menu" role="menu">{items}</div></details>"#,
        label = esc(label)
    )
}

/// What the cloud wisp's state means now, short enough for one row.
fn cloud_line(i: &InstanceView, now: i64) -> String {
    match i.state.as_str() {
        "running" => format!("started {}", ago(i.started_at, now)),
        "starting" => "a few seconds: it opens as soon as it’s up".into(),
        "queued" => "the grid is busy: it starts as soon as there’s room".into(),
        "stopping" => "your tasks and files stay where they are".into(),
        _ if i.stopped_at.is_some_and(|t| t != 0) => format!("since {} · opens in a few seconds", ago(i.stopped_at, now)),
        _ => "starts the first time you open it".into(),
    }
}

fn cloud_row(p: &HomePage, hidden: &str) -> String {
    let st = p.instance.state.as_str();
    let label = state_label(st).map_or_else(|| esc(st), str::to_owned);
    let next = p.instance.next_wake_at.filter(|t| *t != 0);
    let sched = next.map_or_else(String::new, |t| format!(" · next schedule {}", esc(&locale_string(t))));
    let sleep = if st == "running" {
        format!(
            r#"<form method="post" action="/instance/stop">{hidden}<button class="mi">{MOON}<div><span>Sleep now</span><small>Opening it wakes it</small></div></button></form>"#
        )
    } else {
        String::new()
    };
    let items = format!(
        r#"{copy}{sleep}<a class="mi" href="{agents}">{CODE}<div><span>Settings → Agents</span><small>Sign in to Claude Code, Codex, GitHub</small></div></a><a class="mi" href="/account#keys">{KEY_ICON}<div><span>API keys</span><small>For scripts</small></div></a><div class="mi note"><small>Machine: {spec}</small><small>Next schedule: {when}</small></div>"#,
        copy = copy_item(&p.url, "Copy address"),
        agents = esc(&wisp_settings("agents")),
        KEY_ICON = super::icons::KEY,
        spec = esc(&spec(&p.machine, false)),
        when = next.map_or_else(|| "nothing scheduled".to_owned(), |t| esc(&locale_string(t))),
    );
    format!(
        r#"<div class="mrow" data-row="cloud"><span class="w-ic cloud">{CLOUD}</span><div class="m-tx"><b>Cloud wisp</b><small class="st"><span class="dot {cls}"></span>{label} · {line}{sched}</small></div><div class="m-acts"><a class="primary" href="/go">Open{ARROW}</a>{more}</div></div>"#,
        cls = esc(st),
        line = esc(&cloud_line(&p.instance, p.now_ms)),
        more = more_menu("your cloud wisp", &items),
    )
}

fn computer_row(c: &ComputerView, hidden: &str, now: i64) -> String {
    let status = if c.online {
        r#"<span class="dot running"></span>Online"#.to_owned()
    } else if c.last_seen_at.is_some_and(|t| t != 0) {
        format!(r#"<span class="dot"></span>Offline · last seen {}"#, ago(c.last_seen_at, now))
    } else {
        r#"<span class="dot"></span>Offline · never connected · <button class="lnk" type="button" data-add>How to connect</button>"#
            .to_owned()
    };
    // offline, Open still goes: the computer's page waits for it and opens it once it connects (#7)
    let (cls, title) = if c.online { ("primary", "") } else { ("ghost line", r#" title="It opens as soon as that computer connects""#) };
    let items = format!(
        r#"{copy}<form method="post" action="/computers/{id}/remove" data-confirm="Remove it?">{hidden}<button class="mi danger" title="Take it off the grid. wisp on it keeps running.">{X}<div><span>Remove</span><small>wisp on it keeps running</small></div></button></form>"#,
        copy = copy_item(&c.url, "Copy address"),
        id = esc(&c.id),
        X = super::icons::TRASH,
    );
    format!(
        r#"<div class="mrow" data-row="computer"><span class="w-ic">{LAPTOP}</span><div class="m-tx"><b title="{name}">{name}</b><small class="st">{status}</small></div><div class="m-acts"><a class="{cls}" href="/go?computer={id}"{title}>Open{ARROW}</a>{more}</div></div>"#,
        name = esc(&c.name),
        id = esc(&c.id),
        more = more_menu(&c.name, &items),
    )
}

/// No computer yet: one row saying what it'd be, and the way to add one.
fn no_computer_row() -> String {
    format!(
        r#"<div class="mrow empty"><span class="w-ic">{LAPTOP}</span><div class="m-tx"><b>No computers of yours yet</b><small>wisp on your desktop or laptop, reached from anywhere. No SSH, no open ports.</small></div><div class="m-acts"><button class="ghost line" type="button" data-add>{PLUS}Add a computer</button></div></div>"#
    )
}

/// How a computer joins: the steps v1's "Add a computer" card unfolded, opened by `[data-add]` (`HOME_JS`).
fn add_how(cli: &str, host: &str) -> String {
    format!(
        r#"<details class="add" id="add"><summary aria-controls="addHow">Add a computer</summary></details>
        <div class="how" id="addHow">
          <div class="how-head"><b>Add a computer of yours</b><button class="m-x" type="button" data-add-close aria-label="Close">{X}</button></div>
          <input type="radio" name="addHow" id="addCli" checked><input type="radio" name="addHow" id="addApp">
          <div class="seg" role="presentation"><label for="addCli">{CODE}In a terminal</label><label for="addApp">{LAPTOP}With the app</label></div>
          <ol class="steps p-cli">
            <li><span class="n">1</span><div><b>Run this on that computer</b><p>Node 20 or later. It starts wisp in the background if it isn’t running. Have wisp already? <code>wisp computer on</code> does the same.</p>{cli}</div></li>
            <li><span class="n">2</span><div><b>Allow it here</b><p>It prints a code and opens a page from this grid; on a server, open the link it prints in any browser, even your phone’s. Check the code matches and press <i>Allow</i>. The computer then shows up in this list, and in the phone app.</p></div></li>
          </ol>
          <ol class="steps p-app">
            <li><span class="n">1</span><div><b>Run wisp on that computer</b><p>The desktop app from <a href="https://trywisp.dev" target="_blank" rel="noopener">trywisp.dev</a>, or <code>npx wisp-agents</code> in a terminal.</p></div></li>
            <li><span class="n">2</span><div><b>Open Machines → Reach this computer from anywhere</b><p>Type this grid’s address and press <i>Sign in and add it</i>:</p>{host}</div></li>
            <li><span class="n">3</span><div><b>Allow it here</b><p>A page from this grid opens with a code. Check it matches and press <i>Allow</i>. The computer then shows up in this list, and in the phone app.</p></div></li>
          </ol>
        </div>"#,
        X = super::icons::X,
        cli = copy_btn(cli, None),
        host = copy_btn(host, None),
    )
}

/// `wisp computer on` signs in to grid.trywisp.dev unless told another grid.
fn cli_for(host: &str) -> String {
    if host == "grid.trywisp.dev" { "npx wisp-agents computer on".to_owned() } else { format!("npx wisp-agents computer on --grid {host}") }
}

/// + Add (#4): a computer of yours, or another cloud machine (the waitlist dialog, unchanged).
fn add_menu(p: &HomePage) -> String {
    format!(
        r#"<details class="pop add-pop"><summary class="ghost line">{PLUS}Add</summary><div class="menu" role="menu"><button class="mi" type="button" data-add>{LAPTOP}<div><span>A computer of yours</span><small>One command, or a few clicks in the app</small></div></button><div data-live="wish">{wish}</div></div></details>"#,
        wish = add_machine_row(p.machine.wish.as_ref()),
    )
}

pub fn home_page(cfg: &PageCfg, p: &HomePage) -> String {
    let hidden = hidden(&p.csrf);
    let host = host_of(&cfg.public_url);
    let first = p.user.name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&p.user.handle);
    let first = first.split(' ').next().unwrap_or(first);
    let up = usize::from(p.instance.state == "running") + p.computers.iter().filter(|c| c.online).count();
    let rows = if p.computers.is_empty() {
        format!("{}{}", cloud_row(p, &hidden), no_computer_row())
    } else {
        let cs: String = p.computers.iter().map(|c| computer_row(c, &hidden, p.now_ms)).collect();
        format!("{}{cs}", cloud_row(p, &hidden))
    };
    let body = format!(
        r#"{flash}
      <h1 class="h1 hi" id="hi">Hi {first}</h1>
      <section class="block" aria-labelledby="mach">
        <div class="sec-row"><h2 class="sec" id="mach">Your machines <span class="n" data-live="online">{up} of {all} up</span></h2>{add}</div>
        <div class="ml" data-live="machines">{rows}</div>
        {how}
      </section>
      <div data-live="links">{links}</div>
      <div data-live="start">{start}</div>
      {ideas}
      {dialog}
      {COPY_JS}{machines_js}{HOME_JS}{LIVE}"#,
        flash = flash(&p.flash, p.tone),
        first = esc(first),
        all = 1 + p.computers.len(),
        add = add_menu(p),
        how = add_how(&cli_for(host), host),
        links = links::links(p, &hidden),
        start = links::checklist(p),
        ideas = links::ideas(p, host),
        dialog = machines_dialog(&p.machine, &p.user, &hidden),
        machines_js = machines_js(),
    );
    app_page(&AppPage { user: Some(&p.user), csrf: &p.csrf, tab: Tab::Home, ..AppPage::new(cfg, "Home", &body) })
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
