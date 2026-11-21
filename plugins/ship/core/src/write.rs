//! The agent writes the PR (TRY-13): the task's own harness, asked on the side (a quick run, no tools: wisp hands it the
//! diff), gives the title and the description, following the repo's pull request template when it has one. Cursor's and Codex's agents, Devin and Copilot's PR summary all write a PR's text
//! from its diff the same way; wisp's adds what the task was asked and what its agent answered. Pure: the prompt, where
//! a template may be, and the answer read back.

use serde_json::Value;

/// Where GitHub looks for a pull request template, in its order (the first one found is the repo's).
pub const TEMPLATE_PATHS: [&str; 6] = [
    ".github/pull_request_template.md",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "pull_request_template.md",
    "PULL_REQUEST_TEMPLATE.md",
    "docs/pull_request_template.md",
    "docs/PULL_REQUEST_TEMPLATE.md",
];

/// The longest title the dialog takes (its field's `maxLength`).
pub const TITLE_MAX: usize = 100;

/// How the side run answers: the dialog's two fields.
const ANSWER_SHAPE: &str = r#"Answer with only a JSON object: {"title": "…", "body": "…"} (the body is Markdown)."#;

/// The most of the diff the prompt carries; past it the file list still says what changed.
pub const DIFF_MAX: usize = 24_000;

/// What the side run is asked. `task`: the task's title; `asked`: its first prompt; `answered`: its last answer's
/// summary; `diff`: what changed (`diff_text`); `template`: the repo's template, if any.
pub fn prompt(task: &str, asked: &str, answered: &str, diff: &str, template: Option<&str>) -> String {
    let mut parts: Vec<String> = vec![
        "Write the pull request for the changes below. Answer from what is here: don't run any tool or change any file.\n\n".to_owned(),
        format!("The task was: {}\n", one_line(task)),
    ];
    if !asked.trim().is_empty() {
        parts.push(format!("What was asked:\n{}\n", clip(asked.trim(), 1500)));
    }
    if !answered.trim().is_empty() {
        parts.push(format!("What the agent said it did:\n{}\n", clip(answered.trim(), 1500)));
    }
    parts.push(if diff.trim().is_empty() {
        "\nThe branch has no changes yet.\n".to_owned()
    } else {
        format!("\nThe changes (files, then the diff):\n<diff>\n{}\n</diff>\n", clip(diff.trim(), DIFF_MAX))
    });
    parts.push(match template {
        Some(t) if !t.trim().is_empty() => format!(
            "\nThe repository's pull request template follows. Fill it in: keep its headings and checkboxes, tick only what \
             is true, drop HTML comments.\n<template>\n{}\n</template>\n",
            clip(t.trim(), 4000)
        ),
        _ => "\nThe description: what changed and why in a few sentences, then a short list of the notable changes, then how \
              it was tested. Plain words, no filler.\n"
            .to_owned(),
    });
    parts
        .push(format!("\nThe title: one line, at most {TITLE_MAX} characters, in the style of the repository's recent commit subjects.\n"));
    parts.push(ANSWER_SHAPE.to_owned());
    parts.concat()
}

/// A file of the diff, as `diff_text` reads it.
#[derive(Debug)]
pub struct DiffFile<'a> {
    pub path: &'a str,
    pub status: char,
    pub add: u32,
    pub del: u32,
    /// its rows: (`+`, `-` or ` `, the line)
    pub rows: Vec<(char, &'a str)>,
}

/// The files with their counts, then each file's changed lines (its context lines left out): what the prompt carries.
pub fn diff_text(files: &[DiffFile<'_>]) -> String {
    let mut out: Vec<String> = files.iter().map(|f| format!("{} {} (+{} -{})", f.status, f.path, f.add, f.del)).collect();
    for f in files {
        let changed: Vec<String> = f.rows.iter().filter(|(op, _)| *op == '+' || *op == '-').map(|(op, l)| format!("{op}{l}")).collect();
        if !changed.is_empty() {
            out.push(format!("\n--- {}\n{}", f.path, changed.join("\n")));
        }
    }
    out.join("\n")
}

/// The title and body from the side run's JSON answer, or None when it gave neither.
pub fn written(json: Option<&Value>) -> Option<(String, String)> {
    let v = json?;
    let title = one_line(v.get("title").and_then(Value::as_str).unwrap_or_default());
    let body = v.get("body").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
    if title.is_empty() && body.is_empty() {
        return None;
    }
    Some((title.chars().take(TITLE_MAX).collect(), body))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_owned() } else { format!("{}…", s.chars().take(max).collect::<String>()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_prompt_carries_the_task_and_the_repos_template_and_asks_for_json() {
        let p = prompt(
            "Fix the flaky login test",
            "the login test fails 1 in 5",
            "Waited on the session cookie",
            "M src/a.ts (+1 -0)",
            Some("## What\n\n## Why\n- [ ] Tests"),
        );
        assert!(p.contains("The task was: Fix the flaky login test"));
        assert!(p.contains("the login test fails 1 in 5") && p.contains("Waited on the session cookie"));
        assert!(p.contains("<template>\n## What\n\n## Why\n- [ ] Tests\n</template>"));
        assert!(p.contains("\"title\"") && p.contains("don't run any tool"));
        assert!(p.contains("<diff>\nM src/a.ts (+1 -0)\n</diff>"));
        // no template: what a description holds instead; no changes: said so
        let q = prompt("T", "", "", "", None);
        assert!(!q.contains("<template>") && q.contains("how it was tested") && !q.contains("What was asked"));
        assert!(q.contains("no changes yet") && !q.contains("<diff>"));
    }

    #[test]
    fn the_diff_is_the_file_list_then_the_changed_lines() {
        let files = [
            DiffFile { path: "src/a.ts", status: 'M', add: 1, del: 1, rows: vec![(' ', "keep"), ('-', "old"), ('+', "new")] },
            DiffFile { path: "img.png", status: 'A', add: 0, del: 0, rows: Vec::new() },
        ];
        assert_eq!(diff_text(&files), "M src/a.ts (+1 -1)\nA img.png (+0 -0)\n\n--- src/a.ts\n-old\n+new");
    }

    #[test]
    fn the_answer_is_read_back_one_line_title_and_trimmed_body() {
        let w = written(Some(&json!({ "title": "fix(login):  wait on the\nsession cookie ", "body": "\n## What\nWaits.\n" })));
        assert_eq!(w, Some(("fix(login): wait on the session cookie".into(), "## What\nWaits.".into())));
        assert_eq!(written(Some(&json!({ "title": "", "body": " " }))), None);
        assert_eq!(written(None), None);
        let long = written(Some(&json!({ "title": "x".repeat(300), "body": "b" }))).unwrap();
        assert_eq!(long.0.chars().count(), TITLE_MAX);
    }
}
