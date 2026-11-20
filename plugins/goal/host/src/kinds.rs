//! wisp.goal's resource kind (ADR-0028, ADR-0110): the goals a formula declares by name (`goals:`, one per name), and
//! `goals.default`, what every goal takes unless it says otherwise (v1 `goalJudges`, `goalVote`). Acted on, kept in
//! memory and refreshed on apply and at start: a task's goal without its own judges, vote or round limit takes the
//! default's, and a goal named (`goal: green-build` in a task, `--goal green-build`, the composer's pick) is the
//! declared one, its own fields over the default's.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wisp_kernel::{BoxFuture, Call, Result, Setup};
use wisp_resources_api::{KindInfo, Op, RESOURCE_KIND, Resource, ResourceKind, StoredKind, schema_of};

/// The name that holds every goal's defaults: never a goal of its own (nothing can name it).
pub const DEFAULT: &str = "default";

/// A goal: what done means and how it's judged after each turn. `goals.default` has no text: it's every goal's
/// judges, vote, round limit and sandbox.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoalSpec {
    /// what has to be true for it to be met ("npm test passes and the build is green")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// the agent profiles that judge; `[]` on purpose: no judges, the task's own quick check
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judges: Option<Vec<String>>,
    /// majority | all | any
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vote: String,
    /// turns a goal may take before it stops and asks
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rounds: Option<u32>,
    /// its judges run in a sandbox of their own, seeded with the task's folder (ADR-0047)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
}

/// `goals.default`.
pub type Defaults = Arc<Mutex<GoalSpec>>;
/// The declared goals by name (`default` not among them), as stored: their own fields only.
pub type Named = Arc<Mutex<BTreeMap<String, GoalSpec>>>;

struct GoalKind {
    inner: Arc<StoredKind>,
    d: Defaults,
    named: Named,
}

/// The declared goals and the defaults, read from `specs` (name → stored spec).
pub fn split(specs: Vec<(String, Value)>) -> (GoalSpec, BTreeMap<String, GoalSpec>) {
    let mut d = GoalSpec::default();
    let mut named = BTreeMap::new();
    for (name, v) in specs {
        let Ok(spec) = serde_json::from_value::<GoalSpec>(v) else { continue };
        if name == DEFAULT {
            d = spec;
        } else {
            named.insert(name, spec);
        }
    }
    (d, named)
}

async fn refresh(inner: &StoredKind, d: &Defaults, named: &Named) {
    let (defaults, goals) = split(inner.specs().await.unwrap_or_default());
    *d.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = defaults;
    *named.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = goals;
}

/// Problems with one declared goal beyond its schema: a goal says what done means; the defaults don't.
pub fn problems(name: &str, spec: &Value) -> Vec<String> {
    let text = spec.get("text").and_then(Value::as_str).unwrap_or_default().trim();
    let vote = spec.get("vote").and_then(Value::as_str).unwrap_or_default();
    let mut out = Vec::new();
    if name == DEFAULT && !text.is_empty() {
        out.push("goals.default is every goal's judges, vote and round limit: it has no text (declare a goal by name for one)".to_owned());
    }
    if name != DEFAULT && text.is_empty() {
        out.push("a goal says what done means: text: …".to_owned());
    }
    if !vote.is_empty() && !["majority", "all", "any"].contains(&vote) {
        out.push(format!("vote is majority, all or any (not {vote})"));
    }
    out
}

/// "no goal named X: did you mean Y? goals has a, b" (or that none are declared), for a reference that names nothing.
/// At most [`SHOWN`] names, then how many more (audit H6: `--goal nevr` listed all 32), the close one first.
pub fn unknown(name: &str, names: &[String]) -> String {
    let has: Vec<&str> = names.iter().map(String::as_str).filter(|n| *n != DEFAULT).collect();
    if has.is_empty() {
        return format!("no goal named {name}: no goals are declared (goals: in a formula), or write it inline as goal: {{ text: … }}");
    }
    let close =
        has.iter().map(|n| (distance(name, n), *n)).filter(|(d, n)| *d <= 2 && *d < n.len().max(name.len())).min_by_key(|(d, _)| *d);
    let mean = close.map_or_else(String::new, |(_, n)| format!(" did you mean {n}?"));
    let more = has.len().saturating_sub(SHOWN);
    let list = has.iter().take(SHOWN).copied().collect::<Vec<_>>().join(", ");
    let list = if more > 0 { format!("{list} and {more} more (wisp goal declared lists them)") } else { list };
    format!("no goal named {name}:{mean} goals has {list}")
}

/// How many names a refusal lists (as `wisp validate` does).
const SHOWN: usize = 8;

/// Edit distance, an adjacent swap counting one (`nevr` is one from `never`).
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d: Vec<Vec<usize>> = (0..=a.len())
        .map(|i| {
            (0..=b.len())
                .map(|j| {
                    if i == 0 {
                        j
                    } else if j == 0 {
                        i
                    } else {
                        0
                    }
                })
                .collect()
        })
        .collect();
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[a.len()][b.len()]
}

impl ResourceKind for GoalKind {
    fn info(&self) -> KindInfo {
        self.inner.info()
    }
    fn read<'a>(&'a self, call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        self.inner.read(call)
    }
    fn check(&self, r: &Resource) -> Vec<String> {
        problems(&r.name, &r.spec)
    }
    fn apply<'a>(&'a self, call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            self.inner.apply(call, op).await?;
            refresh(&self.inner, &self.d, &self.named).await;
            Ok(())
        })
    }
}

pub fn register(s: &mut Setup) -> (Defaults, Named) {
    let info = KindInfo::new("goal", "goals", "wisp.goal", schema_of::<GoalSpec>())
        .order(35)
        .names(wisp_resources_api::Names::new("judges[]", "agent"));
    let inner = Arc::new(StoredKind::new(info, s.ctx()));
    let d: Defaults = Arc::default();
    let named: Named = Arc::default();
    let k: Arc<dyn ResourceKind> = Arc::new(GoalKind { inner: inner.clone(), d: d.clone(), named: named.clone() });
    s.contribute(&RESOURCE_KIND, k);
    let (d2, n2) = (d.clone(), named.clone());
    s.on_start(move |_ctx| async move {
        refresh(&inner, &d2, &n2).await;
        Ok(())
    });
    (d, named)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn default_is_the_defaults_and_every_other_name_a_goal() {
        let (d, named) = split(vec![
            ("default".into(), json!({ "judges": ["reviewer"], "vote": "majority", "maxRounds": 6 })),
            ("green-build".into(), json!({ "text": "npm test passes", "judges": ["claude"] })),
            ("broken".into(), json!({ "nope": 1 })),
        ]);
        assert_eq!((d.judges, d.vote, d.max_rounds), (Some(vec!["reviewer".to_owned()]), "majority".to_owned(), Some(6)));
        assert_eq!(named.keys().collect::<Vec<_>>(), ["green-build"], "default isn't a goal; a spec that doesn't read is left out");
        assert_eq!(named["green-build"].text, "npm test passes");
    }

    #[test]
    fn a_goal_needs_text_and_the_defaults_have_none() {
        assert!(problems("green-build", &json!({ "judges": ["a"] }))[0].contains("text: …"));
        assert!(problems("default", &json!({ "text": "x" }))[0].contains("has no text"));
        assert_eq!(problems("default", &json!({ "judges": ["a"], "vote": "all" })), Vec::<String>::new());
        assert!(problems("g", &json!({ "text": "x", "vote": "most" }))[0].contains("majority, all or any"));
        assert_eq!(unknown("nope", &["default".into(), "a".into(), "b".into()]), "no goal named nope: goals has a, b");
        assert!(unknown("nope", &["default".into()]).contains("no goals are declared"));
    }

    /// Audit H6: with 32 goals declared, `--goal nevr` listed every one; now 8, how many more, and the close one.
    #[test]
    fn a_refusal_lists_eight_goals_and_the_close_one_audit_h6() {
        let mut names: Vec<String> = (1..=30).map(|i| format!("goal-{i:02}")).collect();
        names.extend(["green-build".to_owned(), "never".to_owned(), "default".to_owned()]);
        let said = unknown("nevr", &names);
        assert!(said.starts_with("no goal named nevr: did you mean never? goals has goal-01,"), "{said}");
        assert!(said.contains("goal-08 and 24 more (wisp goal declared lists them)") && !said.contains("goal-09"), "{said}");
        assert!(!unknown("zzzzzz", &names).contains("did you mean"));
    }
}
