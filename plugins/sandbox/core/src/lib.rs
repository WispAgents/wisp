//! wisp.sandbox's rules, sans-IO (ADR-0047, v1 `design/sandboxes.md`): who owns a sandbox and when it goes, what the
//! sweep drops, how a project travels in (a git bundle and its uncommitted changes) and where it lands, which
//! harnesses may start in one, and reading `claude setup-token`'s output. Time comes in as `now` (ms).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A launch that never got its task goes after this long (v1).
pub const PENDING_MS: i64 = 30 * 60_000;
/// How often the sweep runs.
pub const SWEEP_MS: u64 = 10 * 60_000;
/// A crew job waiting on a full grid tries again after this, doubling to `RETRY_MAX_MS`.
pub const RETRY_MS: i64 = 60_000;
pub const RETRY_MAX_MS: i64 = 30 * 60_000;

/// Who a sandbox belongs to (its machine's `data.owner`). One owner each; it goes with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Owner {
    /// a task's (`/sandbox`): `<peer>:<id>` of the task, as the app keys it
    Task { task: String },
    /// a launch still making its task
    Pending { pending: i64 },
    /// a delegate's: the lead task and the member's name
    Delegate { lead: String, member: String },
    /// one crew job's: the member and the job's task
    Job { crew: String, job: String },
    /// a crew member's own, kept across their jobs
    Member { crew: String },
    /// a goal's judges, for that task
    Judges { judges: String },
}

impl Owner {
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// A sandbox machine as the plugin reads it from `machines.list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// the machine's local id
    pub machine: String,
    /// the grid's sandbox id
    pub sandbox: String,
    pub owner: Option<Owner>,
    /// its drop failed once: the sweep tries again
    pub dropping: bool,
    /// the project's folder there (`data.cwd`, noted when it was placed; empty: not known)
    pub cwd: String,
    /// a patch of its changes is kept here (`data.kept`: a clash or a hold), work its owner never got
    pub kept: bool,
    /// how its link is (`online`, `asleep`, `off`, `error`, …), as `machines.list` says it
    pub state: String,
    /// when its machine was added here (ms): an owner never written, or a job's never named, goes [`PENDING_MS`]
    /// after this
    pub added: i64,
}

/// The sandboxes among `machines.list`'s rows (the ones with `data.sandbox`).
pub fn held(machines: &[Value]) -> Vec<Held> {
    machines
        .iter()
        .filter_map(|m| {
            // `machines.list` flattens a machine's data into it; a stored row keeps it under `data`
            let data = if m.get("sandbox").is_some() { m } else { m.get("data")? };
            let sandbox = data.get("sandbox")?.as_str()?.to_owned();
            Some(Held {
                machine: m.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
                sandbox,
                owner: data.get("owner").and_then(|o| serde_json::from_value(o.clone()).ok()),
                dropping: data.get("dropping").and_then(Value::as_bool).unwrap_or(false),
                cwd: data.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                kept: data.get("kept").and_then(Value::as_bool).unwrap_or(false),
                state: m.get("state").and_then(Value::as_str).unwrap_or_default().to_owned(),
                added: m.get("addedAt").and_then(Value::as_i64).unwrap_or_default(),
            })
        })
        .collect()
}

/// One sandbox as Settings → Computers → Sandboxes lists it: a machine here keeps it, or only the grid does
/// (`elsewhere`: made from another computer, or its machine here gone), which still holds one of the user's slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// its machine here (empty: elsewhere)
    pub machine: String,
    pub sandbox: String,
    pub name: String,
    pub owner: Value,
    pub state: String,
    pub elsewhere: bool,
}

/// The list Settings shows: every sandbox a machine here keeps (its owner and state as they are here), then each one
/// the grid holds that none here keeps. `grid`: the grid's `sandboxes` (`{id, name}` each), None when it couldn't be
/// asked, and then the list is this computer's alone rather than blank.
pub fn listed(machines: &[Value], grid: Option<&[Value]>) -> Vec<Listed> {
    let mut out: Vec<Listed> = machines
        .iter()
        .filter_map(|m| {
            let data = if m.get("sandbox").is_some() { m } else { m.get("data")? };
            let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            Some(Listed {
                machine: s("id"),
                sandbox: data.get("sandbox")?.as_str()?.to_owned(),
                name: s("name"),
                owner: data.get("owner").cloned().unwrap_or(Value::Null),
                state: s("state"),
                elsewhere: false,
            })
        })
        .collect();
    for g in grid.unwrap_or_default() {
        let Some(id) = g.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) else { continue };
        if out.iter().any(|l| l.sandbox == id) {
            continue;
        }
        let name = g.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(id).to_owned();
        out.push(Listed {
            machine: String::new(),
            sandbox: id.to_owned(),
            name,
            owner: Value::Null,
            state: String::new(),
            elsewhere: true,
        });
    }
    out
}

/// Whether a machine may be offered as a place (Run on, a delegate's place, *Their jobs run on*, Machines): never a
/// sandbox, which belongs to its owner.
pub fn placeable(machine: &Value) -> bool {
    machine.get("sandbox").is_none_or(Value::is_null) && machine.pointer("/data/sandbox").is_none_or(Value::is_null)
}

/// A task's own id, from how its owner names it: `<peer>:<id>`, `<machine>|<id>` (a crew job there), or the id.
pub fn bare(task: &str) -> &str {
    task.rsplit([':', '|']).next().unwrap_or(task)
}

/// The sandboxes that go when task `task` (`<peer>:<id>` or a bare id) is deleted: its own, its judges', its
/// delegates', and a crew job's when the job is that task.
pub fn gone_with_task<'a>(all: &'a [Held], task: &str) -> Vec<&'a Held> {
    let me = bare(task);
    all.iter()
        .filter(|h| match &h.owner {
            Some(Owner::Task { task: t } | Owner::Judges { judges: t } | Owner::Delegate { lead: t, .. } | Owner::Job { job: t, .. }) => {
                bare(t) == me
            }
            _ => false,
        })
        .collect()
}

/// The task an owner names (a task's, its judges', a delegate's lead, a crew job's task), as its own id; None for one
/// that names no task (a launch, a crew member's own) or a crew job not named yet.
fn task_of(owner: &Owner) -> Option<&str> {
    match owner {
        Owner::Task { task: t } | Owner::Judges { judges: t } | Owner::Delegate { lead: t, .. } | Owner::Job { job: t, .. } => {
            Some(bare(t)).filter(|id| !id.is_empty())
        }
        Owner::Pending { .. } | Owner::Member { .. } => None,
    }
}

/// Whether an agent of task `me` may drop or re-own a sandbox `owner` holds (ADR-0136): only one its own task owns (its
/// `/sandbox`, its judges', its delegates', a crew job's when the job is its task). A launch's, a crew member's own,
/// one with no owner (the user's `wisp sandbox new`) and another task's are the user's to delete.
pub fn may_agent_touch(owner: Option<&Owner>, me: &str) -> bool {
    let me = bare(me);
    !me.is_empty() && owner.and_then(task_of) == Some(me)
}

/// Why an agent of task `me` may not drop `h`: another task's (or nobody's), or one still holding work its owner never
/// got (a delegate's, `holds_work`), which only the user deletes. None: it may.
pub fn agent_drop_refusal(h: &Held, me: &str, holds_work: bool) -> Option<String> {
    if !may_agent_touch(h.owner.as_ref(), me) {
        return Some(format!("{}: an agent drops only its own task's sandboxes", h.machine));
    }
    holds_work
        .then(|| format!("{} still holds work that never came back; the user deletes it in Settings → Computers → Sandboxes", h.machine))
}

/// Why an agent of task `me` may not make `next` the owner of a sandbox `now` holds. It names its own task's only, and
/// only on one its task owns already, or a crew job's not named yet that it names as a job of the same member (the
/// job it just handed out starting there: crew names it under the agent's call). Never a launch's (`{pending}`), never
/// someone else's box. None: it may.
pub fn agent_own_refusal(now: Option<&Owner>, next: &Owner, me: &str) -> Option<String> {
    let naming_a_job = matches!(
        (now, next),
        (Some(Owner::Job { crew: a, job: was }), Owner::Job { crew: b, job }) if a == b && bare(was).is_empty() && !bare(job).is_empty()
    );
    if naming_a_job || (may_agent_touch(now, me) && may_agent_touch(Some(next), me)) {
        return None;
    }
    Some("an agent names only its own task's sandboxes, as its own task's".to_owned())
}

/// The sandboxes that go when crew member `crew` leaves: their own, and any job's still here.
pub fn gone_with_member<'a>(all: &'a [Held], crew: &str) -> Vec<&'a Held> {
    all.iter().filter(|h| matches!(&h.owner, Some(Owner::Member { crew: c } | Owner::Job { crew: c, .. }) if c == crew)).collect()
}

/// The task id the minute's look asks a sandbox's own wisp about: a task's, or a crew job's once it's named. A job's
/// sandbox is placed as `{crew, job: ""}` and named after its task starts there; asked about `""` meanwhile, runtime
/// answers not found and the sandbox went mid-launch.
pub fn checkable(h: &Held) -> Option<&str> {
    match &h.owner {
        Some(Owner::Task { task: t } | Owner::Job { job: t, .. }) => Some(bare(t)).filter(|id| !id.is_empty()),
        _ => None,
    }
}

/// The sandboxes the minute's look asks, with the task id each is asked about: only those online now. Asking an
/// asleep box wakes it (a routed command is a real request), so none ever slept; asleep, off or in error, it's kept
/// and asked once it's up again.
pub fn to_check(all: &[Held]) -> Vec<(&Held, &str)> {
    all.iter().filter(|h| !h.dropping && h.state == "online").filter_map(|h| checkable(h).map(|id| (h, id))).collect()
}

/// What the sweep drops: a drop that failed, a launch that never got its task, and a crew job's sandbox never named
/// (crew stopped before `sandbox.own`) [`PENDING_MS`] after its machine was added: it would hold one of the user's
/// few sandboxes for good. One with no owner at all stays: `wisp sandbox new` makes the user's own that way (a make
/// here that fails half way drops its box itself).
pub fn sweep(all: &[Held], now: i64) -> Vec<&Held> {
    all.iter()
        .filter(|h| {
            h.dropping
                || match &h.owner {
                    Some(Owner::Pending { pending }) => now - pending > PENDING_MS,
                    Some(Owner::Job { job, .. }) => bare(job).is_empty() && now - h.added > PENDING_MS,
                    _ => false,
                }
        })
        .collect()
}

/// The sandbox a reusable owner already has (a member's own, a delegate's, a task's judges').
pub fn reusable<'a>(all: &'a [Held], owner: &Owner) -> Option<&'a Held> {
    match owner {
        Owner::Member { .. } | Owner::Delegate { .. } | Owner::Judges { .. } => {
            all.iter().find(|h| !h.dropping && h.owner.as_ref() == Some(owner))
        }
        _ => None,
    }
}

/// A project's folder name in a sandbox (`~/code/<name>`): the folder's last part, letters, digits, `.`, `_`, `-`.
pub fn name_of(cwd: &str) -> String {
    let last = cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default();
    let n: String = last.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' }).collect();
    let n = n.trim_matches(['-', '.']).chars().take(60).collect::<String>();
    if n.is_empty() { "project".to_owned() } else { n }
}

/// A crew member's own sandbox takes each job's project into a folder of its own (`~/code/<name>-<seed>`): the
/// member runs jobs side by side in one sandbox, and a fresh copy into one shared `~/code/<name>` wiped the other
/// job's live folder, and the last job's unpushed work. `seed` is any id unique to this seed; its last 8 letters or
/// digits name it. [`name_of`]'s letters, 60 characters at most.
pub fn seed_folder(cwd: &str, seed: &str) -> String {
    let tail: Vec<char> = seed.chars().filter(char::is_ascii_alphanumeric).collect();
    let tail: String = tail[tail.len().saturating_sub(8)..].iter().collect::<String>().to_ascii_lowercase();
    let tail = if tail.is_empty() { "seed".to_owned() } else { tail };
    let base: String = name_of(cwd).chars().take(60 - 1 - tail.len()).collect();
    format!("{}-{tail}", base.trim_end_matches(['-', '.']))
}

/// Where the project goes raw (ADR-0060): `POST /api/sandbox/seed/<key>/<part>` on the sandbox, sent by
/// `machines.send` (every ref as one bundle, and what isn't committed, new files too), then `sandbox.seed {key}`.
pub const SEED_ROUTE: &str = "/sandbox/seed";

/// The upload of `part` (`bundle`, `patch`) for seed `key`, as a file name in `~/.sandbox/incoming`; `None`: not a
/// part. The key is cut to one path segment, whatever it says.
pub fn upload_name(key: &str, part: &str) -> Option<String> {
    let k: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') { c } else { '-' }).take(80).collect();
    let k = if k.trim_matches('-').is_empty() { "seed".to_owned() } else { k };
    matches!(part, "bundle" | "patch").then(|| format!("{k}.{part}"))
}

/// The git commands that unpack it in the sandbox, in `home/code`: clone the bundle, check out the branch it was on,
/// keep its origin and author. The uncommitted patch is applied after (`git apply`), when there is one. The copy keeps
/// `core.autocrlf=false` in its config, so its files are byte for byte what was committed whatever the box's git says.
pub fn seed_commands(bundle: &str, dir: &str, branch: &str, origin: &str) -> Vec<Vec<String>> {
    let mut out = vec![["clone", "--quiet", "-c", "core.autocrlf=false", bundle, dir].map(str::to_owned).to_vec()];
    if !branch.is_empty() {
        out.push(["-C", dir, "checkout", "--quiet", branch].map(str::to_owned).to_vec());
    }
    out.push(["-C", dir, "remote", "remove", "origin"].map(str::to_owned).to_vec());
    if !origin.is_empty() {
        out.push(["-C", dir, "remote", "add", "origin", origin].map(str::to_owned).to_vec());
    }
    out
}

/// The sign-ins every sandbox starts with, as the grid says them: `{"claude": true}` (or a list of names).
pub fn logins_of(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Object(m)) => m.iter().filter(|(_, on)| on.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_owned).or_else(|| x.get("agent").and_then(Value::as_str).map(str::to_owned)))
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether a harness may start in a sandbox, or why not. Claude (and Auto, which picks Claude there) runs on the
/// user's subscription once they signed in for sandboxes; the others sign in inside the sandbox (their device login,
/// asked on the task's first turn), so they may start.
pub fn may_start(agent: &str, logins: &[String]) -> Result<(), String> {
    let claude = matches!(agent, "" | "auto" | "claude");
    if claude && !logins.iter().any(|l| l == "claude") {
        return Err("Sign in to Claude for sandboxes first: Settings → Cloud → Sandboxes → Sign in (once for all of them).".into());
    }
    Ok(())
}

/// When a crew job that met a full grid tries again: after `RETRY_MS`, doubling per try, at most `RETRY_MAX_MS`.
pub fn retry_in(tries: u32) -> i64 {
    RETRY_MS.saturating_mul(1_i64 << tries.min(10)).min(RETRY_MAX_MS)
}

fn plain(out: &str) -> String {
    // the terminal's colours and cursor moves out (CSI: `ESC [ … final`), and its titles and hyperlinks (OSC:
    // `ESC ] … BEL` or `ESC ] … ESC \`): Claude prints its sign-in link as a hyperlink, whose target and text would
    // otherwise run together into one broken link
    let mut s = String::with_capacity(out.len());
    let mut it = out.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' {
            if c != '\u{7}' {
                s.push(c);
            }
            continue;
        }
        match it.next() {
            Some('[') => {
                for c in it.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    s
}

/// The sign-in link `claude setup-token` printed, once it has.
pub fn setup_link(out: &str) -> Option<String> {
    let p = plain(out);
    let at = p.find("https://")?;
    let link: String = p[at..].chars().take_while(|c| !c.is_whitespace()).collect();
    (link.contains("oauth") || link.contains("claude")).then_some(link)
}

/// The token `claude setup-token` printed at the end (`sk-ant-oat01-…`), once it has, the line breaks the
/// terminal put in it taken out.
pub fn setup_token(out: &str) -> Option<String> {
    token_read(out).map(|(t, _)| t)
}

/// [`setup_token`] once it's printed whole: something after it ends it (a blank line, the next words), so a token
/// still coming in over a wrapped line isn't taken half.
pub fn setup_token_whole(out: &str) -> Option<String> {
    token_read(out).and_then(|(t, ended)| ended.then_some(t))
}

/// The token, and whether anything after it ended it.
fn token_read(out: &str) -> Option<(String, bool)> {
    let p = plain(out);
    let at = p.find("sk-ant-oat01-")?;
    // a wrapped token goes on after a single line break; a blank line (or anything else) ends it
    let text: String = p[at..].chars().filter(|c| *c != '\r').collect();
    let mut tok = String::new();
    let mut breaks = 0;
    let mut ended = false;
    for c in text.chars() {
        if c == '\n' {
            breaks += 1;
            if breaks > 1 {
                ended = true;
                break;
            }
            continue;
        }
        if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            ended = true;
            break;
        }
        breaks = 0;
        tok.push(c);
    }
    (tok.len() > 30).then_some((tok, ended))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use serde_json::json;

    fn m(id: &str, data: Value) -> Value {
        json!({ "id": id, "data": data })
    }

    #[test]
    fn a_sandbox_has_one_owner_and_is_never_a_place_you_pick() {
        let list = vec![
            m("mac", json!({ "computer": "c1" })),
            m("s1", json!({ "sandbox": "a1", "owner": { "task": "h:t1" } })),
            m("s2", json!({ "sandbox": "a2", "owner": { "crew": "ada" } })),
            m("s3", json!({ "sandbox": "a3", "owner": { "crew": "ada", "job": "t9" } })),
            m("s4", json!({ "sandbox": "a4", "owner": { "lead": "t1", "member": "codex" } })),
            m("s5", json!({ "sandbox": "a5", "owner": { "judges": "t1" } })),
            m("s6", json!({ "sandbox": "a6", "owner": { "pending": 5 } })),
        ];
        assert_eq!(list.iter().filter(|x| placeable(x)).count(), 1, "only the computer is a place");
        // as `machines.list` gives it: the data flattened into the machine
        let flat = json!({ "id": "s9", "sandbox": "a9", "owner": { "task": "s9:t" }, "url": "http://sb" });
        assert!(!placeable(&flat));
        assert_eq!(held(&[flat])[0].owner, Some(Owner::Task { task: "s9:t".into() }));
        let h = held(&list);
        assert_eq!(h.len(), 6);
        assert_eq!(h[0].owner, Some(Owner::Task { task: "h:t1".into() }));
        assert_eq!(h[1].owner, Some(Owner::Member { crew: "ada".into() }));
        assert_eq!(h[2].owner, Some(Owner::Job { crew: "ada".into(), job: "t9".into() }));
        assert_eq!(h[3].owner, Some(Owner::Delegate { lead: "t1".into(), member: "codex".into() }));
        assert_eq!(h[4].owner, Some(Owner::Judges { judges: "t1".into() }));
        assert_eq!(h[5].owner, Some(Owner::Pending { pending: 5 }));
        assert_eq!(Owner::Job { crew: "ada".into(), job: "t9".into() }.to_value(), json!({ "crew": "ada", "job": "t9" }));
        // the reusable ones come back for the same owner; a task's never does
        assert_eq!(reusable(&h, &Owner::Member { crew: "ada".into() }).map(|x| x.machine.as_str()), Some("s2"));
        assert_eq!(reusable(&h, &Owner::Task { task: "h:t1".into() }), None);
    }

    #[test]
    fn deleting_the_owner_drops_its_sandboxes_whatever_the_sandbox_answers() {
        let list = vec![
            m("s1", json!({ "sandbox": "a1", "owner": { "task": "h:t1" } })),
            m("s3", json!({ "sandbox": "a3", "owner": { "crew": "ada", "job": "m3|t9" } })),
            m("s4", json!({ "sandbox": "a4", "owner": { "lead": "h:t1", "member": "codex" } })),
            m("s5", json!({ "sandbox": "a5", "owner": { "judges": "t1" } })),
            m("s7", json!({ "sandbox": "a7", "owner": { "task": "h:t2" } })),
            m("s2", json!({ "sandbox": "a2", "owner": { "crew": "ada" } })),
        ];
        let h = held(&list);
        let ids = |v: Vec<&Held>| v.iter().map(|x| x.machine.clone()).collect::<Vec<_>>();
        assert_eq!(ids(gone_with_task(&h, "t1")), ["s1", "s4", "s5"], "its own, its delegates', its judges'");
        assert_eq!(ids(gone_with_task(&h, "h:t9")), ["s3"], "a job's goes with the job");
        assert_eq!(ids(gone_with_member(&h, "ada")), ["s3", "s2"]);
    }

    /// SBX-A2: the grid counts every sandbox of the user's against their cap, but Settings listed only the ones a
    /// machine here keeps; one made elsewhere (or whose machine went) held a slot nobody could see or delete.
    #[test]
    fn settings_lists_every_sandbox_the_grid_holds_and_this_computers_alone_when_the_grid_cant_be_asked() {
        let local = vec![
            json!({ "id": "m1", "name": "Sandbox web", "sandbox": "aa", "owner": { "task": "p:t1" }, "state": "asleep" }),
            json!({ "id": "mac", "name": "Mac", "computer": "c1" }),
        ];
        let grid = vec![json!({ "id": "aa", "name": "web" }), json!({ "id": "bb", "name": "api" }), json!({ "id": "cc", "name": "" })];
        let ids = |v: &[Listed]| v.iter().map(|l| (l.sandbox.clone(), l.elsewhere)).collect::<Vec<_>>();
        // local only (the grid lists none)
        let only = listed(&local, Some(&[]));
        assert_eq!(ids(&only), [("aa".to_owned(), false)]);
        assert_eq!((only[0].machine.as_str(), only[0].state.as_str()), ("m1", "asleep"), "its owner and state as here");
        // both: the local row keeps its owner; the grid's others come after, elsewhere
        let both = listed(&local, Some(&grid));
        assert_eq!(ids(&both), [("aa".to_owned(), false), ("bb".to_owned(), true), ("cc".to_owned(), true)]);
        assert_eq!(both[0].owner, json!({ "task": "p:t1" }));
        assert_eq!((both[1].name.as_str(), both[1].machine.as_str(), &both[1].owner), ("api", "", &Value::Null));
        assert_eq!(both[2].name, "cc", "no name: its id");
        // grid only
        assert_eq!(ids(&listed(&[], Some(&grid[1..2]))), [("bb".to_owned(), true)]);
        // the grid unreachable: this computer's, never blank
        assert_eq!(ids(&listed(&local, None)), [("aa".to_owned(), false)]);
    }

    /// SBX-A1: an agent's `sandbox.drop {machine}` and `sandbox.own` reached any sandbox; now only its own task's, and a
    /// delegate's still holding work is the user's to delete.
    #[test]
    fn an_agent_touches_only_its_own_tasks_sandboxes() {
        let h = held(&[
            m("mine", json!({ "sandbox": "a1", "owner": { "task": "p:A" } })),
            m("judges", json!({ "sandbox": "a2", "owner": { "judges": "A" } })),
            m("delegate", json!({ "sandbox": "a3", "owner": { "lead": "h:A", "member": "codex" } })),
            m("job", json!({ "sandbox": "a4", "owner": { "crew": "ada", "job": "m|A" } })),
            m("theirs", json!({ "sandbox": "a5", "owner": { "task": "p:B" } })),
            m("member", json!({ "sandbox": "a6", "owner": { "crew": "ada" } })),
            m("pending", json!({ "sandbox": "a7", "owner": { "pending": 5 } })),
            m("nobody", json!({ "sandbox": "a8" })),
            m("unnamed", json!({ "sandbox": "a9", "owner": { "crew": "ada", "job": "" } })),
        ]);
        let ok: Vec<&str> = h.iter().filter(|x| may_agent_touch(x.owner.as_ref(), "srv:A")).map(|x| x.machine.as_str()).collect();
        assert_eq!(ok, ["mine", "judges", "delegate", "job"], "own task allowed, every other refused");
        assert!(!may_agent_touch(h[0].owner.as_ref(), ""), "no task, nothing");
        // drop: its own goes; another task's is refused; its own still holding work is the user's
        assert_eq!(agent_drop_refusal(&h[0], "A", false), None);
        assert!(agent_drop_refusal(&h[4], "A", false).unwrap().contains("only its own task's"));
        assert!(agent_drop_refusal(&h[2], "A", true).unwrap().contains("the user deletes it"));
        // own: its own as its own; never another's, never a launch's, never `{pending}` on someone else's
        let task = |t: &str| Owner::Task { task: t.into() };
        assert_eq!(agent_own_refusal(h[0].owner.as_ref(), &Owner::Judges { judges: "A".into() }, "A"), None);
        assert!(agent_own_refusal(h[4].owner.as_ref(), &task("p:A"), "A").is_some(), "stealing another task's");
        assert!(agent_own_refusal(h[0].owner.as_ref(), &task("p:B"), "A").is_some(), "giving its own away");
        assert!(agent_own_refusal(h[4].owner.as_ref(), &Owner::Pending { pending: 1 }, "A").is_some(), "pending-steal");
        assert!(agent_own_refusal(h[0].owner.as_ref(), &Owner::Pending { pending: 1 }, "A").is_some());
        assert!(agent_own_refusal(h[6].owner.as_ref(), &task("p:A"), "A").is_some(), "a launch's isn't its own");
        assert!(agent_own_refusal(h[7].owner.as_ref(), &task("p:A"), "A").is_some(), "nobody's is the user's");
        // a job it handed out starting in that member's sandbox: crew names it, under the agent's call
        let job = |crew: &str| Owner::Job { crew: crew.into(), job: "m|J".into() };
        assert_eq!(agent_own_refusal(h[8].owner.as_ref(), &job("ada"), "A"), None);
        assert!(agent_own_refusal(h[8].owner.as_ref(), &job("bob"), "A").is_some(), "another member's job");
        assert!(agent_own_refusal(h[3].owner.as_ref(), &job("ada"), "B").is_some(), "a named job isn't renamed");
    }

    #[test]
    fn the_sweep_drops_failed_drops_and_launches_that_never_finished() {
        let list = vec![
            m("s1", json!({ "sandbox": "a1", "owner": { "pending": 0 } })),
            m("s2", json!({ "sandbox": "a2", "owner": { "pending": PENDING_MS } })),
            m("s3", json!({ "sandbox": "a3", "owner": { "task": "t" }, "dropping": true })),
            m("s4", json!({ "sandbox": "a4", "owner": { "task": "t" } })),
        ];
        let h = held(&list);
        assert_eq!(sweep(&h, PENDING_MS + 1).iter().map(|x| x.machine.as_str()).collect::<Vec<_>>(), ["s1", "s3"]);
    }

    /// A box as `machines.list` gives it: its data flattened in, its link's state, when it was added.
    fn row(id: &str, state: &str, added: i64, owner: Value) -> Value {
        json!({ "id": id, "sandbox": format!("sb-{id}"), "owner": owner, "state": state, "addedAt": added })
    }

    /// SBX-1: a crew job's sandbox is placed as `{crew, job: ""}` and named once its task started there. The minute's
    /// look asked runtime there about task `""` (not found) and deleted it while it was still being seeded; it's never
    /// asked now, and one crew never named goes with the sweep after `PENDING_MS`, as a launch that never got its task.
    #[test]
    fn a_crew_jobs_sandbox_not_named_yet_is_never_checked_and_the_sweep_takes_it_once_it_is_old() {
        let h = held(&[
            row("unnamed", "online", 1_000, json!({ "crew": "ada", "job": "" })),
            row("named", "online", 1_000, json!({ "crew": "ada", "job": "m|t1" })),
            row("task", "online", 1_000, json!({ "task": "p:t2" })),
        ]);
        assert_eq!(checkable(&h[0]), None, "job \"\" isn't asked about");
        assert_eq!(checkable(&h[1]), Some("t1"));
        assert_eq!(checkable(&h[2]), Some("t2"));
        let ids = |v: Vec<&Held>| v.iter().map(|x| x.machine.clone()).collect::<Vec<_>>();
        assert!(sweep(&h, 1_000 + PENDING_MS).is_empty(), "not before PENDING_MS");
        assert_eq!(ids(sweep(&h, 1_001 + PENDING_MS)), ["unnamed"], "after it, only the one never named");
    }

    /// SBX-2: asking an asleep box about its task wakes it (machines sends a routed command to a box that wakes on a
    /// request), so the minute's look kept every task's sandbox awake. Only online boxes are asked.
    #[test]
    fn the_minutes_look_asks_only_online_task_and_job_sandboxes() {
        let mut list = Vec::new();
        for state in ["online", "asleep", "off", "error", "connecting"] {
            list.push(row(&format!("{state}-task"), state, 0, json!({ "task": "p:t1" })));
            list.push(row(&format!("{state}-job"), state, 0, json!({ "crew": "ada", "job": "m|t2" })));
            list.push(row(&format!("{state}-unnamed"), state, 0, json!({ "crew": "ada", "job": "" })));
            list.push(row(&format!("{state}-member"), state, 0, json!({ "crew": "ada" })));
            list.push(row(&format!("{state}-pending"), state, 0, json!({ "pending": 5 })));
        }
        let h = held(&list);
        let asked: Vec<(&str, &str)> = to_check(&h).into_iter().map(|(h, id)| (h.machine.as_str(), id)).collect();
        assert_eq!(asked, [("online-task", "t1"), ("online-job", "t2")]);
    }

    /// SBX-4: the sweep's rules around a make that failed half way. A make drops its own box on failure (host test
    /// `tests/make.rs`); the sweep keeps an ownerless one, which `wisp sandbox new` makes as the user's own, keeps owned
    /// ones, and takes failed drops and old launches as before.
    #[test]
    fn the_sweep_keeps_ownerless_and_owned_sandboxes_and_takes_failed_drops_and_old_launches() {
        let h = held(&[
            json!({ "id": "bare", "sandbox": "a0", "state": "off", "addedAt": 0 }),
            row("member", "off", 0, json!({ "crew": "ada" })),
            row("job", "off", 0, json!({ "crew": "ada", "job": "m|t1" })),
            row("task", "off", 0, json!({ "task": "p:t1" })),
            row("pending", "off", 0, json!({ "pending": 0 })),
            json!({ "id": "dropping", "sandbox": "a5", "owner": { "task": "p:t" }, "dropping": true, "addedAt": PENDING_MS * 9 }),
            json!({ "id": "young", "sandbox": "a6", "addedAt": PENDING_MS }),
        ]);
        assert_eq!(h[0].owner, None);
        let ids: Vec<&str> = sweep(&h, PENDING_MS + 1).iter().map(|x| x.machine.as_str()).collect();
        assert_eq!(ids, ["pending", "dropping"], "a `wisp sandbox new` box (no owner) is the user's");
    }

    /// SBX-3: each seed into a member's own sandbox gets a folder of its own, never another job's.
    #[test]
    fn a_members_seed_lands_in_a_folder_of_its_own() {
        let a = seed_folder("/Users/me/code/web app/", "019a2b3c-0000-7000-8000-00000000abcd");
        let b = seed_folder("/Users/me/code/web app/", "019a2b3c-0000-7000-8000-00000000abce");
        assert_eq!(a, "web-app-0000abcd");
        assert_ne!(a, b, "unique per seed");
        assert_ne!(a, name_of("/Users/me/code/web app/"));
        let long = seed_folder(&"x".repeat(200), "deadbeef");
        assert!(long.len() <= 60 && long.ends_with("-deadbeef"), "{long}");
        assert_eq!(seed_folder("/a/../.ssh", "../x"), "ssh-x", "letters, digits, ., _ and - only");
        assert_eq!(name_of(&seed_folder("/a/web", "k1")), seed_folder("/a/web", "k1"), "the seed's name_of keeps it");
    }

    #[test]
    fn a_projects_work_goes_in_as_a_bundle_and_its_uncommitted_changes() {
        assert_eq!(name_of("/Users/me/code/web app/"), "web-app");
        assert_eq!(name_of("/"), "project");
        assert_eq!(upload_name("seed-1", "bundle").as_deref(), Some("seed-1.bundle"));
        assert_eq!(upload_name("../../.ssh/x", "patch").as_deref(), Some("-------ssh-x.patch"), "one segment, whatever it says");
        assert_eq!(upload_name("..", "bundle").as_deref(), Some("seed.bundle"));
        assert!(upload_name("k", "../x").is_none());
        let seed = seed_commands("/h/in.bundle", "/h/code/web", "fix-login", "git@github.com:me/web.git");
        assert_eq!(seed[0], ["clone", "--quiet", "-c", "core.autocrlf=false", "/h/in.bundle", "/h/code/web"]);
        assert_eq!(seed[1], ["-C", "/h/code/web", "checkout", "--quiet", "fix-login"]);
        assert_eq!(seed.last().unwrap(), &["-C", "/h/code/web", "remote", "add", "origin", "git@github.com:me/web.git"]);
        assert_eq!(seed_commands("b", "d", "", "").len(), 2, "no branch, no origin: clone, and the bundle's origin goes");
    }

    #[test]
    fn a_launch_on_a_harness_not_signed_in_for_sandboxes_is_refused_up_front() {
        assert!(may_start("claude", &[]).unwrap_err().contains("Settings → Cloud → Sandboxes"));
        assert!(may_start("auto", &[]).is_err());
        assert_eq!(may_start("claude", &["claude".into()]), Ok(()));
        assert_eq!(may_start("codex", &[]), Ok(()), "codex signs in inside, with its device login");
        // the grid says them as a map
        assert_eq!(logins_of(Some(&json!({ "claude": true }))), ["claude"]);
        assert_eq!(logins_of(Some(&json!({ "claude": false }))), Vec::<String>::new());
        assert_eq!(logins_of(Some(&json!(["claude"]))), ["claude"]);
    }

    #[test]
    fn a_crew_job_that_met_a_full_grid_tries_again_later_and_later() {
        assert_eq!((retry_in(0), retry_in(1), retry_in(9)), (60_000, 120_000, RETRY_MAX_MS));
    }

    #[test]
    fn claude_setup_token_shows_its_link_and_takes_the_code_and_the_token_goes_to_the_grid() {
        let first = "\u{1b}[2J\u{1b}[1mBrowser didn't open? Use the url below to sign in:\u{1b}[0m\r\n\r\nhttps://claude.ai/oauth/authorize?code=true&client_id=x&state=y\r\n\r\nPaste code here if prompted > ";
        assert_eq!(setup_link(first).as_deref(), Some("https://claude.ai/oauth/authorize?code=true&client_id=x&state=y"));
        assert_eq!(setup_token(first), None);
        let done = "✓ Long-lived authentication token created successfully!\r\n\r\nYour OAuth token (valid for 1 year):\r\n\r\nsk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWx\r\nYz0123456789_-tail\r\n\r\nStore this token securely.";
        assert_eq!(setup_token(done).as_deref(), Some("sk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-tail"));
        assert_eq!(setup_link("no link yet"), None);
    }

    #[test]
    fn a_sign_in_finished_in_the_browser_gives_its_token_once_printed_whole_and_its_link_reads_once() {
        // Claude Code 2.1: the link as a hyperlink (OSC 8), its text the same URL
        let url = "https://claude.ai/oauth/authorize?code=true&client_id=x&state=y";
        let out = format!(
            "\u{1b}[1mBrowser didn't open?\u{1b}[0m Use the URL below\r\n\r\n\u{1b}]8;id=mk1;{url}\u{7}{url}\u{1b}]8;;\u{7}\r\n\r\nPaste code here if prompted"
        );
        assert_eq!(setup_link(&out).as_deref(), Some(url));
        let st = format!("\u{1b}]8;id=mk1;{url}\u{1b}\\{url}\u{1b}]8;;\u{1b}\\ next");
        assert_eq!(setup_link(&st).as_deref(), Some(url));
        // the callback on localhost finished it: no code pasted, the token printed, perhaps wrapped
        let half = "Your OAuth token (valid for 1 year):\r\n\r\nsk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWx";
        assert_eq!(setup_token_whole(half), None, "still coming in");
        let wrapped = format!("{half}\r\nYz0123456789_-tail");
        assert_eq!(setup_token_whole(&wrapped), None, "a wrapped line may go on");
        let whole = format!("{wrapped}\r\n\r\nStore this token securely.");
        assert_eq!(setup_token_whole(&whole).as_deref(), Some("sk-ant-oat01-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-tail"));
    }
}
