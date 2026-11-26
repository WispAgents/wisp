//! wisp.sandbox's contract (ADR-0047): a sandbox on the user's wisp Grid for one owner (a task, a delegate, a crew
//! job or member, a goal's judges), made and seeded with the project, deleted with its owner.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::Command;

pub const PLUGIN_ID: &str = "wisp.sandbox";

/// `/sandbox <prompt>` in New task: the task runs in a sandbox of its own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchArgs {
    pub prompt: String,
    /// the project that goes along (none: an empty folder there)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// a goal for it (as `runtime.create` takes one): its judges run in the sandbox too
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launched {
    /// the sandbox's machine here
    pub machine: String,
    /// its peer id (the task's key is `<peer>:<task>`)
    pub peer: String,
    pub task: String,
    pub sandbox: String,
    /// how the project's `.wisp/sandbox.sh` went, when it ran
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupRun>,
}

/// A run of the project's `.wisp/sandbox.sh` in a sandbox: whether it exited 0, and the end of what it said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupRun {
    pub ok: bool,
    pub tail: String,
}

/// `sandbox.start`: `sandbox.launch` that answers at once. The launch is a row of its own (`sandbox.launch` on
/// [`LAUNCHES_TOPIC`], keyed by `id`) from the first frame: its steps while the sandbox comes up, the task once it
/// started there, or why it couldn't (kept until dismissed or tried again).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArgs {
    /// the client's id for the launch (its row is shown under it at once): 1–64 of `[A-Za-z0-9_-]`
    pub id: String,
    #[serde(flatten)]
    pub launch: LaunchArgs,
}

/// One launch by its id: `sandbox.retry` (start it again), `sandbox.dismiss` (forget a failed one).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchId {
    pub id: String,
}

/// Where the launches' rows are (`sandbox.launch` entities).
pub const LAUNCHES_TOPIC: &str = "plugin:wisp.sandbox:launches";

/// A seeded sandbox for an owner (a delegate, a crew job or member, a goal's judges): made, or the one it has.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceArgs {
    /// `{task}`, `{lead, member}`, `{crew, job}`, `{crew}` or `{judges}` (wisp-sandbox-core `Owner`)
    pub owner: Value,
    /// the folder whose work goes along
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub machine: String,
    pub peer: String,
    pub sandbox: String,
    /// the project's folder in the sandbox
    pub cwd: String,
    /// false: the owner's sandbox from before
    pub made: bool,
    /// how the project's `.wisp/sandbox.sh` went, when it ran
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupRun>,
}

/// What a sandbox's own wisp takes in: the project as a bundle, its uncommitted changes, its origin and author.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedArgs {
    pub name: String,
    /// the upload the bundle and patch came in as (`POST /api/sandbox/seed/<key>/<part>`, ADR-0060); the lead always
    /// sends one, so the project's size isn't bound by a sync message's
    #[serde(default)]
    pub key: String,
    /// with no `key`: `git bundle create --all`, base64 (a small project; none: an empty folder)
    #[serde(default)]
    pub bundle: String,
    /// with no `key`: what isn't committed, as a binary diff from HEAD
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seeded {
    pub cwd: String,
    /// what `.wisp/sandbox.sh` said, when it ran (once it succeeds: a failed run runs again with the next seed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// that run failed (it exited non-zero, or couldn't start); `setup` says how
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub setup_failed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropArgs {
    /// a task: its sandboxes, its judges' and delegates'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// a crew member: theirs and their jobs'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew: Option<String>,
    /// one sandbox, by its machine
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
}

/// A sandbox's owner, once it's known (a crew job's sandbox, owned by the job's task once it exists).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OwnArgs {
    pub machine: String,
    pub owner: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    pub dropped: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct Empty {}

/// A project's changes in a sandbox since it came in (`sandbox.patch`, inside), or brought back here (`sandbox.bring`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchArgs {
    pub cwd: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    pub patch: String,
    /// the project's state the patch goes up to (a tree id there): once the patch came back, `sandbox.rebase` makes it
    /// the base, so the next job's patch holds only that job's changes (empty: a sandbox from before it)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tree: String,
}

/// `sandbox.rebase` (inside a sandbox): what a project's changes are counted from is now `tree`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebaseArgs {
    pub cwd: String,
    pub tree: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BringArgs {
    pub machine: String,
    /// the project's folder in the sandbox
    pub from: String,
    /// the folder here it goes into
    pub into: String,
    /// keep the changes in a file and leave `into` as it is (a lead in Plan, a job that didn't finish)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brought {
    /// files the changes touched (0: nothing changed there)
    pub files: u32,
    /// why they couldn't go in (they clash with changes here)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// the patch, kept here when it didn't go in (a clash, or `keep`), so the work isn't lost
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
}

/// The grid's code for "you have all the sandboxes you may" (grid/sandboxes.js `SANDBOX_CAP`, in a refusal's details).
pub const CAP_CODE: &str = "sandbox_cap";

/// A full grid (it's full, or the user has all the sandboxes they may): a launch or a delegation fails at once; a crew
/// job waits and tries again (doubling to 30 minutes). Told by the grid's typed code; a grid from before it, by its
/// words. Here, not in wisp-sandbox-core, so other plugins (crew) reach it through this api.
pub fn full(e: &wisp_kernel::WireError) -> bool {
    if e.details.as_ref().and_then(|d| d.get("code")).and_then(Value::as_str) == Some(CAP_CODE) {
        return true;
    }
    let m = e.message.to_lowercase();
    m.contains("is full") || m.contains("sandboxes of yours") || m.contains("at most") || m.contains("the most this grid keeps")
}

/// One sandbox as Settings → Cloud → Sandboxes lists it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxView {
    pub machine: String,
    pub peer: String,
    pub sandbox: String,
    pub name: String,
    pub owner: Value,
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// this grid makes sandboxes, and this computer is signed in to it
    pub enabled: bool,
    /// why not, when it doesn't
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// sign-ins every sandbox starts with (`claude`)
    pub logins: Vec<String>,
    /// env var names every sandbox gets (values never come back)
    pub env: Vec<String>,
    pub sandboxes: Vec<SandboxView>,
}

/// `claude setup-token` on a pty here: its link first, then the code pasted back, then the token goes to the grid.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSignIn {
    /// waiting (for its link), link (open it, paste the code), done, failed
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeArgs {
    pub code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvArgs {
    pub key: String,
    /// empty: the variable goes
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArgs {
    pub agent: String,
}

macro_rules! command {
    ($name:ident = $id:literal : $args:ty => $out:ty $(, $ro:ident)? $(; $typed:ident)?) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            $(const READ_ONLY: bool = { let _ = stringify!($ro); true };)?
            type Args = $args;
            $(fn args_ts() -> Option<String> {
                // `; typed`: the generated CLI's flags and --help come from the argument type (ADR-0022)
                let _ = stringify!($typed);
                Some(format!("{}{}", <$args as ::ts_rs::TS>::docs().unwrap_or_default(), <$args as ::ts_rs::TS>::inline(&::ts_rs::Config::default())))
            })?
            type Out = $out;
        }
    };
}

command!(Launch = "sandbox.launch" : LaunchArgs => Launched);
command!(Start = "sandbox.start" : StartArgs => Empty);
command!(Retry = "sandbox.retry" : LaunchId => Empty);
command!(Dismiss = "sandbox.dismiss" : LaunchId => Empty);
command!(Place = "sandbox.place" : PlaceArgs => Placed);
command!(Seed = "sandbox.seed" : SeedArgs => Seeded);
command!(Drop = "sandbox.drop" : DropArgs => Dropped);
command!(Own = "sandbox.own" : OwnArgs => Empty);
command!(PatchCmd = "sandbox.patch" : PatchArgs => Patch, ro);
command!(Rebase = "sandbox.rebase" : RebaseArgs => Empty);
command!(Bring = "sandbox.bring" : BringArgs => Brought);
command!(InfoCmd = "sandbox.info" : Empty => Info, ro; typed);
// typed: it takes nothing, so `wisp sandbox claude begin --x` is a usage error, not a 30 s sign-in run with --x as a field
command!(ClaudeBegin = "sandbox.claude.begin" : Empty => ClaudeSignIn; typed);
command!(ClaudeCode = "sandbox.claude.code" : CodeArgs => ClaudeSignIn);
command!(ClaudeState = "sandbox.claude.state" : Empty => ClaudeSignIn, ro; typed);
command!(Logout = "sandbox.logout" : AgentArgs => Info);
command!(Env = "sandbox.env" : EnvArgs => Info);

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_kernel::WireError;

    /// The grid's real cap refusal (grid/sandboxes.js) is "full" by its code, and by its words from a grid before the
    /// code; before, "the most this grid keeps" matched nothing, and a crew job that met it was lost.
    #[test]
    fn a_full_grid_is_told_by_its_code_and_by_its_words() {
        let cap = "You have 10 sandboxes, the most this grid keeps. Free one in Settings → Computers → Sandboxes first.";
        assert!(full(&WireError::invalid(cap)));
        assert!(full(&WireError::invalid("no").with_details(serde_json::json!({ "code": CAP_CODE, "have": 10, "max": 10 }))));
        assert!(full(&WireError::invalid("This grid is full right now. Try again in a few minutes.")));
        assert!(full(&WireError::invalid("You have 5 sandboxes of yours already: at most 5.")));
        assert!(!full(&WireError::invalid("Sign in first.")));
        assert!(!full(&WireError::invalid("Sign in first.").with_details(serde_json::json!({ "code": "other" }))));
    }
}
