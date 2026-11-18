//! A shared task's answer as a reader sees it (QA round 6 #2): what the agent wrote, and its ```` ```wisp ```` block as the
//! app would show it (the hero's title and summary, the prose, the steps, files, tests, callouts), never as JSON. Only
//! what reads as text is kept: blocks that point at the sharer's machine or act on it (images and videos by path,
//! previews, diffs, terminals, actions, questions) are left out of a public page. Everything is escaped.

use serde_json::Value;

use super::esc;

const FENCE: &str = "```wisp";

/// The answer's own words, and the JSON bodies of its wisp blocks. An unclosed block (a cut-off answer) is dropped
/// from its fence to the end, so half a JSON object never shows.
fn split(text: &str) -> (String, Vec<String>) {
    let mut prose = String::new();
    let mut bodies = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(FENCE) {
        prose.push_str(&rest[..at]);
        let after = &rest[at + FENCE.len()..];
        let Some(end) = after.find("```") else {
            rest = "";
            break;
        };
        bodies.push(after[..end].trim().to_owned());
        rest = &after[end + 3..];
    }
    prose.push_str(rest);
    (prose.trim().to_owned(), bodies)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn items(v: &Value) -> Vec<Value> {
    v.get("items").and_then(Value::as_array).cloned().unwrap_or_default()
}

/// A line: its bold part and the rest, either may be empty.
fn line(bold: &str, rest: &str) -> String {
    match (bold.is_empty(), rest.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("<li><b>{}</b></li>", esc(bold)),
        (true, false) => format!("<li>{}</li>", esc(rest)),
        (false, false) => format!("<li><b>{}</b> {}</li>", esc(bold), esc(rest)),
    }
}

fn list(lines: &[String]) -> String {
    let lines: String = lines.concat();
    if lines.is_empty() { String::new() } else { format!(r#"<ul class="hub-list">{lines}</ul>"#) }
}

/// One block, as text; "" for a block a public page doesn't show.
fn block(b: &Value) -> String {
    match s(b, "type") {
        "hero" => {
            let title = s(b, "title");
            let summary = s(b, "summary");
            let t = if title.is_empty() { String::new() } else { format!(r#"<p class="hub-hero">{}</p>"#, esc(title)) };
            let m = if summary.is_empty() { String::new() } else { format!(r#"<p class="hub-sum">{}</p>"#, esc(summary)) };
            format!("{t}{m}")
        }
        "markdown" => {
            let t = s(b, "text");
            if t.is_empty() { String::new() } else { format!(r#"<div class="hub-prose">{}</div>"#, esc(t)) }
        }
        "steps" => list(&items(b).iter().map(|i| line(s(i, "title"), s(i, "detail"))).collect::<Vec<_>>()),
        "files" | "tree" => list(
            &items(b)
                .iter()
                .map(|i| {
                    let note = [s(i, "change"), s(i, "note")].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(": ");
                    let path = esc(s(i, "path"));
                    if note.is_empty() {
                        format!("<li><code>{path}</code></li>")
                    } else {
                        format!("<li><code>{path}</code> {}</li>", esc(&note))
                    }
                })
                .collect::<Vec<_>>(),
        ),
        "tests" => {
            let n = |k: &str| b.get(k).and_then(Value::as_i64).unwrap_or(0);
            format!(r#"<p class="hub-sum">{}: {} passed, {} failed</p>"#, esc(s(b, "command")), n("passed"), n("failed"))
        }
        "callout" => list(&[line(s(b, "title"), s(b, "text"))]),
        "checklist" => list(
            &items(b).iter().map(|i| line(if i["done"].as_bool() == Some(true) { "✓" } else { "○" }, s(i, "text"))).collect::<Vec<_>>(),
        ),
        "stats" => list(
            &items(b)
                .iter()
                .map(|i| {
                    let value = i.get("value").map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned)).unwrap_or_default();
                    line(s(i, "label"), &format!("{value}{}", s(i, "unit")))
                })
                .collect::<Vec<_>>(),
        ),
        "code" => {
            let code = s(b, "code");
            if code.is_empty() { String::new() } else { format!(r#"<pre class="hub-pre">{}</pre>"#, esc(code)) }
        }
        _ => String::new(),
    }
}

fn blocks_of(body: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("blocks").and_then(Value::as_array).cloned()).unwrap_or_default()
}

/// One answer (`report.answers[]`: `text`, and/or `blocks`), as the page shows it: in one box, its words first, then
/// what its blocks say. An answer carries its blocks parsed (`blocks`) and also as the fenced text they came from: the
/// parsed ones win, the fenced copy is only for an answer without them, and the same block is never shown twice (QA
/// round 7 #2).
pub(super) fn answer_html(a: &Value) -> String {
    let (prose, bodies) = split(s(a, "text"));
    let parsed: Vec<Value> = a.get("blocks").and_then(Value::as_array).cloned().unwrap_or_default();
    let all = if parsed.is_empty() { bodies.iter().flat_map(|b| blocks_of(b)).collect() } else { parsed };
    let mut seen: Vec<&Value> = Vec::new();
    for b in &all {
        if !seen.contains(&b) {
            seen.push(b);
        }
    }
    let shown: String = seen.into_iter().map(block).collect();
    let words = if prose.is_empty() { String::new() } else { format!(r#"<div class="hub-prose">{}</div>"#, esc(&prose)) };
    if words.is_empty() && shown.is_empty() { String::new() } else { format!(r#"<div class="hub-answer">{words}{shown}</div>"#) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_and_the_blocks_come_apart_and_a_cut_block_is_dropped() {
        assert_eq!(split("All done.\n```wisp\n{\"blocks\":[]}\n```\nBye"), ("All done.\n\nBye".into(), vec!["{\"blocks\":[]}".into()]));
        assert_eq!(split("Half.\n```wisp\n{\"blo"), ("Half.".into(), vec![]));
        assert_eq!(split("No block."), ("No block.".into(), vec![]));
    }

    #[test]
    fn an_answer_with_its_blocks_twice_shows_them_once_inside_its_box() {
        // QA round 7 #2: an answer carries the blocks both parsed (`blocks`) and as the fenced text they came from
        let hero = serde_json::json!({ "type": "hero", "title": "Fake claude finished", "summary": "No question asked." });
        let text = format!("All done.\n```wisp\n{}\n```\n", serde_json::json!({ "blocks": [hero.clone()] }));
        let h = answer_html(&serde_json::json!({ "text": text, "blocks": [hero] }));
        assert_eq!(h.matches("Fake claude finished").count(), 1, "{h}");
        assert_eq!(h.matches("No question asked.").count(), 1, "{h}");
        // one box: the words and what the blocks say, all inside it
        assert!(
            h.starts_with(r#"<div class="hub-answer">"#) && h.ends_with("</div>") && h.matches(r#"class="hub-answer""#).count() == 1,
            "{h}"
        );
        let inside = &h[h.find("All done.").unwrap_or(usize::MAX)..];
        assert!(inside.contains("Fake claude finished"), "the title follows the words, in the box: {h}");
        // only the fenced copy (an answer from before `blocks`): still once, still inside
        let fenced = answer_html(
            &serde_json::json!({ "text": "Done.\n```wisp\n{\"blocks\":[{\"type\":\"hero\",\"title\":\"T1\"},{\"type\":\"hero\",\"title\":\"T1\"}]}\n```" }),
        );
        assert_eq!(fenced.matches("T1").count(), 1, "identical blocks once: {fenced}");
        assert!(answer_html(&serde_json::json!({})).is_empty(), "nothing: no empty box");
    }
}
