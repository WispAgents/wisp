//! The keymap: one table, which the keys are handled by and `?` shows (generated, so it can't drift). The goto row
//! is the one row built at draw time ([`goto`]): its letters are the registered views', a plugin's included.

use wisp_tui_api::Registry;

/// Where a binding works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Everywhere,
    List,
    Task,
    Composer,
    View,
}

impl Where {
    pub fn title(self) -> &'static str {
        match self {
            Self::Everywhere => "Everywhere",
            Self::List => "Tasks",
            Self::Task => "The task",
            Self::Composer => "Composer",
            Self::View => "Views",
        }
    }
}

pub const KEYS: &[(Where, &str, &str)] = &[
    (Where::Everywhere, "Ctrl-K  :", "command palette: tasks, views, commands, settings"),
    (Where::Everywhere, "?", "this help (j k scroll it on a small screen)"),
    (Where::Everywhere, "Tab  Shift-Tab", "next zone · in the composer: cycle mode plan → edits → auto"),
    (Where::Everywhere, GOTO, "go to a view (its letters: the views registered, drawn from them)"),
    (Where::Everywhere, "[  ]", "narrow or widen the focused zone (or drag its border)"),
    (Where::Everywhere, "Ctrl-C  q", "quit (q outside the composer)"),
    (Where::List, "j k  ↑ ↓", "move"),
    (Where::List, "Enter  l  →", "open the task"),
    (Where::List, "n", "new task"),
    (Where::List, "i", "reply to the selected task (none: start one)"),
    (Where::List, "/", "search: ↑ ↓ between the matches, Enter opens one"),
    (Where::List, "s", "stop the running task (asks first)"),
    (Where::Task, "i  Enter", "write (the composer)"),
    (Where::Task, "j k  Space", "move between steps · open or close one (a tool call, a diff)"),
    (Where::Task, "o", "open in its harness (Claude Code, Codex…), back here after"),
    (Where::Task, "1-9", "answer the question with that choice"),
    (Where::Task, "y  a  n", "allow · always allow · deny a tool (approvals only, never a question)"),
    (Where::Task, "Esc  h  ←", "back to the list"),
    (Where::Task, "d", "the changes pane (diffs) · again: close it"),
    (Where::Task, "t", "the Thread pane: the task's Notes and how worn it is · again: close it"),
    (Where::Task, "p", "the side pane's next tab (changes, files, terminal, setup, thread)"),
    (Where::Task, "s", "stop the running task (asks first)"),
    (Where::Task, "/", "search the transcript: what matches is marked"),
    (Where::Task, "PgUp PgDn  G  End", "scroll · to the end (and follow)"),
    (Where::Composer, "Enter", "send (queued while a turn runs)"),
    (Where::Composer, "Alt-Enter  Ctrl-J", "new line"),
    (Where::Composer, "@", "mention a task, crew member, machine, My Stuff item or file"),
    (Where::Composer, "/", "a command (/btw a side question, /fork, /stop, /goal, any catalogue command)"),
    (Where::Composer, "↑", "history, in an empty box"),
    (Where::Composer, "Ctrl-E  Ctrl-A", "edit in $EDITOR · New task: the next agent (Auto, each harness)"),
    (Where::Composer, "Alt-↑  Alt-↓  Ctrl-S", "reorder the queued follow-up · send it now"),
    (Where::Composer, "Esc", "leave the composer (never stops the task: s does, and asks first)"),
    (Where::View, "j k", "move"),
    (Where::View, "a letter", "the row's action (shown in the view's footer)"),
    (Where::View, "r", "reload"),
];

/// The goto row's key in [`KEYS`]: drawn as [`goto`] says, from the views registered.
pub const GOTO: &str = "g then";

/// The goto row as `?` shows it: `g then` each registered view's letter, then `t` (the tasks), and where each goes. A
/// static string here missed a plugin's view (history's Blame, `b`) and drifted as views came and went.
pub fn goto(reg: &Registry) -> (String, String) {
    let views: Vec<(char, &str)> = reg.views.iter().filter_map(|v| v.goto().map(|c| (c, v.title()))).collect();
    let letters: Vec<String> = views.iter().map(|(c, _)| c.to_string()).chain(["t".to_owned()]).collect();
    let names: Vec<&str> = views.iter().map(|(_, t)| *t).chain(["tasks"]).collect();
    (format!("{GOTO} {}", letters.join(" ")), names.join(", "))
}
