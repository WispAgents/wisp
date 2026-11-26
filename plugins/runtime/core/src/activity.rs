//! A harness's events as a turn's transcript and head (v1 runner.js `#onEvent`, `stepOf`, `stepOutput`,
//! `clipOutput`). Each event becomes at most one appended step plus an update of the turn's head (resume
//! id, changes, blocked actions, background tasks), applied in the same transaction (plan §4.1: the log is
//! append-only, the head is its projection). O(1) per event except a change to a file the turn already
//! touched (linear in the turn's changed files, as v1).

use serde_json::Value;
use wisp_runtime_api::harness::{HarnessEvent, HarnessStats};
use wisp_runtime_api::model::{Activity, ChangeOp, Edit, FileChange, FullOutput, Mode, ToolKind};

/// How long a turn waits on a background shell (a dev server, a watcher) with nothing new from its CLI before it ends
/// with the answer it already gave (`Head::interim`): an answer that said "I'll report once it's done" over a task that
/// never ends held its turn, and the follow-ups queued behind it, for good. Never while its subagents work (their
/// reports come back into the run).
pub const BACKGROUND_QUIET_MS: i64 = 30 * 60_000;

/// An agent message is kept whole up to this (UTF-16 units, as v1 counted).
pub const MAX_TEXT: usize = 20_000;
/// A tool call's output: its start and end past this.
pub const MAX_OUTPUT: usize = 2_000;

/// The calls whose output the transcript shows: a read returns the file and an edit a confirmation.
pub fn shows_output(kind: ToolKind) -> bool {
    matches!(kind, ToolKind::Run | ToolKind::Search | ToolKind::Web | ToolKind::Tool | ToolKind::Agent)
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The first `n` UTF-16 units (never splitting a character).
fn head16(s: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &s[..i];
        }
    }
    s
}

/// The last `n` UTF-16 units.
fn tail16(s: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices().rev() {
        units += c.len_utf16();
        if units > n {
            return &s[i + c.len_utf8()..];
        }
    }
    s
}

/// v1 `clipOutput`: whole when short, else its first and last lines (the end of a test run or a build
/// is what tells how it went).
pub fn clip_output(text: &str) -> String {
    let out = text.trim_end();
    if utf16_len(out) <= MAX_OUTPUT {
        return out.to_owned();
    }
    let mut head = head16(out, 600);
    let mut tail = tail16(out, 1300);
    if let Some(i) = head.rfind('\n') {
        head = &head[..i];
    }
    if let Some(i) = tail.find('\n') {
        tail = &tail[i + 1..];
    }
    let cut = &out[head.len()..out.len() - tail.len()];
    let lines = cut.matches('\n').count();
    let what = if lines > 1 { format!("{lines} lines") } else { format!("{} characters", utf16_len(cut)) };
    format!("{head}\n… {what} cut …\n{tail}")
}

/// The whole of an output [`clip_output`] cuts, as a step names it (audit F8: what the clip cut was gone for good, a
/// build's middle with it): its hash as the store's blobs name it (BLAKE3), lines and bytes. None when nothing is cut,
/// so a short output writes nothing more.
#[must_use]
pub fn full_of(text: &str) -> Option<FullOutput> {
    let out = text.trim_end();
    (utf16_len(out) > MAX_OUTPUT).then(|| FullOutput {
        blob: format!("blake3:{}", blake3::hash(out.as_bytes()).to_hex()),
        lines: u32::try_from(out.lines().count()).unwrap_or(u32::MAX),
        bytes: out.len() as u64,
    })
}

/// A blocked action the turn's permissions refused (the card after the turn offers to retry it).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlockedAction {
    pub tool: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// per field (`detail`, `rule`), the byte ranges that quote a .env value the task read (ADR-0050): they leave the
    /// server as `••••` (the turn's `blocked` list, the retry prompt). Additive (2026-11-14). Always written, `{}` when
    /// nothing matched: an item with no `hide` was stored before ranges existed, and is masked against the task's
    /// known values instead (`env_mask::mask_blocked_known`).
    #[serde(default)]
    pub hide: std::collections::BTreeMap<String, Vec<[u32; 2]>>,
}

impl BlockedAction {
    /// A refused call as the turn machine keeps it, with where its command line and rule quote one of `values`.
    #[must_use]
    pub fn flagged(tool: String, detail: String, rule: Option<String>, values: &EnvValues) -> Self {
        let mut hide = std::collections::BTreeMap::new();
        for (k, t) in [("detail", Some(detail.as_str())), ("rule", rule.as_deref())] {
            let spans = t.map(|t| wisp_envmask::value_spans(t, &values.0)).unwrap_or_default();
            if !spans.is_empty() {
                hide.insert(k.to_owned(), spans);
            }
        }
        Self { tool, detail, rule, hide }
    }
}

/// What of a turn's refusals wisp can still allow (v1 #777): a run in Full auto was already allowed everything wisp
/// can allow, so what its harness still refused was its own safety check (Claude Code's on a heredoc it can't parse),
/// which no rule lifts. Nothing to retry then; the card still says what was refused.
#[must_use]
pub fn allowable(mode: Mode, blocked: &[BlockedAction]) -> &[BlockedAction] {
    if mode == Mode::Auto { &[] } else { blocked }
}

/// What a running turn knows besides its log: the projection its events update.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Head {
    /// the harness conversation to resume
    pub resume_id: Option<String>,
    /// where the conversation stood at the turn's last word (`HarnessEvent::Session`'s `at`): a fork from this turn
    /// resumes there
    pub resume_at: Option<String>,
    /// the last agent message
    pub text: Option<String>,
    pub changes: Vec<FileChange>,
    pub blocked: Vec<BlockedAction>,
    /// background tasks the CLI says are still running
    pub background: Vec<String>,
    /// a result that left background tasks running: the best we have if nothing follows
    pub interim: Option<(String, Option<HarnessStats>)>,
    /// that result's run still has subagents at work (their reports come back into it), not only a background shell
    pub interim_agents: bool,
    pub tools: Option<Vec<Value>>,
    pub cli: Option<Value>,
    /// the calls that read a .env file: their `tool_output` is flagged `env` (ADR-0050)
    pub env_calls: Vec<String>,
    /// the values of the .env files this task's calls printed (seeded with the earlier turns', `Head::seeded`), so
    /// its thinking, tool errors, tool calls and outputs that quote one are flagged where (`hide`). In memory only:
    /// never written, never logged.
    pub env_values: EnvValues,
    /// whole outputs the steps just made name (`full`), for the host to keep in the store's blobs: taken after each
    /// event ([`Head::take_full`]). Never one that read or quotes a .env value (ADR-0050).
    pub full: Vec<String>,
    /// its host keeps whole outputs (a turn's run): a side run's steps are kept nowhere, so they name none
    pub keep_full: bool,
}

impl Head {
    /// The whole outputs to keep since the last call.
    pub fn take_full(&mut self) -> Vec<String> {
        std::mem::take(&mut self.full)
    }

    /// The turn waits only on a background shell it left up (a dev server), not on its subagents: after
    /// [`BACKGROUND_QUIET_MS`] with nothing new from its CLI it ends with the answer it gave.
    #[must_use]
    pub fn waits_on_shell(&self) -> bool {
        self.interim.is_some() && !self.interim_agents
    }

    /// A turn's head that already knows the .env values its task's earlier turns read (ADR-0050): a value read in
    /// one turn and quoted in a later one is flagged too.
    #[must_use]
    pub fn seeded(env_values: EnvValues) -> Self {
        Self { env_values, ..Self::default() }
    }
}

/// .env values a turn saw. Its `Debug` says how many, never what.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EnvValues(pub Vec<String>);

impl std::fmt::Debug for EnvValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EnvValues({} hidden)", self.0.len())
    }
}

impl EnvValues {
    /// The values a task's stored steps show it read (`env` outputs, and outputs of calls that named a .env file,
    /// flagged or not): what a turn is seeded with when its task's values aren't in memory (after a restart).
    /// Stored outputs are clipped (`MAX_OUTPUT`): what the clip cut can't be told from them.
    #[must_use]
    pub fn of_steps<'a>(steps: impl IntoIterator<Item = &'a Activity> + Clone) -> Self {
        let mut calls: Vec<&str> = Vec::new();
        let mut out = Self::default();
        for a in steps.clone() {
            if let Activity::Tool { file, detail, label, call_id, output, .. } = a
                && crate::env_mask::call_reads_env(file.as_deref(), detail.as_deref(), Some(label))
            {
                calls.extend(call_id.as_deref());
                out.add(output.as_deref().unwrap_or_default());
            }
        }
        for a in steps {
            if let Activity::ToolOutput { call_id, text, env, .. } = a
                && (*env || calls.contains(&call_id.as_str()))
            {
                out.add(text);
            }
        }
        out
    }

    /// Adds what `other` knows.
    pub fn extend(&mut self, other: &Self) {
        for v in &other.0 {
            if !self.0.contains(v) {
                self.0.push(v.clone());
            }
        }
    }

    fn add(&mut self, text: &str) {
        for v in wisp_envmask::env_values(text) {
            if !self.0.contains(&v) {
                self.0.push(v);
            }
        }
    }
}

/// What one event asks of the host besides the head update.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// nothing else (the head changed, or nothing did)
    None,
    /// append this step to the log
    Append(Activity),
    /// the turn's result: end the turn
    Finish {
        text: String,
        is_error: bool,
        stopped: bool,
        stats: Option<HarnessStats>,
        auth_failed: bool,
        /// the API's own word for how it ended (`refusal`); empty when it said none
        stop_reason: String,
    },
    /// a result with background tasks still up: the turn stays open (the live line says so)
    Waiting(Activity),
    /// the account's usage windows (wisp.budget records them through `turn.limits`)
    Limits(Value),
    /// billed to an API key or a profile, not the plan
    Billing(String),
}

/// v1 `stepOf`: a tool call, a message or a failed call as a transcript step.
pub fn step_of(ev: &HarnessEvent) -> Option<Activity> {
    Some(match ev {
        HarnessEvent::Tool { kind, label, file, detail, id, output, title, parent } => Activity::Tool {
            parent: parent.clone(),
            tool: *kind,
            label: label.clone(),
            title: title.clone(),
            file: file.clone(),
            detail: detail.clone(),
            call_id: id.clone(),
            output: output.as_deref().filter(|_| shows_output(*kind)).map(clip_output),
            hide: std::collections::BTreeMap::new(),
            full: None,
        },
        HarnessEvent::Text { text } => Activity::Text { text: head16(text, MAX_TEXT).to_owned(), hide: Vec::new() },
        // a failed call's error comes whole from its parser and is kept like an output, start and end (audit F2: a
        // test run's failure is at its end); it names the call it ends, so the app shows it as that call's output (F3)
        HarnessEvent::ToolError { name, text, id } => Activity::ToolError {
            name: if name.is_empty() { "Tool".into() } else { name.clone() },
            text: if utf16_len(text) <= MAX_OUTPUT { text.clone() } else { clip_output(text) },
            call_id: id.clone(),
            hide: Vec::new(),
            full: None,
        },
        HarnessEvent::Thinking { text } => Activity::Thinking { text: text.clone(), hide: Vec::new() },
        _ => return None,
    })
}

/// The live line while a turn waits on its background tasks.
pub fn waiting_step(tasks: &[String]) -> Activity {
    let label = match tasks {
        [] => "Background work finished, wrapping up".to_owned(),
        [one] => format!("Waiting on background task: {one}"),
        many => format!("Waiting on {} background tasks", many.len()),
    };
    Activity::Tool {
        parent: None,
        tool: ToolKind::Background,
        label: head16(&label, 200).to_owned(),
        file: None,
        detail: None,
        call_id: None,
        output: None,
        title: None,
        hide: std::collections::BTreeMap::new(),
        full: None,
    }
}

fn record_change(head: &mut Head, file: &str, op: ChangeOp, edits: &[Edit]) {
    match head.changes.iter_mut().find(|c| c.file == file) {
        Some(c) => {
            if op == ChangeOp::Add && c.edits.is_empty() {
                c.op = ChangeOp::Add;
            }
            c.edits.extend_from_slice(edits);
        }
        None => head.changes.push(FileChange { file: file.to_owned(), op, edits: edits.to_vec() }),
    }
}

/// A call that names a .env file: its `tool_output` is flagged `env` (ADR-0050).
fn note_env_call(head: &mut Head, ev: &HarnessEvent) {
    if let HarnessEvent::Tool { file, detail, label, id: Some(id), .. } = ev
        && crate::env_mask::call_reads_env(file.as_deref(), detail.as_deref(), Some(label))
    {
        head.env_calls.push(id.clone());
    }
}

/// A step's text that quotes a .env value (ADR-0050): its thinking or a tool's error quoting a value of a .env file
/// this turn read, or a line shaped like one; all of an error from a call that read one. A tool call that printed a
/// .env file inline (its `output`) adds its values.
fn flag_quotes(head: &mut Head, ev: &HarnessEvent, mut a: Activity) -> Activity {
    match (&mut a, ev) {
        (Activity::Tool { label, detail, title, output, hide, .. }, HarnessEvent::Tool { file, detail: d, label: l, output: o, .. }) => {
            let reads = crate::env_mask::call_reads_env(file.as_deref(), d.as_deref(), Some(l));
            if reads && let Some(out) = o {
                head.env_values.add(out);
            }
            // its own command line or label quoting a value (`curl -H "Bearer sk-…"`); an output it printed inline
            // that isn't the .env file's own (that one is masked whole)
            let out = if reads { None } else { output.as_deref() };
            for (k, t) in [("label", Some(&**label)), ("detail", detail.as_deref()), ("title", title.as_deref()), ("output", out)] {
                let spans = t.map(|t| wisp_envmask::value_spans(t, &head.env_values.0)).unwrap_or_default();
                if !spans.is_empty() {
                    hide.insert(k.to_owned(), spans);
                }
            }
        }
        (Activity::Thinking { text: Some(text), hide }, _) => *hide = wisp_envmask::quote_spans(text, &head.env_values.0),
        (Activity::ToolError { text, hide, .. }, HarnessEvent::ToolError { id, .. }) => {
            *hide = if id.as_ref().is_some_and(|id| head.env_calls.contains(id)) {
                wisp_envmask::text_spans(text)
            } else {
                wisp_envmask::quote_spans(text, &head.env_values.0)
            };
        }
        _ => {}
    }
    a
}

/// A clipped step's whole output kept (`full`, [`Head::full`]) unless it read a .env file or quotes a value the task
/// read anywhere in it, the cut part too: that one stays as clipped (ADR-0050).
fn keep_full(head: &mut Head, ev: &HarnessEvent, mut a: Activity) -> Activity {
    if !head.keep_full {
        return a;
    }
    let (whole, env, hidden, full) = match (&mut a, ev) {
        (Activity::Tool { output: Some(_), hide, full, .. }, HarnessEvent::Tool { output: Some(o), file, detail, label, .. }) => {
            let env = crate::env_mask::call_reads_env(file.as_deref(), detail.as_deref(), Some(label));
            (o, env || !wisp_envmask::value_spans(o, &head.env_values.0).is_empty(), !hide.is_empty(), full)
        }
        (Activity::ToolError { hide, full, .. }, HarnessEvent::ToolError { id, text, .. }) => {
            let env = id.as_ref().is_some_and(|id| head.env_calls.contains(id));
            (text, env || !wisp_envmask::quote_spans(text, &head.env_values.0).is_empty(), !hide.is_empty(), full)
        }
        (Activity::ToolOutput { env, hide, full, .. }, HarnessEvent::ToolOutput { text, .. }) => {
            (text, *env || !wisp_envmask::value_spans(text, &head.env_values.0).is_empty(), !hide.is_empty(), full)
        }
        _ => return a,
    };
    if !env
        && !hidden
        && let Some(f) = full_of(whole)
    {
        head.full.push(whole.trim_end().to_owned());
        *full = Some(f);
    }
    a
}

/// Applies one event to the head and says what else to do. `plain`: the turn asked for a plain answer
/// (wisp blocks off, or Chat mode: no end-of-turn report), so its prose is the whole answer; `is_fallback`: whether a text is an answer that broke the protocol (the block
/// parser's `fallback`), passed in so this module stays independent of it.
pub fn apply(head: &mut Head, ev: &HarnessEvent, plain: bool, is_fallback: &dyn Fn(&str) -> bool) -> Effect {
    match ev {
        HarnessEvent::Session { id, at } => {
            head.resume_id = Some(id.clone());
            if at.is_some() {
                head.resume_at.clone_from(at);
            }
        }
        HarnessEvent::Text { text } => {
            head.text = Some(text.clone());
            // the answer's prose: where it quotes a value the task read, or reads like a .env line (ADR-0050)
            return step_of(ev).map_or(Effect::None, |mut a| {
                if let Activity::Text { text, hide } = &mut a {
                    *hide = wisp_envmask::raw_prose_spans(text, &head.env_values.0);
                }
                Effect::Append(a)
            });
        }
        HarnessEvent::Tool { .. } | HarnessEvent::ToolError { .. } | HarnessEvent::Thinking { .. } => {
            note_env_call(head, ev);
            return step_of(ev).map_or(Effect::None, |a| {
                let a = flag_quotes(head, ev, a);
                Effect::Append(keep_full(head, ev, a))
            });
        }
        HarnessEvent::ToolOutput { id, text } => {
            let env = head.env_calls.contains(id);
            if env {
                head.env_values.add(text);
            }
            let clipped = clip_output(text);
            // another command's output quoting a value the task read (`printenv`, `echo $KEY`)
            let hide = if env { Vec::new() } else { wisp_envmask::value_spans(&clipped, &head.env_values.0) };
            let a = Activity::ToolOutput { call_id: id.clone(), text: clipped, env, hide, full: None };
            return Effect::Append(keep_full(head, ev, a));
        }
        HarnessEvent::Change { file, op, edits, early, .. } => {
            if !*early || !head.changes.iter().any(|c| &c.file == file) {
                record_change(head, file, op.normalised(), edits);
            }
        }
        HarnessEvent::Blocked { tool, detail, rule } => {
            // its command line quoting a value the task read (`curl -H "Bearer sk-…"` refused): flagged like a call's
            if !head.blocked.iter().any(|b| &b.tool == tool && &b.detail == detail) {
                head.blocked.push(BlockedAction::flagged(tool.clone(), detail.clone(), rule.clone(), &head.env_values));
            }
        }
        HarnessEvent::Limits { .. } => {
            return Effect::Limits(serde_json::to_value(ev).unwrap_or(Value::Null));
        }
        HarnessEvent::Billing { kind } => {
            return Effect::Billing(serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default());
        }
        HarnessEvent::Tools { list } => head.tools = Some(list.clone()),
        HarnessEvent::Cli { .. } => head.cli = serde_json::to_value(ev).ok(),
        HarnessEvent::Background { tasks } => {
            head.background.clone_from(tasks);
            if head.interim.is_some() {
                return Effect::Waiting(waiting_step(tasks));
            }
        }
        HarnessEvent::Approval { .. } => {}
        HarnessEvent::Result { text, is_error, stopped, stats, auth_failed, background, background_agents, stop_reason, .. } => {
            // Its subagents are still at work: whatever it said, it isn't done; their reports come back into this run
            // and the answer after them ends the turn (v1 #775: a goal is checked on that one, not this).
            if background_agents.unwrap_or(0) > 0 && !*is_error && !*stopped {
                head.interim = Some((text.clone(), stats.clone()));
                head.interim_agents = true;
                return Effect::Waiting(waiting_step(&head.background));
            }
            // It ended its turn in plain text with background tasks still running ("I'll report once it's
            // done"): the CLI wakes it when they finish. A full presentation is a finished task that left
            // something up (a dev server), and so is any answer of a plain or Chat mode turn: prose is all it gives.
            if background.unwrap_or(0) > 0 && !*is_error && !plain && is_fallback(text) {
                head.interim = Some((text.clone(), stats.clone()));
                head.interim_agents = false;
                return Effect::Waiting(waiting_step(&head.background));
            }
            head.interim = None;
            head.interim_agents = false;
            return Effect::Finish {
                text: text.clone(),
                is_error: *is_error,
                stopped: *stopped,
                stats: stats.clone(),
                auth_failed: *auth_failed,
                stop_reason: stop_reason.clone().unwrap_or_default(),
            };
        }
    }
    Effect::None
}

/// The run let go while the turn still ran: the interim result is the best we have, else an error.
pub fn exit_without_result(head: &mut Head) -> Effect {
    match head.interim.take() {
        Some((text, stats)) => {
            Effect::Finish { text, is_error: false, stopped: false, stats, auth_failed: false, stop_reason: String::new() }
        }
        None => Effect::Finish {
            text: "The agent exited without a result.".into(),
            is_error: true,
            stopped: false,
            stats: None,
            auth_failed: false,
            stop_reason: String::new(),
        },
    }
}
