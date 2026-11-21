//! wisp's commands in the TUI (TRY-7), from the same catalogue the CLI builds its verbs from (ADR-0022): each shows by
//! its title with the words it's typed as (`Add a machine · machines add`), the reply box says how to use the one being
//! typed (`machines add <name> [--address] …`), and `/machines add studio --address me@studio` runs it as the CLI
//! would. Pure, for tests.

use serde_json::{Value, json};
use wisp_cli_core::verbs::{self, Catalogue, Verb};

use super::palette::{Item, Pick};

/// Every command as a palette or `/` item: its title (else its words), and its words at the right.
pub fn items(cat: &Catalogue, slash: bool) -> Vec<Item> {
    cat.verbs
        .iter()
        .filter(|v| v.info.ui_only.is_none())
        .map(|v| {
            let path = v.path();
            let label = v.info.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| path.clone());
            Item { label, kind: path.clone(), pick: if slash { Pick::Insert(format!("/{path} ")) } else { Pick::Command(path) } }
        })
        .collect()
}

/// The words of a line, `"quoted words"` and `'these'` kept as one.
pub fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            // inside quotes, or any other character
            (_, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The verb a `/…` line names, and the words after it.
fn verb_of<'a>(cat: &'a Catalogue, line: &str) -> Option<(&'a Verb, Vec<String>)> {
    let words = split(line.strip_prefix('/')?);
    let (v, taken) = cat.resolve(&words)?;
    Some((v, words.get(taken..).map(<[String]>::to_vec).unwrap_or_default()))
}

/// How to use the command being typed, on one line: `Add a machine: machines add <name> [--address] [--port]`; the
/// field the caret is on comes first in the doc tail. None when the line names no command.
pub fn usage(cat: &Catalogue, line: &str) -> Option<String> {
    let (v, _) = verb_of(cat, line)?;
    let pos = v.spec.positionals();
    let mut parts = vec![v.path()];
    parts.extend(pos.iter().map(|f| format!("<{}>", f.flag())));
    parts.extend(v.spec.fields.iter().filter(|f| !pos.iter().any(|p| p.name == f.name)).map(|f| format!("[--{}]", f.flag())));
    if !v.spec.object && !v.spec.none {
        parts.push("[--args JSON]".to_owned());
    }
    let what = v.info.title.as_deref().filter(|t| !t.is_empty()).map_or(String::new(), |t| format!("{t}: "));
    let doc = pos.first().map(|f| f.doc.trim()).filter(|d| !d.is_empty()).map_or(String::new(), |d| format!("   <{}> {d}", pos[0].flag()));
    Some(format!("{what}{}{doc}", parts.join(" ")))
}

/// A `/…` line as a command to run: its id and arguments (the open task's id when it takes one and none was given).
/// None: the line names no command; Err: it does, but the words don't fit (the message says why).
pub fn parse(cat: &Catalogue, line: &str, task: Option<&str>) -> Option<Result<(String, Value), String>> {
    let (v, rest) = verb_of(cat, line)?;
    let wants_id = v.spec.field("id").is_some_and(|f| !f.optional);
    let given = |args: &Value| args.get("id").is_some();
    // no task open and no id typed (no word before the flags, no --id): say what's missing in the TUI's words
    let typed_id = rest.first().is_some_and(|w| !w.starts_with("--")) || rest.iter().any(|w| w == "--id" || w.starts_with("--id="));
    if wants_id && task.is_none() && !typed_id {
        return Some(Err(format!("{} needs a task: open one, or give its id", v.path())));
    }
    // the open task fills a required id the line left out, so `/runtime archive` acts on the task you're in
    let built = match (wants_id, task) {
        (true, Some(t)) if rest.is_empty() || rest.first().is_some_and(|w| w.starts_with("--")) => {
            let mut words = vec![t.to_owned()];
            words.extend(rest.iter().cloned());
            verbs::build(v, &words)
        }
        _ => verbs::build(v, &rest),
    };
    Some(match built {
        Ok(args) if wants_id && !given(&args) => Err(format!("{} needs a task: open one, or give its id", v.path())),
        Ok(args) => Ok((v.info.id.clone(), if args.is_null() && v.spec.object { json!({}) } else { args })),
        Err(u) => Err(u.0),
    })
}
