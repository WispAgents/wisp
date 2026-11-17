//! What a Make a pane agent is told (v1 server/pane-guide.js): how a pane works, what it may use, and how to hand
//! one back. The scopes and calls come from `wisp_stuff_api::pane`, so it never drifts from what the bridge does.
//! v2 had the bridge, the routes and `wisp_save`'s `pin`, but nothing told the task it made a wisp pane: asked for
//! "a hello world pane", an agent built its own harness's kind of pane instead (owner, 2026-11-17).

use std::fmt::Write as _;

use wisp_stuff_api::pane::{CALL_DOCS, MAX_PANES, PANE_CALLS, PANE_USES, PANE_WRITES_PER_MIN};

/// The `ext` a Make a pane task carries: `{"wisp.stuff": {"kind": "pane"}}`.
pub const EXT_KEY: &str = "wisp.stuff";

/// Whether a task's `ext` says it's a Make a pane task.
pub fn is_pane_task(ext: &serde_json::Value) -> bool {
    ext.pointer("/wisp.stuff/kind").and_then(serde_json::Value::as_str) == Some("pane")
}

const EXAMPLE_ASK: &str = "A pane with a count of my running tasks";
const EXAMPLE_MANIFEST: &str = r#"{
  "name": "Running now",
  "description": "How many of your tasks are running.",
  "uses": ["tasks:read"]
}"#;
const EXAMPLE_HTML: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>Running now</title>
<link rel="stylesheet" href="/_wisp/pane.css">
<script src="/_wisp/pane.js"></script>
<div class="card"><b id="count">…</b> <span class="muted">running</span></div>
<p class="muted" id="note"></p>
<script>
  const paint = (tasks) => {
    document.getElementById('count').textContent = tasks.filter((t) => t.state === 'running').length;
  };
  // Refused until the user allows tasks:read from the pane's bar: say so, and wait.
  wisp.tasks.list().then((tasks) => {
    paint(tasks);
    wisp.on('tasks', paint);
  }, (err) => {
    document.getElementById('note').textContent = err.message;
  });
</script>
"#;

/// The reference: everything a pane can touch.
pub fn pane_reference() -> String {
    let mut scopes = String::new();
    for (k, words) in PANE_USES {
        let _ = writeln!(scopes, "- `{k}`: {words}");
    }
    let mut calls = String::new();
    for (method, need) in PANE_CALLS {
        if let Some((_, call, returns)) = CALL_DOCS.iter().find(|(m, _, _)| m == method) {
            let _ = writeln!(calls, "- `{call}` (needs `{need}`) → {returns}");
        }
    }
    format!(
        "## wisp-app.json

{{ \"name\": \"…\", \"description\": \"one line\", \"entry\": \"index.html\", \"uses\": [\"…\"], \"db\": true }}

name is what its bar shows; entry defaults to index.html; db: true gives it a SQLite database of its own (\"db\": \"seed.sql\" makes its tables from that file), which its pages use with import {{ sql }} from '/_wisp/db.js'; once it's published, anyone with the link can read and change that data, so no personal data in it. uses lists what it needs from wisp, from this list and nothing else:

{scopes}
## Its page

<script src=\"/_wisp/pane.js\"></script> gives it `wisp`. <link rel=\"stylesheet\" href=\"/_wisp/pane.css\"> gives it wisp's look: the tokens (var(--ink), var(--muted), var(--faint), var(--bg), var(--bg-2), var(--surface), var(--line), var(--line-2), var(--accent)), light and dark, following the user's theme, and .btn, .btn.primary, .card, .chip, .muted, .faint, .sk (a loading placeholder). Use them, and no colours of your own.

Every call returns a promise.

{calls}- `wisp.context`: {{ where: \"task\" | \"home\", task: id | null, theme: \"light\" | \"dark\" }}.
- `wisp.on('context', fn)`: fn(context) when it changes (another task opened, the theme).

Beside tasks, `context.task` is the open task: repaint when it changes.

## The rules it lives under

- A call the user hasn't allowed rejects. Nothing is allowed until they press Allow… on the pane's bar, so the first paint must make sense without access: catch the rejection and show its message.
- `tasks.launch` and `tasks.followUp` open wisp's own card with the exact text, and only the user's Send sends it. They can cancel, and only one card opens at a time. Handle the rejection every time and say so in the page: it is normal, not a crash. Never retry on your own.
- At most {PANE_WRITES_PER_MIN} writes a minute from a pane, cancelled ones included; past that they reject. Show that error too.
- Changing any file of the app turns `tasks:write` off until the user allows it again; its bar says *Changed since allowed*, and read scopes stay. Say this in your answer when the pane has `tasks:write`, every time you update it.
- An app that uses wisp may only reach its own address. No external scripts, styles, fonts, images or fetch, so no CDN, whatever wisp_save says about CDNs: bundle everything into the folder. Frames are off.
- Links can't open anything: the frame has no popups and can't leave its own address. A link inside the app works; one to anywhere else fails. Show an address as text.
- There are at most {MAX_PANES} panes. It is one frame in a side panel about 400px wide, or a tile on home at least 300px wide and 420px tall that the user can resize, and a full page on phones: it must work narrow, and scroll inside itself."
    )
}

/// The instructions a Make a pane task's agent gets before what the user asked (`ask_tool`: how it asks the user).
pub fn pane_guide(ask_tool: &str) -> String {
    format!(
        "# Making a pane

The user asked for a pane: a small web app that lives in wisp, in My Stuff, pinned beside their tasks or on home. It can read their tasks and more through wisp, if they allow it. It is a wisp pane, not a plugin, extension or panel of the agent you run in. You are in a folder of your own.

1. If what they want is really unclear, ask with {ask_tool}. Otherwise just build it.
2. Build it in a folder here: index.html (with its JS and CSS, one file if that's reasonable; plain HTML and JS, no build step) and wisp-app.json.
3. Ask for the least it needs. Never ask for tasks:write when reading is enough, and leave \"uses\" out if it needs nothing from wisp.
4. wisp_save it with app: <the folder> and pin: \"home\" for a whole-workspace tool (a board, a queue) or pin: \"task\" for something about the open task. Read what the save answers: it says whether it pinned.
5. To change it, edit your folder and wisp_save with the same id and app: <the folder>. The pane reloads on its own, in place. Never touch wisp's own copy, and don't save a second app.
6. End with a stuff block with its id. Tell the user in one or two sentences what it does and, if it uses wisp, that it does nothing until they press Allow… on its bar.

{reference}

## A whole example

The user asked: {EXAMPLE_ASK}

wisp-app.json:

{EXAMPLE_MANIFEST}

index.html:

{EXAMPLE_HTML}",
        reference = pane_reference()
    )
}

/// The prompt a Make a pane task's agent receives: the guide, then what the user asked.
pub fn with_guide(ask_tool: &str, prompt: &str) -> String {
    format!("{}\n\n# What the user asked\n\n{prompt}", pane_guide(ask_tool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guide_names_every_scope_and_call_and_the_ask_comes_last() {
        let g = with_guide("ask_user", "a hello world pane");
        for (scope, _) in PANE_USES {
            assert!(g.contains(&format!("`{scope}`")), "{scope}");
        }
        for (_, call, _) in CALL_DOCS {
            assert!(g.contains(call), "{call}");
        }
        assert!(g.contains("wisp_save") && g.contains("pin: \"home\"") && g.contains("/_wisp/pane.js"));
        assert!(g.ends_with("a hello world pane"));
    }

    #[test]
    fn only_a_pane_task_is_one() {
        assert!(is_pane_task(&serde_json::json!({ "wisp.stuff": { "kind": "pane" } })));
        assert!(!is_pane_task(&serde_json::json!({ "wisp.plugins": { "kind": "cast" } })));
        assert!(!is_pane_task(&serde_json::json!({})));
    }
}
