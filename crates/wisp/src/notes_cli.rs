//! `wisp notes [<task>] [--set section=text …] [--json]` (ADR-0079): a task's Notes, the state its agent keeps (goal,
//! constraints, decisions, what failed, where it stands, open questions, files). `--set` rewrites one section as you
//! (never appends); `section=` empties it. With no task: the task this shell runs in (`WISP_TASK`).
use serde_json::{Value, json};
use wisp_threads_core::notes::{META, Shown, markdown};

use crate::client::{CliError, Client, Result};
use crate::scripted::say;

/// The task and the `--set` pairs out of the words after `notes`.
pub fn parse(words: &[String], env_task: Option<&str>) -> std::result::Result<(String, Vec<(String, String)>), String> {
    let mut task = None;
    let mut sets = Vec::new();
    let mut it = words.iter();
    while let Some(w) = it.next() {
        let pair = if w == "--set" {
            it.next().ok_or("--set needs section=text")?.as_str()
        } else if let Some(p) = w.strip_prefix("--set=") {
            p
        } else if task.is_none() {
            task = Some(w.clone());
            continue;
        } else {
            return Err(format!("unexpected {w:?} (wisp notes <task> --set section=text)"));
        };
        let (k, v) = pair.split_once('=').ok_or_else(|| format!("--set {pair:?}: write it as section=text"))?;
        let k = k.trim().to_ascii_lowercase();
        if !META.iter().any(|m| m.id == k) {
            let ids: Vec<&str> = META.iter().map(|m| m.id).collect();
            return Err(format!("no section {k:?} (one of {})", ids.join(", ")));
        }
        sets.push((k, v.replace("\\n", "\n")));
    }
    let task = task.or_else(|| env_task.filter(|t| !t.is_empty()).map(str::to_owned)).ok_or("which task? wisp notes <task>")?;
    Ok((task, sets))
}

/// The Notes as the terminal shows them: every section with text, in order.
pub fn render(notes: &Value) -> String {
    let empty = Vec::new();
    let sections = notes.get("sections").and_then(Value::as_array).unwrap_or(&empty);
    let shown: Vec<Shown<'_>> =
        sections.iter().filter_map(|s| Some(Shown { id: s.get("id")?.as_str()?, text: s.get("text")?.as_str()? })).collect();
    let md = markdown(&shown, &[], usize::MAX, "wisp notes");
    if md.is_empty() { "No notes yet.".to_owned() } else { md }
}

pub async fn run(c: &Client, words: &[String], json_out: bool) -> Result<i32> {
    let (task, sets) = parse(words, std::env::var("WISP_TASK").ok().as_deref()).map_err(CliError::said)?;
    for (section, text) in &sets {
        c.cmd("threads.notesUpdate", json!({ "id": task, "section": section, "text": text, "by": "you" })).await?;
    }
    let notes = c.cmd("threads.notes", json!({ "id": task })).await?;
    say(&if json_out { notes.to_string() } else { render(&notes) });
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn notes_takes_a_task_and_set_pairs_or_the_shells_task() {
        assert_eq!(parse(&w(&["abc123def456"]), None).unwrap(), ("abc123def456".into(), vec![]));
        assert_eq!(
            parse(&w(&["t1", "--set", "goal=Ship it", "--set=open=Which port?\\nWhy?"]), None).unwrap(),
            ("t1".into(), vec![("goal".into(), "Ship it".into()), ("open".into(), "Which port?\nWhy?".into())])
        );
        assert_eq!(parse(&w(&["--set", "state="]), Some("t9")).unwrap(), ("t9".into(), vec![("state".into(), String::new())]));
        assert!(parse(&w(&[]), None).is_err());
        assert!(parse(&w(&["t1", "--set", "mood=good"]), None).unwrap_err().contains("goal"));
        assert!(parse(&w(&["t1", "--set", "goal"]), None).is_err());
        assert!(parse(&w(&["t1", "t2"]), None).is_err());
    }

    #[test]
    fn the_notes_render_as_markdown_or_say_there_are_none() {
        let n = json!({ "sections": [{ "id": "goal", "text": "Ship" }, { "id": "failed", "text": "a\nb" }, { "id": "open", "text": "" }] });
        let md = render(&n);
        assert!(md.contains("Ship") && md.contains("- a\n- b") && !md.contains("Open"), "{md}");
        assert_eq!(render(&json!({ "sections": [] })), "No notes yet.");
    }
}
