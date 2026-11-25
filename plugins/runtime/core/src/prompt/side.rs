//! /side (v1 server/side.js, its prompts): a side discussion about a task, a task of its own that
//! works in the main task's folder as it is now and gets the main task's latest state with every
//! message. It has the main task's access; a write to a file the main task changed becomes a proposal on the main
//! task, where its harness lets wisp see a write first. These are its texts.

use super::btw::btw_digest;
use super::handoff::{PastTurn, Render};
use super::text::{Re, js_s, trim};

static SIDE_CMD: Re = Re::new(concat!("(?i)^/side(?:", js_s!(), "+|$)"));

/// The opening message in "/side why …" ('' for a bare /side), or None when the text isn't a /side.
pub fn side_ask(text: &str) -> Option<String> {
    let t = trim(text);
    SIDE_CMD.is_match(t).then(|| trim(&SIDE_CMD.replace(t, "")).to_owned())
}

/// What a side run's write to a file the main task changed answers its CLI: nothing was written, the change waits on
/// the main task as a proposal the user applies or discards there.
pub const SIDE_PROPOSED: &str = "Not written here: the main task changed this file, so your change was proposed to the main task. The user applies or discards it there. Do not retry it or write the file another way; say what you proposed.";

/// The task a side discussion is about (`sideOf`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SideOf<'a> {
    pub id: &'a str,
    pub title: &'a str,
}

/// The system prompt's part for a side discussion. `session_tool`: how the harness names `wisp_session` ('' without
/// wisp's tools). `proposes`: its file writes go to wisp first (its harness can stop a write until wisp answers), and a
/// write to a file the main task changed becomes a proposal on the main task; else its writes land as they are.
pub fn side_note(of: SideOf<'_>, session_tool: &str, proposes: bool) -> String {
    let tool = if session_tool.is_empty() { String::new() } else { format!("`{session_tool}` with id {} reads its whole record.", of.id) };
    let read = format!("`git status`, `git diff`, `git log` show what it changed. {tool}");
    let open = format!(
        "\n\n# wisp side discussion\n\nThis task is a side discussion about another wisp task, the main task “{}” (wisp task {}). Another agent run does the main task's work; here you talk it over with the user: explain what it did and why, review it, weigh options, plan what to ask it next.\n",
        of.title, of.id
    );
    let latest = "- Every message from the user starts with the main task's latest state (under \"The main task now\"). It may have moved on since your last answer: go by the latest one.";
    let writes = if proposes {
        "- wisp sees each file edit or write before it happens. A file the main task hasn't changed is written at once. A file it changed is not: your change goes to the main task as a proposal (a diff the user applies or discards there) and your call answers \"proposed to the main task\". Don't retry it or write that file another way (shell, `sed -i`, `>`, `cp`); say what you proposed."
    } else {
        "- Your writes land in the main task's folder as they are: nothing checks them first. Leave the files the main task changed to it unless the user asks you to change them; say what to tell the main task instead."
    };
    format!(
        "{open}
- You share the main task's folder: the files here are its work as it is right now ({read}) What you change lands in that work, and the main task may be editing at the same time.
- You have the main task's access: run commands, tests, builds and the app, start tasks, use wisp's tools and the connectors, and edit what the user asks you to. Never hand the user a command to run, or tell them to ask the main task for something, that you could do here yourself: do it and report what it showed. Leave the main task's ongoing work to it unless the user asks you to change it.
{writes}
- Don't commit, push, reset, stash, rebase or merge: that rewrites the main task's work. wisp's own Ship, rollback, fork, goals and terminal stay on the main task.
{latest}
- Answer as for any wisp task, wisp presentation block included."
    )
}

/// The main task as a side discussion sees it before each message.
#[derive(Debug, Clone, Copy)]
pub struct MainTask<'a> {
    pub title: &'a str,
    pub turns: &'a [PastTurn<'a>],
}

/// What goes before each of the user's messages: the main task as it stands now (None once it's gone).
pub fn side_prompt(main: Option<MainTask<'_>>, text: &str, now_ms: i64, render: Render<'_>) -> String {
    let head = match main {
        Some(m) => {
            let digest = btw_digest(m.turns, now_ms, render);
            let digest = if digest.is_empty() { "_Nothing has run on it yet._".to_owned() } else { digest };
            format!("## The main task now: “{}”\n\n{digest}", m.title)
        }
        None => {
            "## The main task now\n\n_The main task was deleted: only what was said here, and its folder as it is, are left._".to_owned()
        }
    };
    format!("{head}\n\n---\n\n## The user, in this side discussion\n\n{text}")
}
