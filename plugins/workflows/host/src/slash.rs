//! `/workflow …` as a whole prompt (ADR-0119), in New task's box, a task's reply box, the CLI, the phone and channels
//! alike: the task's agent becomes the run's lead. `/workflow <saved name> [k=v …]` has it run that workflow;
//! `/workflow <words>` has it plan one for what the words ask and run it. Either way the agent starts it with
//! `workflows_run` and wisp tells it every step's result when the run ends, on any harness. The prompt stays as typed
//! (the task shows what you wrote): the lead's guidance is a turn note in front of it (`runtime.prompt`'s `before`),
//! naming the tool as this harness does. `/workflow` alone is refused before the task or follow-up is made.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// What a prompt says as a `/workflow` command: None when it isn't one; `Some("")` for `/workflow` alone.
pub fn typed(prompt: &str) -> Option<&str> {
    let t = prompt.trim();
    let rest = t.strip_prefix('/')?;
    let (head, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    (head.eq_ignore_ascii_case("workflow") || head.eq_ignore_ascii_case("wf")).then(|| tail.trim())
}

pub const SAY_WHAT: &str =
    "Say what the workflow should do after /workflow, like “/workflow audit every route for missing auth checks”, or name a saved one.";

/// A workflow that runs by name (yours or a prebuilt one), with its inputs' names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub name: String,
    pub inputs: Vec<String>,
}

/// `name k=v …` when `name` runs by name: its name and inputs. A workflow of one input takes the words after its name
/// as that input (`/workflow two-ways make the parser faster`); words that are neither are a job in words.
fn saved<'a>(words: &'a str, known: &[Known]) -> Option<(&'a str, BTreeMap<String, Value>)> {
    let (name, rest) = words.split_once(char::is_whitespace).unwrap_or((words, ""));
    let rest = rest.trim();
    let w = known.iter().find(|k| k.name == name)?;
    let mut inputs = BTreeMap::new();
    let pairs: Option<Vec<(&str, &str)>> = rest.split_whitespace().map(|x| x.split_once('=')).collect();
    match pairs {
        Some(p) if !rest.is_empty() => {
            for (k, v) in p {
                inputs.insert(k.to_owned(), json!(v));
            }
        }
        _ if rest.is_empty() => {}
        _ if w.inputs.len() == 1 => {
            inputs.insert(w.inputs[0].clone(), json!(rest));
        }
        _ => return None,
    }
    Some((name, inputs))
}

/// The note the agent gets in front of `/workflow <words>` (None: the words are missing); `tool`: how this harness
/// names `workflows_run` (None: it can't reach wisp's tools, so it does the job itself).
pub fn lead_note(words: &str, names: &[Known], tool: Option<&str>) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let Some(tool) = tool else {
        return Some("<wisp-workflow>\nThe user typed /workflow, but this run can't reach wisp's tools: do the job below yourself, step by step.\n</wisp-workflow>".to_owned());
    };
    if let Some((name, inputs)) = saved(words, names) {
        let inputs = if inputs.is_empty() { String::new() } else { format!(", \"inputs\": {}", json!(inputs)) };
        return Some(format!(
            "<wisp-workflow>\nThe user's message below is wisp's /workflow command: run the wisp workflow `{name}` with the \
             `{tool}` tool ({{\"name\": \"{name}\"{inputs}}}). Don't do its steps yourself. Once it started, end your turn: \
             wisp sends you every step's result when the run ends; then tell the user what it found and did, with the tasks to open.\n</wisp-workflow>"
        ));
    }
    Some(format!(
        "<wisp-workflow>\nThe user's message below is wisp's /workflow command: run the job as a wisp workflow rather than by \
         yourself. Look around just enough to plan, then write a workflow spec and start it with the `{tool}` tool (its \
         description says how to write one):\n\
         - split the work into steps, each its own agent with a self-contained prompt (a step sees only its prompt);\n\
         - fan out with `foreach` over a list an earlier step gives (give that step an `output` schema with the list), or a \
         `matrix` to have several agents try the same thing;\n\
         - group the steps in phases (Explore, Build, Verify) and end with a step that checks or reviews the others' work;\n\
         - give the run a short name (kebab-case) and a one-line description; steps run in this task's folder, at this \
         task's mode or lower.\n\
         Once it started, end your turn: wisp sends you every step's result when the run ends. Then answer the user with \
         what it found and did, and the tasks to open.\n</wisp-workflow>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_slash_workflow_and_nothing_else() {
        assert_eq!(typed("/workflow audit the routes"), Some("audit the routes"));
        assert_eq!(typed("  /Workflow  "), Some(""));
        assert_eq!(typed("/wf x"), Some("x"));
        assert_eq!(typed("/workflows"), None);
        assert_eq!(typed("run /workflow x"), None);
    }

    #[test]
    fn a_saved_name_runs_that_workflow_and_words_plan_one() {
        let names = vec![
            Known { name: "release-check".to_owned(), inputs: vec!["version".to_owned(), "notes".to_owned()] },
            Known { name: "two-ways".to_owned(), inputs: vec!["change".to_owned()] },
        ];
        let tool = Some("mcp__wisp__workflows_run");
        let p = lead_note("release-check version=2.1", &names, tool).unwrap_or_default();
        assert!(
            p.contains("{\"name\": \"release-check\", \"inputs\": {\"version\":\"2.1\"}}") && p.contains("`mcp__wisp__workflows_run`"),
            "{p}"
        );
        let p = lead_note("audit every route", &names, tool).unwrap_or_default();
        assert!(p.contains("foreach") && p.contains("mcp__wisp__workflows_run"), "{p}");
        // a saved name followed by words that aren't inputs is a request in words
        assert!(lead_note("release-check the docs", &names, tool).unwrap_or_default().contains("Look around"));
        assert!(lead_note("audit", &names, None).unwrap_or_default().contains("yourself"), "no wisp tools: it does the job");
        assert_eq!(lead_note("", &names, tool), None);
        // one input: the words after its name are it
        let p = lead_note("two-ways make the parser faster", &names, tool).unwrap_or_default();
        assert!(p.contains(r#""inputs": {"change":"make the parser faster"}"#), "{p}");
    }
}
