//! `config.toml`'s `[mcp_servers.wisp]` from `wisp skill install --agent codex` moves out of the way of the run's
//! own `wisp` server (telemetry, 2026-11-24: "url is not supported for stdio in `mcp_servers.wisp`", every Codex task).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wisp_codex_core::move_history_server;

const OLD: &str = "model = \"gpt-5\"\n\n[mcp_servers.wisp]\ncommand = \"/usr/local/bin/wisp\"\nargs = [\"mcp\"]\n\n[projects.\"/w\"]\ntrust_level = \"trusted\"\n";

#[test]
fn wisps_own_history_entry_is_renamed_and_the_rest_kept() {
    let new = move_history_server(OLD).unwrap();
    assert_eq!(
        new,
        "model = \"gpt-5\"\n\n[mcp_servers.wisp-history]\ncommand = \"/usr/local/bin/wisp\"\nargs = [\"mcp\"]\n\n[projects.\"/w\"]\ntrust_level = \"trusted\"\n"
    );
    assert_eq!(move_history_server(&new), None, "once moved, nothing more to do");
}

#[test]
fn windows_paths_npx_and_crlf_are_wisps_too() {
    let win = "[mcp_servers.wisp]\r\ncommand = \"C:\\\\Users\\\\a\\\\wisp.exe\"\r\nargs = [\"mcp\"]\r\n";
    assert_eq!(
        move_history_server(win).unwrap(),
        "[mcp_servers.wisp-history]\r\ncommand = \"C:\\\\Users\\\\a\\\\wisp.exe\"\r\nargs = [\"mcp\"]\r\n"
    );
    let npx = "[mcp_servers.wisp] # wisp\ncommand = \"npx\"\nargs = [\"-y\", \"wisp-agents\", \"mcp\"]\n";
    assert!(move_history_server(npx).unwrap().starts_with("[mcp_servers.wisp-history]\ncommand = \"npx\""));
}

#[test]
fn an_existing_history_table_wins_and_the_old_one_goes() {
    let both = format!("{OLD}[mcp_servers.wisp-history]\ncommand = \"wisp\"\nargs = [\"mcp\"]\n");
    let new = move_history_server(&both).unwrap();
    assert!(!new.contains("[mcp_servers.wisp]"), "{new}");
    assert_eq!(new.matches("[mcp_servers.wisp-history]").count(), 1, "{new}");
    assert!(new.contains("[projects.\"/w\"]") && new.starts_with("model = "), "{new}");
}

#[test]
fn a_wisp_server_the_user_wrote_for_something_else_stays() {
    for text in [
        "[mcp_servers.wisp]\nurl = \"http://127.0.0.1:4477/api/mcp\"\n",
        "[mcp_servers.wisp]\ncommand = \"node\"\nargs = [\"my-wisp.js\"]\n",
        "[mcp_servers.wisp]\ncommand = \"wisp\"\nargs = [\"serve\"]\n",
        "[mcp_servers.wispy]\ncommand = \"wisp\"\nargs = [\"mcp\"]\n",
        "model = \"o3\"\n",
        "",
    ] {
        assert_eq!(move_history_server(text), None, "{text}");
    }
}
