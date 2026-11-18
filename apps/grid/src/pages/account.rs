//! Account (`/account`), signed in: signed-in devices, the Telegram bot for `on laptop`, how you
//! sign in, sign out, and delete.

use super::home::{ComputerView, flash, hidden};
use super::icons::{GLOBE, KEY, LAPTOP, PHONE, SHIELD, TRASH};
use super::join::logo;
use super::layout::{AppPage, Tab, app_page, avatar};
use super::scripts::{COPY_JS, LIVE};
use super::time::ago;
use super::{PageCfg, PageUser, Provider, Tone, esc};
use crate::http::encode_component;

/// A device signed in to the account (the desktop or phone app, wisp on a computer).
#[derive(Debug, Clone, Default)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub created_at: Option<i64>,
    pub last_seen_at: Option<i64>,
}

/// One way the account signs in.
#[derive(Debug, Clone, Default)]
pub struct IdentityView {
    pub provider: String,
    pub login: Option<String>,
    pub email: Option<String>,
}

/// The Telegram bot that routes `on laptop …` (`crate::route`). What was given never shows again.
#[derive(Debug, Clone, Default)]
pub struct RouteView {
    pub set: bool,
    pub bot: Option<String>,
    pub senders: Vec<String>,
    /// Telegram no longer sends this bot's messages to the grid
    pub moved: bool,
    /// the cloud wisp has the same bot
    pub shared: bool,
}

/// One API key as Account lists it: never the key, its last four characters to tell it apart.
#[derive(Debug, Clone, Default)]
pub struct KeyView {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub created_at: Option<i64>,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct AccountPage {
    pub user: PageUser,
    pub devices: Vec<DeviceView>,
    pub computers: Vec<ComputerView>,
    pub identities: Vec<IdentityView>,
    pub providers: Vec<Provider>,
    /// `None`: this grid routes no channels
    pub route: Option<RouteView>,
    /// the account's live API keys, newest first (#6)
    pub keys: Vec<KeyView>,
    /// a key just made: (its name, the key), said this once and never again
    pub new_key: Option<(String, String)>,
    pub csrf: String,
    pub flash: String,
    pub tone: Tone,
    pub now_ms: i64,
}

fn route_warn(title: &str, text: &str) -> String {
    format!(r#"<div class="r route-warn"><span class="ic">{SHIELD}</span><div class="tx"><b>{title}</b><small>{text}</small></div></div>"#)
}

fn route_section(route: &RouteView, computers: &[ComputerView], hidden: &str) -> String {
    let name = esc(computers.first().map_or("laptop", |c| c.name.as_str()));
    let n = route.senders.len();
    let inner = if route.set {
        let bot = route.bot.as_deref().map_or_else(|| "Your bot".to_owned(), |b| format!("@{}", esc(b)));
        let who = if n == 1 { "One person".to_owned() } else { format!("{n} people") };
        let moved = if route.moved {
            route_warn(
                "The webhook moved",
                "Telegram no longer sends this bot’s messages to the grid, so they aren’t reaching your computers. Something else with this bot’s token set it. Remove the bot here and connect it again.",
            )
        } else {
            String::new()
        };
        let shared = if route.shared {
            route_warn(
                "Your cloud wisp also has this bot",
                "Anything on your cloud wisp can read this bot’s token, and with it your messages and reports, or speak as this bot. It can’t start anything on your computers. Make a new bot with BotFather, then Remove this one and connect that.",
            )
        } else {
            String::new()
        };
        format!(
            r#"<div class="r wrap"><span class="ic">{SHIELD}</span><div class="tx"><b>{bot}</b><small>{who} can send: {senders}</small></div><form class="inline" method="post" action="/channels/route/remove">{hidden}<button class="ghost quiet">Remove</button></form></div>{moved}{shared}"#,
            senders = esc(&route.senders.join(", "))
        )
    } else {
        format!(
            r#"<form class="route-f" method="post" action="/channels/route" autocomplete="off">{hidden}
        <label><span>Bot token</span><input class="input" name="token" type="password" placeholder="From BotFather" autocomplete="off" autocapitalize="none" spellcheck="false" required></label>
        <label><span>Who may send</span><input class="input" name="senders" inputmode="numeric" placeholder="Your Telegram user id" autocomplete="off" required></label>
        <p class="hint">Make a new bot for this with BotFather, not the one your cloud wisp uses. Anything on your cloud wisp can read that bot’s token, and with it what goes through here. The grid keeps this token sealed and sets the webhook with a secret of its own.</p>
        <button class="primary">Connect</button></form>"#
        )
    };
    format!(
        r#"<h2 class="sec" id="route">Telegram to your computers</h2>
      <p class="hint">Tell your bot <code>on {name} fix the login</code> and it runs on that computer, once that computer has switched on <b>Settings → Notifications → Accept tasks from channels</b> itself. The grid reads these messages and your cloud wisp is never asked. The bot does nothing else: anything else gets a line saying so.</p>
      <div class="group" data-live="route">{inner}</div>

      "#
    )
}

/// What a device is (#11): a computer added to the grid (and as what), the phone app, or wisp on a computer.
fn device_kind(d: &DeviceView, computers: &[ComputerView]) -> (String, &'static str) {
    if let Some(c) = computers.iter().find(|c| c.device_id == d.id) {
        // the same name twice says nothing: only a computer renamed on the grid says what it's called there
        return (if c.name == d.name { "Computer on the grid".into() } else { format!("Computer on the grid as {}", c.name) }, LAPTOP);
    }
    let n = d.name.to_lowercase();
    if ["phone", "android", "pixel", "galaxy", "ipad"].iter().any(|w| n.contains(w)) {
        return ("Phone app".into(), PHONE);
    }
    ("wisp on a computer".into(), LAPTOP)
}

/// API keys (#6): make one here (said once), see when each was last used, revoke it. A key opens its cloud wisp's
/// API and nothing else (`trust`), so it lives with the account, not inside the wisp it opens.
fn keys(p: &AccountPage, hidden: &str) -> String {
    let fresh = p.new_key.as_ref().map_or_else(String::new, |(name, key)| {
        format!(
            r#"<div class="r new-key" role="status"><span class="ic">{KEY}</span><div class="tx"><b>{name} is made. Copy it now: it won’t be shown again.</b><code class="key">{k}</code></div><button class="primary" type="button" data-copy="{k}"><span>Copy</span></button></div>"#,
            name = esc(name),
            k = esc(key)
        )
    });
    let rows: String = p
        .keys
        .iter()
        .map(|k| {
            format!(
                r#"<div class="r"><span class="ic">{KEY}</span><div class="tx"><b>{name}</b><small>…{hint} · made {made} · last used {used}</small></div><form class="inline" method="post" action="/keys/{id}/revoke" data-confirm="Revoke it?">{hidden}<button class="ghost line">Revoke</button></form></div>"#,
                name = esc(&k.name),
                hint = esc(&k.hint),
                made = ago(k.created_at, p.now_ms),
                used = ago(k.last_used_at, p.now_ms),
                id = esc(&k.id),
            )
        })
        .collect();
    let none = if p.keys.is_empty() && p.new_key.is_none() {
        format!(
            r#"<div class="r"><span class="ic">{KEY}</span><div class="tx"><b>No keys yet</b><small>Name one below. A script sends it as <code>Authorization: Bearer …</code> to your cloud wisp’s address.</small></div></div>"#
        )
    } else {
        String::new()
    };
    format!(
        r#"{fresh}{rows}{none}<form class="r key-f" method="post" action="/keys">{hidden}<input class="input" name="name" placeholder="A name, like deploy bot" aria-label="The new key’s name" maxlength="60" autocomplete="off" required><button class="ghost line">Make a key</button></form>"#
    )
}

fn devices(p: &AccountPage, hidden: &str) -> String {
    if p.devices.is_empty() {
        return format!(
            r#"<div class="r"><span class="ic">{LAPTOP}</span><div class="tx"><b>No devices yet</b><small>wisp on a computer and the phone app sign in here once, with a code.</small></div></div>"#
        );
    }
    p.devices
        .iter()
        .map(|d| {
            let (kind, icon) = device_kind(d, &p.computers);
            format!(
                r#"<div class="r"><span class="ic">{icon}</span><div class="tx"><b>{name}</b><small>{kind} · added {added} · last used {used}</small></div>
            <form class="inline" method="post" action="/devices/{id}/revoke" data-confirm="Sign it out?">{hidden}<button class="ghost quiet">Remove</button></form></div>"#,
                name = esc(&d.name),
                kind = esc(&kind),
                added = ago(d.created_at, p.now_ms),
                used = ago(d.last_seen_at, p.now_ms),
                id = esc(&d.id),
            )
        })
        .collect()
}

fn ways(p: &AccountPage, hidden: &str) -> String {
    let label = |id: &str| p.providers.iter().find(|x| x.id == id).map_or_else(|| id.to_owned(), |x| x.label.clone());
    let many = p.identities.len() > 1;
    let mut out: String = p
        .identities
        .iter()
        .map(|i| {
            let who = i.login.as_deref().filter(|s| !s.is_empty()).or(i.email.as_deref()).unwrap_or_default();
            let unlink = if many {
                format!(r#"<form class="inline" method="post" action="/identities/unlink">{hidden}<input type="hidden" name="provider" value="{}"><button class="ghost">Disconnect</button></form>"#, esc(&i.provider))
            } else {
                String::new()
            };
            format!(
                r#"<div class="r"><span class="ic">{}</span><div class="tx"><b>{}</b><small>{}</small></div>
          {unlink}</div>"#,
                logo(&i.provider),
                esc(&label(&i.provider)),
                esc(who)
            )
        })
        .collect();
    // a development grid's sign-in by handle keeps no identity row: say how this account gets in all the same
    if p.identities.is_empty() {
        let (title, how) = if p.providers.iter().any(|x| x.id == "dev") {
            ("Development sign-in", format!("By handle ({}), on this development grid", p.user.handle))
        } else {
            ("No sign-in connected", "Connect one below so you can get back in".to_owned())
        };
        out.push_str(&format!(
            r#"<div class="r"><span class="ic">{}</span><div class="tx"><b>{title}</b><small>{}</small></div></div>"#,
            logo("dev"),
            esc(&how)
        ));
    }
    out.push_str("\n        ");
    for prov in p.providers.iter().filter(|x| x.id != "dev" && !p.identities.iter().any(|i| i.provider == x.id)) {
        out.push_str(&format!(
            r#"<div class="r"><span class="ic">{}</span><div class="tx"><b>{}</b><small>Connect it too, and either one signs you in</small></div><a class="ghost line" href="/auth/{}/start?link=1">Connect</a></div>"#,
            logo(&prov.id),
            esc(&prov.label),
            encode_component(&prov.id)
        ));
    }
    out
}

pub fn account_page(cfg: &PageCfg, p: &AccountPage) -> String {
    let hidden = hidden(&p.csrf);
    let u = &p.user;
    let shown = u.name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&u.handle);
    // the second line says something the first doesn't: the email, else the handle if the name isn't it
    let sub = u.email.as_deref().filter(|e| !e.is_empty()).unwrap_or(if shown == u.handle { "" } else { &u.handle });
    let route_link = if p.route.is_some() { r##"<a href="#route">Telegram to your computers</a>"## } else { "" };
    let route = p.route.as_ref().map_or_else(String::new, |r| route_section(r, &p.computers, &hidden));
    let handle = esc(&u.handle);
    let body = format!(
        r##"{flash}<div class="acct">
      <aside class="a-side">
        <div class="a-who">{av}<div><b title="{shown}">{shown}</b><small title="{sub}">{sub}</small></div></div>
        <nav class="a-nav" aria-label="Account"><a href="#devices">Signed-in devices</a><a href="#keys">API keys</a>{route_link}<a href="#ways">How you sign in</a><a href="#out">Sign out</a><a href="#delete">Danger zone</a></nav>
      </aside>
      <div class="a-main">
      <h1 class="h1">Account</h1>
      <h2 class="sec" id="devices">Signed-in devices</h2>
      <p class="hint">Apps that can open your wisps. Removing a computer’s device also takes it off the grid.</p>
      <div class="group" data-live="devices">{devices}</div>

      <h2 class="sec" id="keys">API keys</h2>
      <p class="hint">For scripts: a key starts tasks in your cloud wisp, and opens nothing else.</p>
      <div class="group">{keys}</div>

      {route}<h2 class="sec" id="ways">How you sign in</h2>
      <div class="group">{ways}
      </div>

      <h2 class="sec" id="out">Sign out</h2>
      <div class="group">
        <div class="r wrap"><span class="ic">{GLOBE}</span><div class="tx"><b>This browser</b><small>Your cloud wisp keeps running.</small></div><form class="inline" method="post" action="/signout">{hidden}<button class="ghost line">Sign out</button></form></div>
        <div class="r wrap"><span class="ic">{SHIELD}</span><div class="tx"><b>Every browser</b><small>Signs this account out of the grid in every browser. Devices stay signed in: remove them above.</small></div><form class="inline" method="post" action="/signout?all=1">{hidden}<button class="ghost line">Sign out everywhere</button></form></div>
      </div>

      <details class="danger-zone" id="delete"><summary><h2 class="sec">Danger zone</h2><small>Delete your account</small></summary>
      <div class="group"><form class="r del" method="post" action="/account/delete"><span class="ic">{TRASH}</span><div class="tx"><b>Delete your account and your cloud wisp</b><small>Your cloud wisp, its files and your agents’ sign-ins there are deleted for good. Your computers keep their own wisp.</small></div>
        {hidden}<input class="input" name="confirm" placeholder="Type {handle}" aria-label="Type {handle} to confirm" autocomplete="off" autocapitalize="none" spellcheck="false" required pattern="{handle}"><button class="ghost line danger">Delete</button></form></div>
      </details>
      </div></div>
      <script>(()=>{{const open=()=>{{if(location.hash==='#delete'){{const d=document.getElementById('delete');d.open=true;d.scrollIntoView()}}}};open();addEventListener('hashchange',open)}})()</script>
      {COPY_JS}{LIVE}"##,
        flash = flash(&p.flash, p.tone),
        av = avatar(u),
        shown = esc(shown),
        sub = esc(sub),
        devices = devices(p, &hidden),
        keys = keys(p, &hidden),
        ways = ways(p, &hidden),
    );
    app_page(&AppPage { user: Some(u), csrf: &p.csrf, tab: Tab::Account, ..AppPage::new(cfg, "Account", &body) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> AccountPage {
        AccountPage {
            user: PageUser { handle: "ada".into(), email: Some("ada@example.com".into()), ..PageUser::default() },
            identities: vec![IdentityView { provider: "github".into(), login: Some("ada".into()), email: None }],
            providers: vec![
                Provider { id: "github".into(), label: "GitHub".into() },
                Provider { id: "google".into(), label: "Google".into() },
                Provider { id: "dev".into(), label: "Dev".into() },
            ],
            now_ms: 1_793_000_000_000,
            ..AccountPage::default()
        }
    }

    #[test]
    fn one_way_in_cant_be_disconnected_and_others_can_be_linked() {
        let a = account_page(&PageCfg::default(), &page());
        assert!(!a.contains("Disconnect"));
        assert!(a.contains(r#"href="/auth/google/start?link=1""#));
        assert!(!a.contains("/auth/dev/start"));
        assert!(a.contains("No devices yet"));
        assert!(!a.contains(r##"href="#route""##));
        assert!(a.contains(r#"pattern="ada""#));
    }

    #[test]
    fn keys_are_listed_said_once_and_made_from_a_form() {
        let mut p = page();
        let a = account_page(&PageCfg::default(), &p);
        assert!(a.contains(r##"<a href="#keys">API keys</a>"##) && a.contains("No keys yet"));
        assert!(a.contains(r#"<form class="r key-f" method="post" action="/keys">"#));
        p.keys = vec![KeyView {
            id: "k1".into(),
            name: "deploy <bot>".into(),
            hint: "a7Qz".into(),
            created_at: Some(p.now_ms - 60_000),
            last_used_at: None,
        }];
        p.new_key = Some(("deploy <bot>".into(), "wgk_secret".into()));
        let a = account_page(&PageCfg::default(), &p);
        assert!(a.contains("deploy &#60;bot&#62; is made. Copy it now: it won’t be shown again."));
        assert!(a.contains(r#"data-copy="wgk_secret""#));
        assert!(a.contains("…a7Qz · made 1 min ago · last used never"));
        assert!(a.contains(r#"action="/keys/k1/revoke" data-confirm="Revoke it?""#));
        assert!(!a.contains("No keys yet"));
    }

    #[test]
    fn delete_is_folded_in_the_danger_zone_and_opens_from_its_link() {
        let a = account_page(&PageCfg::default(), &page());
        assert!(a.contains(r#"<details class="danger-zone" id="delete">"#));
        assert!(a.contains("location.hash==='#delete'"));
    }

    #[test]
    fn devices_say_what_they_are() {
        let mut p = page();
        p.devices = vec![
            DeviceView { id: "d2".into(), name: "Pixel 9".into(), ..DeviceView::default() },
            DeviceView { id: "d3".into(), name: "work laptop".into(), ..DeviceView::default() },
        ];
        let a = account_page(&PageCfg::default(), &p);
        assert!(a.contains("<b>Pixel 9</b><small>Phone app · added never"));
        assert!(a.contains("<b>work laptop</b><small>wisp on a computer · added never"));
    }

    #[test]
    fn devices_say_which_computer_they_are() {
        let mut p = page();
        p.devices =
            vec![DeviceView { id: "d1".into(), name: "Ada’s iPhone".into(), created_at: Some(p.now_ms - 90_000), last_seen_at: None }];
        p.computers = vec![ComputerView { device_id: "d1".into(), name: "mac".into(), ..ComputerView::default() }];
        p.route =
            Some(RouteView { set: true, bot: Some("ada_bot".into()), senders: vec!["1".into(), "2".into()], moved: true, shared: false });
        let a = account_page(&PageCfg::default(), &p);
        assert!(a.contains("Computer on the grid as mac · added 2 min ago · last used never"));
        assert!(a.contains("<b>@ada_bot</b><small>2 people can send: 1, 2</small>"));
        assert!(a.contains("The webhook moved"));
        assert!(!a.contains("also has this bot"));
    }
}
