//! Declared projects: `runtime.projectAdd`, `runtime.projectTest`, `runtime.discover`, `runtime.folders`, `runtime.projectRun`.

use std::sync::Arc;

use wisp_kernel::{Setup, WireError};
use wisp_resources_api::StoredKind;
use wisp_runtime_api::commands as c;

use super::track;

/// `runtime.discover`: git repos under your home that aren't projects yet (`found.rs`).
pub(super) fn discover(s: &mut Setup, known: Arc<StoredKind>) {
    s.command::<c::Discover, _, _>(move |_ctx, _call, _a| {
        let known = known.clone();
        async move {
            let paths: Vec<String> = known
                .specs()
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(_, s)| s.get("path").and_then(serde_json::Value::as_str).map(str::to_owned))
                .collect();
            let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from);
            Ok(match home {
                Some(h) => crate::found::discover(&h, &paths).await,
                None => Vec::new(),
            })
        }
    });
}

/// `runtime.folders`: the folders a typed path could go on to (`found.rs` `folders`).
pub(super) fn folders(s: &mut Setup) {
    s.command::<c::Folders, _, _>(move |_ctx, _call, a| async move {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from);
        Ok(crate::found::folders(&a.q, home.as_deref()).await)
    });
}

/// `runtime.projectAdd`: a folder as a project, named here (ADR-0076, ADR-0077), so every client (`wisp add`, Projects'
/// Add, Run on, an agent's MCP tool) gets one rule: the same folder is the project it is, a taken name gets `-2`. The
/// name is chosen and the row written in one store transaction, so two adds at once never take one name.
pub(super) fn project_add(s: &mut Setup, projects: Arc<StoredKind>) {
    s.command::<c::ProjectAdd, _, _>(move |_ctx, _call, a| {
        let projects = projects.clone();
        async move {
            // spelled once where it enters (ADR-0071): `~`, `/repo/.`, `\\?\C:\x`, `RUNNER~1` all as stored; a
            // `file://` URL is its folder
            let given = file_url_path(a.path.trim());
            let given = given.as_str();
            let path = crate::kinds::project_path(given).await;
            // in plain words, as `wisp add` says it (audit A F3: it named a command id)
            wisp_runtime_api::paths::Folder::of(&path).map_err(|_| {
                WireError::invalid(format!("{} isn't a full path: type the folder's full path, like ~/code/app", given.trim()))
            })?;
            // a project is a folder that's there (audit A F1: a missing folder and a file became projects)
            match tokio::fs::metadata(&path).await {
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err(WireError::invalid(format!("{path} is a file, not a folder: pick the folder it's in"))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(WireError::invalid(format!("No folder at {path}"))),
                Err(e) => return Err(WireError::invalid(format!("Can't open {path}: {e}"))),
            }
            // a resource name: no path separators or `:` (`project.<name>` in a formula), no control characters
            if let Some(n) =
                a.name.as_deref().map(str::trim).filter(|n| n.chars().any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':')))
            {
                return Err(WireError::invalid(format!("name: {n:?} can't be a project's name (no /, \\, : or control characters)")));
            }
            let name = a.name.clone();
            let at = path.clone();
            let (name, created) = projects.put_with(move |list| crate::kinds::project_add(list, &at, name.as_deref())).await?;
            Ok(c::ProjectAdded { name, path, created })
        }
    });
}

/// A `file://` URL's path (`file:///a/My%20App`, `file://localhost/a` → `/a/My App`, `/a`); anything else as given.
fn file_url_path(given: &str) -> String {
    let Some(rest) = given.strip_prefix("file://") else { return given.to_owned() };
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(h), Some(l)) = (bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex))
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'%'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `runtime.projectTest`: a declared project's test command, written through its resource kind.
pub(super) fn project_test(s: &mut Setup, tests_kind: Arc<StoredKind>) {
    s.command::<c::ProjectTest, _, _>(move |_ctx, call, a| {
        let kind = tests_kind.clone();
        async move {
            use wisp_resources_api::ResourceKind as _;
            let list = kind.specs().await.unwrap_or_default();
            let (_, before) = list
                .into_iter()
                .find(|(n, _)| *n == a.name)
                .ok_or_else(|| WireError::not_found(format!("No project called {}", a.name)))?;
            let spec = crate::kinds::with_test(&before, &a.test_command);
            let op = wisp_resources_api::Op {
                op: wisp_resources_api::OpKind::Update,
                kind: "project".into(),
                name: a.name.clone(),
                spec,
                id: None,
                changes: Vec::new(),
                before: Some(before),
            };
            kind.apply(&call, &op).await?;
            Ok(c::Ack {})
        }
    });
}

/// A project's run line in the platform's shell: `sh -c` off Windows, `cmd.exe /d /s /c` on it, where there is no
/// `sh` unless Git's happens to be on `PATH` (wisp-proc hands a `cmd /c` line to `ConPTY` as it is written).
fn run_argv(line: &str, windows: bool) -> Vec<String> {
    let v: &[&str] = if windows { &["cmd.exe", "/d", "/s", "/c"] } else { &["sh", "-c"] };
    v.iter().map(|x| (*x).to_owned()).chain([line.to_owned()]).collect()
}

/// `runtime.projectRun`: a declared project's run scripts, in a shell terminal.
pub(super) fn project_run(s: &mut Setup, projects: Arc<StoredKind>) {
    s.command::<c::ProjectRun, _, _>(move |ctx, call, a| {
        let projects = projects.clone();
        async move {
            let list = projects.specs().await.unwrap_or_default();
            // spelled as the projects' paths are (ADR-0071): `C:\Users\RUNNER~1\…\proj\src` is in the project stored
            // as `C:\Users\runneradmin\…\proj` (CI 8bb8ccd0 refused it "isn't in a declared project")
            let cwd = wisp_proc::plain_str(&a.cwd).await;
            let (name, p) = crate::kinds::project_for(&list, &cwd)
                .ok_or_else(|| WireError::not_found(format!("{} isn't in a declared project (projects: in a formula)", a.cwd)))?;
            if p.run.is_empty() {
                return Err(WireError::invalid(format!("{name} has no run scripts (run: in its project)")));
            }
            let argv = run_argv(&p.run.join(" && "), cfg!(windows));
            let args = wisp_terminal_api::commands::OpenArgs {
                kind: wisp_terminal_api::TermKind::Shell,
                id: format!("run:{name}"),
                argv,
                cwd: p.path.clone(),
                ..Default::default()
            };
            let out = ctx.run::<wisp_terminal_api::commands::Open>(&call, args).await?;
            track(&ctx, "run_app", serde_json::json!({ "source": "project" })).await;
            serde_json::to_value(out).map_err(|e| WireError::invalid(format!("terminal.open: {e}")))
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_projects_run_line_goes_to_cmd_exe_on_windows_and_sh_elsewhere() {
        assert_eq!(run_argv("echo serving > ran.txt && npm start", false), ["sh", "-c", "echo serving > ran.txt && npm start"]);
        assert_eq!(
            run_argv("echo serving > ran.txt && npm start", true),
            ["cmd.exe", "/d", "/s", "/c", "echo serving > ran.txt && npm start"]
        );
    }
}
