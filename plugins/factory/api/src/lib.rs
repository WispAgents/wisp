//! wisp-factory-api: software factories (ADR-0114). A factory runs a task through stations in order, each its
//! own agent with its own prompt, a gate that is code in the middle, a person's yes at the spec and the merge,
//! and a loop back to build with a limit:
//!
//! ```yaml
//! factories:
//!   default:
//!     stations:
//!       spec:   { agent: planner, approve: human }
//!       build:  { agent: builder, worktree: true }      # or agents: [claude, codex]
//!       checks: { run: [npm test, npm run lint] }
//!       review: { agents: [reviewer], vote: majority }
//!       ship:   { pr: true, merge: human }
//!     loop: { back_to: build, max: 3, then: ask }
//! ```
//!
//! A run is a parent task (held: it never runs an agent itself) whose children are its stations; where it stands is
//! kept on the parent (`ext["wisp.factory"]`, a [`RunView`]).
//!
//! size-ok: the factory kind's whole contract: its five stations, the loop, the checks of a spec and the run's view

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use wisp_kernel::Command;

pub const PLUGIN_ID: &str = "wisp.factory";
/// The ext slot on a run's parent task (its [`RunView`]) and on each station's task (`{ run, station }`).
pub const EXT: &str = PLUGIN_ID;
/// The factory that's always there (a declared one of that name replaces it).
pub const DEFAULT: &str = "default";
/// The stations, in the order a run goes through them.
pub const STATIONS: [&str; 5] = ["spec", "build", "checks", "review", "ship"];
/// The votes a review counts by (a goal's words, ADR-0110).
pub const VOTES: [&str; 3] = ["majority", "all", "any"];
/// Rounds a run takes when its loop doesn't say.
pub const MAX_ROUNDS: u32 = 3;
/// Minutes a check command may run when its station doesn't say.
pub const CHECK_MINUTES: u64 = 10;

/// Writes the spec: its answer is what every later station is given. Read-only (Plan mode).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecStation {
    /// an agent profile's name or a harness ('' : the task's, else your default)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    /// more instructions for it
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    /// `human` (the default): the run waits for your yes on the spec; `none`: it goes on by itself
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub approve: String,
}

/// Builds it, in a worktree of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildStation {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    /// two or more build it side by side, each in its own worktree; the first whose checks pass goes on
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// a worktree of its own (default: yes, in a project)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    /// plan | edits | auto (default: the run's)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
}

/// The gate that is code: commands run in the build's folder, in order; one that exits non-zero fails it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecksStation {
    /// each `sh -c` in the build's folder (none: the project's own `test` command)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<String>,
    /// minutes each may run (default 10)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Reviewers read the build's folder and each says APPROVE or CHANGES: what to fix.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStation {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// majority (the default) | all | any
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vote: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
}

/// Opens the PR from the build's worktree, and merges it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShipStation {
    /// open a PR (default: yes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<bool>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub draft: bool,
    /// human (the default): it asks before merging | auto: merges once it's open | none: leaves it open
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub merge: String,
}

/// A factory's stations: the ones it has, always run in this order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<SpecStation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildStation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<ChecksStation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewStation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ship: Option<ShipStation>,
}

/// Where a failed check or review goes, how many rounds it may take, and what happens past them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    /// build (the default) | spec
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub back_to: String,
    /// rounds in all (default 3; 0: a failure goes straight to `then`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// ask (the default): it asks you whether to keep going | stop: the run ends failed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub then: String,
}

/// A software factory (`factories:` in a formula).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FactorySpec {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub stations: Stations,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_back: Option<Loop>,
    /// `{ usd: 20 }`: no new station starts once the run's stations have cost this much
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Value>,
}

impl FactorySpec {
    /// The stations it has, in order.
    pub fn station_ids(&self) -> Vec<&'static str> {
        let s = &self.stations;
        let has = [s.spec.is_some(), s.build.is_some(), s.checks.is_some(), s.review.is_some(), s.ship.is_some()];
        STATIONS.iter().zip(has).filter(|(_, h)| *h).map(|(id, _)| *id).collect()
    }

    /// The loop as a run reads it: (`back_to`, `max`, `then`).
    pub fn loop_rule(&self) -> (String, Option<u32>, String) {
        let l = self.loop_back.clone().unwrap_or_default();
        let back = if l.back_to.is_empty() { "build".to_owned() } else { l.back_to };
        let then = if l.then.is_empty() { "ask".to_owned() } else { l.then };
        (back, l.max, then)
    }

    /// `budget.usd`, if any.
    pub fn budget_usd(&self) -> Option<f64> {
        self.budget.as_ref().and_then(|b| b.get("usd")).and_then(Value::as_f64)
    }

    /// Every agent its stations name, with the field that names it (for references, ADR-0111).
    pub fn agents(&self) -> Vec<(String, String)> {
        let s = &self.stations;
        let mut out = Vec::new();
        let mut one = |field: &str, a: &str| {
            if !a.trim().is_empty() {
                out.push((field.to_owned(), a.trim().to_owned()));
            }
        };
        if let Some(x) = &s.spec {
            one("stations.spec.agent", &x.agent);
        }
        if let Some(x) = &s.build {
            one("stations.build.agent", &x.agent);
            x.agents.iter().for_each(|a| one("stations.build.agents", a));
        }
        if let Some(x) = &s.review {
            one("stations.review.agent", &x.agent);
            x.agents.iter().for_each(|a| one("stations.review.agents", a));
        }
        out
    }
}

/// What's wrong with a factory, one line each (unknown stations and fields are refused when it's read).
pub fn check(f: &FactorySpec) -> Vec<String> {
    let mut out = Vec::new();
    let s = &f.stations;
    if s.build.is_none() {
        out.push("a factory needs a build station (stations: spec, build, checks, review, ship)".to_owned());
    }
    if let Some(spec) = &s.spec
        && !["", "human", "none"].contains(&spec.approve.as_str())
    {
        out.push(format!("stations.spec.approve is human or none (not {:?})", spec.approve));
    }
    if let Some(b) = &s.build {
        if !b.agent.is_empty() && !b.agents.is_empty() {
            out.push("stations.build: agent or agents, not both".to_owned());
        }
        if !["", "plan", "edits", "auto"].contains(&b.mode.as_str()) {
            out.push(format!("stations.build.mode is plan, edits or auto (not {:?})", b.mode));
        }
    }
    if let Some(c) = &s.checks {
        if c.run.iter().any(|r| r.trim().is_empty()) {
            out.push("stations.checks.run: a command can't be empty".to_owned());
        }
        if c.timeout == Some(0) {
            out.push("stations.checks.timeout is minutes, at least 1".to_owned());
        }
    }
    if let Some(r) = &s.review {
        if !r.agent.is_empty() && !r.agents.is_empty() {
            out.push("stations.review: agent or agents, not both".to_owned());
        }
        if !r.vote.is_empty() && !VOTES.contains(&r.vote.as_str()) {
            out.push(format!("stations.review.vote is majority, all or any (not {:?})", r.vote));
        }
    }
    if let Some(sh) = &s.ship
        && !["", "human", "auto", "none"].contains(&sh.merge.as_str())
    {
        out.push(format!("stations.ship.merge is human, auto or none (not {:?})", sh.merge));
    }
    if let Some(l) = &f.loop_back {
        let has = f.station_ids();
        let back = l.back_to.as_str();
        if !back.is_empty() && (!["build", "spec"].contains(&back) || !has.contains(&back)) {
            out.push(format!("loop.back_to is a station the factory has, build or spec (not {:?})", l.back_to));
        }
        if !["", "ask", "stop"].contains(&l.then.as_str()) {
            out.push(format!("loop.then is ask or stop (not {:?})", l.then));
        }
    }
    if f.budget.is_some() && f.budget_usd().is_none_or(|u| u <= 0.0) {
        out.push("budget is { usd: <more than 0> }".to_owned());
    }
    out
}

/// The factory that's always there: spec (you approve it) → build → checks (the project's test) → review → ship (a PR
/// you merge), back to build at most 3 rounds, then it asks you.
pub fn builtin_default() -> FactorySpec {
    FactorySpec {
        description: "Spec you approve, build in its own worktree, the project's tests, a review, a PR you merge".to_owned(),
        stations: Stations {
            spec: Some(SpecStation { approve: "human".into(), ..SpecStation::default() }),
            build: Some(BuildStation::default()),
            checks: Some(ChecksStation::default()),
            review: Some(ReviewStation::default()),
            ship: Some(ShipStation { pr: Some(true), merge: "human".into(), ..ShipStation::default() }),
        },
        loop_back: Some(Loop { back_to: "build".into(), max: Some(MAX_ROUNDS), then: "ask".into() }),
        budget: None,
    }
}

/// A factory with its name, as it's stored (`factory.list`, `factory.set`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedFactory {
    pub name: String,
    #[serde(flatten)]
    pub spec: FactorySpec,
}

/// `factory.names`: every factory a run can name, the built-in `default` among them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "factory/")]
pub struct FactoryRow {
    pub name: String,
    pub description: String,
    /// the built-in one (no declared factory has its name)
    pub builtin: bool,
    /// its stations, in order
    pub stations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameArgs {
    pub name: String,
}

/// `factory.stop`: a run's parent task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "factory/")]
pub struct StopArgs {
    pub id: String,
}

/// Where a run stands (`state` on its [`RunView`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "factory/")]
pub enum RunState {
    #[default]
    Running,
    /// it waits on your answer (`waiting`)
    Waiting,
    Done,
    Failed,
    Stopped,
}

/// Where a station stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "factory/")]
pub enum StationState {
    /// not reached yet
    #[default]
    Waiting,
    Running,
    /// it waits on your answer
    Asking,
    Done,
    Failed,
    Skipped,
}

/// One station of a run, as its parent shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "factory/")]
pub struct StationView {
    /// spec | build | checks | review | ship
    pub id: String,
    pub state: StationState,
    /// its tasks (a builder or reviewer each), in order
    pub tasks: Vec<String>,
    /// the round it last ran in (1-based; 0: not yet)
    pub round: u32,
    /// what it gave, short (the spec's first lines, `2 of 3 passed`, `2 approve, 1 asks for changes`, the PR)
    pub output: String,
    /// what it gave, whole: the spec's text, each check's command, exit code and last lines, each reviewer's verdict
    #[ts(type = "unknown")]
    pub detail: Value,
    /// why it failed or was skipped
    pub why: String,
}

/// What the run waits on you for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "factory/")]
pub struct Waiting {
    /// the station that asks
    pub station: String,
    pub question: String,
    pub options: Vec<String>,
    /// the question's id on the parent task (`runtime.answer`'s `askId`)
    pub ask_id: String,
}

/// A run, on its parent task (`ext["wisp.factory"]`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "factory/")]
pub struct RunView {
    pub factory: String,
    pub state: RunState,
    /// the station it's at ('' once it ended)
    pub station: String,
    /// the round it's in (1-based)
    pub round: u32,
    /// rounds in all before it asks or stops (0: none)
    pub max: u32,
    pub stations: Vec<StationView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub waiting: Option<Waiting>,
    /// why it ended failed or stopped
    pub why: String,
    /// what its stations have cost, in USD
    pub cost: f64,
    /// the PR it opened
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pr: Option<String>,
}

macro_rules! command {
    ($name:ident = $id:literal [$perm:literal] $(read $ro:ident)? : $args:ty => $out:ty) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
            $(const READ_ONLY: bool = { let _ = stringify!($ro); true };)?
            type Args = $args;
            type Out = $out;
        }
    };
}

command!(List = "factory.list" ["factory:read"] read yes : Value => Vec<NamedFactory>);
command!(Names = "factory.names" ["factory:read"] read yes : Value => Vec<FactoryRow>);
command!(Set = "factory.set" ["factory:write"] : NamedFactory => NamedFactory);
command!(Remove = "factory.remove" ["factory:write"] : NameArgs => Value);
command!(Stop = "factory.stop" ["factory:run"] : StopArgs => Value);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn read(v: Value) -> Result<FactorySpec, String> {
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    #[test]
    fn a_factory_needs_a_build_and_only_names_the_five_stations() {
        let no_build = read(json!({ "stations": { "spec": {}, "review": {} } })).unwrap();
        assert!(check(&no_build).iter().any(|p| p.contains("needs a build station")), "{:?}", check(&no_build));
        let unknown = read(json!({ "stations": { "build": {}, "deploy": {} } })).unwrap_err();
        assert!(unknown.contains("deploy") && unknown.contains("checks") && unknown.contains("ship"), "{unknown}");
        assert!(read(json!({ "stations": { "build": { "agnet": "x" } } })).unwrap_err().contains("agnet"));
        let bad = read(json!({
            "stations": { "spec": { "approve": "boss" }, "build": { "agent": "a", "agents": ["b"] }, "checks": { "run": [" "] },
                          "review": { "vote": "most" }, "ship": { "merge": "yes" } },
            "loop": { "back_to": "review", "then": "retry" },
            "budget": { "usd": 0 }
        }))
        .unwrap();
        let got = check(&bad);
        for want in [
            "approve is human or none",
            "agent or agents",
            "can't be empty",
            "vote is majority",
            "merge is human",
            "back_to",
            "then is ask",
            "budget",
        ] {
            assert!(got.iter().any(|p| p.contains(want)), "{want}: {got:?}");
        }
        // back_to spec needs a spec station
        let f = read(json!({ "stations": { "build": {} }, "loop": { "back_to": "spec" } })).unwrap();
        assert!(check(&f).iter().any(|p| p.contains("back_to")));
        let ok = read(json!({ "stations": { "build": { "agents": ["claude", "codex"] }, "checks": { "run": ["npm test"] } }, "loop": { "max": 0, "then": "stop" } })).unwrap();
        assert_eq!(check(&ok), Vec::<String>::new());
        assert_eq!(ok.station_ids(), ["build", "checks"]);
        assert_eq!(ok.loop_rule(), ("build".to_owned(), Some(0), "stop".to_owned()));
        assert_eq!(
            ok.agents(),
            [("stations.build.agents".to_owned(), "claude".to_owned()), ("stations.build.agents".to_owned(), "codex".to_owned())]
        );
    }

    #[test]
    fn the_built_in_default_is_spec_build_checks_review_ship() {
        let d = builtin_default();
        assert_eq!(d.station_ids(), STATIONS);
        assert_eq!(check(&d), Vec::<String>::new());
        assert_eq!(d.loop_rule(), ("build".to_owned(), Some(3), "ask".to_owned()));
        assert!(d.agents().is_empty(), "every station runs on the task's agent or your default");
        // it reads back as it's written: a formula can declare the same thing
        let back: FactorySpec = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }
}
