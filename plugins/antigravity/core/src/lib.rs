//! wisp.antigravity core (sans-IO): `agy --input-format stream-json --output-format stream-json`'s
//! parser (v1 adapters/antigravity.js: one init, step_update events, one result per turn), its tool
//! sentences, the argv and the stdin `user` event. agy takes no MCP server or system prompt by flag:
//! no ask tool, and the protocol goes ahead of the first message.

// Docs quote agy's own JSON keys and PascalCase parameters as prose.
#![allow(clippy::doc_markdown)]

pub mod usage;

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};
pub use usage::usage_limits;
use wisp_harness_kit::js::{self, truthy};
use wisp_harness_kit::protocol::exit_text;
use wisp_harness_kit::{
    Exit, HarnessEvent, IoStep, LineParser, Out, Re, RunPaths, Stats, Tool, ToolKind, TurnResult, clip, clip_value, json_line, rel_value,
};

/// Headless runs give up after --print-timeout (5m by default); wisp tasks routinely take longer.
pub const PRINT_TIMEOUT: &str = "24h";
static EDIT_TOOL: Re = Re::new(r"(?i)write|replace|edit|delete");
static PERMISSION: Re = Re::new(r"(?i)PERMISSION");
static EFFORT_SUFFIX: Re = Re::new(r"^(.+)-(low|medium|high|xhigh|max)$");
/// agy's levels, lowest first: a folded model lists its own in this order.
const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
static MODEL_LINE: Re = Re::new(r"^([\w.\-]+)(?:\t|\s{2,})\s*(.+)$");

pub fn args(mode: &str, model: &str, effort: &str, resume_id: Option<&str>, attachment_dirs: &[String]) -> Vec<String> {
    let mut a: Vec<String> =
        ["--input-format", "stream-json", "--output-format", "stream-json", "--print-timeout", PRINT_TIMEOUT].map(str::to_owned).to_vec();
    match mode {
        "plan" => a.extend(["--mode".into(), "plan".into()]),
        "auto" => a.push("--dangerously-skip-permissions".into()),
        _ => a.extend(["--mode".into(), "accept-edits".into()]),
    }
    if !model.is_empty() {
        a.extend(["--model".into(), model.into()]);
    }
    // a slug that holds its level takes no --effort: agy exits with "conflicts with --effort" (grid, 2026-11-24)
    if !effort.is_empty() && !effort_in_id(model) {
        a.extend(["--effort".into(), effort.into()]);
    }
    if let Some(r) = resume_id.filter(|r| !r.is_empty()) {
        a.extend(["--conversation".into(), r.into()]);
    }
    let mut seen = Vec::new();
    for d in attachment_dirs {
        if !seen.contains(d) {
            seen.push(d.clone());
            a.extend(["--add-dir".into(), d.clone()]);
        }
    }
    a
}

/// The prompt as agy's one stdin `user` event.
pub fn stdin_event(content: &str) -> String {
    format!("{}\n", json!({ "event": "user", "message": { "content": content } }))
}

/// Whether `model`'s slug holds its thinking level (`gemini-3-flash-high`, a pick saved before models were folded):
/// it takes no `--effort`.
#[must_use]
pub fn effort_in_id(model: &str) -> bool {
    EFFORT_SUFFIX.is_match(model)
}

/// One model of `agy models`, its levels folded in: `efforts` is the levels agy lists for it, lowest first (empty: none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgyModel {
    pub id: String,
    pub label: String,
    pub efforts: Vec<String>,
}

/// `agy models`: "slug<TAB>Label" (or space-padded) lines, one per model and level (`gemini-4-neon-high`, "Gemini 4
/// Neon (High)"). The levels of one model fold into one row (`gemini-4-neon`, "Gemini 4 Neon", low…xhigh) so the
/// picker reads model + level like every other harness; the launch passes them as `--model` + `--effort` and agy joins
/// them (it refuses a level that model lacks). A slug without a level (`gemini-pro-orchestrator`) takes none.
pub fn list_models(out: &str) -> Vec<AgyModel> {
    let mut models: Vec<AgyModel> = Vec::new();
    for l in out.lines() {
        let Some(c) = MODEL_LINE.captures(js::trim(l)) else { continue };
        let (Some(slug), label) = (c.get(1).map(|m| m.as_str()), js::trim(c.get(2).map_or("", |m| m.as_str()))) else { continue };
        let level = EFFORT_SUFFIX.captures(slug).and_then(|e| Some((e.get(1)?.as_str(), e.get(2)?.as_str())));
        let (id, label, level) = match level {
            Some((base, lv)) => (base, strip_level(label, lv), Some(lv)),
            None => (slug, label.to_owned(), None),
        };
        let at = models.iter().position(|m| m.id == id).unwrap_or_else(|| {
            models.push(AgyModel { id: id.to_owned(), label, efforts: Vec::new() });
            models.len() - 1
        });
        if let (Some(lv), Some(m)) = (level, models.get_mut(at))
            && !m.efforts.iter().any(|e| e == lv)
        {
            m.efforts.push(lv.to_owned());
        }
    }
    for m in &mut models {
        m.efforts.sort_by_key(|e| LEVELS.iter().position(|l| l == e));
    }
    models
}

/// "Gemini 4 Neon (High)" → "Gemini 4 Neon": the level's own parenthesis only.
fn strip_level(label: &str, level: &str) -> String {
    label
        .strip_suffix(')')
        .and_then(|l| l.rsplit_once(" ("))
        .filter(|(_, lv)| lv.eq_ignore_ascii_case(level))
        .map_or_else(|| label.to_owned(), |(base, _)| base.to_owned())
}

/// Tool parameters are PascalCase (TargetFile, AbsolutePath, CommandLine, …).
fn file_of(p: &Value) -> Option<&Value> {
    ["TargetFile", "AbsolutePath", "FilePath", "File", "Path", "path", "file_path"]
        .into_iter()
        .map(|k| &p[k])
        .find(|v| !v.is_null() && v.as_str() != Some(""))
}

#[allow(clippy::many_single_char_names)] // v1 describeTool's own names
pub fn describe_tool(name: &str, p: &Value, paths: &RunPaths) -> Tool {
    let f = file_of(p).map_or_else(String::new, |v| rel_value(v, paths));
    let n = name.to_lowercase();
    let t = |k: ToolKind, l: String| Tool::new(k, l);
    if n == "run_command" {
        let (label, title) = wisp_harness_kit::run_label(&js::str(&p["CommandLine"]), paths, None);
        let mut x = t(ToolKind::Run, label);
        x.title = title; // the whole one, when the label was cut (v1 #780)
        if !p["CommandLine"].is_null() {
            x.detail = Some(js::string(&p["CommandLine"]));
        }
        return x;
    }
    if EDIT_TOOL.is_match(&n) && !f.is_empty() {
        return t(ToolKind::Edit, format!("{} {f}", if n.contains("delete") { "Deleting" } else { "Editing" })).file(f);
    }
    if (n.starts_with("view_") || n == "read_file" || n == "read") && !f.is_empty() {
        return t(ToolKind::Read, format!("Reading {f}")).file(f);
    }
    let or = |a: &str, b: &str| js::or(&p[a], &p[b]).clone();
    if n == "list_dir" {
        let d = rel_value(&p["DirectoryPath"], paths);
        return t(ToolKind::Search, format!("Listing {}", if d.is_empty() { "." } else { &d }));
    }
    if n == "find_by_name" {
        return t(ToolKind::Search, format!("Finding {}", clip_value(&p["Pattern"], 50)));
    }
    if n.contains("grep") {
        return t(ToolKind::Search, format!("Searching for “{}”", clip_value(&or("Query", "Pattern"), 40)));
    }
    if n.contains("codesearch") || n.contains("code_search") || n.contains("codebase") {
        return t(ToolKind::Search, format!("Searching the codebase: {}", clip_value(&or("Query", "query"), 50)));
    }
    if n.contains("search_web") || n.contains("web_search") {
        return t(ToolKind::Web, format!("Searching the web: {}", clip_value(&or("query", "Query"), 50)));
    }
    if n.contains("url") || n.contains("fetch") {
        return t(ToolKind::Web, format!("Reading {}", clip_value(&or("Url", "url"), 60)));
    }
    if n.contains("browser") {
        let task = or("Task", "TaskName");
        return t(ToolKind::Web, format!("Browsing: {}", if truthy(&task) { clip_value(&task, 60) } else { "browser".into() }));
    }
    if n.contains("task_boundary") || n.contains("todo") {
        let s = or("TaskStatus", "TaskName");
        return t(ToolKind::Plan, if truthy(&s) { clip_value(&s, 80) } else { "Planning".into() });
    }
    t(ToolKind::Tool, format!("Using {}", n.replace('_', " ")))
}

#[derive(Debug, Default)]
pub struct AgyParser {
    paths: RunPaths,
    text: HashMap<String, String>,
    shown: HashSet<String>,
    saw_result: bool,
    session: Option<String>,
}

impl AgyParser {
    pub fn new(paths: RunPaths) -> Self {
        Self { paths, ..Self::default() }
    }

    fn session_from(&mut self, id: &Value, out: &mut Vec<Out>) {
        if let Some(id) = id.as_str().filter(|s| !s.is_empty())
            && self.session.as_deref() != Some(id)
        {
            self.session = Some(id.to_owned());
            out.push(HarnessEvent::session(id).into());
        }
    }

    fn step(&mut self, s: &Value, out: &mut Vec<Out>) {
        self.session_from(&s["conversation_id"], out);
        let done = s["state"] == "DONE";
        let idx = js::string(&s["step_index"]);
        if s["step_type"] == "agent_response" {
            let t = self.text.entry(idx).or_default();
            t.push_str(&js::str(&s["text_delta"]));
            if done && js::has_text(t) {
                out.push(HarnessEvent::text(t.clone()).into());
            }
            return;
        }
        if truthy(&s["subagent_info"]) && !self.shown.contains(&idx) {
            self.shown.insert(idx);
            let sub = &s["subagent_info"]["subagents"][0];
            let who = if sub.is_object() { format!(" to {}", js::string(js::or(&sub["role"], &sub["type_name"]))) } else { String::new() };
            out.push(HarnessEvent::Tool(Tool::new(ToolKind::Agent, format!("Delegating{who}"))).into());
            return;
        }
        if s["step_type"] != "tool" {
            return;
        }
        let info = &s["tool_info"];
        let name = js::str(js::or(&s["tool_name"], &info["name"]));
        if name.is_empty() {
            return;
        }
        let empty = Value::Object(Map::new());
        let params = if info["parameters"].is_object() { &info["parameters"] } else { &empty };
        let file = file_of(params).map(|v| rel_value(v, &self.paths)).filter(|f| !f.is_empty());
        let editing = file.is_some() && EDIT_TOOL.is_match(&name);
        // announce once the arguments are known: on ACTIVE if it carries them, else on DONE
        let has_args = params.as_object().is_some_and(|o| !o.is_empty());
        if !self.shown.contains(&idx) && (done || has_args) {
            self.shown.insert(idx.clone());
            if editing
                && !done
                && let Some(f) = &file
            {
                out.push(Out::Io(IoStep::Remember { file: f.clone() }));
            }
            out.push(HarnessEvent::Tool(describe_tool(&name, params, &self.paths)).into());
        }
        if !done {
            return;
        }
        if truthy(&info["error"]) {
            let e = &info["error"];
            let why = [&e["message"], &e["type"]].into_iter().find(|v| truthy(v)).map_or_else(|| "failed".to_owned(), js::string);
            out.push(HarnessEvent::ToolError { name: Some(name.clone()), text: clip(&why, 400), id: None }.into());
            if PERMISSION.is_match(&js::str(&e["type"])) {
                let d = [&params["CommandLine"]]
                    .into_iter()
                    .find(|v| !v.is_null())
                    .cloned()
                    .or_else(|| file_of(params).cloned())
                    .or_else(|| Some(params["Url"].clone()).filter(|v| !v.is_null()));
                out.push(HarnessEvent::Blocked { tool: name, detail: d.map_or_else(String::new, |v| js::string(&v)), rule: None }.into());
            }
        } else if editing && let Some(f) = file {
            out.push(Out::Io(IoStep::Changed { file: f }));
        }
    }
}

impl LineParser for AgyParser {
    fn line(&mut self, line: &str, _now_ms: i64, out: &mut Vec<Out>) {
        let Some(msg) = json_line(line) else { return };
        match js::str_ref(&msg["event"]) {
            "init" => self.session_from(&msg["conversation_id"], out),
            "step_update" => {
                let s = if msg["step_update"].is_object() { msg["step_update"].clone() } else { json!({}) };
                self.step(&s, out);
            }
            "result" => {
                let r = &msg["result"];
                self.session_from(&r["conversation_id"], out);
                self.saw_result = true;
                let ok = r["status"] == "SUCCESS";
                let text = if ok {
                    js::str(&r["response"])
                } else {
                    [&r["error"], &r["response"]]
                        .into_iter()
                        .find(|v| truthy(v))
                        .map_or_else(|| format!("Antigravity ended with status {}", js::string(&r["status"])), js::string)
                };
                let mut stats = Map::new();
                if let Some(s) = r["duration_seconds"].as_f64() {
                    stats.insert("durationMs".into(), js::json_num((s * 1000.0).round()));
                }
                for (k, v) in [("inputTokens", &r["usage"]["input_tokens"]), ("outputTokens", &r["usage"]["output_tokens"])] {
                    if !v.is_null() {
                        stats.insert(k.into(), v.clone());
                    }
                }
                if r["usage"]["cache_read_tokens"].as_f64().is_some_and(|n| n > 0.0) {
                    stats.insert("cacheReadTokens".into(), r["usage"]["cache_read_tokens"].clone());
                }
                let stats: Option<Box<Stats>> = serde_json::from_value(Value::Object(stats)).ok().map(Box::new);
                out.push(HarnessEvent::Result(TurnResult { text, is_error: !ok, stats, ..TurnResult::default() }).into());
            }
            _ => {}
        }
    }

    fn close(&mut self, exit: &Exit, _now_ms: i64, out: &mut Vec<Out>) {
        if self.saw_result {
            return;
        }
        let text = if exit.stderr.is_empty() { exit_text("agy", exit.code, exit.signal.as_deref()) } else { exit.stderr.clone() };
        out.push(HarnessEvent::Result(TurnResult { stopped: Some(exit.stopped), ..TurnResult::error(text) }).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_words_and_models() {
        for p in [&EDIT_TOOL, &PERMISSION, &EFFORT_SUFFIX, &MODEL_LINE] {
            assert!(p.compiles(), "{}", p.pattern());
        }
        let p = RunPaths::same("/w");
        assert_eq!(describe_tool("grep_search", &json!({ "Query": "add" }), &p).label, "Searching for “add”");
        assert_eq!(describe_tool("write_to_file", &json!({ "TargetFile": "/w/a.js" }), &p).label, "Editing a.js");
        let read = describe_tool("read_file", &json!({ "Path": "/w/src/a.ts" }), &p);
        assert_eq!(read.kind, ToolKind::Read);
        assert_eq!(read.label, "Reading src/a.ts");
        assert_eq!(read.file.as_deref(), Some("src/a.ts"));
        assert_eq!(describe_tool("read_url_content", &json!({ "Url": "https://x.dev" }), &p).label, "Reading https://x.dev");
        // v1 adapters.test.js "antigravity: detect lists agy models"
        let m = list_models("Fetching models…\ngemini-3-pro-high\tGemini 3 Pro (High)\nclaude-sonnet-4-5   Claude Sonnet 4.5\n");
        let lv = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            m,
            [
                AgyModel { id: "gemini-3-pro".into(), label: "Gemini 3 Pro".into(), efforts: lv(&["high"]) },
                AgyModel { id: "claude-sonnet-4-5".into(), label: "Claude Sonnet 4.5".into(), efforts: vec![] }
            ]
        );
        assert_eq!(stdin_event("hi"), "{\"event\":\"user\",\"message\":{\"content\":\"hi\"}}\n");
    }

    #[test]
    fn a_model_whose_id_holds_its_level_gets_no_effort_flag() {
        // grid 2026-11-24: `--model gemini-3.8-flash-high --effort max` made agy exit with "conflicts with --effort"
        let has = |a: &[String], f: &str| a.iter().any(|x| x == f);
        let baked = args("auto", "gemini-3.8-flash-high", "max", None, &[]);
        assert!(has(&baked, "gemini-3.8-flash-high") && !has(&baked, "--effort"), "{baked:?}");
        let plain = args("auto", "claude-sonnet-4-5", "high", None, &[]);
        assert!(plain.windows(2).any(|w| w[0] == "--effort" && w[1] == "high"), "{plain:?}");
        // the Default (no model) keeps the picked level
        assert!(args("auto", "", "low", None, &[]).windows(2).any(|w| w[0] == "--effort" && w[1] == "low"));
        assert!(effort_in_id("gemini-3-pro-low") && effort_in_id("gemini-3-flash-medium") && effort_in_id("gemini-4-neon-xhigh"));
        // a saved xhigh slug took --effort too and agy refused it
        assert!(!has(&args("auto", "gemini-4-neon-xhigh", "high", None, &[]), "--effort"));
        // a folded model: agy joins --model and --effort itself
        let folded = args("auto", "gemini-4-neon", "xhigh", None, &[]);
        assert!(folded.windows(4).any(|w| w == ["--model", "gemini-4-neon", "--effort", "xhigh"]), "{folded:?}");
        assert!(!effort_in_id("gemini-3-pro") && !effort_in_id("gemini-3-pro-xhighway") && !effort_in_id(""));
    }

    #[test]
    fn agy_models_fold_one_row_per_model_with_its_own_levels() {
        // `agy models` 2026-11-26: one slug per model and level, in no order; the orchestrators have no level
        let out = "Fetching available models...\n\
            gemini-4-neon-high\tGemini 4 Neon (High)\n\
            gemini-4-neon-xhigh\tGemini 4 Neon (XHigh)\n\
            gemini-4-neon-medium\tGemini 4 Neon (Medium)\n\
            gemini-4-neon-low\tGemini 4 Neon (Low)\n\
            gemini-4-carbon-medium\tGemini 4 Carbon (Medium)\n\
            gemini-4-carbon-high\tGemini 4 Carbon (High)\n\
            gemini-4-carbon-low\tGemini 4 Carbon (Low)\n\
            gemini-pro-orchestrator-neon\tOrchestrator (Neon)\n\
            gemini-pro-orchestrator\tOrchestrator (Carbon)\n\
            claude-opus-5-5-low\tClaude Opus 5.5 (Low)\n\
            claude-opus-5-5-high\tClaude Opus 5.5 (High)\n\
            gpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n";
        let got: Vec<(String, String, String)> = list_models(out).into_iter().map(|m| (m.id, m.label, m.efforts.join(","))).collect();
        let want = [
            ("gemini-4-neon", "Gemini 4 Neon", "low,medium,high,xhigh"),
            ("gemini-4-carbon", "Gemini 4 Carbon", "low,medium,high"),
            ("gemini-pro-orchestrator-neon", "Orchestrator (Neon)", ""),
            ("gemini-pro-orchestrator", "Orchestrator (Carbon)", ""),
            ("claude-opus-5-5", "Claude Opus 5.5", "low,high"),
            ("gpt-oss-120b", "GPT-OSS 120B", "medium"),
        ]
        .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()));
        assert_eq!(got, want);
        // a label without the level's parenthesis stays whole
        assert_eq!(list_models("x-high\tX")[0].label, "X");
        assert_eq!(strip_level("Orchestrator (Neon)", "high"), "Orchestrator (Neon)");
    }

    #[test]
    fn text_is_the_steps_deltas() {
        let mut p = AgyParser::new(RunPaths::same("/w"));
        let mut out = Vec::new();
        p.line(
            r#"{"event":"step_update","step_update":{"step_index":4,"state":"ACTIVE","step_type":"agent_response","text_delta":"Fixed "}}"#,
            0,
            &mut out,
        );
        assert_eq!(out, Vec::<Out>::new());
        p.line(
            r#"{"event":"step_update","step_update":{"step_index":4,"state":"DONE","step_type":"agent_response","text_delta":"it."}}"#,
            1,
            &mut out,
        );
        assert!(matches!(out.last(), Some(Out::Event(HarnessEvent::Text { text })) if text == "Fixed it."));
    }

    #[test]
    fn cache_read_tokens_reported_when_nonzero_and_omitted_when_zero() {
        let mut p = AgyParser::new(RunPaths::same("/w"));
        let mut out = Vec::new();
        p.line(
            r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"ok","duration_seconds":1.2,"usage":{"input_tokens":100,"output_tokens":20,"cache_read_tokens":60}}}"#,
            0,
            &mut out,
        );
        let Some(Out::Event(HarnessEvent::Result(r))) = out.last() else { panic!("expected result") };
        let st = r.stats.as_ref().expect("stats");
        assert_eq!(st.input_tokens, Some(json!(100)));
        assert_eq!(st.output_tokens, Some(json!(20)));
        assert_eq!(st.cache_read_tokens, Some(json!(60)));

        let mut p0 = AgyParser::new(RunPaths::same("/w"));
        let mut out0 = Vec::new();
        p0.line(
            r#"{"event":"result","result":{"conversation_id":"c2","status":"SUCCESS","response":"ok","usage":{"input_tokens":10,"output_tokens":5,"cache_read_tokens":0}}}"#,
            0,
            &mut out0,
        );
        let Some(Out::Event(HarnessEvent::Result(r0))) = out0.last() else { panic!("expected result") };
        assert_eq!(r0.stats.as_ref().and_then(|s| s.cache_read_tokens.as_ref()), None);
    }
}
