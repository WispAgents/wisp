//! Which of the driver's tools a task gets. The driver's own `readOnlyHint` says what only looks; wisp adds
//! a deny list of what no task should do through it, whatever its mode.

use serde_json::Value;

/// Never offered, whatever the task's mode. Each one is either unsafe through a shared gateway or costs every
/// run its schema for little: the driver's own settings, updates and installs (`set_config`, `get_config`,
/// `check_for_update`, `install_ffmpeg`, `install_extension`, `check_permissions`), the AGPL perception
/// extension wisp never installs (`parse_visual_regions`), a recorded run replayed blind
/// (`replay_trajectory`), the agent cursor's looks (`set_agent_cursor_*`, `get_agent_cursor_state`), screen
/// recordings (`start_recording`, `stop_recording`, `get_recording_state`: a later change can offer them with
/// the file landing in the task), and the raw mouse buttons (`mouse_button_down`/`up`, `mouse_drag`,
/// `parallel_mouse_drag`: `drag` does it). Of cua-driver 0.62.1's 65 tools on Linux a box's run sees 46, one on
/// the user's own computer 44 ([`DENIED_LOCAL`]).
pub const DENIED: [&str; 19] = [
    "set_config",
    "get_config",
    "check_for_update",
    "install_ffmpeg",
    "install_extension",
    "check_permissions",
    "parse_visual_regions",
    "replay_trajectory",
    "set_agent_cursor_enabled",
    "set_agent_cursor_motion",
    "set_agent_cursor_theme",
    "get_agent_cursor_state",
    "start_recording",
    "stop_recording",
    "get_recording_state",
    "mouse_button_down",
    "mouse_button_up",
    "mouse_drag",
    "parallel_mouse_drag",
];

/// Also never offered on the user's own computer (a box's clipboard holds nothing of theirs): what they
/// copied may be a password.
pub const DENIED_LOCAL: [&str; 2] = ["clipboard_read", "clipboard_write"];

/// Where the driver runs: on the user's own computer, or in a grid box or sandbox (the box is the sandbox).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Local,
    Box,
}

/// What a task may do, decided per call from its run (never from the request).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    /// Plan mode or a side discussion: only tools that look
    pub read_only: bool,
    pub place: Place,
}

/// The tool's `annotations.readOnlyHint`; a tool without one changes something.
pub fn looks_only(tool: &Value) -> bool {
    tool.pointer("/annotations/readOnlyHint").and_then(Value::as_bool).unwrap_or(false)
}

/// Why a tool named `name` isn't offered to this task, or `None` when it is.
pub fn refusal(name: &str, tool: Option<&Value>, reach: Reach) -> Option<String> {
    if DENIED.contains(&name) || (reach.place == Place::Local && DENIED_LOCAL.contains(&name)) {
        return Some(format!("wisp: {name} isn't offered through wisp's computer use."));
    }
    let tool = tool?;
    if reach.read_only && !looks_only(tool) {
        return Some(format!("wisp: this task is in Plan mode, so it can look at the screen but not use it ({name} changes something)."));
    }
    None
}

/// The tools this task sees, in the driver's order.
pub fn visible(tools: &[Value], reach: Reach) -> Vec<Value> {
    tools
        .iter()
        .filter(|t| {
            let name = t.get("name").and_then(Value::as_str).unwrap_or_default();
            !name.is_empty() && refusal(name, Some(t), reach).is_none()
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, ro: Option<bool>) -> Value {
        match ro {
            Some(r) => json!({ "name": name, "annotations": { "readOnlyHint": r } }),
            None => json!({ "name": name }),
        }
    }

    const AUTO_LOCAL: Reach = Reach { read_only: false, place: Place::Local };
    const PLAN_BOX: Reach = Reach { read_only: true, place: Place::Box };

    #[test]
    fn plan_mode_sees_only_what_looks() {
        let tools = [tool("get_desktop_state", Some(true)), tool("click", Some(false)), tool("odd", None)];
        let names: Vec<_> = visible(&tools, PLAN_BOX).iter().map(|t| t["name"].clone()).collect();
        assert_eq!(names, vec![json!("get_desktop_state")], "a tool with no hint counts as one that changes something");
        assert_eq!(visible(&tools, AUTO_LOCAL).len(), 3);
        assert!(refusal("click", Some(&tools[1]), PLAN_BOX).is_some_and(|r| r.contains("Plan mode")));
    }

    #[test]
    fn the_deny_list_wins_in_every_mode_and_the_clipboard_only_locally() {
        let cfg = tool("set_config", Some(false));
        let clip = tool("clipboard_read", Some(true));
        for reach in [AUTO_LOCAL, PLAN_BOX, Reach { read_only: false, place: Place::Box }] {
            assert!(refusal("set_config", Some(&cfg), reach).is_some(), "{reach:?}");
            assert!(refusal("parse_visual_regions", None, reach).is_some(), "the AGPL extension, even when the driver doesn't list it");
        }
        assert!(refusal("clipboard_read", Some(&clip), AUTO_LOCAL).is_some());
        assert!(refusal("clipboard_read", Some(&clip), PLAN_BOX).is_none(), "a box's clipboard is the box's");
    }

    #[test]
    fn a_tool_the_driver_doesnt_list_isnt_refused_here() {
        // the gateway answers "unknown tool" for it; the policy only judges tools it knows
        assert_eq!(refusal("no_such_tool", None, PLAN_BOX), None);
    }
}
