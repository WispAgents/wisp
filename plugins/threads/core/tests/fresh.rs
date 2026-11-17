//! The brief a fresh thread starts from (v1 test/threads.test.js `freshBrief`): the Notes one level down, else what
//! was asked first; the files the task changed, latest kept, each once; the last turns; capped.
#![allow(clippy::unwrap_used)]

use wisp_threads_core::fresh::{BRIEF_MAX, BriefIn, FILES_MAX, Recent, brief, clip};

#[test]
fn the_brief_has_the_notes_the_work_and_the_last_turns() {
    let files = vec!["src/a.rs".to_owned(), "src/b.rs".to_owned(), "src/a.rs".to_owned()];
    let recent = [
        Recent { n: 7, prompt: "fix the parser", answer: "Fixed; tests pass.\n```wisp\n{\"blocks\":[]}\n```" },
        Recent { n: 8, prompt: "", answer: "" },
    ];
    let b = brief(&BriefIn {
        notes: "## Goal\nImport CSV\n\n## Constraints\n- No Redis",
        first: "build an importer",
        files: &files,
        branch: Some("wisp/import"),
        recent: &recent,
        tool: "mcp__wisp__wisp_notes",
    });
    assert!(b.starts_with("# Fresh thread"), "{b}");
    assert!(b.contains("## Task Notes\n\n### Goal\nImport CSV") && b.contains("### Constraints\n- No Redis"), "{b}");
    assert!(!b.contains("What was asked first"), "the Notes stand for it");
    assert!(b.contains("- Branch `wisp/import`") && b.contains("`src/b.rs`, `src/a.rs`"), "each file once, latest last: {b}");
    assert!(b.contains("## The last turn\n\n### Turn 7") && b.contains("**Answer:** Fixed; tests pass."), "an empty turn is left out: {b}");
    assert!(!b.contains("```wisp") && !b.contains("blocks"), "the answer's words, not its presentation's JSON: {b}");
    assert!(b.contains("mcp__wisp__wisp_notes"));
}

#[test]
fn without_notes_it_starts_from_what_was_asked_first_and_it_is_capped() {
    let b = brief(&BriefIn { first: "build an importer", tool: "wisp_notes", ..BriefIn::default() });
    assert!(b.contains("## What was asked first\n\nbuild an importer") && !b.contains("## The work"), "{b}");
    let files: Vec<String> = (0..100).map(|i| format!("f{i}.rs")).collect();
    let huge = "é".repeat(40_000);
    let recent = [Recent { n: 1, prompt: &huge, answer: &huge }];
    let b = brief(&BriefIn { notes: &format!("## Goal\n{huge}"), files: &files, recent: &recent, tool: "t", ..BriefIn::default() });
    assert!(b.chars().count() <= BRIEF_MAX && b.ends_with('…'));
    let b = brief(&BriefIn { files: &files, tool: "t", ..BriefIn::default() });
    assert!(
        b.contains("`f99.rs`") && !b.contains("`f59.rs`") && b.contains(&format!("`f{}.rs`", 100 - FILES_MAX)),
        "the latest {FILES_MAX}"
    );
    assert_eq!(clip("héllo", 3), "hé…");
    assert_eq!(clip("hé", 3), "hé");
}
