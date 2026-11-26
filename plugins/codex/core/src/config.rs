//! Codex's `config.toml`, as text: the edits wisp makes to the table it wrote there itself.

/// The name wisp's repo-history server (`wisp mcp`) goes by in Codex, as in every run wisp starts.
pub const HISTORY_SERVER: &str = "wisp-history";

/// `config.toml` with the `[mcp_servers.wisp]` table that `wisp skill install --agent codex` (and the docs) used to
/// write renamed to `[mcp_servers.wisp-history]`, or `None` when there is nothing to move.
///
/// That table ran `wisp mcp`, the repo-history server, under the name wisp's own run server has. Codex merges each
/// run's `-c mcp_servers.wisp.url=…` into it, gets a stdio server with a url, and refuses its whole config ("url is
/// not supported for stdio"): no Codex task started. Only wisp's own entry moves (it runs `wisp mcp` or
/// `npx wisp-agents mcp`, and has no url); a `wisp` server the user wrote for something else stays as it is. With a
/// `wisp-history` table already there, the old one is dropped instead.
#[must_use]
pub fn move_history_server(config: &str) -> Option<String> {
    let lines: Vec<&str> = config.split_inclusive('\n').collect();
    let start = lines.iter().position(|l| is_header(l, &["wisp", "\"wisp\"", "'wisp'"]))?;
    let end = lines.iter().skip(start + 1).position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |i| start + 1 + i);
    let body = &lines[start + 1..end];
    let value = |key: &str| {
        body.iter().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().to_owned())
        })
    };
    let (command, args) = (value("command")?, value("args").unwrap_or_default());
    let program = command.trim_matches(['"', '\'']).replace('\\', "/");
    let program = program.rsplit('/').next().unwrap_or_default().to_ascii_lowercase();
    let runs_wisp = matches!(program.as_str(), "wisp" | "wisp.exe") || (program.starts_with("npx") && args.contains("wisp-agents"));
    if value("url").is_some() || !runs_wisp || !args.contains("\"mcp\"") {
        return None;
    }
    let has_history = lines.iter().any(|l| is_header(l, &[HISTORY_SERVER, &format!("\"{HISTORY_SERVER}\"")]));
    let mut out: String = lines[..start].concat();
    if !has_history {
        let header = lines[start];
        out.push_str("[mcp_servers.");
        out.push_str(HISTORY_SERVER);
        out.push(']');
        out.push_str(&header[header.trim_end().len()..]);
        out.push_str(&body.concat());
    }
    out.push_str(&lines[end..].concat());
    Some(out)
}

/// `[mcp_servers.<name>]` for one of `names`, a trailing comment allowed.
fn is_header(line: &str, names: &[&str]) -> bool {
    let l = line.trim();
    let l = l.split_once('#').map_or(l, |(h, _)| h.trim_end());
    l.strip_prefix("[mcp_servers.").and_then(|r| r.strip_suffix(']')).is_some_and(|n| names.contains(&n.trim()))
}
