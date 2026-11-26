//! The TUI's state and what keys, the mouse and pastes do to it. Pure: a key changes the [`App`] at once (the
//! optimistic result, ADR-0017) and returns the [`Effect`]s the run loop carries out (commands, a topic to watch,
//! `$EDITOR`); their answers come back through [`App::done`], which keeps or rolls back what the key showed.

#![allow(clippy::many_single_char_names)] // `k` (the key), `v` (a view), `r` (its rows), `p` (the palette)

use std::collections::BTreeMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use serde_json::{Value, json};
use wisp_tui_api::{Registry, Source, Theme};

use super::composer::{Composer, Edit};
use super::model::{Tasks, Transcript, s};
use super::palette::{Item, Palette, Pick};

/// The zone with the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    List,
    Task,
    Side,
    Composer,
}

/// What the side pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Changes,
    Files,
    Terminal,
    Setup,
    /// the task's Notes and how worn its thread is (the GUI's Thread tab, ADR-0097)
    Thread,
}

impl Side {
    pub fn title(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Files => "Files",
            Self::Terminal => "Terminal",
            Self::Setup => "Setup",
            Self::Thread => "Thread",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Changes => Self::Files,
            Self::Files => Self::Terminal,
            Self::Terminal => Self::Setup,
            Self::Setup => Self::Thread,
            Self::Thread => Self::Changes,
        }
    }
}

/// plan → edits → auto (Shift-Tab), as runtime's `Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Plan,
    Edits,
    Auto,
}

impl Mode {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Edits => "edits",
            Self::Auto => "auto",
        }
    }
    /// A mode as runtime writes it (`"plan"`); anything else isn't one.
    pub fn of(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(Self::Plan),
            "edits" => Some(Self::Edits),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
    /// How much it lets the agent do, as runtime ranks it (`Mode::rank`): a cap is lifted by a choice above it.
    pub fn rank(self) -> u8 {
        match self {
            Self::Plan => 0,
            Self::Edits => 1,
            Self::Auto => 2,
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Plan => Self::Edits,
            Self::Edits => Self::Auto,
            Self::Auto => Self::Plan,
        }
    }
}

/// Which picker the palette is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum For {
    Everything,
    At,
    Slash,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    Help,
    Palette(Palette, For),
    /// a row action that asks first: (question, command, args)
    Confirm(String, String, Value),
    /// a view whose source asks for a value first (`"{?File}"`): (view id, label, what's typed)
    Ask(String, String, String),
    /// a row action that asks for a value first (`"{?Name}"`, TRY-7: a new crew member, a job): (label, command, args,
    /// what's typed)
    AskAct(String, String, Value, String),
}

/// A view's rows, as its source last gave them.
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(clippy::struct_field_names)] // `views["crew"].rows`
pub struct Rows {
    pub rows: Vec<Value>,
    /// what the view's source asked for, answered (`"{?File}"`), kept for reloads
    pub asked: Option<String>,
    pub sel: usize,
    pub loading: bool,
    pub error: String,
}

/// What the run loop does after a command answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Then {
    Nothing,
    /// the rows of this view
    View(String),
    /// the `/` menu's commands
    Catalogue,
    /// the `@` picker's crew, machines and My Stuff
    Mentions(String),
    /// a new task: the optimistic row `local` is replaced by the real one; refused, its text and attachments go back
    /// in the box
    Created(String, String, Vec<String>),
    /// `runtime.agents`: the harnesses the New task page offers
    Agents,
    /// a follow-up shown at once under the composer: dropped when it lands, back in the box when refused
    Sent(String),
    /// a stop shown as pending on the task
    Stopped(String),
    /// an answer to the open ask: refused because the ask changed, it says so and isn't sent again
    Answered,
    /// a /btw on this task: its entry (the question, the answer when it lands) goes in the transcript
    Btw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Quit,
    /// watch this task's topic (None: none)
    Watch(Option<String>),
    Cmd {
        id: String,
        args: Value,
        then: Then,
    },
    /// a new task or a follow-up, with its attachments (paths: the loop reads their size)
    Send {
        task: Option<String>,
        text: String,
        attachments: Vec<String>,
        /// a new task's mode; a follow-up's only when the user chose one (Shift-Tab) while the task was open: sending
        /// the bar's mode every time asked a capped task for more than its cap, which runtime reads as the user lifting it
        mode: Option<Mode>,
        queue: bool,
        local: String,
        /// a new task's agent picked on the New task page (None: Auto, wisp's default)
        agent: Option<String>,
    },
    /// the composer's text in `$EDITOR`; what's saved comes back with [`App::edited`]
    Editor(String),
    /// Open in <harness>: the terminal handed to `wisp open <task>` (the harness resumes the task there), then back
    OpenIn(String),
    /// the project's files for the `@` picker (`git ls-files`), back as `Then::Mentions("file")`
    Files,
}

/// A toast: its text, whether it's an error, and the frame it goes at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub bad: bool,
    pub until: u64,
}

/// The layout the last frame drew, for the mouse (zone rects and borders).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hit {
    pub list: ratatui::layout::Rect,
    pub task: ratatui::layout::Rect,
    pub side: ratatui::layout::Rect,
    pub composer: ratatui::layout::Rect,
    /// a screen row of the sidebar → the task drawn there (each task is two lines)
    pub rows: BTreeMap<u16, String>,
    /// a screen row of the transcript → the tool step drawn there (a click opens or closes it)
    pub tools: BTreeMap<u16, i64>,
}

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // independent screen flags (new task, online, goto, ascii), not a state machine
pub struct App {
    pub reg: Registry,
    pub th: Theme,
    pub cwd: String,
    pub tasks: Tasks,
    /// the selected task's id (kept across reorders)
    pub sel: Option<String>,
    pub open: Option<Transcript>,
    pub composer: Composer,
    /// the composer starts a new task (n), not a follow-up
    pub new_task: bool,
    pub focus: Zone,
    pub side: Option<Side>,
    /// the open task's mode, as its row says, until the user picks another (Shift-Tab)
    pub mode: Mode,
    /// the user pressed Shift-Tab since this task was opened: only then does a follow-up carry `mode`
    pub mode_touched: bool,
    /// the New task page's mode (Shift-Tab there), apart from the open task's
    pub launch_mode: Mode,
    pub overlay: Option<Overlay>,
    /// a view instead of the task (None: the tasks)
    pub screen: Option<String>,
    pub views: BTreeMap<String, Rows>,
    /// the sidebar's and the side pane's widths, in columns
    pub widths: (u16, u16),
    /// the transcript's scroll from its end (0: following)
    pub scroll: u16,
    /// the tool step the cursor is on (its seq)
    pub cursor: Option<i64>,
    pub search: Option<String>,
    pub toasts: Vec<Toast>,
    pub online: bool,
    /// the wall clock in ms, set by the run loop each frame (the rows' "5m"); 0: no times shown
    pub now: i64,
    /// `$TERM` is dumb: ASCII glyphs only
    pub ascii: bool,
    /// the frame count (spinners, toasts)
    pub tick: u64,
    /// `g` was pressed: the next key is a goto
    pub goto: bool,
    /// how far the `?` help is scrolled
    pub help_scroll: u16,
    /// the `/` menu's commands: (id, title)
    pub catalogue: Vec<(String, String)>,
    /// the harnesses installed here (id, name), for the New task page's agent (TRY-7)
    pub agents: Vec<(String, String)>,
    /// the New task page's agent: '' is Auto (wisp's default); Ctrl-A steps through `agents`
    pub launch_agent: String,
    /// the same commands with their words and arguments, as the CLI types them (`commands.rs`, TRY-7)
    pub cat: wisp_cli_core::verbs::Catalogue,
    /// the `@` picker's items by kind (crew, machines, stuff, files)
    pub mentions: BTreeMap<String, Vec<Item>>,
    /// follow-ups sent, not yet in the session's queue or a turn: (local id, text)
    pub sending: Vec<(String, String)>,
    /// tasks with a stop on its way
    pub stopping: Vec<String>,
    pub hit: Hit,
    /// the border being dragged
    pub dragging: Option<Zone>,
    n: u64,
}

/// The New task page's next agent: Auto, then each installed harness, then Auto again.
fn next_agent(agents: &[(String, String)], now: &str) -> String {
    let at = agents.iter().position(|(id, _)| id == now);
    match at {
        None if now.is_empty() => agents.first().map(|(id, _)| id.clone()).unwrap_or_default(),
        Some(i) if i + 1 < agents.len() => agents[i + 1].0.clone(),
        _ => String::new(),
    }
}

/// A command that never reached wisp (the client's `wisp at …: error sending request for url …`), in plain words;
/// None: wisp answered, and its own words say why.
fn unreachable(e: &str) -> Option<String> {
    let l = e.to_lowercase();
    ["error sending request", "connection refused", "connection reset", "tcp connect error", "timed out", "broken pipe"]
        .iter()
        .any(|x| l.contains(x))
        .then(|| "wisp isn't answering; your prompt is still in the box".to_owned())
}

fn ctrl(k: &KeyEvent, c: char) -> bool {
    k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char(c)
}

fn alt(k: &KeyEvent) -> bool {
    k.modifiers.contains(KeyModifiers::ALT)
}

const TOAST_FRAMES: u64 = 60 * 4;

impl App {
    pub fn new(reg: Registry, th: Theme, cwd: &str) -> Self {
        Self {
            reg,
            th,
            cwd: cwd.to_owned(),
            tasks: Tasks::default(),
            sel: None,
            open: None,
            composer: Composer::default(),
            new_task: false,
            focus: Zone::List,
            side: None,
            mode: Mode::Auto,
            mode_touched: false,
            launch_mode: Mode::Auto,
            overlay: None,
            screen: None,
            views: BTreeMap::new(),
            widths: (34, 56),
            scroll: 0,
            cursor: None,
            search: None,
            toasts: Vec::new(),
            online: true,
            now: 0,
            ascii: false,
            tick: 0,
            goto: false,
            help_scroll: 0,
            catalogue: Vec::new(),
            cat: wisp_cli_core::verbs::Catalogue::default(),
            agents: Vec::new(),
            launch_agent: String::new(),
            mentions: BTreeMap::new(),
            sending: Vec::new(),
            stopping: Vec::new(),
            hit: Hit::default(),
            dragging: None,
            n: 0,
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, bad: bool) {
        self.toasts.push(Toast { text: text.into(), bad, until: self.tick + TOAST_FRAMES });
    }

    /// One frame went by.
    pub fn frame(&mut self) {
        self.tick += 1;
        let t = self.tick;
        self.toasts.retain(|x| x.until > t);
    }

    fn local(&mut self) -> String {
        self.n += 1;
        format!("local-{}", self.n)
    }

    /// The tasks in the order the sidebar shows them.
    pub fn order(&self) -> Vec<String> {
        let q = self.search.as_ref().filter(|_| self.focus == Zone::List).map(|q| q.to_lowercase()).unwrap_or_default();
        self.tasks
            .ordered()
            .into_iter()
            .filter(|(_, _, t)| q.is_empty() || s(t, "title").to_lowercase().contains(&q))
            .map(|(_, id, _)| id.clone())
            .collect()
    }

    fn index(&self) -> Option<usize> {
        let o = self.order();
        self.sel.as_ref().and_then(|id| o.iter().position(|x| x == id))
    }

    /// The list changed. Nothing selected yet (the first snapshot): the newest task, the top row.
    pub fn listed(&mut self, changed: &[String]) -> Vec<Effect> {
        let _ = changed;
        let gone = self.sel.as_ref().is_some_and(|id| !self.tasks.rows.contains_key(id));
        // a new task being written (or one that didn't start, back in the box) keeps the box: selecting would file
        // the draft under another task
        if (self.sel.is_none() && !self.new_task) || gone {
            let first = self.order().first().cloned();
            return self.select(first);
        }
        Vec::new()
    }

    fn select(&mut self, id: Option<String>) -> Vec<Effect> {
        if id == self.sel {
            return Vec::new();
        }
        let from = self.sel.clone().unwrap_or_default();
        self.composer.switch(&from, id.as_deref().unwrap_or_default());
        self.sel.clone_from(&id);
        self.open = id.as_deref().map(Transcript::new);
        self.scroll = 0;
        self.cursor = None;
        // the bar shows this task's mode, and a reply leaves it as it is until the user picks one
        self.mode_touched = false;
        self.mode = id.as_ref().and_then(|id| self.tasks.rows.get(id)).and_then(|t| Mode::of(s(t, "mode"))).unwrap_or(Mode::Auto);
        vec![Effect::Watch(id)]
    }

    /// The open task's row came (its topic): the bar follows its mode, unless the user picked one since.
    pub fn task_mode_came(&mut self) {
        if self.mode_touched {
            return;
        }
        if let Some(m) = self.open.as_ref().and_then(|t| Mode::of(s(&t.session(), "mode"))) {
            self.mode = m;
        }
    }

    /// The open task's mode cap (a channel's or a crew job's, F-chan-10): its row's `capped`, or its session's slot.
    pub fn cap(&self) -> Option<Mode> {
        let row = self.sel.as_ref().and_then(|id| self.tasks.rows.get(id)).and_then(|t| Mode::of(s(t, "capped")));
        row.or_else(|| {
            let session = self.open.as_ref()?.session();
            let c = session.pointer("/ext/wisp.runtime/capped").filter(|c| !c.is_null())?;
            // a cap with no readable mode is Edits, as runtime reads it
            Some(c.get("mode").and_then(Value::as_str).and_then(Mode::of).unwrap_or(Mode::Edits))
        })
    }

    /// The mode the bar shows: the New task page's, or the open task's.
    pub fn shown_mode(&self) -> Mode {
        if self.new_task { self.launch_mode } else { self.mode }
    }

    /// Shift-Tab: the next mode for the new task, or for this task's next reply (warned when it lifts a cap).
    fn cycle_mode(&mut self) {
        if self.new_task {
            self.launch_mode = self.launch_mode.next();
            return;
        }
        self.mode = self.mode.next();
        self.mode_touched = true;
        if let Some(c) = self.cap().filter(|c| self.mode.rank() > c.rank()) {
            self.toast(format!("Sending in {} lifts this task's {} cap", self.mode.wire(), c.wire()), false);
        }
    }

    fn moved(&mut self, by: isize) -> Vec<Effect> {
        let o = self.order();
        if o.is_empty() {
            return Vec::new();
        }
        let i = self.index().map_or(0, |i| i.saturating_add_signed(by).min(o.len() - 1));
        self.select(o.get(i).cloned())
    }

    pub fn running(&self) -> bool {
        self.open.as_ref().is_some_and(|t| t.running().is_some())
    }

    /// The tool steps of the open task, in order (what j/k move between).
    pub fn steps(&self) -> Vec<i64> {
        let Some(t) = &self.open else { return Vec::new() };
        t.turns().iter().flat_map(|turn| t.events(s(turn, "id"))).filter(|(_, b)| s(b, "kind") == "tool").map(|(seq, _)| seq).collect()
    }

    fn step(&mut self, by: isize) {
        let st = self.steps();
        if st.is_empty() {
            return;
        }
        let i = self
            .cursor
            .and_then(|c| st.iter().position(|x| *x == c))
            .map_or(st.len() - 1, |i| i.saturating_add_signed(by).min(st.len() - 1));
        self.cursor = Some(st[i]);
    }

    /// A key.
    pub fn key(&mut self, k: KeyEvent) -> Vec<Effect> {
        if ctrl(&k, 'c') {
            return vec![Effect::Quit];
        }
        if let Some(o) = self.overlay.take() {
            return self.overlay_key(o, k);
        }
        if ctrl(&k, 'k') || (k.code == KeyCode::Char(':') && self.focus != Zone::Composer) {
            self.palette();
            return Vec::new();
        }
        if self.goto && self.focus != Zone::Composer {
            self.goto = false;
            if let KeyCode::Char(c) = k.code {
                return self.go(c);
            }
        }
        if self.search.is_some() && matches!(self.focus, Zone::List | Zone::Task) {
            return self.search_key(k);
        }
        if k.code == KeyCode::BackTab {
            self.cycle_mode();
            return Vec::new();
        }
        if self.focus == Zone::Composer {
            return self.composer_key(k);
        }
        if k.code == KeyCode::Tab {
            self.focus = match self.focus {
                Zone::List => Zone::Task,
                Zone::Task if self.side.is_some() => Zone::Side,
                _ => Zone::List,
            };
            return Vec::new();
        }
        match k.code {
            KeyCode::Char('?') => {
                self.overlay = Some(Overlay::Help);
                return Vec::new();
            }
            KeyCode::Char('q') => return vec![Effect::Quit],
            KeyCode::Char('g') => {
                self.goto = true;
                return Vec::new();
            }
            KeyCode::Char('[') => {
                self.resize(-2);
                return Vec::new();
            }
            KeyCode::Char(']') => {
                self.resize(2);
                return Vec::new();
            }
            _ => {}
        }
        if let Some(v) = self.screen.clone() {
            return self.view_key(&v, k);
        }
        match self.focus {
            Zone::List => self.list_key(k),
            Zone::Task | Zone::Side => self.task_key(k),
            Zone::Composer => Vec::new(),
        }
    }

    /// `/` search: the list filters as you type, ↑ ↓ (Ctrl-P Ctrl-N) move between the matches and Enter opens the
    /// highlighted one (the first, when the highlighted row is filtered out); with no match, Enter does nothing. In a
    /// task it marks what matches.
    fn search_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        let by = match k.code {
            KeyCode::Down => 1,
            KeyCode::Up => -1,
            _ if ctrl(&k, 'n') => 1,
            _ if ctrl(&k, 'p') => -1,
            _ => 0,
        };
        if by != 0 && self.focus == Zone::List {
            let o = self.order();
            if o.is_empty() {
                return Vec::new();
            }
            // a highlighted row the query filtered out counts as the first match
            let i = self.index().unwrap_or(0).saturating_add_signed(by).min(o.len() - 1);
            return self.select(o.get(i).cloned());
        }
        match k.code {
            KeyCode::Enter if self.focus == Zone::List => {
                let o = self.order();
                let Some(pick) = self.index().map(|i| o[i].clone()).or_else(|| o.first().cloned()) else { return Vec::new() };
                self.search = None;
                self.focus = Zone::Task;
                return self.select(Some(pick));
            }
            KeyCode::Esc | KeyCode::Enter => self.search = None,
            KeyCode::Backspace => {
                if let Some(q) = self.search.as_mut() {
                    q.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(q) = self.search.as_mut() {
                    q.push(c);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn resize(&mut self, by: i16) {
        let w = if self.focus == Zone::Side { &mut self.widths.1 } else { &mut self.widths.0 };
        *w = w.saturating_add_signed(by).clamp(20, 90);
    }

    fn go(&mut self, c: char) -> Vec<Effect> {
        if c == 't' {
            self.screen = None;
            self.focus = Zone::List;
            return Vec::new();
        }
        let Some(v) = self.reg.views.iter().find(|v| v.goto() == Some(c)).map(|v| v.id().to_owned()) else { return Vec::new() };
        self.open_view(&v)
    }

    pub fn open_view(&mut self, id: &str) -> Vec<Effect> {
        self.screen = Some(id.to_owned());
        self.focus = Zone::Task;
        self.load(id)
    }

    fn load(&mut self, id: &str) -> Vec<Effect> {
        let Some(v) = self.reg.find(id) else { return Vec::new() };
        let r = self.views.entry(id.to_owned()).or_default();
        match v.source() {
            Source::Command { id: cmd, args, .. } => {
                // a source that asks (a file to blame): the answer first, then the load
                if let Some(label) = wisp_tui_api::asks(&args) {
                    let Some(answer) = r.asked.clone() else {
                        self.overlay = Some(Overlay::Ask(id.to_owned(), label, String::new()));
                        return Vec::new();
                    };
                    r.loading = true;
                    let args = wisp_tui_api::fill_asked(&args, &answer, &self.cwd);
                    return vec![Effect::Cmd { id: cmd, args, then: Then::View(id.to_owned()) }];
                }
                r.loading = true;
                let args = wisp_tui_api::fill_asked(&args, "", &self.cwd);
                vec![Effect::Cmd { id: cmd, args, then: Then::View(id.to_owned()) }]
            }
            // a topic view is fed by the loop's watch of it
            Source::Topic { .. } => Vec::new(),
        }
    }

    fn view_key(&mut self, id: &str, k: KeyEvent) -> Vec<Effect> {
        let Some(v) = self.reg.find(id) else { return Vec::new() };
        let r = self.views.entry(id.to_owned()).or_default();
        let n = r.rows.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
                self.screen = None;
                self.focus = Zone::List;
            }
            KeyCode::Down | KeyCode::Char('j') => r.sel = (r.sel + 1).min(n.saturating_sub(1)),
            KeyCode::Up | KeyCode::Char('k') => r.sel = r.sel.saturating_sub(1),
            KeyCode::Char('r') => return self.load(id),
            // a view that asked for something (a file to blame): ask again
            KeyCode::Char('A') if matches!(v.source(), Source::Command { ref args, .. } if wisp_tui_api::asks(args).is_some()) => {
                let label = match v.source() {
                    Source::Command { args, .. } => wisp_tui_api::asks(&args).unwrap_or_default(),
                    Source::Topic { .. } => String::new(),
                };
                let had = r.asked.clone().unwrap_or_default();
                self.overlay = Some(Overlay::Ask(id.to_owned(), label, had));
                return Vec::new();
            }
            KeyCode::Char(c) => {
                let Some(a) = v.actions().into_iter().find(|a| a.key == c) else { return Vec::new() };
                // an action that reads its row needs one; one that doesn't (a new member) works on an empty table
                let reads_row = a.args.to_string().contains("{/");
                let row = match r.rows.get(r.sel).cloned() {
                    Some(row) => row,
                    None if !reads_row => Value::Null,
                    None => return Vec::new(),
                };
                {
                    let args = wisp_tui_api::fill(&a.args, &row);
                    if let Some(label) = wisp_tui_api::asks(&args) {
                        self.overlay = Some(Overlay::AskAct(label, a.command, args, String::new()));
                        return Vec::new();
                    }
                    if a.confirm {
                        self.overlay = Some(Overlay::Confirm(
                            format!(
                                "{} {}?",
                                a.label,
                                wisp_tui_api::cell(&row, &v.columns().first().map(|c| c.pointer.clone()).unwrap_or_default())
                            ),
                            a.command,
                            args,
                        ));
                        return Vec::new();
                    }
                    return vec![Effect::Cmd { id: a.command, args, then: Then::View(id.to_owned()) }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn list_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.moved(1),
            KeyCode::Up | KeyCode::Char('k') => self.moved(-1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.focus = Zone::Task;
                Vec::new()
            }
            // i writes to the selected task; with none there is nothing to reply to, so it starts one, as n does
            KeyCode::Char('i') => {
                self.new_task = self.sel.is_none();
                self.focus = Zone::Composer;
                Vec::new()
            }
            KeyCode::Char('n') => {
                self.new_task = true;
                self.focus = Zone::Composer;
                Vec::new()
            }
            KeyCode::Char('/') => {
                self.search = Some(String::new());
                Vec::new()
            }
            KeyCode::Char('s') => self.ask_stop(),
            _ => Vec::new(),
        }
    }

    /// The selected task has a turn running: its transcript says so, or (not loaded yet) its row.
    fn sel_running(&self) -> bool {
        let Some(id) = &self.sel else { return false };
        let open = self.open.as_ref().filter(|t| t.task == *id);
        open.is_some_and(|t| t.running().is_some()) || self.tasks.rows.get(id).is_some_and(|t| s(t, "state") == "running")
    }

    /// `s` or `/stop`: a stop ends the whole run, so it asks first, and only a running task has one to end. Esc
    /// stopped it too, and Esc is how you go back: one key pressed to leave a pane killed the task.
    fn ask_stop(&mut self) -> Vec<Effect> {
        let Some(id) = self.sel.clone().filter(|id| !self.stopping.contains(id)) else { return Vec::new() };
        if !self.sel_running() {
            return Vec::new();
        }
        let title =
            self.tasks.rows.get(&id).map(|t| s(t, "title").to_owned()).filter(|t| !t.is_empty()).unwrap_or_else(|| "this task".into());
        self.overlay = Some(Overlay::Confirm(format!("Stop {title}?"), "runtime.stop".into(), json!({ "id": id })));
        Vec::new()
    }

    /// The stop, confirmed: one on its way per task.
    fn stop(&mut self, id: String) -> Vec<Effect> {
        if self.stopping.contains(&id) {
            return Vec::new();
        }
        self.stopping.push(id.clone());
        vec![Effect::Cmd { id: "runtime.stop".into(), args: json!({ "id": id }), then: Then::Stopped(id) }]
    }

    /// The answer to the open ask, naming it (`askId`): a setup card is answered only that way, and an answer to a
    /// question since answered or replaced is refused instead of landing on the next one.
    fn answer(&mut self, text: &str) -> Vec<Effect> {
        let Some(ask) = self.open.as_ref().and_then(Transcript::ask) else { return Vec::new() };
        let id = self.sel.clone().unwrap_or_default();
        vec![Effect::Cmd { id: "runtime.answer".into(), args: json!({ "id": id, "text": text, "askId": ask.id }), then: Then::Answered }]
    }

    /// y a n on an approval: its own Allow / Always / Deny. On a question they're no answer (they fall through).
    fn approve(&mut self, word: &str) -> Vec<Effect> {
        let pick = self
            .open
            .as_ref()
            .and_then(Transcript::ask)
            .filter(super::model::Ask::is_approval)
            .and_then(|a| a.option(word).map(str::to_owned));
        pick.map_or_else(Vec::new, |o| self.answer(&o))
    }

    fn task_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        let ask = self.open.as_ref().and_then(Transcript::ask);
        let approval = ask.as_ref().is_some_and(super::model::Ask::is_approval);
        match k.code {
            KeyCode::Char(c @ '1'..='9') if ask.is_some() => {
                let opts = ask.map(|a| a.options).unwrap_or_default();
                let i = usize::from(u8::try_from(c).unwrap_or(b'1') - b'1');
                opts.get(i).cloned().map_or_else(Vec::new, |o| self.answer(&o))
            }
            KeyCode::Char('y') if approval => self.approve("allow"),
            KeyCode::Char('a') if approval => self.approve("always"),
            KeyCode::Char('n') if approval => self.approve("deny"),
            KeyCode::Char('i') | KeyCode::Enter => {
                if self.sel.is_some() {
                    self.new_task = false;
                    self.focus = Zone::Composer;
                }
                Vec::new()
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.step(1);
                Vec::new()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.step(-1);
                Vec::new()
            }
            // o: Open in <harness> (v1's Take over): the terminal goes to `wisp open <task>` until it's done
            KeyCode::Char('o') => {
                self.sel.clone().filter(|id| !id.starts_with("local-")).map_or_else(Vec::new, |id| vec![Effect::OpenIn(id)])
            }
            KeyCode::Char(' ') => {
                if let (Some(c), Some(t)) = (self.cursor, self.open.as_mut()) {
                    t.toggle(c);
                }
                Vec::new()
            }
            KeyCode::Char('d') => {
                self.side = match self.side {
                    Some(Side::Changes) => None,
                    _ => Some(Side::Changes),
                };
                Vec::new()
            }
            // t: the Thread pane (its Notes and health), as the GUI's Thread tab; again: close it
            KeyCode::Char('t') => {
                self.side = match self.side {
                    Some(Side::Thread) => None,
                    _ => Some(Side::Thread),
                };
                Vec::new()
            }
            KeyCode::Char('p') if self.side.is_some() => {
                self.side = self.side.map(Side::next);
                Vec::new()
            }
            KeyCode::Char('s') => self.ask_stop(),
            // Esc only ever goes back: stopping is `s`, and it asks
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
                self.focus = Zone::List;
                Vec::new()
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_add(10);
                Vec::new()
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_sub(10);
                Vec::new()
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.scroll = 0;
                Vec::new()
            }
            KeyCode::Char('/') => {
                self.search = Some(String::new());
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn composer_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        let newline = k.code == KeyCode::Enter && (alt(&k) || k.modifiers.contains(KeyModifiers::SHIFT)) || ctrl(&k, 'j');
        if newline {
            self.composer.newline();
            return Vec::new();
        }
        if ctrl(&k, 'a') && self.new_task {
            self.launch_agent = next_agent(&self.agents, &self.launch_agent);
            return Vec::new();
        }
        if ctrl(&k, 'e') {
            // the editor gets every collapsed paste put back: a `[pasted 40 lines]` token saved back lost the paste
            return vec![Effect::Editor(self.composer.expanded())];
        }
        if ctrl(&k, 's') {
            return self.send_now();
        }
        if alt(&k) && matches!(k.code, KeyCode::Up | KeyCode::Down) {
            return self.reorder(if k.code == KeyCode::Up { -1 } else { 1 });
        }
        match k.code {
            KeyCode::Enter => return self.send(),
            // Esc leaves the composer, a turn running or not: stopping is `s` (or /stop), and it asks
            KeyCode::Esc => {
                self.focus = if self.new_task { Zone::List } else { Zone::Task };
                self.new_task = false;
            }
            KeyCode::Tab => self.focus = Zone::List,
            KeyCode::Backspace => self.composer.backspace(),
            KeyCode::Left => self.composer.left(),
            KeyCode::Right => self.composer.right(),
            KeyCode::Up => self.composer.up(),
            KeyCode::Down => self.composer.down(),
            KeyCode::Char(c) => match self.composer.char(c) {
                Edit::At => return self.picker(For::At),
                Edit::Slash => return self.picker(For::Slash),
                Edit::Typed => {}
            },
            _ => {}
        }
        Vec::new()
    }

    /// The queued follow-ups as the session has them (whole items, for `runtime.queue.edit`).
    fn queue_items(&self) -> Vec<Value> {
        self.open.as_ref().and_then(|t| t.session().get("queue").and_then(Value::as_array).cloned()).unwrap_or_default()
    }

    fn reorder(&mut self, by: isize) -> Vec<Effect> {
        let mut items = self.queue_items();
        if items.len() < 2 {
            return Vec::new();
        }
        // the last one moves (the one just queued), the way the GUI's drag does it
        let from = items.len() - 1;
        let to = from.saturating_add_signed(by).min(items.len() - 1);
        let it = items.remove(from);
        items.insert(to, it);
        let id = self.sel.clone().unwrap_or_default();
        vec![Effect::Cmd { id: "runtime.queue.edit".into(), args: json!({ "id": id, "items": items }), then: Then::Nothing }]
    }

    fn send_now(&mut self) -> Vec<Effect> {
        let Some(first) = self.queue_items().into_iter().next() else { return Vec::new() };
        let id = self.sel.clone().unwrap_or_default();
        vec![Effect::Cmd { id: "runtime.queue.sendNow".into(), args: json!({ "id": id, "itemId": first["id"] }), then: Then::Nothing }]
    }

    /// Enter in the composer: a new task, a follow-up (queued while a turn runs), or a `/` command.
    fn send(&mut self) -> Vec<Effect> {
        let attachments = self.composer.attachments.clone();
        let text = self.composer.take();
        if text.is_empty() {
            return Vec::new();
        }
        let task = if self.new_task { None } else { self.sel.clone() };
        if let Some(rest) = text.strip_prefix('/')
            && let Some(fx) = self.slash(rest, task.as_deref())
        {
            return fx;
        }
        let local = self.local();
        if task.is_none() {
            // the new task shows at once, at the top, selected; the real one replaces it
            self.tasks.rows.insert(
                local.clone(),
                json!({ "id": local, "title": text.lines().next().unwrap_or_default(), "state": "starting", "mode": self.launch_mode.wire(), "updated_at": i64::MAX }),
            );
            let fx = self.select(Some(local.clone()));
            drop(fx);
            self.new_task = false;
            self.focus = Zone::Task;
        } else {
            self.sending.push((local.clone(), text.clone()));
        }
        let queue = self.running();
        let agent = if task.is_none() && !self.launch_agent.is_empty() { Some(self.launch_agent.clone()) } else { None };
        let mode = if task.is_none() { Some(self.launch_mode) } else { self.mode_touched.then_some(self.mode) };
        vec![Effect::Send { task, text, attachments, mode, queue, local, agent }]
    }

    /// `/btw …`, `/fork`, `/stop`, or `/<catalogue command> [json args]` (the open task's id by default).
    fn slash(&mut self, rest: &str, task: Option<&str>) -> Option<Vec<Effect>> {
        let (word, args) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(w, a)| (w, a.trim()));
        let id = task.unwrap_or_default();
        let fx = match word {
            // runtime.btw reads `question` (BtwArgs): `text` was refused every time. It needs a task to ask on
            "btw" => {
                if task.is_none() {
                    self.toast("Open a task to ask it on the side", true);
                    return Some(Vec::new());
                }
                if args.is_empty() {
                    self.toast("/btw needs a question", true);
                    return Some(Vec::new());
                }
                Effect::Cmd { id: "runtime.btw".into(), args: json!({ "id": id, "question": args }), then: Then::Btw(id.to_owned()) }
            }
            "fork" => Effect::Cmd { id: "runtime.fork".into(), args: json!({ "id": id }), then: Then::Nothing },
            "stop" => return Some(self.ask_stop()),
            "help" => {
                self.overlay = Some(Overlay::Help);
                return Some(Vec::new());
            }
            _ if super::commands::parse(&self.cat, &format!("/{rest}"), task).is_some() => {
                match super::commands::parse(&self.cat, &format!("/{rest}"), task) {
                    Some(Ok((cmd, a))) => Effect::Cmd { id: cmd, args: a, then: Then::Nothing },
                    Some(Err(why)) => {
                        // the line stays in the box to fix
                        self.toast(why, true);
                        return Some(Vec::new());
                    }
                    None => return None,
                }
            }
            w if self.catalogue.iter().any(|(c, _)| c == w) => {
                let a = serde_json::from_str::<Value>(args)
                    .unwrap_or_else(|_| if args.is_empty() { json!({ "id": id }) } else { json!({ "id": id, "text": args }) });
                Effect::Cmd { id: w.to_owned(), args: a, then: Then::Nothing }
            }
            _ => return None,
        };
        Some(vec![fx])
    }

    fn palette(&mut self) {
        let mut items: Vec<Item> = self
            .order()
            .into_iter()
            .map(|id| Item { label: s(&self.tasks.rows[&id], "title").to_owned(), kind: "task".into(), pick: Pick::OpenTask(id) })
            .collect();
        items.extend(self.reg.views.iter().map(|v| Item {
            label: v.title().to_owned(),
            kind: "view".into(),
            pick: Pick::View(v.id().to_owned()),
        }));
        items.extend(super::commands::items(&self.cat, false));
        if let Some(rows) = self.views.get("settings") {
            items.extend(rows.rows.iter().map(|r| Item {
                label: s(r, "key").to_owned(),
                kind: "setting".into(),
                pick: Pick::Setting(s(r, "key").to_owned()),
            }));
        }
        self.overlay = Some(Overlay::Palette(Palette::new("Go to", items), For::Everything));
    }

    fn picker(&mut self, f: For) -> Vec<Effect> {
        let items: Vec<Item> = if f == For::Slash {
            ["btw", "fork", "stop", "help"]
                .iter()
                .map(|c| Item { label: (*c).to_owned(), kind: "wisp".into(), pick: Pick::Insert(format!("/{c} ")) })
                .chain(super::commands::items(&self.cat, true))
                .collect()
        } else {
            let mut v: Vec<Item> = self
                .order()
                .into_iter()
                .map(|id| Item {
                    label: s(&self.tasks.rows[&id], "title").to_owned(),
                    kind: "task".into(),
                    pick: Pick::Insert(format!("@[{}](task:{id}) ", s(&self.tasks.rows[&id], "title"))),
                })
                .collect();
            v.extend(self.mentions.values().flatten().cloned());
            v
        };
        self.overlay = Some(Overlay::Palette(Palette::new(if f == For::Slash { "Command" } else { "Mention" }, items), f));
        let mut fx = Vec::new();
        if f == For::Slash && self.catalogue.is_empty() {
            fx.push(Effect::Cmd { id: "cli.catalogue".into(), args: json!({}), then: Then::Catalogue });
        }
        if f == For::At && self.mentions.is_empty() {
            for (cmd, kind) in [("crew.list", "crew"), ("machines.list", "machine"), ("stuff.list", "stuff")] {
                fx.push(Effect::Cmd { id: cmd.into(), args: json!({}), then: Then::Mentions(kind.into()) });
            }
            fx.push(Effect::Files);
        }
        fx
    }

    /// A row action asking for its value (TRY-7): Enter runs it with what's typed, Esc drops it.
    fn ask_act_key(&mut self, label: String, cmd: String, args: Value, mut text: String, k: KeyEvent) -> Vec<Effect> {
        match k.code {
            KeyCode::Esc => return Vec::new(),
            KeyCode::Enter if !text.trim().is_empty() => {
                let args = wisp_tui_api::fill_asked(&args, text.trim(), &self.cwd);
                let then = self.screen.clone().map_or(Then::Nothing, Then::View);
                return vec![Effect::Cmd { id: cmd, args, then }];
            }
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Char(c) => text.push(c),
            _ => {}
        }
        self.overlay = Some(Overlay::AskAct(label, cmd, args, text));
        Vec::new()
    }

    fn overlay_key(&mut self, o: Overlay, k: KeyEvent) -> Vec<Effect> {
        match o {
            // j k scroll a help taller than the screen; any other key closes it
            Overlay::Help => {
                match k.code {
                    KeyCode::Char('j') | KeyCode::Down => self.help_scroll = self.help_scroll.saturating_add(1),
                    KeyCode::Char('k') | KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                    _ => {
                        self.help_scroll = 0;
                        return Vec::new();
                    }
                }
                self.overlay = Some(Overlay::Help);
                Vec::new()
            }
            Overlay::Ask(view, label, mut text) => {
                match k.code {
                    KeyCode::Esc => {
                        // nothing asked yet: the view has nothing to show, so it closes too
                        if self.views.get(&view).is_none_or(|r| r.asked.is_none()) {
                            self.screen = None;
                            self.focus = Zone::List;
                        }
                        return Vec::new();
                    }
                    KeyCode::Enter if !text.trim().is_empty() => {
                        self.views.entry(view.clone()).or_default().asked = Some(text.trim().to_owned());
                        return self.load(&view);
                    }
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    KeyCode::Char(c) => text.push(c),
                    _ => {}
                }
                self.overlay = Some(Overlay::Ask(view, label, text));
                Vec::new()
            }
            Overlay::AskAct(label, cmd, args, text) => self.ask_act_key(label, cmd, args, text, k),
            Overlay::Confirm(_, cmd, args) => {
                if matches!(k.code, KeyCode::Char('y') | KeyCode::Enter) {
                    // a stop asked for with `s` or /stop: one on its way per task, shown pending on its row
                    if cmd == "runtime.stop" {
                        return self.stop(s(&args, "id").to_owned());
                    }
                    let then = self.screen.clone().map_or(Then::Nothing, Then::View);
                    return vec![Effect::Cmd { id: cmd, args, then }];
                }
                Vec::new()
            }
            Overlay::Palette(mut p, f) => {
                let lead = if f == For::Slash { '/' } else { '@' };
                match k.code {
                    KeyCode::Esc => return Vec::new(),
                    KeyCode::Enter | KeyCode::Tab => {
                        if let Some(it) = p.chosen() {
                            return self.picked(it, f, lead);
                        }
                        return Vec::new();
                    }
                    KeyCode::Down => p.sel = (p.sel + 1).min(p.shown().len().saturating_sub(1)),
                    KeyCode::Up => p.sel = p.sel.saturating_sub(1),
                    KeyCode::Backspace => {
                        if f == For::Everything {
                            p.query.pop();
                        } else {
                            if p.query.is_empty() {
                                self.composer.backspace();
                                return Vec::new();
                            }
                            self.composer.backspace();
                            p.query = self.composer.word(lead);
                        }
                        p.sel = 0;
                    }
                    KeyCode::Char(c) => {
                        if f == For::Everything {
                            p.query.push(c);
                        } else if c == ' ' && f == For::At {
                            self.composer.insert(" ");
                            return Vec::new();
                        } else {
                            self.composer.insert(&c.to_string());
                            p.query = self.composer.word(lead);
                        }
                        p.sel = 0;
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Palette(p, f));
                Vec::new()
            }
        }
    }

    fn picked(&mut self, it: Item, f: For, lead: char) -> Vec<Effect> {
        match it.pick {
            Pick::OpenTask(id) => {
                self.screen = None;
                self.focus = Zone::Task;
                self.select(Some(id))
            }
            Pick::View(v) => self.open_view(&v),
            Pick::Setting(_) => self.open_view("settings"),
            // the command goes before what was written, which stays (set() emptied the draft, its pastes and images)
            Pick::Command(id) => {
                self.focus = Zone::Composer;
                self.composer.prefix(&format!("/{id} "));
                Vec::new()
            }
            Pick::Insert(text) => {
                if f == For::Everything {
                    self.composer.insert(&text);
                } else {
                    self.composer.replace_word(lead, &text);
                }
                self.focus = Zone::Composer;
                Vec::new()
            }
        }
    }

    /// A paste (bracketed): into the composer, which collapses a long one.
    pub fn paste(&mut self, text: &str) {
        if self.focus != Zone::Composer {
            self.new_task = self.sel.is_none();
            self.focus = Zone::Composer;
        }
        self.composer.paste(text);
    }

    /// `$EDITOR` came back: what it saved replaces the box, or (no editor, a non-zero exit) the box stays as it was
    /// and a toast says why.
    pub fn edited(&mut self, got: std::result::Result<String, String>) {
        match got {
            Ok(text) => self.composer.set(text.trim_end_matches('\n')),
            Err(why) => self.toast(format!("Editor didn't save ({why})"), true),
        }
    }

    /// Open in <harness> came back: a failure says so (the TUI took the screen back over whatever the child printed).
    pub fn opened_in(&mut self, id: &str, got: std::result::Result<(), String>) {
        if let Err(why) = got {
            let agent = self.tasks.rows.get(id).map(|t| s(t, "agent").to_owned()).filter(|a| !a.is_empty());
            self.toast(format!("Couldn't open in {} ({why})", agent.as_deref().unwrap_or("its harness")), true);
        }
    }

    /// The mouse: a click focuses a zone (and picks a row, opens a step), the wheel scrolls, a drag on a border
    /// resizes.
    pub fn mouse(&mut self, m: MouseEvent) -> Vec<Effect> {
        let h = self.hit.clone();
        let inside = |r: ratatui::layout::Rect| m.column >= r.x && m.column < r.x + r.width && m.row >= r.y && m.row < r.y + r.height;
        match m.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                match self.dragging {
                    Some(Zone::List) => self.widths.0 = m.column.saturating_sub(h.list.x).clamp(20, 90),
                    Some(Zone::Side) => self.widths.1 = (h.side.x + h.side.width).saturating_sub(m.column).clamp(20, 90),
                    _ => {}
                }
                Vec::new()
            }
            MouseEventKind::Up(_) => {
                self.dragging = None;
                Vec::new()
            }
            // a press on a border starts a resize
            MouseEventKind::Down(MouseButton::Left) if h.list.width > 0 && m.column.abs_diff(h.list.x + h.list.width) <= 1 => {
                self.dragging = Some(Zone::List);
                Vec::new()
            }
            MouseEventKind::Down(MouseButton::Left) if h.side.width > 0 && m.column.abs_diff(h.side.x.saturating_sub(1)) <= 1 => {
                self.dragging = Some(Zone::Side);
                Vec::new()
            }
            MouseEventKind::ScrollUp if inside(h.task) => {
                self.scroll = self.scroll.saturating_add(3);
                Vec::new()
            }
            MouseEventKind::ScrollDown if inside(h.task) => {
                self.scroll = self.scroll.saturating_sub(3);
                Vec::new()
            }
            MouseEventKind::ScrollDown if inside(h.list) => self.moved(1),
            MouseEventKind::ScrollUp if inside(h.list) => self.moved(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                if inside(h.composer) {
                    self.focus = Zone::Composer;
                } else if inside(h.list) {
                    self.focus = Zone::List;
                    if let Some(id) = h.rows.get(&m.row).cloned() {
                        return self.select(Some(id));
                    }
                } else if inside(h.side) {
                    self.focus = Zone::Side;
                } else if inside(h.task) {
                    self.focus = Zone::Task;
                    if let Some(seq) = h.tools.get(&m.row).copied()
                        && let Some(t) = self.open.as_mut()
                    {
                        self.cursor = Some(seq);
                        t.toggle(seq);
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A command answered.
    /// `runtime.agents`: the installed harnesses New task offers.
    fn agents_came(&mut self, v: &Value) {
        self.agents = v
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a.get("available") == Some(&Value::Bool(true)))
            .map(|a| (s(a, "id").to_owned(), s(a, "name").to_owned()))
            .collect();
    }

    /// `cli.catalogue`: the commands, for the `/` menu and the palette (one opened before it came gets them now).
    fn catalogue_came(&mut self, v: Value) {
        self.catalogue = v.as_array().into_iter().flatten().map(|c| (s(c, "id").to_owned(), s(c, "title").to_owned())).collect();
        let list: Vec<wisp_kernel::CommandInfo> = serde_json::from_value(v).unwrap_or_default();
        self.cat = wisp_cli_core::verbs::Catalogue::new(list);
        match &mut self.overlay {
            Some(Overlay::Palette(p, For::Slash)) => p.items.extend(super::commands::items(&self.cat, true)),
            Some(Overlay::Palette(p, For::Everything)) => p.items.extend(super::commands::items(&self.cat, false)),
            _ => {}
        }
    }

    /// A new task that didn't start: the prompt comes back to New task, nothing lost (jumping to another task hid it
    /// in that task's draft). A box already holding another draft is left alone, and the toast says so.
    fn not_started(&mut self, local: &str, text: &str, attachments: Vec<String>, e: &str) -> Vec<Effect> {
        let here = self.sel.as_deref() == Some(local);
        if here {
            self.sel = None;
            self.open = None;
            self.mode_touched = false;
        }
        let back = self.composer.is_empty();
        if back {
            self.composer.set(text);
            self.composer.attachments = attachments;
        }
        if here || back {
            self.new_task = true;
            self.screen = None;
            self.focus = Zone::Composer;
        }
        let said = match unreachable(e) {
            Some(plain) if back => plain,
            Some(_) => format!("Didn't start \"{}\": wisp isn't answering", text.lines().next().unwrap_or_default()),
            None => format!("Didn't start: {e}"),
        };
        self.toast(said, true);
        if here { vec![Effect::Watch(None)] } else { Vec::new() }
    }

    /// `runtime.btw` answered with its entry (pending: the answer lands on the session later): into the transcript,
    /// as a side entry. No entry in the answer: what it said, as a toast.
    fn btw_came(&mut self, task: &str, got: std::result::Result<Value, String>) {
        match got {
            Ok(v) if v.get("question").is_some() => match self.open.as_mut().filter(|t| t.task == task) {
                Some(t) => {
                    t.asked.retain(|x| s(x, "id") != s(&v, "id"));
                    t.asked.push(v);
                    self.scroll = 0;
                }
                None => self.toast(format!("Asked on the side: {}", s(&v, "question")), false),
            },
            Ok(v) => {
                let said = v.as_str().map_or_else(|| s(&v, "answer").to_owned(), str::to_owned);
                if !said.is_empty() {
                    self.toast(said, false);
                }
            }
            Err(e) => self.toast(format!("/btw: {e}"), true),
        }
    }

    pub fn done(&mut self, then: Then, got: std::result::Result<Value, String>) -> Vec<Effect> {
        match (then, got) {
            // no agents list: New task offers Auto only
            (Then::Nothing | Then::Answered, Ok(_)) | (Then::Agents, Err(_)) => {}
            // an answer refused (the ask changed: the run loop put it in plain words) is said, never sent again
            (Then::Nothing | Then::Catalogue | Then::Mentions(_) | Then::Answered, Err(e)) => self.toast(e, true),
            (Then::Btw(task), got) => self.btw_came(&task, got),
            (Then::View(v), got) => {
                let rows = self.views.entry(v.clone()).or_default();
                rows.loading = false;
                match got {
                    Ok(val) => {
                        let ptr = match self.reg.find(&v).map(|x| x.source()) {
                            Some(Source::Command { rows, .. }) => rows,
                            _ => String::new(),
                        };
                        let list = if ptr.is_empty() { &val } else { val.pointer(&ptr).unwrap_or(&Value::Null) };
                        // a row action's answer isn't the list: read the list again
                        if let Some(a) = list.as_array() {
                            rows.rows = self.reg.find(&v).map_or_else(|| a.clone(), |x| x.shape(a.clone()));
                            rows.sel = rows.sel.min(rows.rows.len().saturating_sub(1));
                            rows.error.clear();
                        } else {
                            return self.load(&v);
                        }
                    }
                    Err(e) => {
                        rows.error.clone_from(&e);
                        self.toast(e, true);
                    }
                }
            }
            (Then::Agents, Ok(v)) => self.agents_came(&v),
            (Then::Catalogue, Ok(v)) => self.catalogue_came(v),
            (Then::Mentions(kind), Ok(v)) => {
                let items: Vec<Item> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        if let Some(path) = r.as_str() {
                            return Item { label: path.to_owned(), kind: kind.clone(), pick: Pick::Insert(format!("@{path} ")) };
                        }
                        let name =
                            [s(r, "name"), s(r, "title"), s(r, "id")].into_iter().find(|x| !x.is_empty()).unwrap_or_default().to_owned();
                        Item { label: name.clone(), kind: kind.clone(), pick: Pick::Insert(format!("@[{name}]({kind}:{}) ", s(r, "id"))) }
                    })
                    .collect();
                if let Some(Overlay::Palette(p, For::At)) = &mut self.overlay {
                    p.items.extend(items.iter().cloned());
                }
                self.mentions.insert(kind, items);
            }
            (Then::Created(local, text, attachments), got) => {
                self.tasks.rows.remove(&local);
                match got {
                    Ok(v) => {
                        let id = s(&v, "id").to_owned();
                        if !self.tasks.rows.contains_key(&id) {
                            self.tasks.rows.insert(
                                id.clone(),
                                json!({ "id": id, "title": s(&v, "title"), "state": "starting", "updated_at": i64::MAX - 1 }),
                            );
                        }
                        self.sel = None;
                        return self.select(Some(id));
                    }
                    Err(e) => return self.not_started(&local, &text, attachments, &e),
                }
            }
            (Then::Sent(local), got) => {
                let Some(i) = self.sending.iter().position(|(l, _)| *l == local) else { return Vec::new() };
                let (_, text) = self.sending.remove(i);
                if let Err(e) = got {
                    // refused: what was written comes back, nothing lost
                    let back = self.composer.is_empty();
                    if back {
                        self.composer.set(&text);
                    }
                    let said = unreachable(&e).filter(|_| back).unwrap_or_else(|| format!("Not sent: {e}"));
                    self.toast(said, true);
                }
            }
            (Then::Stopped(id), got) => {
                self.stopping.retain(|x| *x != id);
                if let Err(e) = got {
                    self.toast(format!("Didn't stop: {e}"), true);
                }
            }
        }
        Vec::new()
    }
}
