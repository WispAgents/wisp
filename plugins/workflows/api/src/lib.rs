//! wisp-workflows-api: orchestration as code (ADR-0028). A workflow is a DAG of task specs:
//!
//! ```yaml
//! workflows:
//!   - name: release-check
//!     tasks:
//!       tests:  { agent: fast, prompt: Run the tests and report failures }
//!       triage: { needs: [tests], if: "${{ tasks.tests.output.failed > 0 }}", agent: reviewer,
//!                 prompt: "Fix these: ${{ tasks.tests.output.failures }}" }
//!       audit:  { matrix: { agent: [claude, codex] }, prompt: Audit the diff }
//!       report: { needs: [triage, audit], prompt: Write the release note }
//!     concurrency: 2
//!     budget: { usd: 20 }
//! ```
//!
//! A run is a parent task whose children are its steps: every frontend shows it as one row.

/// `${{ … }}` expressions (ADR-0028), also triggers' `if:` (ADR-0043).
pub mod expr;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::Command;

pub const PLUGIN_ID: &str = "wisp.workflows";
/// The ext slot on a run's parent task: its steps and their state.
pub const EXT: &str = PLUGIN_ID;

/// One step of a workflow: a task spec plus when it runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepSpec {
    /// what to do; `${{ tasks.<step>.output.<field> }}`, `${{ inputs.<name> }}` and `${{ matrix.<key> }}`
    /// are filled in
    pub prompt: String,
    /// prompts after the first, each when the turn before ends
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<String>,
    /// steps that must end first
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    /// what the step is, in a few words, for the run's view (default: its name)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// the run's view groups steps by phase, in the order phases first appear (`Explore`, `Build`, `Review`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phase: String,
    /// one run per item of a list an earlier step gave, made once its needs end:
    /// `${{ tasks.plan.output.files }}`; the prompt reads the item as `${{ item }}` and its place as `${{ index }}`
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub foreach: String,
    /// run only when this is true: `${{ tasks.tests.output.failed > 0 }}` (skipped otherwise)
    #[serde(default, rename = "if", skip_serializing_if = "String::is_empty")]
    pub when: String,
    /// one run per combination: `{ agent: [claude, codex] }`
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub matrix: BTreeMap<String, Vec<Value>>,
    /// an agent profile's name or a harness
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effort: String,
    /// plan | edits | auto
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    /// the folder it runs in (default: the run's)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// the computer it runs on (a machine's name; '' = this one)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on: String,
    /// its output's shape (a JSON Schema); without one, the output is what its answer gives
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    /// tries again this many times when it fails
    #[serde(default, skip_serializing_if = "is_zero")]
    pub retries: u32,
    /// minutes it may run before it is stopped and counts as failed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    /// a failure doesn't fail the run (the steps after it still see it failed)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if takes a reference
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A workflow: its steps by name, and what bounds a run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowSpec {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// the folder its steps run in unless they say
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// values a run takes (`wisp run release-check version=2.1`), with their defaults
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Value>,
    pub tasks: BTreeMap<String, StepSpec>,
    /// at most this many steps run at once (0: no limit)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub concurrency: u32,
    /// `{ usd: 20 }`: no new step starts once the run has cost this much
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Value>,
    /// what starts it by itself (`{ trigger: nightly }`); by hand otherwise
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedWorkflow {
    pub name: String,
    #[serde(flatten)]
    pub spec: WorkflowSpec,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NameArgs {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArgs {
    /// a saved workflow's name; with `spec`, the run's name (default `workflow`)
    #[serde(default)]
    pub name: String,
    /// a workflow written for this run (not saved): its `tasks`, `concurrency`, `budget`, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<WorkflowSpec>,
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
    /// the folder its steps run in, over the workflow's
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// the task the run belongs to (its steps show under it, and it is told when the run ends); an agent's run is
    /// always its own task's
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// false: the task isn't sent the result when the run ends (default true when the run belongs to a task)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<bool>,
    /// the trigger that started the run (`trigger:<name>`): its task and every step carry it as their lineage, so the
    /// trigger never fires again on its own run's steps. Additive (2026-11-25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggered_by: Option<String>,
}

/// `workflows.run`: the run's parent task.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Started {
    pub id: String,
    /// what the caller reads back (an agent: where to watch it, that it will be told)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
}

/// `workflows.status` / `workflows.stop`: a run (its task's id).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunId {
    pub id: String,
}

/// `workflows.edit`: changes a run while it goes (ADR-0121). Applied in this order, all or nothing: `change`, `add`,
/// `concurrency`/`budget`, `skip`, `retry`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditArgs {
    /// the run
    pub id: String,
    /// steps (a name: all its runs; or a run's id: `fix[2]`) to skip: the ones working stop, the steps after them run
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
    /// failed or skipped steps to run again (what their failure skipped runs again too)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retry: Vec<String>,
    /// steps to add, by name, as in a spec
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub add: BTreeMap<String, StepSpec>,
    /// steps that haven't started, by name: the fields to set (null clears one)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub change: BTreeMap<String, Value>,
    /// at most this many steps at once (0: no limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<u32>,
    /// `{ usd: 10 }`; null: no budget
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Value>,
}

/// A workflow run started under a task (`workflow.started`). Durable: `wisp events`, `events_why` and triggers read it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStarted {
    /// the task the run belongs to (its steps are its children)
    pub session_id: String,
    pub run: String,
    pub workflow: String,
    pub steps: u32,
}
impl wisp_kernel::Event for WorkflowStarted {
    const TYPE: &'static str = "workflow.started";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
    fn key(&self) -> Option<String> {
        Some(self.run.clone())
    }
}

/// A workflow run ended (`workflow.ended`): done, failed or stopped, with how many steps ended done and what it cost.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEnded {
    pub session_id: String,
    pub run: String,
    pub workflow: String,
    /// `done`, `failed` or `stopped`
    pub state: String,
    pub done: u32,
    pub total: u32,
    pub cost_usd: f64,
}
impl wisp_kernel::Event for WorkflowEnded {
    const TYPE: &'static str = "workflow.ended";
    const DURABLE: bool = true;
    fn subject(&self) -> Option<String> {
        Some(self.session_id.clone())
    }
    fn key(&self) -> Option<String> {
        Some(self.run.clone())
    }
}

/// A step's state in a run, on the parent task (`ext["wisp.workflows"].steps`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    #[default]
    Waiting,
    Running,
    Done,
    Failed,
    Skipped,
}

macro_rules! command {
    ($name:ident = $id:literal [$perm:literal] $(read $ro:ident)? : $args:ty => $out:ty $(; $typed:ident)?) => {
        #[derive(Debug)]
        pub struct $name;
        impl Command for $name {
            const ID: &'static str = $id;
            const PERMISSION: Option<&'static str> = Some($perm);
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

command!(List = "workflows.list" ["workflows:read"] read yes : Value => Vec<NamedWorkflow>; typed);
command!(Set = "workflows.set" ["workflows:write"] : NamedWorkflow => NamedWorkflow);
command!(Remove = "workflows.remove" ["workflows:write"] : NameArgs => Value);
command!(Starters = "workflows.starters" ["workflows:read"] read yes : Value => Vec<NamedWorkflow>; typed);
command!(Status = "workflows.status" ["workflows:read"] read yes : RunId => Value);
command!(Stop = "workflows.stop" ["workflows:run"] : RunId => Value);

/// `workflows.edit`: its arguments as the generated CLI and the MCP tool show them.
pub const EDIT_TS: &str = r#"/** Change a workflow run of this task while it goes: skip a step (the ones working stop; the steps after it still run), retry a failed or skipped one (what its failure skipped runs again too), add steps, change a step that hasn't started, or change the run's concurrency and budget. A step is named by its name (all its runs) or a run's id from workflows_status (fix[2]). All or nothing: a refusal says why and changes nothing. Works on a run that ended too: retry or add starts it again. */ { /** the run's id (workflows_run gave it) */ id: string, /** steps to skip */ skip?: Array<string>, /** failed or skipped steps to run again */ retry?: Array<string>, /** steps to add by name: { "check-docs": { "prompt": "…", "needs": ["fix"] } } */ add?: Record<string, unknown>, /** steps that haven't started by name: the fields to set: { "report": { "agent": "codex" } } */ change?: Record<string, unknown>, /** at most this many steps at once (0: no limit) */ concurrency?: number, /** { "usd": 10 } (null: none) */ budget?: Record<string, unknown> }"#;

#[derive(Debug)]
pub struct Edit;
impl Command for Edit {
    const ID: &'static str = "workflows.edit";
    const PERMISSION: Option<&'static str> = Some("workflows:run");
    type Args = EditArgs;
    type Out = Value;
    fn args_ts() -> Option<String> {
        Some(EDIT_TS.to_owned())
    }
}

/// `workflows.run`: its arguments as the generated CLI and the MCP tool show them, with how to write a workflow.
pub const RUN_TS: &str = r#"/** Run a workflow: a DAG of tasks, each its own agent on any harness, under this task. Returns at once; the task is told when the run ends (each step's state and output), and the user watches it live on the task. Steps run in the task's folder.
Write `spec` for this run, or name a saved workflow. spec = { tasks: { <step>: { prompt, needs?, phase?, title?, agent?, model?, effort?, mode?, if?, matrix?, foreach?, output?, retries?, timeout?, optional? } }, concurrency?, budget?: { usd } }.
- needs: [steps] that end first; a step reads their results in its prompt as ${{ tasks.<step>.output }} (or .output.<field>, .output.text).
- foreach: "${{ tasks.plan.output.items }}": one run per item of a list an earlier step gave (fan-out decided at run time); its prompt reads ${{ item }} and ${{ index }}. A later step reading ${{ tasks.<step>.output }} gets every run's [{ matrix: { item }, state, output }].
- matrix: { agent: [claude, codex] }: one run per value (a matrix key that is a field sets it), read as ${{ matrix.agent }}.
- output: a JSON Schema: the step is asked to end with an output block of that shape, so the next steps read fields (an items list for a foreach).
- if: "${{ tasks.tests.output.failed > 0 }}": skipped unless true. phase: groups steps in the view (Explore, Build, Review). mode: plan | edits | auto (never above this task's).
Make every prompt self-contained: a step sees only its prompt. While it runs, workflows_edit skips, retries, adds or changes steps; wisp tells you when a step fails. */ { /** a saved workflow's name (or the run's name with spec) */ name?: string, /** the workflow to run when it isn't a saved one (see above) */ spec?: Record<string, unknown>, /** values for the workflow's inputs: { version: "2.1" } */ inputs?: Record<string, unknown>, /** the folder its steps run in (the user's runs only; an agent's run in its task's) */ project?: string, /** the task it runs under (an agent's is its own) */ task?: string, /** false: don't tell the task when it ends */ notify?: boolean }"#;

#[derive(Debug)]
pub struct Run;
impl Command for Run {
    const ID: &'static str = "workflows.run";
    const PERMISSION: Option<&'static str> = Some("workflows:run");
    type Args = RunArgs;
    type Out = Started;
    fn args_ts() -> Option<String> {
        Some(RUN_TS.to_owned())
    }
}
