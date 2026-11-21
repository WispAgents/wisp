//! A delegate's copy on another machine (ADR-0059), the pure part: where a copy lives (always under wisp's own
//! folder there), which folders give changes back, the git commands that pack, unpack and diff, and the words.

use std::path::{Component, Path, PathBuf};

/// The folder (under the plugin's own) that holds every copy: `<wisp>/plugins/wisp.machines/copies`.
pub const COPIES: &str = "copies";
/// Uploads waiting for their `machines.takeIn`: `<wisp>/plugins/wisp.machines/incoming`.
pub const INCOMING: &str = "incoming";
/// Patches that didn't go in, kept here: `<wisp>/plugins/wisp.machines/patches`.
pub const PATCHES: &str = "patches";
/// The biggest bundle sent (v1 relay `MAX_PATCH`); the upload route takes a little more.
pub const MAX_BUNDLE: usize = wisp_machines_api::MAX_CARRY;
/// The upload route here (`POST /api/machines/carry/<key>/<part>`, ADR-0059).
pub const CARRY_ROUTE: &str = "/machines/carry";
/// A machine's drops that didn't reach it yet (its row's `data`), for the sweep (ADR-0060).
pub const UNCARRY: &str = "uncarry";
/// How often the sweep tries them again (and whenever the machine comes online).
pub const SWEEP_MS: u64 = 10 * 60_000;
/// The drops the sweep gave up on (its row's `data`): each one's lead, task and why, the machine's note that a copy
/// stayed there for you to remove by hand.
pub const STUCK: &str = "uncarryStuck";
/// How many times a machine that answered may refuse a drop (a copy it can't remove) before the sweep gives up on it:
/// an hour of sweeps. A machine that can't be reached never counts (it takes the drop once it's back).
pub const GIVE_UP: u32 = 6;
/// The biggest patch that comes back over the link.
pub const MAX_PATCH: usize = 16 * 1024 * 1024;
/// The marker of what a copy was when it came in (its changes are the difference from it).
pub const BASE_FILE: &str = ".wisp-copy-base";

/// A name that can only be one path segment: letters, digits, `-`, `_` and `.` kept, the rest `-`, no leading dot,
/// at most 80 long. Empty or all dots: `work`.
pub fn segment(s: &str) -> String {
    let mut out: String =
        s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).take(80).collect();
    while out.starts_with('.') {
        out.remove(0);
    }
    if out.trim_matches(['-', '.']).is_empty() { "work".to_owned() } else { out }
}

/// The project's folder name, from the lead's folder: its last segment.
pub fn name_of(cwd: &str) -> String {
    segment(cwd.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default())
}

/// The copy for `key` of project `name`: `<plugin>/copies/<key>/<name>`, whatever the two say.
pub fn copy_dir(plugin: &Path, key: &str, name: &str) -> PathBuf {
    plugin.join(COPIES).join(segment(key)).join(segment(name))
}

/// The upload of `part` (`bundle`, `patch`) for `key`; `None`: not a part.
pub fn incoming(plugin: &Path, key: &str, part: &str) -> Option<PathBuf> {
    matches!(part, "bundle" | "patch").then(|| plugin.join(INCOMING).join(format!("{}.{part}", segment(key))))
}

/// Whether `cwd` is a copy carried here: two segments below `<plugin>/copies`, with no `..` on the way.
pub fn is_copy(plugin: &Path, cwd: &str) -> bool {
    let p = Path::new(cwd);
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    p.strip_prefix(plugin.join(COPIES)).is_ok_and(|rest| rest.components().count() == 2)
}

/// `git` args that pack the lead's folder (in `cwd`) into `bundle`: its HEAD and branch, with their history; `all`:
/// every ref (a sandbox's project, ADR-0047).
pub fn pack_commands(bundle: &str, branch: &str, all: bool) -> Vec<Vec<String>> {
    let mut b = vec!["bundle".to_owned(), "create".to_owned(), bundle.to_owned()];
    if all {
        b.push("--all".to_owned());
    } else {
        b.push("HEAD".to_owned());
        if !branch.is_empty() {
            b.push(branch.to_owned());
        }
    }
    vec![b]
}

/// Whether `to` may take a `machines.send`: a route prefix of plain segments (`/sandbox/seed`), never the sync
/// stream or the command route.
pub fn send_route_ok(to: &str) -> bool {
    let Some(rest) = to.strip_prefix('/') else { return false };
    let segs: Vec<&str> = rest.split('/').collect();
    !rest.is_empty()
        && segs.iter().all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        && !matches!(segs.first().copied(), Some("cmd" | "events" | "peer"))
}

/// The start of every copy's and upload's name for lead `lead`'s delegates (their key is `<lead>-<member>`).
pub fn lead_prefix(lead: &str) -> String {
    format!("{}-", segment(lead))
}

/// Whether a copy folder (under `copies/`) or an upload (under `incoming/`) named `name` is lead `lead`'s.
pub fn of_lead(name: &str, lead: &str) -> bool {
    !lead.trim().is_empty() && name.starts_with(&lead_prefix(lead))
}

/// One drop a machine didn't take yet: a lead's delegate there (its task, its copies).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Drop {
    pub lead: String,
    #[serde(default)]
    pub task: String,
    /// how many times the machine answered and refused it (`GIVE_UP`)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tries: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's skip_serializing_if takes a reference")]
fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Drop {
    /// The same drop (its lead and task), however often it was tried.
    pub fn same(&self, o: &Self) -> bool {
        self.lead == o.lead && self.task == o.task
    }
}

/// A drop the sweep gave up on, as its machine's row keeps it (`data.uncarryStuck`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stuck {
    pub lead: String,
    #[serde(default)]
    pub task: String,
    /// what the machine said, last time
    #[serde(default)]
    pub why: String,
}

/// The drops kept in a machine's row (`data.uncarry`); anything else there reads as none.
pub fn pending(data: Option<&serde_json::Value>) -> Vec<Drop> {
    data.and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// `list` with `d` in it once (one already there keeps its tries).
pub fn with_drop(mut list: Vec<Drop>, d: Drop) -> Vec<Drop> {
    if !list.iter().any(|x| x.same(&d)) {
        list.push(d);
    }
    list
}

/// `list` without `d`.
pub fn without(list: Vec<Drop>, d: &Drop) -> Vec<Drop> {
    list.into_iter().filter(|x| !x.same(d)).collect()
}

/// The machine answered and refused `d` (`why`), as its row's `data` keeps it: one more try counted; at `GIVE_UP` the
/// sweep lets it go, and the row's `uncarryStuck` says which copy stayed there and why (once per drop). True when it
/// gave up.
pub fn refused(data: &mut std::collections::BTreeMap<String, serde_json::Value>, d: &Drop, why: &str) -> bool {
    let mut list = pending(data.get(UNCARRY));
    let Some(at) = list.iter().position(|x| x.same(d)) else { return false };
    list[at].tries += 1;
    let gave_up = list[at].tries >= GIVE_UP;
    if gave_up {
        list.remove(at);
        let mut stuck: Vec<Stuck> = data.get(STUCK).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        stuck.retain(|x| !(x.lead == d.lead && x.task == d.task));
        stuck.push(Stuck { lead: d.lead.clone(), task: d.task.clone(), why: why.to_owned() });
        data.insert(STUCK.to_owned(), serde_json::to_value(&stuck).unwrap_or(serde_json::Value::Null));
    }
    if list.is_empty() {
        data.remove(UNCARRY);
    } else {
        data.insert(UNCARRY.to_owned(), serde_json::to_value(&list).unwrap_or(serde_json::Value::Null));
    }
    gave_up
}

/// The `git` args that unpack it, run in the copies folder: a clone of the bundle, the lead's commit checked out on
/// the lead's branch (`wisp/delegate` when it was detached), its remote gone (a delegate never pushes). The copy is
/// wisp's own, so it keeps `core.autocrlf=false` in its config: every file comes out byte for byte as committed and the
/// lead's patch applies as it was made, whatever the machine's git says (Git for Windows' system default is `true`,
/// which would check out CRLF and make every line a change). The lead's own checkout keeps the user's settings.
pub fn unpack_commands(bundle: &str, dir: &str, head: &str, branch: &str) -> Vec<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    let on = if branch.is_empty() { "wisp/delegate" } else { branch };
    vec![
        s(&["clone", "--quiet", "--no-checkout", "-c", "core.autocrlf=false", bundle, dir]),
        s(&["-C", dir, "checkout", "--quiet", "-B", on, head]),
        s(&["-C", dir, "remote", "remove", "origin"]),
    ]
}

/// The files a patch touches.
pub fn files_in(patch: &str) -> u32 {
    u32::try_from(patch.lines().filter(|l| l.starts_with("diff --git ")).count()).unwrap_or(u32::MAX)
}

/// The words for a lead's folder that can't go.
pub fn not_a_repo(cwd: &str) -> String {
    format!("{cwd} isn't a git repo with a commit: only a repo's work goes to another machine")
}

pub fn too_big(mb: usize) -> String {
    format!("The project is too big to send ({mb} MB; at most {} MB).", MAX_BUNDLE >> 20)
}

pub fn not_a_copy() -> &'static str {
    "Only a copy wisp carried to this machine gives changes back."
}

pub fn clash(kept: &str, why: &str) -> String {
    format!("Its changes clash with yours here, so none went in. They are kept in {kept}. ({why})")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_always_under_wisps_own_folder() {
        let plugin = Path::new("/h/.wisp/plugins/wisp.machines");
        assert_eq!(copy_dir(plugin, "lead1-ada", "app"), plugin.join("copies/lead1-ada/app"));
        // whatever the key and name say, two segments below copies
        for (k, n) in [("../../code", "../app"), ("/etc", "."), ("a/b", "..\\x"), ("", "")] {
            let d = copy_dir(plugin, k, n);
            assert!(d.starts_with(plugin.join(COPIES)), "{d:?}");
            assert!(is_copy(plugin, d.to_str().unwrap()), "{d:?}");
        }
        assert_eq!(segment("../../code"), "-..-code");
        assert_eq!(name_of("/Users/me/code/my app/"), "my-app");
    }

    #[test]
    fn only_a_carried_copy_gives_changes_back() {
        let plugin = Path::new("/h/.wisp/plugins/wisp.machines");
        assert!(is_copy(plugin, "/h/.wisp/plugins/wisp.machines/copies/k/app"));
        for no in [
            "/Users/me/code/app",
            "/h/.wisp/plugins/wisp.machines/copies/k",
            "/h/.wisp/plugins/wisp.machines/copies/k/app/sub",
            "/h/.wisp/plugins/wisp.machines/copies/k/../../../../../Users/me/app",
        ] {
            assert!(!is_copy(plugin, no), "{no}");
        }
        assert!(incoming(plugin, "k", "bundle").is_some());
        assert!(incoming(plugin, "k", "../x").is_none());
    }

    #[test]
    fn a_detached_lead_gets_a_branch_of_its_own_there() {
        let c = unpack_commands("/b", "/d", "abc", "");
        assert_eq!(c[0], ["clone", "--quiet", "--no-checkout", "-c", "core.autocrlf=false", "/b", "/d"], "a byte-exact copy");
        assert_eq!(c[1], ["-C", "/d", "checkout", "--quiet", "-B", "wisp/delegate", "abc"]);
        let c = unpack_commands("/b", "/d", "abc", "fix");
        assert_eq!(c[1][5], "fix");
        assert_eq!(pack_commands("/b", "fix", false)[0], ["bundle", "create", "/b", "HEAD", "fix"]);
        assert_eq!(pack_commands("/b", "fix", true)[0], ["bundle", "create", "/b", "--all"]);
        assert_eq!(files_in("diff --git a/x b/x\n+1\ndiff --git a/y b/y\n"), 2);
    }

    #[test]
    fn a_send_goes_only_to_a_route_prefix() {
        for ok in ["/sandbox/seed", CARRY_ROUTE, "/a_b/c-1"] {
            assert!(send_route_ok(ok), "{ok}");
        }
        for no in ["", "/", "sandbox/seed", "/cmd", "/events", "/peer/m1/api/cmd", "/a/../cmd", "/a//b", "/a/b/", "/a?b", "/a b"] {
            assert!(!send_route_ok(no), "{no}");
        }
        assert!(too_big(300).contains("at most 256 MB"), "{}", too_big(300));
    }

    #[test]
    fn dropping_a_leads_copies_takes_only_that_leads() {
        assert!(of_lead(&segment("t1-far"), "t1"));
        assert!(of_lead("t1-far.bundle", "t1"));
        assert!(!of_lead("t10-far", "t1"), "another lead whose id starts the same");
        assert!(!of_lead("t1", "t1"));
        assert!(!of_lead("work-far", ""), "no lead: nothing");
        let l = with_drop(Vec::new(), Drop { lead: "t1".into(), task: "r1".into(), tries: 0 });
        let l = with_drop(l, Drop { lead: "t1".into(), task: "r1".into(), tries: 0 });
        assert_eq!(l.len(), 1);
        let v = serde_json::to_value(&l).unwrap();
        assert_eq!(pending(Some(&v)), l);
        assert!(pending(Some(&serde_json::json!("junk"))).is_empty());
    }

    /// A copy a machine can never remove isn't tried forever: each refusal counts, the count survives the drop being
    /// asked again, and at `GIVE_UP` it leaves the sweep and the machine's row notes it (once); another drop is untouched.
    #[test]
    fn a_drop_the_machine_keeps_refusing_is_given_up_with_a_note_on_its_row() {
        let d = Drop { lead: "t1".into(), task: "r1".into(), tries: 0 };
        let other = Drop { lead: "t2".into(), task: String::new(), tries: 0 };
        let mut data = std::collections::BTreeMap::new();
        let list = with_drop(with_drop(Vec::new(), d.clone()), other.clone());
        data.insert(UNCARRY.to_owned(), serde_json::to_value(&list).unwrap());
        for n in 1..GIVE_UP {
            assert!(!refused(&mut data, &d, "a copy stayed"), "try {n}");
            // asked again meanwhile (the lead's delete retried): still one, its count kept
            let again = with_drop(pending(data.get(UNCARRY)), d.clone());
            assert_eq!(again.len(), 2);
            assert_eq!(again.iter().find(|x| x.same(&d)).unwrap().tries, n);
        }
        assert!(refused(&mut data, &d, "a copy stayed: t1-far: Directory not empty"), "given up at GIVE_UP");
        assert_eq!(pending(data.get(UNCARRY)), vec![other.clone()], "only the other drop is swept");
        assert_eq!(data[STUCK], serde_json::json!([{ "lead": "t1", "task": "r1", "why": "a copy stayed: t1-far: Directory not empty" }]));
        assert!(!refused(&mut data, &d, "again"), "gone from the sweep: nothing more counted");
        assert_eq!(data[STUCK].as_array().unwrap().len(), 1);
        // a drop never tried still reads as it did (no tries field)
        assert_eq!(serde_json::to_value(&other).unwrap(), serde_json::json!({ "lead": "t2", "task": "" }));
    }
}
