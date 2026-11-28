//! `wisp team …` for a team's hub (ADR-0148): its address, people, invite links, deploy keys, secrets and deploys,
//! through this wisp's grid sign-in (`grid-client.hub*`). `wisp team new|ls|show|add|rm` stay in `cloud_cli.rs`.
//!
//! A secret's value or a harness token never goes on the command line (the shell's history would keep it): it's piped
//! in or typed hidden, as `wisp secret set` takes one.
use serde_json::{Value, json};
use wisp_cli_core::term::{BOLD, DIM, GREEN, YELLOW, paint};

use crate::client::{CliError, Client, Result};
use crate::code::Opts;
use crate::scripted::say;

/// The `wisp team` words this file answers.
pub const OPS: [&str; 23] = [
    "help",
    "hub",
    "open",
    "restart",
    "delete",
    "invite",
    "invites",
    "uninvite",
    "join",
    "role",
    "keys",
    "key",
    "unkey",
    "secrets",
    "secret",
    "unset",
    "login",
    "logout",
    "deploys",
    "url",
    "urls",
    "url-revoke",
    "github",
];

pub const USAGE: &str = "wisp team new <team> [name]        a team, and its hub: its own wisp at <team>.<grid>
wisp team open|hub|restart <team>   the hub's address; turn on a hub for an older team; restart it
wisp team invite <team> [member|viewer|admin] [N] [--expires 7d]   an invite link for N people (1)
wisp team invites|uninvite <team> [id]   open invite links; revoke one
wisp team join <link>               take an invite link
wisp team role <team> <handle> <role>   owner | admin | member | viewer
wisp team key <team> <name> [deploy|plan]   a deploy key for CI, shown once (WISP_HUB_KEY)
wisp team keys|unkey <team> [id]    the team's deploy keys; revoke one
wisp team secret <team> NAME        one of the hub's env vars, piped in or typed hidden
wisp team secrets|unset <team> [NAME]   their names; take one out
wisp team login|logout <team> claude   the hub's Claude sign-in (claude setup-token, piped in or typed)
wisp team deploys <team>            what was deployed to the hub, by whom, from where
wisp team url <team> <trigger>      a URL that starts the hub's webhook trigger from the internet, shown once
wisp team urls|url-revoke <team> [id]   the hub's trigger URLs; revoke one
wisp team github <team> [connect|disconnect]   the hub's GitHub App: connected to what; install it; untie it
wisp team delete <team> --yes       the team, its hub and everything on it";

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// An invite's or a deploy key's role or rights word, a count, or `--expires Nd`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InviteAsk {
    pub role: String,
    pub uses: Option<i64>,
    pub days: Option<i64>,
}

/// `[member|viewer|admin] [N]` and `--expires 3d` (days; `2w` is 14).
pub fn invite_ask(words: &[String], expires: Option<&str>) -> Result<InviteAsk> {
    let mut a = InviteAsk::default();
    for w in words {
        match w.as_str() {
            "member" | "viewer" | "admin" | "owner" => a.role.clone_from(w),
            n if n.parse::<i64>().is_ok() => a.uses = n.parse().ok(),
            other => {
                return Err(CliError::usage(format!("wisp team invite: {other}? a role (member, viewer, admin) or a number of people")));
            }
        }
    }
    if let Some(e) = expires {
        let (n, unit) = e.trim().split_at(e.trim().len().saturating_sub(1));
        let n: i64 = n.parse().map_err(|_| CliError::usage("--expires: a number of days, like 7d or 2w"))?;
        a.days = Some(match unit {
            "d" => n,
            "w" => n * 7,
            _ => return Err(CliError::usage("--expires: a number of days, like 7d or 2w")),
        });
    }
    Ok(a)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `5m ago`, `in 7d`, or `never`.
fn when(ms: &Value) -> String {
    ms.as_i64().map_or_else(|| "never".to_owned(), |t| wisp_cli_core::human::ago(t, now_ms()))
}

pub async fn team(c: &Client, o: &Opts) -> Result<i32> {
    let st = crate::cli::style();
    let word = |i: usize| o.words.get(i).cloned().unwrap_or_default();
    let (op, team) = (word(0), word(1));
    let need =
        |what: &str| -> Result<()> { if team.is_empty() { Err(CliError::usage(format!("wisp team {op} <team>{what}"))) } else { Ok(()) } };
    let hub = |args: Value| async move { c.cmd("grid-client.hub", args).await };
    let out = match op.as_str() {
        "help" => {
            say(USAGE);
            return Ok(0);
        }
        "join" => {
            let v = hub(json!({ "op": "join", "code": team })).await?;
            say(&format!("  {} you're in {} as {}: {}", paint(&st, GREEN, "✓"), s(&v, "slug"), s(&v, "role"), s(&v, "url")));
            v
        }
        "open" | "hub" | "restart" => {
            need("")?;
            let ask = match op.as_str() {
                "hub" => "create",
                "restart" => "restart",
                _ => "show",
            };
            let v = hub(json!({ "op": ask, "team": team })).await?;
            if op == "restart" {
                say(&format!("  {} {team}'s hub restarts on its next request, with its secrets as they are now", paint(&st, GREEN, "✓")));
            } else {
                say(&format!(
                    "  {} {}  {}",
                    paint(&st, BOLD, &team),
                    s(&v, "url"),
                    paint(&st, DIM, &format!("{} · you: {}", s(&v, "state"), s(&v, "role")))
                ));
            }
            v
        }
        "delete" => {
            need(" --yes")?;
            if !o.yes {
                return Err(CliError::said(format!("this deletes {team}, its hub and everything on it: wisp team delete {team} --yes")));
            }
            hub(json!({ "op": "delete", "team": team, "confirm": team })).await?;
            say(&format!("  {} {team} is deleted, with its hub", paint(&st, GREEN, "✓")));
            Value::Null
        }
        "invite" => {
            need(" [member|viewer|admin] [N]")?;
            let ask = invite_ask(o.words.get(2..).unwrap_or_default(), o.expires.as_deref())?;
            let v = c.cmd("grid-client.hubInvite", json!({ "team": team, "role": ask.role, "uses": ask.uses, "days": ask.days })).await?;
            say(&format!(
                "  {} an invite to {team} as {} for {} {}:",
                paint(&st, GREEN, "✓"),
                s(&v, "role"),
                v["usesLeft"],
                if v["usesLeft"] == 1 { "person" } else { "people" }
            ));
            say(&format!("    {}", paint(&st, BOLD, s(&v, "url"))));
            say(&paint(&st, DIM, &format!("    shown once; it expires {}", when(&v["expiresAt"]))));
            v
        }
        "invites" => {
            need("")?;
            let v = hub(json!({ "op": "invites", "team": team })).await?;
            for i in v["invites"].as_array().into_iter().flatten() {
                say(&format!("  {}  {:<7} {} left  by {}", s(i, "id"), s(i, "role"), i["usesLeft"], s(i, "createdBy")));
            }
            v
        }
        "uninvite" | "unkey" => {
            need(" <id>")?;
            hub(json!({ "op": op, "team": team, "id": word(2) })).await?;
            say(&format!("  {} {} revoked", paint(&st, GREEN, "✓"), if op == "unkey" { "key" } else { "invite" }));
            Value::Null
        }
        "role" => {
            need(" <handle> <role>")?;
            c.cmd("grid-client.teams", json!({ "op": "add", "slug": team, "handle": word(2), "role": word(3) })).await?;
            say(&format!("  {} {} is {} in {team}", paint(&st, GREEN, "✓"), word(2), word(3)));
            Value::Null
        }
        "key" | "keys" | "secret" | "login" | "secrets" | "unset" | "logout" => keys_and_secrets(c, &op, &team, o, &st).await?,
        "url" | "urls" | "url-revoke" => trigger_urls(c, &op, &team, o, &st).await?,
        "github" => {
            need("")?;
            github(c, &team, &word(2), &st).await?
        }
        "deploys" => {
            need("")?;
            let v = hub(json!({ "op": "deploys", "team": team })).await?;
            print_deploys(&v, &st);
            v
        }
        _ => return Err(CliError::usage(USAGE)),
    };
    if o.json {
        say(&out.to_string());
    }
    Ok(0)
}

/// `wisp team github <team> [connect|disconnect]`: the hub's GitHub App (ADR-0148 §8e).
async fn github(c: &Client, team: &str, sub: &str, st: &wisp_cli_core::Style) -> Result<Value> {
    let op = match sub {
        "" | "show" | "status" => "github",
        "connect" => "github-connect",
        "disconnect" => "github-disconnect",
        _ => return Err(CliError::usage("wisp team github <team> [connect|disconnect]")),
    };
    let v = c.cmd("grid-client.hub", json!({ "op": op, "team": team })).await?;
    match op {
        "github-connect" => {
            say(&format!("  Open this to install the grid's GitHub App on your org and pick {team}'s repos:"));
            say(&format!("    {}", paint(st, BOLD, s(&v, "url"))));
            say(&paint(st, DIM, "    signed in to the grid in that browser as you; the link holds for 15 minutes"));
        }
        "github-disconnect" => {
            let was = v["disconnected"] == true;
            say(&format!(
                "  {} {}",
                paint(st, GREEN, "✓"),
                if was { "GitHub untied: the hub uses a GH_TOKEN secret, if set" } else { "it wasn't connected" }
            ));
        }
        _ => match (v["app"].is_null(), v["connected"].is_null()) {
            (true, _) => say("  This grid has no GitHub App: give the hub a GH_TOKEN secret (wisp team secret <team> GH_TOKEN)."),
            (false, true) => say(&format!("  Not connected. wisp team github {team} connect")),
            (false, false) => say(&format!(
                "  {} connected to {} by {}",
                paint(st, GREEN, "●"),
                paint(st, BOLD, s(&v["connected"], "account")),
                s(&v["connected"], "connectedBy")
            )),
        },
    }
    Ok(v)
}

/// `wisp team key|keys|secret|login|secrets|unset|logout …`: a hub's deploy keys and secrets.
async fn keys_and_secrets(c: &Client, op: &str, team: &str, o: &Opts, st: &wisp_cli_core::Style) -> Result<Value> {
    let word = |i: usize| o.words.get(i).cloned().unwrap_or_default();
    if team.is_empty() {
        return Err(CliError::usage(format!("wisp team {op} <team> …")));
    }
    let hub = |args: Value| async move { c.cmd("grid-client.hub", args).await };
    Ok(match op {
        "key" => {
            let v = c.cmd("grid-client.hubKeyCreate", json!({ "team": team, "name": word(2), "rights": word(3) })).await?;
            say(&format!("  {} deploy key {} ({}), shown once:", paint(st, GREEN, "✓"), s(&v, "name"), s(&v, "rights")));
            say(&format!("    {}", paint(st, BOLD, s(&v, "key"))));
            say(&paint(
                st,
                DIM,
                &format!("    in CI: WISP_HUB_KEY=<it> wisp grid deploy   (gh secret set WISP_HUB_KEY)   hub: {}", s(&v, "url")),
            ));
            v
        }
        "keys" => {
            let v = hub(json!({ "op": "keys", "team": team })).await?;
            for k in v["keys"].as_array().into_iter().flatten() {
                say(&format!(
                    "  {}  {:<20} {:<6} …{}  used {}",
                    s(k, "id"),
                    s(k, "name"),
                    s(k, "rights"),
                    s(k, "hint"),
                    when(&k["lastUsedAt"])
                ));
            }
            v
        }
        "secret" | "login" => {
            let name = if op == "login" && word(2).is_empty() { "claude".to_owned() } else { word(2) };
            let value = crate::secret_cli::value(if op == "login" { "token (claude setup-token)" } else { &name })?;
            let v =
                c.cmd("grid-client.hubSecretSet", json!({ "team": team, "name": name, "value": value, "login": op == "login" })).await?;
            say(&format!("  {} {name} set for {team}'s hub", paint(st, GREEN, "✓")));
            if v["restart"] == true {
                say(&paint(st, YELLOW, &format!("    the hub is running: wisp team restart {team} to give it the new value")));
            }
            v
        }
        "secrets" => {
            let v = hub(json!({ "op": "secrets", "team": team })).await?;
            for n in v["env"].as_array().into_iter().flatten() {
                say(&format!("  {}", n.as_str().unwrap_or_default()));
            }
            for (a, on) in v["logins"].as_object().into_iter().flatten() {
                say(&format!("  {a} sign-in: {}", if on == true { "set" } else { "not set" }));
            }
            v
        }
        "unset" | "logout" => {
            let name = if op == "logout" && word(2).is_empty() { "claude".to_owned() } else { word(2) };
            hub(json!({ "op": op, "team": team, "name": name })).await?;
            say(&format!("  {} {name} taken out of {team}'s hub", paint(st, GREEN, "✓")));
            Value::Null
        }
        _ => return Err(CliError::usage(USAGE)),
    })
}

/// `wisp team url|urls|url-revoke …`: the hub's webhook trigger URLs (ADR-0148 with ADR-0147's tokens).
async fn trigger_urls(c: &Client, op: &str, team: &str, o: &Opts, st: &wisp_cli_core::Style) -> Result<Value> {
    let word = |i: usize| o.words.get(i).cloned().unwrap_or_default();
    if team.is_empty() || (op != "urls" && word(2).is_empty()) {
        return Err(CliError::usage(format!(
            "wisp team {op} <team>{}",
            if op == "urls" {
                ""
            } else if op == "url" {
                " <trigger>"
            } else {
                " <id>"
            }
        )));
    }
    Ok(match op {
        "url" => {
            let v = c.cmd("grid-client.hubHookCreate", json!({ "team": team, "trigger": word(2) })).await?;
            say(&format!("  {} a URL for {}'s trigger {}, shown once:", paint(st, GREEN, "✓"), team, s(&v, "trigger")));
            say(&format!("    {}", paint(st, BOLD, s(&v, "url"))));
            say(&paint(st, DIM, "    GitHub: repo Settings → Webhooks → this URL, application/json, the trigger's secret"));
            v
        }
        "urls" => {
            let v = c.cmd("grid-client.hub", json!({ "op": "urls", "team": team })).await?;
            for h in v["hooks"].as_array().into_iter().flatten() {
                say(&format!("  {}  {:<24} …{}  called {}", s(h, "id"), s(h, "trigger"), s(h, "hint"), when(&h["lastUsedAt"])));
            }
            v
        }
        "url-revoke" => {
            c.cmd("grid-client.hub", json!({ "op": "unurl", "team": team, "id": word(2) })).await?;
            say(&format!("  {} trigger URL revoked: it opens nothing now", paint(st, GREEN, "✓")));
            Value::Null
        }
        _ => return Err(CliError::usage(USAGE)),
    })
}

/// `{deploys: [...]}` as lines: `#3  done  +2 ~1 -0  ci-main  acme/agents@a1b2c3  5 min ago`.
pub fn print_deploys(v: &Value, st: &wisp_cli_core::Style) {
    let list = v["deploys"].as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        say("  nothing deployed yet (wisp grid deploy)");
    }
    for d in &list {
        let n = |k: &str| d["summary"][k].as_i64().unwrap_or_default();
        let counts = format!("+{} ~{} -{}", n("created"), n("updated"), n("deleted"));
        let state = s(d, "state");
        let state = if state == "done" { paint(st, GREEN, state) } else { paint(st, YELLOW, state) };
        say(&format!(
            "  #{:<4} {state:<8} {counts:<12} {:<16} {}  {}",
            d["n"],
            s(d, "byName"),
            s(d, "source"),
            paint(st, DIM, &when(&d["startedAt"]))
        ));
        if let Some(e) = d["error"].as_str() {
            say(&format!("         {}", paint(st, DIM, e)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn an_invites_words_are_a_role_a_count_and_an_expiry() {
        assert_eq!(invite_ask(&[], None).unwrap(), InviteAsk::default());
        let a = invite_ask(&words(&["admin", "5"]), Some("2w")).unwrap();
        assert_eq!(a, InviteAsk { role: "admin".into(), uses: Some(5), days: Some(14) });
        assert_eq!(invite_ask(&[], Some("3d")).unwrap().days, Some(3));
        assert!(invite_ask(&words(&["boss"]), None).is_err());
        assert!(invite_ask(&[], Some("3h")).is_err() && invite_ask(&[], Some("d")).is_err());
    }
}
