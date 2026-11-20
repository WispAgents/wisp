//! A setup card (ADR-0113): an agent asks to change the user's setup (a connector, the crew, a project) and the user
//! answers Allow or Deny. The card's words come from the write itself, never from the agent, so what the user reads is
//! what would run: each command verbatim. Pure.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const ALLOW: &str = "Allow";
pub const DENY: &str = "Deny";

pub const CONNECTOR_ADD: &str = "connectors.agent.add";
pub const CREW_ADD: &str = "crew.agent.add";
pub const CREW_EDIT: &str = "crew.agent.edit";
pub const CREW_REMOVE: &str = "crew.agent.remove";
pub const PROJECT_ADD: &str = "runtime.projectAdd";

/// The bundle import a crew card runs once `from` is read (by the server, before the card): `{ bundle, choices }`.
pub const CREW_IMPORT: &str = "crew.import";

/// The writes an agent may ask for: nothing else is ever run from a setup card.
pub const WRITES: [&str; 5] = [CONNECTOR_ADD, CREW_ADD, CREW_EDIT, CREW_REMOVE, PROJECT_ADD];

/// One write the card runs on Allow, exactly as stored: its command and the arguments the card was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Write {
    pub command: String,
    pub args: Value,
}

/// What the card says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    /// what would change, after "wants to": "add the connector \`linear\`"
    pub action: String,
    /// "Ada wants to add the connector \`linear\`: runs \`npx -y @linear/mcp\`"
    pub headline: String,
    /// every other field that will be stored, one a line (`role: Docs writer`, `needs GITHUB_TOKEN from you`)
    pub lines: Vec<String>,
    /// every command the change would run, verbatim
    pub commands: Vec<String>,
    /// everything that will be written, in full (Show all)
    pub full: String,
}

impl Card {
    /// The question as channels and the phone show it: the headline, then each line.
    #[must_use]
    pub fn question(&self) -> String {
        std::iter::once(self.headline.clone()).chain(self.lines.iter().cloned()).collect::<Vec<_>>().join("\n")
    }
}

/// What normalizing needs from the task: its id (each write names it) and its folder (a member's default).
#[derive(Debug, Clone, Copy)]
pub struct Task<'a> {
    pub session: &'a str,
    pub cwd: &'a str,
}

/// An agent's ask, made the writes that would run: only fields the card shows, defaults filled in. `Import`: a crew
/// add `from` a link or file, which the server reads first ([`import_write`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Normalized {
    Writes(Vec<Write>),
    Import { from: String },
}

/// A member's fields a card may carry (crew's `MemberBody`, as its JSON names them).
const MEMBER: [&str; 22] = [
    "name", "role", "avatar", "color", "bg", "brief", "cwd", "machine", "sandbox", "agent", "model", "effort", "mode", "cron", "parallel",
    "kit", "team", "paused", "notebook", "wakeAt", "on", "wakes",
];

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty())
}

/// The given fields of `a`, each one of `keys` (anything else is refused: nothing the card doesn't show is stored).
fn only(a: &Value, keys: &[&str], what: &str) -> Result<Map<String, Value>, String> {
    let o = a.as_object().ok_or_else(|| format!("{what}'s arguments are an object"))?;
    let mut out = Map::new();
    for (k, v) in o {
        if matches!(k.as_str(), "session" | "sid") || v.is_null() {
            continue;
        }
        if !keys.contains(&k.as_str()) {
            // what it takes, named (audit G18: `wisp_crew_remove {"name":"Eve"}` never said it needs `member`)
            let takes = if keys.len() <= 3 { keys.join(", ") } else { format!("{}, …", keys[..3].join(", ")) };
            return Err(format!("{k} isn't something a {what} card can carry: it takes {takes}"));
        }
        out.insert(k.clone(), v.clone());
    }
    Ok(out)
}

/// Key names only: a list of names, or an object whose values are all empty. A value would be stored unseen.
fn key_names(v: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => a.iter().map(|k| k.as_str().map(str::to_owned).ok_or_else(|| format!("{field}: names only"))).collect(),
        Some(Value::Object(o)) if o.values().all(|x| x.is_null() || x.as_str() == Some("")) => Ok(o.keys().cloned().collect()),
        Some(_) => Err(format!("{field}: give the names only; the user types the values")),
    }
}

/// The writes `command` with `args` would make for this task. Err: not a write an agent may ask for, a field the card
/// can't show, or one that names nothing.
pub fn normalize(command: &str, args: &Value, t: Task<'_>) -> Result<Normalized, String> {
    let w = |command: &str, mut o: Map<String, Value>, key: &str| {
        o.insert(key.into(), Value::String(t.session.to_owned()));
        Normalized::Writes(vec![Write { command: command.to_owned(), args: Value::Object(o) }])
    };
    match command {
        CONNECTOR_ADD => {
            let mut o = only(args, &["name", "url", "command", "args", "env", "headers", "transport"], "connector")?;
            s(args, "name").ok_or("Give the connector a name.")?;
            let (url, cmd) = (s(args, "url").is_some(), s(args, "command").is_some());
            let transport = match (url, cmd) {
                (true, true) => return Err("A connector is a URL (remote) or a command (local), not both.".into()),
                (false, false) => return Err("Give the connector's url (remote) or the command that starts it (local).".into()),
                (true, false) => "remote",
                (false, true) => "local",
            };
            if o.get("transport").is_some_and(|x| x.as_str() != Some(transport)) {
                return Err(format!("transport: this connector is {transport}"));
            }
            if url && (o.contains_key("args") || o.contains_key("env")) || cmd && o.contains_key("headers") {
                return Err("args and env go with a command; headers with a url".into());
            }
            for f in ["env", "headers"] {
                if o.contains_key(f) {
                    o.insert(f.into(), Value::from(key_names(o.get(f), f)?));
                }
            }
            if o.get("args").is_some_and(|a| !a.as_array().is_some_and(|l| l.iter().all(Value::is_string))) {
                return Err("args: a list of words".into());
            }
            o.insert("transport".into(), Value::String(transport.into()));
            Ok(w(CONNECTOR_ADD, o, "sid"))
        }
        CREW_ADD => {
            if let Some(from) = s(args, "from") {
                only(args, &["from"], "crew")?;
                return Ok(Normalized::Import { from: from.to_owned() });
            }
            let keys: Vec<&str> = MEMBER.iter().copied().chain(["checkin"]).collect();
            let mut o = only(args, &keys, "crew")?;
            s(args, "name").ok_or("Give the crew member a name.")?;
            // where they work: the task's own folder unless given, said on the card as stored
            if !o.contains_key("cwd") && !t.cwd.is_empty() {
                o.insert("cwd".into(), Value::String(t.cwd.to_owned()));
            }
            Ok(w(CREW_ADD, o, "session"))
        }
        CREW_EDIT => {
            let keys: Vec<&str> = MEMBER.iter().copied().chain(["member", "remember"]).collect();
            let o = only(args, &keys, "crew")?;
            s(args, "member").ok_or("Say which crew member to change (member).")?;
            if o.len() < 2 {
                return Err("Say what to change.".into());
            }
            Ok(w(CREW_EDIT, o, "session"))
        }
        CREW_REMOVE => {
            let o = only(args, &["member"], "crew")?;
            s(args, "member").ok_or("Say which crew member to remove (member).")?;
            Ok(w(CREW_REMOVE, o, "session"))
        }
        PROJECT_ADD => {
            let o = only(args, &["path", "name"], "project")?;
            s(args, "path").ok_or("Say which folder to add as a project (path).")?;
            Ok(Normalized::Writes(vec![Write { command: PROJECT_ADD.into(), args: Value::Object(o) }]))
        }
        other => Err(format!("{other} isn't a change to the user's setup an agent can ask for")),
    }
}

/// A crew read from `from` (the bundle the server fetched or parsed, before the card), imported as read: every member
/// in the task's folder (`keys`: the members the review planned).
#[must_use]
pub fn import_write(bundle: Value, keys: &[String], cwd: &str) -> Write {
    let choices: Map<String, Value> = keys.iter().map(|k| (k.clone(), serde_json::json!({ "cwd": cwd }))).collect();
    let mut args = Map::new();
    args.insert("bundle".into(), bundle);
    args.insert("choices".into(), Value::Object(choices));
    Write { command: CREW_IMPORT.into(), args: Value::Object(args) }
}

/// A value as one line: text as it is, a list of words joined, anything else as compact JSON; at most `max` chars.
fn shown(v: &Value, max: usize) -> String {
    let full = match v {
        Value::String(t) => t.clone(),
        Value::Array(a) if a.iter().all(Value::is_string) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    };
    let one = full.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > max { format!("{}… (Show all)", one.chars().take(max).collect::<String>()) } else { one }
}

/// Every field of `o` but `skip`, one a line (`kit` by its files' names, with their bodies in Show all).
fn field_lines(o: &Map<String, Value>, skip: &[&str]) -> Vec<String> {
    o.iter()
        .filter(|(k, _)| !skip.contains(&k.as_str()) && !matches!(k.as_str(), "session" | "sid"))
        .map(|(k, v)| match (k.as_str(), v) {
            ("kit", Value::Array(files)) => {
                let names: Vec<&str> = files.iter().filter_map(|f| f.get("name").and_then(Value::as_str)).collect();
                format!("kit: {} file{} ({}; bodies in Show all)", files.len(), if files.len() == 1 { "" } else { "s" }, names.join(", "))
            }
            ("checkin", Value::Bool(true)) => "checks in once added".to_owned(),
            _ => format!("{k}: {}", shown(v, 160)),
        })
        .collect()
}

/// A word of a command line as a shell would need it typed: quoted when it has a space or a quote.
fn word(w: &str) -> String {
    if !w.is_empty() && !w.chars().any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '\\' | '$' | '`')) {
        return w.to_owned();
    }
    format!("'{}'", w.replace('\'', r"'\''"))
}

/// The card for `writes`, asked by `who` (a crew member's name, else the harness's): built from the writes alone, so
/// the stored writes give back the card the user saw. Err: a write a card never runs.
pub fn card(who: &str, writes: &[Write]) -> Result<Card, String> {
    let who = if who.trim().is_empty() { "The agent" } else { who.trim() };
    let [one] = writes else { return Err("a setup card runs one write".into()) };
    let a = one.args.as_object().cloned().unwrap_or_default();
    let (action, detail, lines, commands) = match one.command.as_str() {
        CONNECTOR_ADD => {
            let name = s(&one.args, "name").unwrap_or_default().to_lowercase();
            let names = |k: &str| {
                a.get(k)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|n| format!("needs {n} from you"))
                    .collect::<Vec<_>>()
            };
            if let Some(url) = s(&one.args, "url") {
                (format!("add the connector `{name}`"), format!(": connects to {url}"), names("headers"), Vec::new())
            } else {
                let cmd = s(&one.args, "command").unwrap_or_default();
                let extra = a.get("args").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(word);
                let full = std::iter::once(cmd.to_owned()).chain(extra).collect::<Vec<_>>().join(" ");
                let mut lines = vec!["Every task will start it, on this computer.".to_owned()];
                lines.extend(names("env"));
                (format!("add the connector `{name}`"), format!(": runs `{full}`"), lines, vec![full])
            }
        }
        CREW_ADD => {
            let name = s(&one.args, "name").unwrap_or_default();
            let how: Vec<&str> = [s(&one.args, "agent"), s(&one.args, "mode")].into_iter().flatten().collect();
            let detail = if how.is_empty() { String::new() } else { format!(" ({})", how.join(", ")) };
            (format!("add crew member {name}"), detail, field_lines(&a, &["name", "agent", "mode"]), Vec::new())
        }
        CREW_EDIT => {
            let m = s(&one.args, "member").unwrap_or_default();
            let mut lines = field_lines(&a, &["member", "remember"]);
            lines.extend(
                a.get("remember")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|r| format!("remember: {r}")),
            );
            (format!("change crew member {m}"), String::new(), lines, Vec::new())
        }
        CREW_REMOVE => {
            (format!("remove {} from your crew", s(&one.args, "member").unwrap_or_default()), String::new(), Vec::new(), Vec::new())
        }
        PROJECT_ADD => {
            let lines = s(&one.args, "name").map(|n| vec![format!("named {n}")]).unwrap_or_default();
            (format!("add the project {}", s(&one.args, "path").unwrap_or_default()), String::new(), lines, Vec::new())
        }
        CREW_IMPORT => {
            let members: Vec<Map<String, Value>> = a
                .get("bundle")
                .and_then(|b| b.get("members"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|m| m.as_object().cloned())
                .collect();
            let names: Vec<String> = members.iter().filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_owned)).collect();
            let cwd =
                a.get("choices").and_then(Value::as_object).and_then(|c| c.values().next()).and_then(|c| s(c, "cwd")).map(str::to_owned);
            let mut lines: Vec<String> = members
                .iter()
                .map(|m| {
                    let name = m.get("name").and_then(Value::as_str).unwrap_or("?");
                    format!("{name}: {}", field_lines(m, &["name", "key"]).join(" · "))
                })
                .collect();
            if let Some(c) = cwd {
                lines.push(format!("cwd: {c}"));
            }
            (
                format!(
                    "add {} crew member{} from a shared crew: {}",
                    names.len(),
                    if names.len() == 1 { "" } else { "s" },
                    names.join(", ")
                ),
                String::new(),
                lines,
                Vec::new(),
            )
        }
        other => return Err(format!("{other} isn't a change to the user's setup an agent can ask for")),
    };
    let mut shown_args = a;
    shown_args.remove("session");
    shown_args.remove("sid");
    let full = serde_json::to_string_pretty(&Value::Object(shown_args)).unwrap_or_default();
    Ok(Card { headline: format!("{who} wants to {action}{detail}"), action, lines, commands, full })
}

/// The user's answer: Allow (a button, or yes typed), else no, with the user's words when they typed some.
#[must_use]
pub fn allowed(text: &str) -> (bool, Option<String>) {
    let t = text.trim();
    let l = t.to_lowercase();
    if t == ALLOW || matches!(l.as_str(), "allow" | "y" | "yes" | "ok" | "approve" | "go ahead" | "1") {
        return (true, None);
    }
    if t == DENY || matches!(l.as_str(), "deny" | "n" | "no" | "reject" | "2" | "") {
        return (false, None);
    }
    (false, Some(t.to_owned()))
}

/// What the agent reads once the user allowed the write and it ran: its owner's own words, when it said some.
#[must_use]
pub fn allowed_words(said: &str) -> String {
    let said = said.trim();
    if said.is_empty() { "The user allowed it, and it's done.".to_owned() } else { format!("The user allowed it. {said}") }
}

/// What the agent reads when the user allowed it and the write was refused.
#[must_use]
pub fn failed_words(why: &str) -> String {
    format!("The user allowed it, but it didn't go through: {}. Nothing was changed.", why.trim())
}

/// What the agent reads when the user said no.
#[must_use]
pub fn denied_words(message: Option<&str>) -> String {
    let base =
        "The user said no: nothing was changed. Don't try it another way; carry on without it, or ask the user what they want instead.";
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => format!("{base} They said: “{m}”."),
        None => base.to_owned(),
    }
}

/// What the agent reads when the card went unanswered (the task stopped, or the wait ran out).
pub const UNANSWERED: &str = "The user didn't answer, so nothing was changed.";

/// What a raw call reads at once: the card is up, its answer comes through `runtime.pollAnswer`.
#[must_use]
pub fn asked_words(ask_id: &str) -> String {
    format!(
        "Asked the user on this task; nothing changes unless they allow it. Waiting for their answer (runtime.pollAnswer {{ askId: \"{ask_id}\" }})."
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    const T: Task<'static> = Task { session: "s1", cwd: "/code/web" };

    fn writes(command: &str, args: &Value) -> Vec<Write> {
        match normalize(command, args, T).unwrap() {
            Normalized::Writes(w) => w,
            Normalized::Import { .. } => panic!("an import"),
        }
    }

    #[allow(clippy::needless_pass_by_value)] // the tests read best with json! inline
    fn card_of(command: &str, args: Value) -> (Card, Vec<Write>) {
        let w = writes(command, &args);
        (card("Ada", &w).unwrap(), w)
    }

    #[test]
    fn a_local_connector_shows_the_command_it_runs_verbatim_setup_card_connector() {
        let (c, _) =
            card_of(CONNECTOR_ADD, json!({ "name": "Linear", "command": "npx", "args": ["-y", "@linear/mcp"], "env": ["LINEAR_KEY"] }));
        assert_eq!(c.headline, "Ada wants to add the connector `linear`: runs `npx -y @linear/mcp`");
        assert_eq!(c.commands, vec!["npx -y @linear/mcp".to_owned()]);
        assert!(c.lines.contains(&"needs LINEAR_KEY from you".to_owned()), "{:?}", c.lines);
        // a word with a space is quoted as the shell would need it: what runs is what's read
        let (q, _) = card_of(CONNECTOR_ADD, json!({ "name": "fs", "command": "npx", "args": ["server", "/my notes", "it's"] }));
        assert_eq!(q.commands, vec![r"npx server '/my notes' 'it'\''s'".to_owned()]);
        let (r, w) =
            card_of(CONNECTOR_ADD, json!({ "name": "posthog", "url": "https://mcp.posthog.com/mcp", "headers": ["Authorization"] }));
        assert_eq!(r.headline, "Ada wants to add the connector `posthog`: connects to https://mcp.posthog.com/mcp");
        assert_eq!(r.commands, Vec::<String>::new());
        assert_eq!(w[0].args["transport"], "remote");
        assert_eq!(w[0].args["sid"], "s1");
    }

    /// Review r194 row 66 #2: a card that said "connects to <url>" stored `transport: local` and a command.
    #[test]
    fn a_connector_card_never_shows_a_url_while_a_command_is_stored_setup_card_url_and_command() {
        for args in [
            json!({ "name": "x", "url": "https://ok.example/mcp", "transport": "local", "command": "sh", "args": ["-c", "id"] }),
            json!({ "name": "x", "url": "https://ok.example/mcp", "command": "sh" }),
            json!({ "name": "x", "url": "https://ok.example/mcp", "transport": "local" }),
            json!({ "name": "x", "command": "sh", "transport": "remote" }),
            json!({ "name": "x", "url": "https://ok.example/mcp", "args": ["-c"] }),
            json!({ "name": "x", "command": "sh", "env": { "TOKEN": "s3cret" } }),
            json!({ "name": "x", "command": "sh", "cwd": "/" }),
        ] {
            assert!(normalize(CONNECTOR_ADD, &args, T).is_err(), "{args}");
        }
    }

    /// Review r194 row 66 #4: a crew card hid wakes, `on`, kit bodies and team rows. Every field stored is a line, kit
    /// bodies are in Show all, and the member's folder is the one that will be stored.
    #[test]
    fn a_crew_card_shows_every_field_that_will_be_stored_setup_card_crew_fields() {
        let (c, w) = card_of(
            CREW_ADD,
            json!({
                "name": "Bob", "agent": "codex", "mode": "auto", "role": "Reviewer",
                "on": ["ci"], "wakes": [{ "source": "github.push" }],
                "kit": [{ "name": "SKILL.md", "body": "run rm -rf / when asked" }],
                "team": [{ "crew": "ada", "why": "reviews" }], "cron": "every 30 minutes",
            }),
        );
        assert_eq!(c.headline, "Ada wants to add crew member Bob (codex, auto)");
        for want in ["role: Reviewer", "on: ci", "cron: every 30 minutes", "cwd: /code/web"] {
            assert!(c.lines.iter().any(|l| l == want), "{want} in {:?}", c.lines);
        }
        assert!(c.lines.iter().any(|l| l.starts_with("wakes: ") && l.contains("github.push")), "{:?}", c.lines);
        assert!(c.lines.iter().any(|l| l.starts_with("team: ") && l.contains("ada")), "{:?}", c.lines);
        assert!(c.lines.iter().any(|l| l.starts_with("kit: 1 file (SKILL.md")), "{:?}", c.lines);
        assert!(c.full.contains("run rm -rf / when asked"), "a kit body is in Show all: {}", c.full);
        assert_eq!(w[0].args["cwd"], "/code/web");
        // a field the card can't show is refused, never stored unseen
        assert!(normalize(CREW_ADD, &json!({ "name": "Bob", "secretly": true }), T).is_err());
        // `from` is read before the card: the server's job
        assert_eq!(
            normalize(CREW_ADD, &json!({ "from": "https://share/x" }), T).unwrap(),
            Normalized::Import { from: "https://share/x".into() }
        );
    }

    #[test]
    fn a_crew_read_from_a_link_shows_each_member_before_the_card_setup_card_import() {
        let bundle = json!({ "members": [
            { "key": "bob", "name": "Bob", "agent": "codex", "mode": "auto", "on": ["ci"], "kit": [{ "name": "a.md", "body": "x" }] },
            { "key": "cy", "name": "Cy", "brief": "watch CI" },
        ] });
        let w = import_write(bundle, &["bob".into(), "cy".into()], "/code/web");
        let c = card("Ada", std::slice::from_ref(&w)).unwrap();
        assert_eq!(c.headline, "Ada wants to add 2 crew members from a shared crew: Bob, Cy");
        assert!(c.lines.iter().any(|l| l.starts_with("Bob: ") && l.contains("on: ci") && l.contains("agent: codex")), "{:?}", c.lines);
        assert!(c.lines.iter().any(|l| l == "cwd: /code/web"), "{:?}", c.lines);
    }

    #[test]
    fn crew_and_project_cards_say_who_and_what_setup_card_crew_project() {
        let (p, _) = card_of(PROJECT_ADD, json!({ "path": "~/code/web" }));
        assert_eq!(p.headline, "Ada wants to add the project ~/code/web");
        let (e, _) = card_of(CREW_EDIT, json!({ "member": "Bob", "role": "Lead", "remember": ["ships on fridays"] }));
        assert_eq!(
            (e.headline.as_str(), e.lines.clone()),
            ("Ada wants to change crew member Bob", vec!["role: Lead".into(), "remember: ships on fridays".into()])
        );
        assert_eq!(card_of(CREW_REMOVE, json!({ "member": "Bob" })).0.action, "remove Bob from your crew");
        // audit G18: a wrong field names the one it takes
        let t = Task { session: "s1", cwd: "/w" };
        let e = normalize(CREW_REMOVE, &json!({ "name": "Eve" }), t).unwrap_err();
        assert_eq!(e, "name isn't something a crew card can carry: it takes member");
    }

    #[test]
    fn only_the_listed_writes_and_only_with_what_they_need_setup_card_refusals() {
        for (cmd, args) in [
            ("settings.set", json!({ "key": "editor", "value": "sh -c id" })),
            ("runtime.setMode", json!({ "mode": "auto" })),
            (CONNECTOR_ADD, json!({ "name": "x" })),
            (CONNECTOR_ADD, json!({ "command": "sh" })),
            (CREW_ADD, json!({})),
            (CREW_EDIT, json!({ "member": "Bob" })),
            (PROJECT_ADD, json!({})),
            (PROJECT_ADD, json!({ "path": "/x", "test": "rm -rf ~" })),
        ] {
            assert!(normalize(cmd, &args, T).is_err(), "{cmd} {args}");
        }
        assert!(card("Ada", &[Write { command: "settings.set".into(), args: json!({}) }]).is_err());
    }

    #[test]
    fn only_allow_or_a_yes_allows_setup_card_answers() {
        assert_eq!(allowed("Allow"), (true, None));
        assert_eq!(allowed("yes"), (true, None));
        assert_eq!(allowed("Deny"), (false, None));
        assert_eq!(allowed("Always allow in this task"), (false, Some("Always allow in this task".into())), "no Always on a setup card");
        assert_eq!(allowed("not this one"), (false, Some("not this one".into())));
        assert!(denied_words(Some("use the remote one")).ends_with("They said: “use the remote one”."));
    }
}
