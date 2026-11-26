//! Connecting a device (`/activate`: type the code, then Allow), and the plain pages: a message
//! (`Nothing here`, `Device connected`, errors) and the one shown while a sleeping wisp wakes.

use super::layout::{AppPage, Tab, app_page};
use super::{PageCfg, PageUser, Tone, esc};
use crate::http::encode_component;

/// Opens the Android app (its intent filter in mobile/android/…/AndroidManifest.xml).
pub const APP_URL: &str = "dev.trywisp.mobile://signed-in";

/// The device asking to be let in.
#[derive(Debug, Clone, Default)]
pub struct DeviceAsk {
    pub name: String,
    pub user_code: String,
}

#[derive(Debug, Clone, Default)]
pub struct ActivatePage {
    pub code: String,
    /// the code matched a device that's waiting
    pub device: Option<DeviceAsk>,
    /// from the Android app: Allow sends it back there
    pub app: bool,
    pub csrf: String,
    pub error: String,
}

pub fn activate_page(cfg: &PageCfg, user: &PageUser, p: &ActivatePage) -> String {
    let app = if p.app { r#"<input type="hidden" name="app" value="android">"# } else { "" };
    let csrf = esc(&p.csrf);
    let body = match &p.device {
        None => {
            let error = if p.error.is_empty() { String::new() } else { format!(r#"<div class="flash bad">{}</div>"#, esc(&p.error)) };
            format!(
                r#"<h1 class="h1">Connect a device</h1><p class="sub">Type the code wisp shows on your computer or phone.</p>{error}
        <form method="get" action="/activate" style="margin-top:20px;display:flex;flex-direction:column;gap:10px"><input class="input code" name="code" value="{}" placeholder="ABCD-EFGH" autocomplete="off" autocapitalize="characters" autofocus required>{app}
        <button class="primary tall">Continue</button></form>"#,
                esc(&p.code)
            )
        }
        Some(d) => {
            let back = format!("/activate?code={}{}", d.user_code, if p.app { "&app=android" } else { "" });
            format!(
                r#"<h1 class="h1">Let this device in?</h1>
      <p class="sub"><b>{name}</b> asks to use your wisp as <b>{handle}</b>. Allow it only if you just started this on your own computer or phone, and the code matches.</p>
      <div class="code">{code}</div>
      <form method="post" action="/activate" style="display:flex;gap:10px">
        <input type="hidden" name="csrf" value="{csrf}"><input type="hidden" name="code" value="{code}">{app}
        <button class="ghost line tall" style="flex:1;justify-content:center" name="answer" value="deny">Don’t allow</button>
        <button class="primary tall" style="flex:1" name="answer" value="allow">Allow</button>
      </form>
      <form method="post" action="/signout?return={ret}" style="margin-top:16px;text-align:center">
        <input type="hidden" name="csrf" value="{csrf}"><span class="sub">Not {handle}?</span> <button class="ghost">Use another account</button>
      </form>"#,
                name = esc(&d.name),
                handle = esc(&user.handle),
                code = esc(&d.user_code),
                ret = encode_component(&back),
            )
        }
    };
    app_page(&AppPage { user: Some(user), csrf: &p.csrf, narrow: true, tab: Tab::Neither, ..AppPage::new(cfg, "Connect a device", &body) })
}

/// A page that says one thing, with a link onwards.
#[derive(Debug, Clone, Default)]
pub struct MessagePage {
    pub title: String,
    pub text: String,
    /// (href, label)
    pub link: Option<(String, String)>,
    pub tone: Tone,
    /// signed in to the grid: its header, as on Home (QA round 6 #4)
    pub user: Option<PageUser>,
}

pub fn message_page(cfg: &PageCfg, p: &MessagePage) -> String {
    let text = if p.tone == Tone::Plain {
        format!(r#"<p class="sub">{}</p>"#, esc(&p.text))
    } else {
        format!(r#"<div class="flash {}">{}</div>"#, p.tone.class(), esc(&p.text))
    };
    let link = p.link.as_ref().map_or_else(String::new, |(href, label)| {
        format!(r#"<p style="margin-top:20px"><a class="primary tall" href="{}">{}</a></p>"#, esc(href), esc(label))
    });
    let body = format!(r#"<h1 class="h1">{}</h1>{text}{link}"#, esc(&p.title));
    app_page(&AppPage { narrow: true, ..AppPage::new(cfg, &p.title, &body) }.signed(p.user.as_ref()))
}

/// A published app's address with nothing behind it (v1 server.js:1322).
pub fn site_not_found(cfg: &PageCfg) -> String {
    message_page(
        cfg,
        &MessagePage {
            title: "Nothing here".into(),
            text: "This app was unpublished, or its link has a typo.".into(),
            link: Some((cfg.public_url.clone(), "Home".into())),
            tone: Tone::Plain,
            user: None,
        },
    )
}

/// A computer of yours that isn't connected (#7), at its own address.
#[derive(Debug, Clone, Default)]
pub struct OfflinePage {
    pub name: String,
    pub last_seen_at: Option<i64>,
    pub now_ms: i64,
    /// `/go` on the grid: your cloud wisp, to keep working meanwhile
    pub cloud: String,
    /// the grid's home
    pub home: String,
    /// its owner, signed in at this address: the grid's header, linking to the grid
    pub user: Option<PageUser>,
}

/// Not a dead end: when it was last seen, a wait that opens it by itself once it connects (its address, `/_grid/offline`,
/// polled, says `{online: true}`; then a reload goes on to what was asked for), the cloud wisp to keep working meanwhile, and what to do on that computer.
pub fn offline_page(cfg: &PageCfg, p: &OfflinePage) -> String {
    let seen = match p.last_seen_at.filter(|t| *t != 0) {
        Some(_) => format!("Last seen {}. ", super::time::ago(p.last_seen_at, p.now_ms)),
        None => "It hasn’t connected yet. ".into(),
    };
    let body = format!(
        r#"<h1 class="h1">{name} is offline</h1><p class="sub">{seen}wisp on {name} comes back when that computer is on and wisp runs there.</p>
      <div class="wait-row" role="status"><span class="dot starting"></span><span id="msg">Waiting for it… this page opens it as soon as it connects.</span></div>
      <p class="acts"><a class="primary tall" href="{cloud}">Open your cloud wisp instead</a><a class="ghost line tall" href="{home}">Home</a></p>
      <details class="fold"><summary>On that computer?</summary><p class="sub">Start wisp there, or run <code>wisp computer on</code> in a terminal.</p></details>
      <script>(async function poll(n){{try{{const r=await fetch(location.href,{{cache:'no-store',headers:{{accept:'application/json'}}}});const j=await r.json();if(j.online)return location.reload()}}catch{{}}setTimeout(()=>poll(n+1),n<40?3000:10000)}})(0)</script>"#,
        name = esc(&p.name),
        cloud = esc(&p.cloud),
        home = esc(&p.home),
    );
    let title = format!("{} is offline", p.name);
    app_page(&AppPage { narrow: true, home: &p.home, ..AppPage::new(cfg, &title, &body) }.signed(p.user.as_ref()))
}

/// What the page at a box's address says while its wisp isn't up.
#[derive(Debug, Clone, Default)]
pub struct WakingPage {
    /// `starting`, `queued` or `failed`
    pub state: String,
    /// its place in line while queued (1: next)
    pub position: Option<usize>,
    /// why its start didn't work (`failed`)
    pub error: String,
    /// the grid's home
    pub home: String,
}

const WAKING: (&str, &str) = ("Waking your wisp", "It was asleep. A few seconds: this page opens it as soon as it’s up.");
const QUEUED: &str = "Waiting for room";
const FAILED: (&str, &str) = ("Your wisp didn’t start", "It stopped before it opened.");

/// "You’re next in line" / "You’re number 3 in line", and what happens then.
fn in_line(at: usize) -> String {
    let place = if at <= 1 { "You’re next in line".to_owned() } else { format!("You’re number {at} in line") };
    format!("The grid is busy right now. {place}: this page opens your wisp as soon as there’s room.")
}

/// Shown at a box's address the moment a browser opens it asleep, while it starts in the background: never a blank
/// tab. It follows `/_grid/state` (starting, its place in line, or why it didn't start) and reloads to what was asked
/// for once it's running. A failed start says why with Try again (a reload starts it again and waits again), and
/// Home; never a dead end.
pub fn waking_page(cfg: &PageCfg, p: &WakingPage) -> String {
    let failed = p.state == "failed";
    let (title, text) = match p.state.as_str() {
        "queued" => (QUEUED.to_owned(), in_line(p.position.unwrap_or(1))),
        "failed" => (FAILED.0.to_owned(), if p.error.is_empty() { FAILED.1.to_owned() } else { p.error.clone() }),
        _ => (WAKING.0.to_owned(), WAKING.1.to_owned()),
    };
    let js = |s: &str| serde_json::to_string(s).unwrap_or_default();
    let line = in_line(2);
    let (before, after) = line.split_once("number 2").unwrap_or((&line, ""));
    let next = in_line(1);
    let body = format!(
        r#"<div class="center" style="padding-top:48px"><div class="orb"></div><h1 class="h1" id="t">{title}</h1><p class="sub" id="msg" role="status">{text}</p>
      <p class="acts" id="acts" style="justify-content:center"{hidden}><button class="primary tall" id="again" type="button">Try again</button><a class="ghost line tall" href="{home}">Home</a></p></div>
      <script>(function(){{const t=document.getElementById('t'),m=document.getElementById('msg'),a=document.getElementById('acts');document.getElementById('again').onclick=()=>location.reload();
const say=(h,s,f)=>{{t.textContent=h;m.textContent=s;a.hidden=!f}};
async function poll(n){{let j={{}};try{{const r=await fetch('/_grid/state',{{cache:'no-store'}});j=await r.json()}}catch{{}}
if(j.state==='running')return location.reload();
if(j.state==='queued')say({qt},j.position>1?{qb}+'number '+j.position+{qa}:{qn},false);
else if(j.state==='stopped'&&(j.error||n>3))return say({ft},j.error||{fb},true);
else if(j.state==='starting')say({wt},{wb},false);
setTimeout(()=>poll(n+1),n<20?700:2000)}}{kick}}})()</script>"#,
        title = esc(&title),
        text = esc(&text),
        hidden = if failed { "" } else { " hidden" },
        home = esc(&p.home),
        qt = js(QUEUED),
        qb = js(before),
        qa = js(after),
        qn = js(&next),
        ft = js(FAILED.0),
        fb = js(FAILED.1),
        wt = js(WAKING.0),
        wb = js(WAKING.1),
        // a page that says it failed waits for Try again
        kick = if failed { "" } else { "poll(0)" },
    );
    app_page(&AppPage { narrow: true, ..AppPage::new(cfg, &title, &body) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PageCfg {
        PageCfg { name: "wisp Grid".into(), domain: "grid.test".into(), public_url: "https://grid.test".into() }
    }

    #[test]
    fn a_code_to_type_then_allow() {
        let u = PageUser { handle: "ada".into(), ..PageUser::default() };
        let ask = activate_page(&cfg(), &u, &ActivatePage { code: "<x>".into(), ..ActivatePage::default() });
        assert!(ask.contains("Type the code wisp shows"));
        assert!(ask.contains(r#"value="&#60;x&#62;""#));
        let allow = activate_page(
            &cfg(),
            &u,
            &ActivatePage {
                device: Some(DeviceAsk { name: "Pixel".into(), user_code: "ABCD-EFGH".into() }),
                app: true,
                csrf: "t".into(),
                ..ActivatePage::default()
            },
        );
        assert!(allow.contains("<b>Pixel</b> asks to use your wisp as <b>ada</b>"));
        assert!(allow.contains("/signout?return=%2Factivate%3Fcode%3DABCD-EFGH%26app%3Dandroid"));
    }

    #[test]
    fn an_offline_computer_waits_and_offers_a_way_on() {
        let p = OfflinePage {
            name: "Studio".into(),
            last_seen_at: Some(1_000),
            now_ms: 1_000 + 7_200_000,
            cloud: "https://grid.test/go".into(),
            home: "https://grid.test".into(),
            user: None,
        };
        let h = offline_page(&cfg(), &p);
        assert!(h.contains("Studio is offline") && h.contains("Last seen 2 h ago."));
        assert!(h.contains("this page opens it as soon as it connects") && h.contains("if(j.online)return location.reload()"));
        assert!(h.contains(r#"href="https://grid.test/go">Open your cloud wisp instead"#));
        let never = offline_page(&cfg(), &OfflinePage { name: "New".into(), ..OfflinePage::default() });
        assert!(never.contains("It hasn’t connected yet."));
    }

    #[test]
    fn a_signed_in_message_page_has_the_grid_s_header_and_one_elsewhere_links_home() {
        let u = PageUser { handle: "ada".into(), ..PageUser::default() };
        let m = MessagePage { title: "Nothing here".into(), text: "No.".into(), user: Some(u.clone()), ..MessagePage::default() };
        assert!(message_page(&cfg(), &m).contains(r#"<a href="/">Home</a><a href="/account">Account</a>"#));
        assert!(!message_page(&cfg(), &MessagePage { user: None, ..m }).contains(r#"<a href="/account">"#));
        // the offline page sits at the computer's own address: its header links to the grid's
        let p = OfflinePage {
            name: "Studio".into(),
            user: Some(u),
            cloud: "https://grid.test/go".into(),
            home: "https://grid.test".into(),
            ..OfflinePage::default()
        };
        let h = offline_page(&cfg(), &p);
        assert!(h.contains(r#"<a href="https://grid.test/">Home</a><a href="https://grid.test/account">Account</a>"#), "{h}");
    }

    #[test]
    fn a_bad_message_is_a_flash() {
        let p = message_page(
            &cfg(),
            &MessagePage {
                title: "Slow down".into(),
                text: "Too many".into(),
                link: Some(("/".into(), "Back".into())),
                tone: Tone::Bad,
                user: None,
            },
        );
        assert!(p.contains(r#"<div class="flash bad">Too many</div>"#));
        assert!(p.contains(r#"<a class="primary tall" href="/">Back</a>"#));
        assert!(site_not_found(&cfg()).contains("This app was unpublished"));
        assert!(waking_page(&cfg(), &WakingPage::default()).contains("It was asleep. A few seconds"));
    }

    /// The waiting page says where it stands now: starting, its place in line (grid-r2-1: `Queued` is real now), or
    /// why it didn't start with Try again, never a dead end.
    #[test]
    fn the_waiting_page_says_starting_its_place_in_line_or_why_with_try_again() {
        let home = "https://grid.test".to_owned();
        let starting = waking_page(&cfg(), &WakingPage { state: "starting".into(), home: home.clone(), ..WakingPage::default() });
        assert!(starting.contains(r#"<h1 class="h1" id="t">Waking your wisp</h1>"#), "{starting}");
        assert!(starting.contains(r#"id="acts" style="justify-content:center" hidden>"#), "no Try again while it starts");
        assert!(starting.contains("poll(0)") && starting.contains("fetch('/_grid/state'"));
        let queued =
            waking_page(&cfg(), &WakingPage { state: "queued".into(), position: Some(3), home: home.clone(), ..WakingPage::default() });
        assert!(queued.contains("Waiting for room") && queued.contains("You’re number 3 in line: this page opens your wisp"), "{queued}");
        let next =
            waking_page(&cfg(), &WakingPage { state: "queued".into(), position: Some(1), home: home.clone(), ..WakingPage::default() });
        assert!(next.contains("You’re next in line"));
        let failed = waking_page(
            &cfg(),
            &WakingPage { state: "failed".into(), error: "wisp didn't start in the box: <gone>".into(), home, ..WakingPage::default() },
        );
        assert!(
            failed.contains("Your wisp didn’t start") && failed.contains("wisp didn&#39;t start in the box: &#60;gone&#62;"),
            "{failed}"
        );
        assert!(failed.contains(r#"id="acts" style="justify-content:center"><button class="primary tall" id="again" type="button">Try again</button><a class="ghost line tall" href="https://grid.test">Home</a>"#), "{failed}");
        assert!(!failed.contains("poll(0)"), "a failed page waits for Try again");
        assert!(!failed.contains(">Back<"), "no dead end");
    }
}
