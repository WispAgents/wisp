//! wisp.factory's TUI (ADR-0024, ADR-0114): a `tui.input@1` in the task header, where a factory run stands
//! (`ext["wisp.factory"]`): each station's mark in order, the one it's at, and the round of the limit, in the run's
//! tone. The GUI's stations band, one span wide.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use serde_json::Value;
use wisp_tui_api::{InputCx, Registry, Slot, Theme, TuiInput};

/// What wisp.factory adds to the TUI.
pub fn register(r: &mut Registry) {
    r.input(FactoryLine);
}

/// The run's stations in the task header.
#[derive(Debug)]
pub struct FactoryLine;

/// `✓ spec · ● review 2/3 · ○ ship`: a mark per station, the one it's at with its round.
pub fn line(run: &Value) -> Option<String> {
    let stations = run.get("stations")?.as_array()?;
    let at = run.get("station").and_then(Value::as_str).unwrap_or_default();
    let (round, max) = (run.get("round").and_then(Value::as_u64).unwrap_or(1), run.get("max").and_then(Value::as_u64).unwrap_or(0));
    let parts: Vec<String> = stations
        .iter()
        .map(|s| {
            let id = s.get("id").and_then(Value::as_str).unwrap_or_default();
            let mark = match s.get("state").and_then(Value::as_str).unwrap_or_default() {
                "done" => "✓",
                "failed" => "✗",
                "skipped" => "–",
                "running" => "●",
                "asking" => "?",
                _ => "○",
            };
            if id == at && round > 1 && max > 0 { format!("{mark} {id} {round}/{max}") } else { format!("{mark} {id}") }
        })
        .collect();
    let name = run.get("factory").and_then(Value::as_str).unwrap_or("factory");
    Some(format!("{name}: {}", parts.join(" · ")))
}

impl TuiInput for FactoryLine {
    fn slot(&self) -> Slot {
        Slot::TaskHeader
    }
    fn render(&self, cx: &InputCx, th: &Theme) -> Option<Span<'static>> {
        let run = cx.task.pointer("/ext/wisp.factory").filter(|r| r.get("stations").is_some())?;
        let tone = match run.get("state").and_then(Value::as_str).unwrap_or_default() {
            "done" => th.good,
            "failed" => th.bad,
            "waiting" | "stopped" => th.warn,
            _ => th.accent_ink,
        };
        Some(Span::styled(line(run)?, Style::default().fg(tone).add_modifier(Modifier::BOLD)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_factory_run_shows_its_stations_and_round_in_the_task_header() {
        let th = Theme::DARK;
        let run = json!({ "factory": "default", "state": "waiting", "station": "review", "round": 2, "max": 3,
            "stations": [{ "id": "spec", "state": "done" }, { "id": "build", "state": "done" }, { "id": "review", "state": "asking" }, { "id": "ship", "state": "waiting" }] });
        let cx = InputCx { task: json!({ "ext": { "wisp.factory": run } }), ..InputCx::default() };
        let s = FactoryLine.render(&cx, &th).unwrap();
        assert_eq!((s.content.as_ref(), s.style.fg), ("default: ✓ spec · ✓ build · ? review 2/3 · ○ ship", Some(th.warn)));
        assert!(FactoryLine.render(&InputCx::default(), &th).is_none(), "no run: nothing");
        let station = InputCx { task: json!({ "ext": { "wisp.factory": { "run": "r", "station": "build" } } }), ..InputCx::default() };
        assert!(FactoryLine.render(&station, &th).is_none(), "a station's own task shows nothing here");
        let mut r = Registry::default();
        register(&mut r);
        assert_eq!(r.inputs.len(), 1);
    }
}
