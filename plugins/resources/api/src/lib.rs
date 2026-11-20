//! wisp-resources-api: wisp as code (ADR-0026, ADR-0028). Everything a user sets up is a *resource*:
//! a kind, a stable name and a typed spec. Each plugin owns its kinds through one point,
//! `resource.kind@1`; the kernel knows none. `wisp.resources` plans, applies and exports over all of them.
//!
//! A kind gives:
//! - its schema (JSON Schema generated from the Rust spec type and its doc comments: [`schema_of`]);
//! - `read`: the current state, as specs;
//! - optionally its own `plan`; otherwise the generic one (wisp-resources-core) applies: a desired spec
//!   is a patch over the current one, so the fields a formula leaves out keep their value;
//! - `apply(op)`: through the owner's own commands, so permissions, events and the change feed work as
//!   for a click in Settings.

mod adapters;

pub use adapters::{CrudKind, SettingsKind, StoredKind, stored, stored_published, stored_published_tidied, stored_published_unique};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::{BoxFuture, Call, Command, Point, Result};

pub use schemars;

pub const PLUGIN_ID: &str = "wisp.resources";
/// The formula format this build reads and writes (`wisp: 1`).
pub const FORMAT: u32 = 1;

/// One declared thing: `crew/Ada`, `setting/worktrees.max`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    pub kind: String,
    pub name: String,
    /// the spec as the kind's schema describes it (secrets as `${secret:NAME}` references)
    #[serde(default)]
    pub spec: Value,
    /// the owner's own id for it, when that isn't its name (read only; never exported)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Resource {
    pub fn new(kind: impl Into<String>, name: impl Into<String>, spec: Value) -> Self {
        Self { kind: kind.into(), name: name.into(), spec, id: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Create,
    Update,
    Delete,
}

/// One field a plan changes (`model`: `sonnet` → `opus`); a path into the spec, `/`-separated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    pub op: OpKind,
    pub kind: String,
    pub name: String,
    /// the whole spec after the op (create, update), or the one that goes (delete)
    pub spec: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<FieldChange>,
    /// an update's spec before it: what a rollback puts back
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Value>,
}

/// Who a kind's resources belong to: exports leave out `personal` and `device` ones unless asked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Shared,
    /// yours (harness accounts): `--include account`
    Personal,
    /// this device's (theme, layout): `--include ui`
    Device,
}

/// What a kind is, for the engine, the CLI and Settings → Import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // each is a fact a formula's reader needs about the kind
pub struct KindInfo {
    /// `crew`, `setting`
    pub kind: String,
    /// its section in a formula (`crew:`, `agents:`, `settings:`)
    pub section: String,
    /// the plugin that owns it
    pub owner: String,
    #[serde(default)]
    pub scope: Scope,
    /// it changes who may act (capability, channel, machine, trigger): a foreign formula needs `--trust`
    #[serde(default)]
    pub sensitive: bool,
    /// applied in this order (lower first; deletes go in reverse)
    #[serde(default)]
    pub order: i32,
    /// the section is a map of name → value (`settings: { worktrees.max: 4 }`), not a list of named specs
    #[serde(default)]
    pub map_form: bool,
    /// the section is one object, the kind's one resource (`budget: { stopAt: 75 }`), named `default`
    #[serde(default)]
    pub singleton: bool,
    /// its specs are some of the owner's settings: the generic `setting` kind leaves those keys to it
    #[serde(default)]
    pub claims_settings: bool,
    /// other section names a formula may use for it (`recipes:` for `tasks:`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// JSON Schema of one spec
    pub schema: Value,
    /// one line for `wisp schema` and Settings → Import
    #[serde(default)]
    pub doc: String,
    /// why the kind is put away for now (Auto off: "Auto is off (runtime.auto), so the router is put away"): no
    /// export, kinds list, formula, plan or apply takes it, and a plan naming it says this. Additive (2026-11-19,
    /// setup audit D2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away: Option<String>,
    /// The fields of its specs that name another kind's resource (a task's `agent`, a trigger's `task`), checked by the
    /// plan wherever a formula is read: one that names nothing is refused, naming what there is. Additive (2026-11-20,
    /// ADR-0111).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<Names>,
}

/// A field that names another kind's resource ([`KindInfo::names`]). `field` is a path into the spec: `agent`,
/// `connectors[]` (each item), `members.*.agent` (each value's `agent`), `team[].crew`. Only strings are names: an inline
/// object (`judges: [{ harness: codex }]`), a URL, a path, a `${…}` left unevaluated or one of `skip` (an event word
/// beside trigger names) is never checked. `acyclic`: these names may not loop back to where they start (a crew member
/// who delegates to one who delegates back). Additive (2026-11-20, ADR-0111).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Names {
    pub field: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub acyclic: bool,
}

impl Names {
    #[must_use]
    pub fn new(field: &str, kind: &str) -> Self {
        Self { field: field.into(), kind: kind.into(), skip: Vec::new(), acyclic: false }
    }
    /// Words in this field that aren't names (`finished` among a notification's trigger names).
    #[must_use]
    pub fn skip(mut self, words: &[&str]) -> Self {
        self.skip = words.iter().map(|w| (*w).to_owned()).collect();
        self
    }
    #[must_use]
    pub fn acyclic(mut self) -> Self {
        self.acyclic = true;
        self
    }
}

/// Whether `s` could be a resource's name rather than something else in a name field: a URL, a path, an expression or
/// `~/…` never is.
#[must_use]
pub fn is_name(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && !s.contains("://") && !s.contains('/') && !s.contains('\\') && !s.contains("${") && !s.starts_with('~')
}

/// Every name `n` finds in `spec` ([`Names`]'s path rules), in order.
#[must_use]
pub fn names_in(spec: &Value, n: &Names) -> Vec<String> {
    fn walk(v: &Value, path: &[&str], out: &mut Vec<String>) {
        let Some((head, rest)) = path.split_first() else {
            if let Some(s) = v.as_str() {
                out.push(s.trim().to_owned());
            }
            return;
        };
        if *head == "*" {
            for x in v.as_object().into_iter().flat_map(|m| m.values()) {
                walk(x, rest, out);
            }
            return;
        }
        let (key, each) = head.strip_suffix("[]").map_or((*head, false), |k| (k, true));
        let Some(x) = (if key.is_empty() { Some(v) } else { v.get(key) }) else { return };
        if each {
            for item in x.as_array().into_iter().flatten() {
                walk(item, rest, out);
            }
        } else {
            walk(x, rest, out);
        }
    }
    let parts: Vec<&str> = n.field.split('.').collect();
    let mut out = Vec::new();
    walk(spec, &parts, &mut out);
    out.retain(|s| is_name(s) && !n.skip.iter().any(|w| w == s));
    out
}

impl KindInfo {
    pub fn new(kind: &str, section: &str, owner: &str, schema: Value) -> Self {
        let doc = schema.get("description").and_then(Value::as_str).unwrap_or_default().to_owned();
        Self {
            kind: kind.into(),
            section: section.into(),
            owner: owner.into(),
            scope: Scope::Shared,
            sensitive: false,
            order: 100,
            map_form: false,
            singleton: false,
            claims_settings: false,
            aliases: Vec::new(),
            schema,
            doc,
            away: None,
            names: Vec::new(),
        }
    }
    /// A field of its specs that names `kind`'s resources ([`KindInfo::names`]).
    #[must_use]
    pub fn names(mut self, n: Names) -> Self {
        self.names.push(n);
        self
    }
    /// Put away for now, saying why ([`KindInfo::away`]).
    #[must_use]
    pub fn away(mut self, why: impl Into<String>) -> Self {
        self.away = Some(why.into());
        self
    }
    #[must_use]
    pub fn order(mut self, n: i32) -> Self {
        self.order = n;
        self
    }
    #[must_use]
    pub fn scope(mut self, s: Scope) -> Self {
        self.scope = s;
        self
    }
    #[must_use]
    pub fn sensitive(mut self) -> Self {
        self.sensitive = true;
        self
    }
    #[must_use]
    pub fn map_form(mut self) -> Self {
        self.map_form = true;
        self
    }
    #[must_use]
    pub fn alias(mut self, section: &str) -> Self {
        self.aliases.push(section.to_owned());
        self
    }
    /// whether `name` is this kind's kind, section or an alias
    pub fn named(&self, name: &str) -> bool {
        self.kind == name || self.section == name || self.aliases.iter().any(|a| a == name)
    }
    #[must_use]
    pub fn singleton(mut self) -> Self {
        self.singleton = true;
        self
    }
}

/// A resource another one names ([`ResourceKind::refs`]): kind `kind`'s `name`, in the naming spec's field `field`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ref {
    pub kind: String,
    pub name: String,
    pub field: String,
}

/// The JSON Schema of `T`, from its Rust type and doc comments.
pub fn schema_of<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap_or(Value::Null)
}

/// A resource kind (`resource.kind@1`). The owner keeps its `Ctx` to run its own commands.
pub trait ResourceKind: Send + Sync {
    fn info(&self) -> KindInfo;
    /// The current state, one resource per thing, as specs.
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>>;
    /// The kind's own plan; None: the generic one (a desired spec patches the current one).
    fn plan(&self, _current: &[Resource], _desired: &[Resource], _prune: bool) -> Option<Vec<Op>> {
        None
    }
    /// The plugin whose settings this kind declares (the generic `setting` kind leaves them out). Cheap:
    /// it is asked while another kind builds its own info.
    fn claims_settings(&self) -> Option<String> {
        None
    }
    /// A desired spec in the form `read` gives (`every day at 2` → `0 2 * * *`), so an unchanged thing
    /// plans as unchanged.
    fn normalize(&self, _spec: &mut Value) {}
    /// [`ResourceKind::normalize`] knowing the resource's name, for a kind whose form depends on it (a setting's value
    /// as its declaration takes it: `worktrees.max: off`, read by YAML as false, is `"off"`, setup audit D3). The
    /// default is `normalize`. Additive (2026-11-19).
    fn normalize_named(&self, _name: &str, spec: &mut Value) {
        self.normalize(spec);
    }
    /// Problems beyond the schema (a project's path that isn't absolute, a cron that doesn't parse).
    fn check(&self, _r: &Resource) -> Vec<String> {
        Vec::new()
    }
    /// Problems across the kind's resources as the apply would leave them (a cycle of crew wakes). Additive
    /// (2026-11-09, ADR-0043).
    fn check_all(&self, _after: &[Resource]) -> Vec<String> {
        Vec::new()
    }
    /// What a spec names of another kind's (a task's `goal: green-build` is goal `green-build`): the plan refuses one
    /// that names nothing there as the apply would leave it, so `wisp validate` says it before anything runs. A kind's
    /// `default` (`goals.default`) is never one to name. Additive (2026-11-19, ADR-0110).
    fn refs(&self, _r: &Resource) -> Vec<Ref> {
        Vec::new()
    }
    /// The kinds [`ResourceKind::refs`] may name (`["goal"]` for tasks): a plan that removes one of those reads this
    /// kind's resources it leaves as they are, so a goal removed from under a task that names it is refused too.
    /// Additive (2026-11-19, ADR-0110).
    fn refers_to(&self) -> Vec<String> {
        Vec::new()
    }
    /// Names a reference to this kind may use that aren't its resources: an agent field names a harness (`codex`) or
    /// `auto` as well as a declared agent, a machine field `this`. The plan takes them as declared. Additive
    /// (2026-11-20, ADR-0111).
    fn also_named<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Vec<String>> {
        Box::pin(async { Vec::new() })
    }
    /// The op in plain words, for the trust prompt ("lets Telegram chat 123 start tasks").
    fn describe(&self, _op: &Op) -> Option<String> {
        None
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>>;
    /// `from` renamed `to`, keeping what it is (`wisp mv`, ADR-0034). The default makes `to` with `from`'s
    /// spec, then removes `from`; a kind that can rename in place (one write, same id) overrides it.
    /// Additive (2026-11-09).
    fn rename<'a>(&'a self, call: &'a Call, from: &'a str, to: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let now = self.read(call).await?;
            let r = now
                .iter()
                .find(|r| r.name == from)
                .ok_or_else(|| wisp_kernel::WireError::not_found(format!("wisp has no {} named {from}", self.info().kind)))?;
            let kind = self.info().kind;
            self.apply(
                call,
                &Op {
                    op: OpKind::Create,
                    kind: kind.clone(),
                    name: to.to_owned(),
                    spec: r.spec.clone(),
                    id: None,
                    changes: Vec::new(),
                    before: None,
                },
            )
            .await?;
            self.apply(
                call,
                &Op {
                    op: OpKind::Delete,
                    kind,
                    name: from.to_owned(),
                    spec: r.spec.clone(),
                    id: r.id.clone(),
                    changes: Vec::new(),
                    before: None,
                },
            )
            .await
        })
    }
}

pub const RESOURCE_KIND: Point<dyn ResourceKind> = Point::new("resource.kind", 1);

/// What `apply` would do.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub ops: Vec<Op>,
    /// resources the formula declares that are already so
    pub unchanged: usize,
    /// `${secret:NAME}` references with no secret of that name here
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_secrets: Vec<String>,
    /// ops on kinds that change who may act, in plain words (a foreign formula needs `--trust`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitive: Vec<String>,
    /// schema and kind problems: apply refuses while there are any
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// `kind/name` of the updates that are drift: wisp's copy changed since the last apply (in the app,
    /// by a command), not the files. Additive (2026-11-09).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drift: Vec<String>,
}

/// `resources.rename`: one resource renamed in place (`wisp mv`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameArgs {
    pub kind: String,
    pub from: String,
    pub to: String,
}

/// `resources.refresh`: what wisp has now becomes the baseline of these resources (`--refresh-only`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshArgs {
    /// `kind/name`
    pub resources: Vec<String>,
}

/// `resources.plan` / `resources.apply`: resources already read from a formula (the CLI does the
/// files, includes, profiles and `${env:…}`; secrets stay references).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanArgs {
    pub resources: Vec<Resource>,
    /// also delete what the formula doesn't declare, in the kinds it mentions
    #[serde(default)]
    pub prune: bool,
    /// the formula came from somewhere else (a URL, a gist, a teammate)
    #[serde(default)]
    pub foreign: bool,
    /// the user said yes to its sensitive ops (`--trust`)
    #[serde(default)]
    pub trust: bool,
    /// `kind.name` addresses to delete (`wisp destroy`), whatever `resources` says. Additive (2026-11-09).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub destroy: Vec<String>,
    /// the sections (kinds) the formula names, an empty one too (`triggers: []`): with `prune`, a kind named here with
    /// nothing left in it is emptied. Without it a formula whose last trigger was deleted pruned nothing (audit A #15).
    /// Additive (2026-11-18).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub plan: Plan,
    pub applied: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ExportArgs {
    /// only these kinds (or sections); empty: every shared kind
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub kinds: Vec<String>,
    /// personal and device kinds to add (`account`, `ui`)
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub include: Vec<String>,
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

command!(Kinds = "resources.kinds" ["resources:read"] read yes : Value => Vec<KindInfo>; typed);
command!(PlanCmd = "resources.plan" ["resources:read"] read yes : PlanArgs => Plan);
command!(Apply = "resources.apply" ["resources:apply"] : PlanArgs => Applied);
command!(Export = "resources.export" ["resources:read"] read yes : ExportArgs => Vec<Resource>; typed);
command!(Rename = "resources.rename" ["resources:apply"] : RenameArgs => Value);
command!(Refresh = "resources.refresh" ["resources:apply"] : RefreshArgs => Vec<String>);

/// `resources.read`: a formula pasted into the app's Import (YAML, JSON or TOML), as the resources it declares and
/// the secrets it references; nothing changes. Additive (2026-11-11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadArgs {
    pub text: String,
    /// `yaml` (the default), `json` or `toml`
    #[serde(default)]
    pub format: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReadOut {
    pub resources: Vec<Resource>,
    pub secrets: Vec<String>,
}

/// `resources.write`: the setup (or some kinds of it) as a formula's text, as `wisp export` writes it, for the
/// app's Share setup. Additive (2026-11-11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct WriteArgs {
    /// only these kinds; empty: every shared kind
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub kinds: Vec<String>,
    /// `yaml` (the default), `json` or `toml`
    #[serde(default)]
    #[ts(as = "Option<String>", optional)]
    pub format: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteOut {
    pub text: String,
    pub count: u32,
}

/// `resources.secretSet` / `resources.secretRemove` (`wisp secret set|rm NAME`): a secret a formula references as
/// `${secret:NAME}`, kept in the secrets file, never in the store or a formula. A name is letters, digits, `_` and
/// `-`, so it can't reach a plugin's own secrets (theirs are `ns/…` or `ns:…`). Additive (2026-11-17).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretArgs {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
}

/// What a secret write did: its name, and whether it's there now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretOut {
    pub name: String,
    pub set: bool,
}

/// A secret's name as a formula references it: letters, digits, `_` and `-`, at most 64.
pub fn secret_name_ok(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

command!(SecretSet = "resources.secretSet" ["resources:secrets"] : SecretArgs => SecretOut);
command!(SecretRemove = "resources.secretRemove" ["resources:secrets"] : SecretArgs => SecretOut);

command!(Read = "resources.read" ["resources:read"] read yes : ReadArgs => ReadOut);
command!(Write = "resources.write" ["resources:read"] read yes : WriteArgs => WriteOut; typed);

#[cfg(test)]
mod names_tests {
    use super::*;
    use serde_json::json;

    /// ADR-0111: every field that names another kind's resource is found by its path, and only names are: an inline
    /// object, a URL, a path, an expression and a skipped word aren't.
    #[test]
    fn a_name_field_finds_its_names_by_path_and_only_names() {
        let spec = json!({
            "agent": "reviewer",
            "connectors": ["linear", "https://mcp.example.com/sse", "${connector.x}"],
            "members": { "a": { "agent": "codex" }, "b": { "agent": { "harness": "claude" } } },
            "team": [{ "crew": "Ada" }, { "crew": "Bo" }],
            "judges": ["critic", { "harness": "codex" }],
            "on": ["finished", "nightly"],
            "project": "/work/app",
        });
        let n = |f: &str| names_in(&spec, &Names::new(f, "x"));
        assert_eq!(n("agent"), ["reviewer"]);
        assert_eq!(n("connectors[]"), ["linear"], "a URL and an expression aren't names");
        assert_eq!(n("members.*.agent"), ["codex"], "an inline agent isn't a name");
        assert_eq!(n("team[].crew"), ["Ada", "Bo"]);
        assert_eq!(n("judges[]"), ["critic"]);
        assert_eq!(n("project"), Vec::<String>::new(), "a path isn't a name");
        assert_eq!(names_in(&spec, &Names::new("on[]", "trigger").skip(&["finished"])), ["nightly"]);
        assert_eq!(n("nothing.here[]"), Vec::<String>::new());
    }
}

/// `resources.overlay` (ADR-0111): a formula as one task's overlay, nothing applied, and a task resolved under it with
/// every name it gives checked: the same answer for `wisp run --formula`, `wisp "…" --formula`, a task file
/// (`formula: ./wisp.yml`, `module.team`), `wisp_launch`'s `formula` and New task's Formula…. Give the formula as
/// `resources` already read (the CLI reads files, URLs and gists itself), `text` (a formula or a task file: New task's
/// picked file), or `files` on this computer (relative to `cwd`). Additive (2026-11-20).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct OverlayArgs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Value>>")]
    pub resources: Vec<Resource>,
    /// a formula's or a task file's text (`task:` and its `formula:`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub text: String,
    /// what `text` is called (`release.task.yml`): its format, and what problems name
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub name: String,
    /// formula files on this computer, in order (a later one patches an earlier one)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub files: Vec<String>,
    /// where relative files and a task file's `module.x` (this folder's `.wisp/`) are read from
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub cwd: String,
    /// the task file's own folder, when its path is known: its `formula: ./x` and `module.x` are read from there (else
    /// from `cwd`, said in `notes`). Additive (2026-11-20, review r194 #5)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub dir: String,
    /// a task to resolve under the formula (`runtime.create`'s args: prompt, agent, goal, …)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub task: Option<Value>,
    /// a task the formula declares (`tasks: { release: … }`), run by its name: its spec, `tasks.default` under it
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(optional, as = "Option<String>")]
    pub run: String,
    /// that task's `{params}`, by name and then by position
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    #[ts(optional, as = "Option<std::collections::BTreeMap<String, String>>")]
    pub params: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayOut {
    /// `runtime.create`'s `formula`
    pub overlay: Value,
    /// what one task doesn't take (`crew/ada`): `wisp apply` keeps those
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left: Vec<String>,
    /// the task resolved (`runtime.create`'s args but `formula`): its declared goal inlined, one declared agent its own
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    /// the tasks the formula declares (but `tasks.default`), and its agents: what New task's Formula… lists
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// every file the formula was read from
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// names that name nothing, a file that loops, what didn't read: the task isn't started while there are any
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// what to know about how it was read (a picked task file's `./x` read from the project folder)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

command!(Overlay = "resources.overlay" ["resources:overlay"] read yes : OverlayArgs => OverlayOut; typed);

/// The kinds an agent's formula may declare (review r194 round 2 #1): agents, skills (as text), teams and goals. Anything
/// else (a connector or harness runs a command on the host, a project runs its setup, a crew member, a trigger, a task, a
/// channel…) is the user's to declare.
pub const AGENT_FORMULA_KINDS: [&str; 4] = ["agent", "skill", "team", "goal"];

fn section_of(kind: &str) -> String {
    match kind {
        "crew" | "budget" | "ui" | "router" => kind.to_owned(),
        "harness" => "harnesses".into(),
        "capability" => "capabilities".into(),
        k => format!("{k}s"),
    }
}

/// Why an agent's formula (a task's `wisp_launch`, an agent's `runtime.create`) can't be run as written: one plain line
/// each. Empty: it may. The user's own formulas aren't held to this.
#[must_use]
pub fn agent_formula_problems(resources: &[Resource]) -> Vec<String> {
    fn secret(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => s.find("${secret:").map(|i| s[i..].split('}').next().map_or_else(String::new, |x| format!("{x}}}"))),
            Value::Array(a) => a.iter().find_map(secret),
            Value::Object(o) => o.values().find_map(secret),
            _ => None,
        }
    }
    let mut out = Vec::new();
    for r in resources {
        let at = format!("{}.{}", section_of(&r.kind), r.name);
        if !AGENT_FORMULA_KINDS.contains(&r.kind.as_str()) {
            let line =
                format!("an agent's formula can't declare {} ({at}): the user declares those; name ones they set up", section_of(&r.kind));
            if !out.contains(&line) {
                out.push(line);
            }
            continue;
        }
        if let Some(s) = secret(&r.spec) {
            out.push(format!("an agent's formula can't use secrets ({at}: {s})"));
        }
        if r.kind == "skill" && r.spec.get("path").and_then(Value::as_str).is_some_and(|p| !p.is_empty()) {
            out.push(format!("an agent's formula gives a skill its text, not a path ({at})"));
        }
        if r.kind == "agent" && r.spec.get("account").and_then(Value::as_str).is_some_and(|p| !p.is_empty()) {
            out.push(format!("an agent's formula can't name an account ({at}): its task uses its own"));
        }
    }
    out
}

/// [`agent_formula_problems`] for an overlay as `runtime.create` takes it (`{ agents: [{ name, … }], connectors: … }`).
#[must_use]
pub fn agent_overlay_problems(overlay: &Value) -> Vec<String> {
    let mut rs = Vec::new();
    for (section, v) in overlay.as_object().into_iter().flatten() {
        let kind = match section.as_str() {
            "agents" => "agent",
            "skills" => "skill",
            "teams" => "team",
            "goals" => "goal",
            "harnesses" => "harness",
            "projects" => "project",
            "connectors" => "connector",
            other => other.trim_end_matches('s'),
        };
        let items: Vec<(String, Value)> = match v {
            Value::Array(a) => a.iter().map(|x| (x.get("name").and_then(Value::as_str).unwrap_or("?").to_owned(), x.clone())).collect(),
            Value::Object(m) => m.iter().map(|(k, x)| (k.clone(), x.clone())).collect(),
            other => vec![("?".to_owned(), other.clone())],
        };
        for (n, spec) in items {
            rs.push(Resource::new(kind, n, spec));
        }
    }
    agent_formula_problems(&rs)
}

#[cfg(test)]
mod agent_formula_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_agents_formula_declares_only_agents_skills_teams_and_goals_without_secrets() {
        let ok = [Resource::new("agent", "x", json!({ "harness": "claude" })), Resource::new("skill", "s", json!({ "body": "b" }))];
        assert_eq!(agent_formula_problems(&ok), Vec::<String>::new());
        let bad = [
            Resource::new("connector", "evil", json!({ "command": "touch" })),
            Resource::new("agent", "x", json!({ "instructions": "use ${secret:TOKEN} now", "account": "work" })),
            Resource::new("skill", "s", json!({ "path": "/etc/hosts" })),
            Resource::new("harness", "claude", json!({ "path": "/bin/sh" })),
        ];
        let p = agent_formula_problems(&bad);
        assert!(p[0].starts_with("an agent's formula can't declare connectors (connectors.evil)"), "{p:?}");
        assert!(p.contains(&"an agent's formula can't use secrets (agents.x: ${secret:TOKEN})".to_owned()), "{p:?}");
        assert!(p.iter().any(|l| l.starts_with("an agent's formula can't name an account")), "{p:?}");
        assert!(p.iter().any(|l| l.starts_with("an agent's formula gives a skill its text")), "{p:?}");
        assert!(p.iter().any(|l| l.starts_with("an agent's formula can't declare harnesses")), "{p:?}");
        assert_eq!(agent_overlay_problems(&json!({ "connectors": [{ "name": "evil", "command": "x" }] })).len(), 1);
    }
}
