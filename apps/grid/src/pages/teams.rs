//! Teams (`/teams`) and a team's hub (`/teams/<team>`), signed in (ADR-0148): the hubs you're in and a new team; then
//! one hub: open it, its people, invite links, deploy keys, secrets, deploys, and deleting it. Account's rows and
//! forms, one section each; what someone may change shows only to an owner or admin.
use super::home::{flash, hidden};
use super::icons::{ARROW, BOLT, CLOUD, CODE, KEY, PLUS, SHARE, SHIELD, STACK, TRASH};
use super::layout::{AppPage, Tab, app_page, state_label};
use super::scripts::{COPY_JS, LIVE};
use super::time::{ago, until};
use super::{PageCfg, PageUser, Tone, esc};

/// A team as `/teams` lists it.
#[derive(Debug, Clone, Default)]
pub struct TeamRow {
    pub slug: String,
    pub name: String,
    pub role: String,
    /// its hub's address ('' for a team from before hubs)
    pub url: String,
    pub state: String,
}

#[derive(Debug, Clone, Default)]
pub struct TeamsPage {
    pub user: PageUser,
    pub teams: Vec<TeamRow>,
    pub csrf: String,
    pub flash: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, Default)]
pub struct MemberRow {
    pub handle: String,
    pub role: String,
}

#[derive(Debug, Clone, Default)]
pub struct InviteRow {
    pub id: String,
    pub role: String,
    pub uses_left: i64,
    pub expires_at: i64,
    pub created_by: String,
}

#[derive(Debug, Clone, Default)]
pub struct HubKeyRow {
    pub id: String,
    pub name: String,
    pub hint: String,
    pub rights: String,
    pub created_by: String,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct DeployRow {
    pub n: i64,
    pub by: String,
    pub source: String,
    pub state: String,
    /// created, updated, deleted
    pub counts: (i64, i64, i64),
    pub error: Option<String>,
    pub started_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TeamPage {
    pub user: PageUser,
    pub slug: String,
    pub name: String,
    pub url: String,
    pub state: String,
    /// your role in it
    pub role: String,
    pub members: Vec<MemberRow>,
    pub invites: Vec<InviteRow>,
    pub keys: Vec<HubKeyRow>,
    /// the hub's env vars' names, and its harness sign-ins (agent, set)
    pub env: Vec<String>,
    pub logins: Vec<(String, bool)>,
    /// the grid keeps secrets (it has a master key)
    pub sealed: bool,
    pub deploys: Vec<DeployRow>,
    /// a key or an invite link just made: (what, the value), said this once
    pub fresh: Option<(String, String)>,
    pub csrf: String,
    pub flash: String,
    pub tone: Tone,
    pub now_ms: i64,
}

impl TeamPage {
    fn manages(&self) -> bool {
        matches!(self.role.as_str(), "owner" | "admin")
    }
}

fn state_pill(state: &str) -> String {
    state_label(state).map_or_else(String::new, |l| {
        let on = if state == "running" { " on" } else { "" };
        format!(r#"<span class="pill{on}"><span class="dot"></span>{l}</span>"#)
    })
}

fn role_select(name: &str, roles: &[&str], pick: &str) -> String {
    let opts: String =
        roles.iter().map(|r| format!(r#"<option value="{r}"{}>{r}</option>"#, if *r == pick { " selected" } else { "" })).collect();
    format!(r#"<select class="input sel" name="{name}" aria-label="Role">{opts}</select>"#)
}

pub fn teams_page(cfg: &PageCfg, p: &TeamsPage) -> String {
    let hidden = hidden(&p.csrf);
    let rows: String = p
        .teams
        .iter()
        .map(|t| {
            let sub = if t.url.is_empty() { "No hub yet".to_owned() } else { super::home::bare(&t.url).to_owned() };
            format!(
                r#"<a class="r link" href="/teams/{slug}"><span class="ic">{STACK}</span><div class="tx"><b>{name} {pill}</b><small>{sub} · you’re {role}</small></div><span class="ic">{ARROW}</span></a>"#,
                slug = esc(&t.slug),
                name = esc(&t.name),
                pill = state_pill(&t.state),
                sub = esc(&sub),
                role = esc(&t.role),
            )
        })
        .collect();
    let none = if p.teams.is_empty() {
        format!(
            r#"<div class="r"><span class="ic">{STACK}</span><div class="tx"><b>No teams yet</b><small>A team gets a hub: its own wisp on this grid, where the agents and triggers you share run. Make one below, or open an invite link a teammate sent you.</small></div></div>"#
        )
    } else {
        String::new()
    };
    let body = format!(
        r#"{flash}<div class="acct"><div class="a-main solo">
      <h1 class="h1">Teams</h1>
      <p class="hint">A team’s hub is a wisp the team owns, at its own address: its people open it, and its agents, projects and triggers deploy to it from a folder (<code>wisp grid init</code>), from your terminal or CI.</p>
      <div class="group">{rows}{none}</div>
      <h2 class="sec" id="new">New team</h2>
      <div class="group"><form class="r key-f" method="post" action="/teams">{hidden}<span class="ic">{PLUS}</span><input class="input" name="slug" placeholder="its name, like acme" aria-label="The team's name (its address)" pattern="[a-z0-9][a-z0-9-]{{1,39}}" maxlength="40" autocomplete="off" autocapitalize="none" spellcheck="false" required><input class="input" name="name" placeholder="shown as (optional)" aria-label="What it shows as" maxlength="60" autocomplete="off"><button class="primary">Make the team</button></form></div>
      <p class="hint">Or from a terminal: <code>wisp team new acme</code>.</p>
      </div></div>"#,
        flash = flash(&p.flash, p.tone),
    );
    app_page(&AppPage { user: Some(&p.user), csrf: &p.csrf, tab: Tab::Teams, ..AppPage::new(cfg, "Teams", &body) })
}

fn members(p: &TeamPage, hidden: &str) -> String {
    let base = format!("/teams/{}", esc(&p.slug));
    let rows: String = p
        .members
        .iter()
        .map(|m| {
            let me = m.handle == p.user.handle;
            let act = if p.manages() && !me {
                format!(
                    r#"<form class="inline" method="post" action="{base}/members/{h}/remove" data-confirm="Take {h} out of {slug}?">{hidden}<button class="ghost quiet">Remove</button></form>"#,
                    h = esc(&m.handle),
                    slug = esc(&p.slug)
                )
            } else if me && p.role != "owner" {
                format!(
                    r#"<form class="inline" method="post" action="{base}/members/{h}/remove" data-confirm="Leave {slug}?">{hidden}<button class="ghost quiet">Leave</button></form>"#,
                    h = esc(&m.handle),
                    slug = esc(&p.slug)
                )
            } else {
                String::new()
            };
            format!(
                r#"<div class="r"><span class="av sm">{i}</span><div class="tx"><b>{h}{you}</b><small>{role}</small></div>{act}</div>"#,
                i = esc(&m.handle.chars().next().unwrap_or('?').to_uppercase().to_string()),
                h = esc(&m.handle),
                you = if me { " (you)" } else { "" },
                role = esc(&m.role),
            )
        })
        .collect();
    let add = if p.manages() {
        let roles: &[&str] = if p.role == "owner" { &["member", "viewer", "admin", "owner"] } else { &["member", "viewer"] };
        format!(
            r#"<form class="r key-f" method="post" action="{base}/members">{hidden}<input class="input" name="handle" placeholder="their handle on this grid" aria-label="Their handle" autocomplete="off" autocapitalize="none" spellcheck="false" required>{sel}<button class="ghost line">Add</button></form>"#,
            sel = role_select("role", roles, "member")
        )
    } else {
        String::new()
    };
    format!("{rows}{add}")
}

fn invites(p: &TeamPage, hidden: &str) -> String {
    let base = format!("/teams/{}", esc(&p.slug));
    let fresh = match &p.fresh {
        Some((what, url)) if what == "invite" => format!(
            r#"<div class="r new-key" role="status"><span class="ic">{SHARE}</span><div class="tx"><b>Send this link: it’s shown once.</b><code class="key">{u}</code></div><button class="primary" type="button" data-copy="{u}"><span>Copy</span></button></div>"#,
            u = esc(url)
        ),
        _ => String::new(),
    };
    let rows: String = p
        .invites
        .iter()
        .map(|i| {
            format!(
                r#"<div class="r"><span class="ic">{SHARE}</span><div class="tx"><b>As {role} · {n} left</b><small>by {by} · ends {exp}</small></div><form class="inline" method="post" action="{base}/invites/{id}/revoke" data-confirm="Revoke this invite?">{hidden}<button class="ghost line">Revoke</button></form></div>"#,
                role = esc(&i.role),
                n = i.uses_left,
                by = esc(&i.created_by),
                exp = until(i.expires_at, p.now_ms),
                id = esc(&i.id),
            )
        })
        .collect();
    let roles: &[&str] = if p.role == "owner" { &["member", "viewer", "admin"] } else { &["member", "viewer"] };
    format!(
        r#"{fresh}{rows}<form class="r key-f" method="post" action="{base}/invites">{hidden}{sel}<input class="input num" name="uses" type="number" min="1" max="100" value="1" aria-label="How many people it adds"><select class="input sel" name="days" aria-label="How long it lasts"><option value="1">a day</option><option value="7" selected>a week</option><option value="30">30 days</option></select><button class="ghost line">Make a link</button></form>"#,
        sel = role_select("role", roles, "member")
    )
}

fn keys(p: &TeamPage, hidden: &str) -> String {
    let base = format!("/teams/{}", esc(&p.slug));
    let fresh = match &p.fresh {
        Some((what, key)) if what != "invite" => format!(
            r#"<div class="r new-key" role="status"><span class="ic">{KEY}</span><div class="tx"><b>{name} is made. Copy it now: it won’t be shown again.</b><code class="key">{k}</code><small>In GitHub: <code>gh secret set WISP_HUB_KEY</code>, then the workflow <code>wisp grid init</code> wrote deploys on every merge.</small></div><button class="primary" type="button" data-copy="{k}"><span>Copy</span></button></div>"#,
            name = esc(what),
            k = esc(key)
        ),
        _ => String::new(),
    };
    let rows: String = p
        .keys
        .iter()
        .map(|k| {
            format!(
                r#"<div class="r"><span class="ic">{KEY}</span><div class="tx"><b>{name} <span class="pill">{rights}</span></b><small>…{hint} · by {by} · last used {used}</small></div><form class="inline" method="post" action="{base}/keys/{id}/revoke" data-confirm="Revoke it? CI using it stops deploying.">{hidden}<button class="ghost line">Revoke</button></form></div>"#,
                name = esc(&k.name),
                rights = esc(&k.rights),
                hint = esc(&k.hint),
                by = esc(&k.created_by),
                used = ago(k.last_used_at, p.now_ms),
                id = esc(&k.id),
            )
        })
        .collect();
    format!(
        r#"{fresh}{rows}<form class="r key-f" method="post" action="{base}/keys">{hidden}<input class="input" name="name" placeholder="A name, like ci-main" aria-label="The new key’s name" maxlength="60" autocomplete="off" required><select class="input sel" name="rights" aria-label="What it may do"><option value="deploy">plan and deploy</option><option value="plan">plan only</option></select><button class="ghost line">Make a key</button></form>"#
    )
}

fn secrets(p: &TeamPage, hidden: &str) -> String {
    let base = format!("/teams/{}", esc(&p.slug));
    if !p.sealed {
        return format!(
            r#"<div class="r"><span class="ic">{SHIELD}</span><div class="tx"><b>This grid keeps no secrets</b><small>It has no master key. Its admin sets one (GRID_MASTER_KEY, or secrets/master.json in its data folder).</small></div></div>"#
        );
    }
    let logins: String = p
        .logins
        .iter()
        .map(|(agent, set)| {
            let (act, form) = if *set {
                let out = format!(
                    r#"<form class="inline" method="post" action="{base}/secrets/logins/{a}/remove" data-confirm="Sign the hub out of {a}?">{hidden}<button class="ghost line">Sign out</button></form>"#,
                    a = esc(agent)
                );
                (out, String::new())
            } else {
                let form = format!(
                    r#"<form class="r key-f" method="post" action="{base}/secrets/logins/{a}" autocomplete="off">{hidden}<input class="input" name="token" type="password" placeholder="its token, from claude setup-token" aria-label="Its token" autocomplete="off" required><button class="ghost line">Sign in</button></form>"#,
                    a = esc(agent)
                );
                (String::new(), form)
            };
            format!(
                r#"<div class="r wrap"><span class="ic">{BOLT}</span><div class="tx"><b>{name}</b><small>{state}</small></div>{act}</div>{form}"#,
                name = if agent == "claude" { "Claude".to_owned() } else { esc(agent) },
                state = if *set { "The hub and its sandboxes are signed in" } else { "Not signed in: its agents can’t run Claude yet" },
            )
        })
        .collect();
    let env: String = p
        .env
        .iter()
        .map(|k| {
            format!(
                r#"<div class="r"><span class="ic">{CODE}</span><div class="tx"><b><code>{k}</code></b><small>set · its value is never shown</small></div><form class="inline" method="post" action="{base}/secrets/{k}/remove" data-confirm="Take {k} out?">{hidden}<button class="ghost line">Remove</button></form></div>"#,
                k = esc(k)
            )
        })
        .collect();
    format!(
        r#"{logins}{env}<form class="r key-f" method="post" action="{base}/secrets" autocomplete="off">{hidden}<input class="input" name="key" placeholder="NAME, like GH_TOKEN" aria-label="The variable's name" pattern="[A-Za-z_][A-Za-z0-9_]{{0,63}}" autocapitalize="characters" spellcheck="false" required><input class="input" name="value" type="password" placeholder="its value" aria-label="Its value" required><button class="ghost line">Set</button></form>"#
    )
}

fn deploys(p: &TeamPage) -> String {
    if p.deploys.is_empty() {
        return format!(
            r#"<div class="r"><span class="ic">{CLOUD}</span><div class="tx"><b>Nothing deployed yet</b><small>In a folder: <code>wisp grid init {s}</code>, then <code>wisp grid deploy</code>. CI deploys with a key from above.</small></div></div>"#,
            s = esc(&p.slug)
        );
    }
    p.deploys
        .iter()
        .map(|d| {
            let (c, u, x) = d.counts;
            let bad = if d.state == "done" { "" } else { " bad" };
            let err = d.error.as_deref().map_or_else(String::new, |e| format!(" · {}", esc(e)));
            format!(
                r#"<div class="r"><span class="ic">{CLOUD}</span><div class="tx"><b>#{n} <span class="pill{bad}">{state}</span> <span class="counts">+{c} ~{u} −{x}</span></b><small>{by} · {src} · {when}{err}</small></div></div>"#,
                n = d.n,
                state = esc(&d.state),
                by = esc(&d.by),
                src = esc(&d.source),
                when = ago(Some(d.started_at), p.now_ms),
            )
        })
        .collect()
}

pub fn team_page(cfg: &PageCfg, p: &TeamPage) -> String {
    let hidden = hidden(&p.csrf);
    let base = format!("/teams/{}", esc(&p.slug));
    let manage = p.manages();
    let nav_more =
        if manage { r##"<a href="#invites">Invite links</a><a href="#keys">Deploy keys</a><a href="#secrets">Secrets</a>"## } else { "" };
    let owner_zone = if p.role == "owner" {
        format!(
            r#"<details class="danger-zone" id="delete"><summary><h2 class="sec">Danger zone</h2><small>Delete the team and its hub</small></summary>
      <div class="group"><form class="r del" method="post" action="{base}/delete"><span class="ic">{TRASH}</span><div class="tx"><b>Delete {slug} and its hub</b><small>The hub, its files, tasks, secrets, keys and invite links are deleted for good.</small></div>
        {hidden}<input class="input" name="confirm" placeholder="Type {slug}" aria-label="Type {slug} to confirm" autocomplete="off" autocapitalize="none" spellcheck="false" required pattern="{slug}"><button class="ghost line danger">Delete</button></form></div>
      </details>"#,
            slug = esc(&p.slug)
        )
    } else {
        String::new()
    };
    let managed = if manage {
        format!(
            r#"<h2 class="sec" id="invites">Invite links</h2>
      <p class="hint">A link adds whoever opens it, signed in to this grid, with its role. It’s shown once.</p>
      <div class="group">{invites}</div>

      <h2 class="sec" id="keys">Deploy keys</h2>
      <p class="hint">For CI: a key plans or deploys this team’s folder to the hub (<code>WISP_HUB_KEY</code>), and opens nothing else.</p>
      <div class="group">{keys}</div>

      <h2 class="sec" id="secrets">Secrets</h2>
      <p class="hint">The hub and its sandboxes start with these: <code>GH_TOKEN</code> for its repos, the harness sign-ins its agents run with. A formula names them; it never holds them.</p>
      <div class="group">{secrets}</div>
"#,
            invites = invites(p, &hidden),
            keys = keys(p, &hidden),
            secrets = secrets(p, &hidden),
        )
    } else {
        String::new()
    };
    let body = format!(
        r##"{flash}<div class="acct">
      <aside class="a-side">
        <div class="a-who"><span class="av">{init}</span><div><b title="{name}">{name}</b><small>you’re {role}</small></div></div>
        <nav class="a-nav" aria-label="{name}"><a href="#hub">Hub</a><a href="#people">People</a>{nav_more}<a href="#deploys">Deploys</a></nav>
      </aside>
      <div class="a-main">
      <p class="crumb"><a href="/teams">Teams</a></p>
      <h1 class="h1">{name}</h1>
      <div class="group" id="hub"><div class="r wrap"><span class="ic">{STACK}</span><div class="tx"><b>Hub {pill}</b><small>{bare}</small></div><button class="ghost line" type="button" data-copy="{url}"><span>Copy</span></button><a class="primary" href="/go?hub={slug}">Open the hub</a></div></div>

      <h2 class="sec" id="people">People</h2>
      <p class="hint">Owners and admins run the hub; members start tasks and triggers on it; viewers read.</p>
      <div class="group">{members}</div>

      {managed}
      <h2 class="sec" id="deploys">Deploys</h2>
      <p class="hint">What was deployed to the hub, by whom and from which commit (<code>wisp grid deploys</code>).</p>
      <div class="group">{deploys}</div>

      {owner_zone}
      </div></div>
      {COPY_JS}{LIVE}"##,
        flash = flash(&p.flash, p.tone),
        init = esc(&p.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
        name = esc(&p.name),
        role = esc(&p.role),
        pill = state_pill(&p.state),
        bare = esc(super::home::bare(&p.url)),
        url = esc(&p.url),
        slug = esc(&p.slug),
        members = members(p, &hidden),
        deploys = deploys(p),
    );
    app_page(&AppPage { user: Some(&p.user), csrf: &p.csrf, tab: Tab::Teams, ..AppPage::new(cfg, &p.name, &body) })
}

#[cfg(test)]
#[path = "teams_tests.rs"]
mod tests;
