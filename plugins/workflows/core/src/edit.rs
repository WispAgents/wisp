//! Changing a run while it goes (ADR-0121): skip a step, try one again, add steps, change one that hasn't started,
//! change the run's concurrency and budget. A target is a node id (`fix[2]`, `audit[agent=codex]`) or a step's name
//! (every node of it). Each change is checked as a workflow is when it starts (names, cycles, what expressions read);
//! a refusal says why and changes nothing.

use serde_json::Value;
use wisp_workflows_api::{StepSpec, StepState, WorkflowSpec};

use crate::{NEEDS_FAILED, NodeRun, Run, check, expand};

impl Run {
    /// The nodes `target` names: that node, else every node of that step.
    fn targets(&self, target: &str) -> Vec<usize> {
        let exact: Vec<usize> = self.nodes.iter().enumerate().filter(|(_, n)| n.id == target).map(|(i, _)| i).collect();
        if !exact.is_empty() {
            return exact;
        }
        self.nodes.iter().enumerate().filter(|(_, n)| n.step == target).map(|(i, _)| i).collect()
    }

    fn named(&self, target: &str) -> Result<Vec<usize>, String> {
        let at = self.targets(target);
        if at.is_empty() {
            let all: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
            return Err(format!("no step {target} in this run (its steps: {})", all.join(", ")));
        }
        Ok(at)
    }

    /// Skips `target`'s nodes that wait or work (`why`: who skipped it); the steps after it still run without it. The
    /// tasks still working are returned to stop.
    pub fn skip(&mut self, target: &str, why: &str, now: i64) -> Result<Vec<String>, String> {
        let at = self.named(target)?;
        let mut stop = Vec::new();
        let mut any = false;
        for i in at {
            let id = self.nodes[i].id.clone();
            let Some(r) = self.runs.get_mut(&id) else { continue };
            match r.state {
                StepState::Running => {
                    stop.extend(r.task.clone());
                    r.ended_at = Some(now);
                }
                StepState::Waiting => {}
                _ => continue,
            }
            r.state = StepState::Skipped;
            why.clone_into(&mut r.why);
            any = true;
        }
        if !any {
            return Err(format!("{target} already ended: retry it to run it again"));
        }
        Ok(stop)
    }

    /// Tries `target`'s failed or skipped nodes again (their tries start over), and opens again the steps that were
    /// skipped because one they need failed.
    pub fn retry(&mut self, target: &str) -> Result<(), String> {
        let at = self.named(target)?;
        let mut opened: Vec<String> = Vec::new();
        for i in at {
            let (id, step) = (self.nodes[i].id.clone(), self.nodes[i].step.clone());
            let Some(r) = self.runs.get_mut(&id) else { continue };
            if matches!(r.state, StepState::Failed | StepState::Skipped) {
                *r = NodeRun { cost: r.cost, ..NodeRun::default() };
                if !opened.contains(&step) {
                    opened.push(step);
                }
            }
        }
        if opened.is_empty() {
            return Err(format!("nothing to retry: {target} is waiting, working or done"));
        }
        // what was skipped for want of them waits again, and what was skipped for want of those
        loop {
            let reopen: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| n.spec.needs.iter().any(|x| opened.contains(x)))
                .filter(|n| self.runs.get(&n.id).is_some_and(|r| r.state == StepState::Skipped && r.why == NEEDS_FAILED))
                .map(|n| n.id.clone())
                .collect();
            if reopen.is_empty() {
                break;
            }
            for id in reopen {
                if let Some(r) = self.runs.get_mut(&id) {
                    *r = NodeRun::default();
                }
                if let Some(n) = self.nodes.iter().find(|n| n.id == id)
                    && !opened.contains(&n.step)
                {
                    opened.push(n.step.clone());
                }
            }
        }
        Ok(())
    }

    /// The workflow with `name` set to `spec`, if it checks.
    fn with_step(&self, name: &str, spec: StepSpec) -> Result<WorkflowSpec, String> {
        let mut w = self.spec.clone();
        w.tasks.insert(name.to_owned(), spec);
        let problems = check(&w);
        if problems.is_empty() { Ok(w) } else { Err(problems.join("; ")) }
    }

    /// Adds a step to the run: it starts once its needs end (a need that already ended counts at once).
    pub fn add(&mut self, name: &str, spec: StepSpec) -> Result<(), String> {
        if name.trim().is_empty() || !name.chars().all(|c| c.is_alphanumeric() || "-_".contains(c)) {
            return Err(format!("a step's name is letters, digits, - and _ (not {name:?})"));
        }
        if self.spec.tasks.contains_key(name) {
            return Err(format!("the run already has a step {name}: change it, or pick another name"));
        }
        self.spec = self.with_step(name, spec.clone())?;
        self.put_nodes(name, spec, None);
        Ok(())
    }

    /// Changes step `name`, which hasn't started: `patch` is the fields to set (null clears one), as in a spec.
    pub fn change(&mut self, name: &str, patch: &Value) -> Result<(), String> {
        let Some(old) = self.spec.tasks.get(name).cloned() else {
            return Err(format!("no step {name} in this run"));
        };
        let started = self
            .nodes
            .iter()
            .filter(|n| n.step == name)
            .any(|n| self.runs.get(&n.id).is_none_or(|r| r.state != StepState::Waiting || r.attempts > 0));
        if started {
            return Err(format!("{name} has started: only a step that hasn't can change (skip it, and add another)"));
        }
        let Some(fields) = patch.as_object() else {
            return Err("change: each step's change is an object of its fields".to_owned());
        };
        let mut merged = serde_json::to_value(&old).map_err(|e| e.to_string())?;
        if let Some(m) = merged.as_object_mut() {
            for (k, v) in fields {
                if v.is_null() {
                    m.remove(k);
                } else {
                    m.insert(k.clone(), v.clone());
                }
            }
        }
        let spec: StepSpec = serde_json::from_value(merged).map_err(|e| format!("{name}: {e}"))?;
        self.spec = self.with_step(name, spec.clone())?;
        let at = self.nodes.iter().position(|n| n.step == name);
        let gone: Vec<String> = self.nodes.iter().filter(|n| n.step == name).map(|n| n.id.clone()).collect();
        self.nodes.retain(|n| n.step != name);
        for id in gone {
            self.runs.remove(&id);
        }
        self.put_nodes(name, spec, at);
        Ok(())
    }

    /// The step's nodes (one, a matrix's, or a foreach not made yet), at `at` or last.
    fn put_nodes(&mut self, name: &str, spec: StepSpec, at: Option<usize>) {
        let one = WorkflowSpec { tasks: [(name.to_owned(), spec)].into(), ..WorkflowSpec::default() };
        let made = expand(&one);
        for n in &made {
            self.runs.insert(n.id.clone(), NodeRun::default());
        }
        let at = at.unwrap_or(self.nodes.len()).min(self.nodes.len());
        self.nodes.splice(at..at, made);
    }

    /// The run's concurrency (0: no limit) and budget (`{ usd }`; null: none).
    pub fn limits(&mut self, concurrency: Option<u32>, budget: Option<&Value>) {
        if let Some(c) = concurrency {
            self.spec.concurrency = c;
        }
        if let Some(b) = budget {
            self.spec.budget = (!b.is_null()).then(|| b.clone());
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::Action;

    fn step(prompt: &str, needs: &[&str]) -> StepSpec {
        StepSpec { prompt: prompt.into(), needs: needs.iter().map(|s| (*s).to_owned()).collect(), ..StepSpec::default() }
    }

    fn run(steps: Vec<(&str, StepSpec)>) -> Run {
        let w = WorkflowSpec { tasks: steps.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(), ..WorkflowSpec::default() };
        Run::new(w, &BTreeMap::new())
    }

    fn starts(a: &[Action]) -> Vec<String> {
        a.iter().filter_map(|x| if let Action::Start { node, .. } = x { Some(node.clone()) } else { None }).collect()
    }

    #[test]
    fn a_skipped_step_lets_the_steps_after_it_run() {
        let mut r = run(vec![("a", step("a", &[])), ("b", step("b", &["a"]))]);
        r.next(0);
        r.started("a", "T1");
        assert_eq!(r.skip("a", "skipped by the agent", 5), Ok(vec!["T1".to_owned()]), "its task is stopped");
        assert_eq!(starts(&r.next(6)), ["b"], "skipped isn't failed");
        assert!(r.skip("a", "x", 7).is_err_and(|e| e.contains("retry")));
        assert!(r.skip("ghost", "x", 7).is_err_and(|e| e.contains("its steps: a, b")));
    }

    #[test]
    fn a_retry_runs_it_again_and_opens_what_its_failure_skipped() {
        let mut r = run(vec![("a", step("a", &[])), ("b", step("b ${{ tasks.a.output }}", &["a"])), ("c", step("c", &["b"]))]);
        r.next(0);
        r.finished("a", false, Value::Null, "it ended error", 0.3);
        r.next(1);
        assert_eq!((r.runs["b"].state, r.runs["c"].state), (StepState::Skipped, StepState::Skipped));
        assert!(r.over());
        r.retry("a").unwrap();
        assert!(!r.over());
        assert_eq!((r.runs["a"].state, r.runs["b"].state, r.runs["c"].state), (StepState::Waiting, StepState::Waiting, StepState::Waiting));
        assert!((r.runs["a"].cost - 0.3).abs() < 1e-9, "what it cost stays counted");
        assert_eq!(starts(&r.next(2)), ["a"]);
        assert!(r.retry("a").is_err_and(|e| e.contains("nothing to retry")));
    }

    #[test]
    fn a_step_added_runs_after_its_needs_and_a_bad_one_is_refused() {
        let mut r = run(vec![("a", step("a", &[]))]);
        r.next(0);
        r.finished("a", true, json!({ "text": "found 2" }), "", 0.0);
        r.next(1);
        assert!(r.over());
        r.add("fix", step("Fix: ${{ tasks.a.output.text }}", &["a"])).unwrap();
        let a = r.next(2);
        assert!(matches!(&a[..], [Action::Start { node, prompt, .. }] if node == "fix" && prompt == "Fix: found 2"), "{a:?}");
        assert!(r.add("fix", step("x", &[])).is_err_and(|e| e.contains("already has")));
        assert!(r.add("bad", step("x", &["ghost"])).is_err_and(|e| e.contains("no task named ghost")));
        assert!(r.add("two words", step("x", &[])).is_err());
        assert!(!r.spec.tasks.contains_key("bad"), "a refusal changes nothing");
    }

    #[test]
    fn only_a_step_that_hasnt_started_changes() {
        let mut r = run(vec![("a", step("a", &[])), ("b", step("b", &["a"]))]);
        r.next(0);
        r.change("b", &json!({ "prompt": "B now", "agent": "codex", "matrix": { "model": ["x", "y"] } })).unwrap();
        assert_eq!(r.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["a", "b[model=x]", "b[model=y]"]);
        assert!(r.change("a", &json!({ "prompt": "late" })).is_err_and(|e| e.contains("has started")));
        assert!(r.change("b", &json!({ "needs": ["b"] })).is_err());
        assert!(r.change("b", &json!({ "nope": 1 })).is_err_and(|e| e.contains("nope")));
        r.finished("a", true, json!({}), "", 0.0);
        assert_eq!(starts(&r.next(1)), ["b[model=x]", "b[model=y]"]);
        assert!(r.nodes.iter().all(|n| n.step != "b" || n.spec.agent == "codex"));
    }

    #[test]
    fn concurrency_and_budget_change_what_starts() {
        let mut r = run(vec![("a", step("a", &[])), ("b", step("b", &[])), ("c", step("c", &[]))]);
        r.limits(Some(1), None);
        assert_eq!(starts(&r.next(0)), ["a"]);
        r.limits(Some(0), Some(&json!({ "usd": 5 })));
        assert_eq!(starts(&r.next(1)), ["b", "c"]);
        r.limits(None, Some(&Value::Null));
        assert_eq!(r.spec.budget, None);
    }
}
