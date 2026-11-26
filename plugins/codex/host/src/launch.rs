//! What to spawn for a Codex run (v1 codex.js `run()`): `codex exec --json …`, the prompt on stdin.
//! Plain data from a [`HarnessRun`]; the lean base prompt is a launch file the runtime writes.

use std::fmt::Write as _;

use wisp_codex_core::{CodexArgs, LEAN_BASE, args, mcp_env};
use wisp_harness_kit::protocol::{FileNote, attachment_note, with_protocol};
use wisp_runtime_api::harness::{HarnessRun, Launch, LaunchFile, Stdin};
use wisp_runtime_api::model::Mode;

pub const DEFAULT_BIN: &str = "codex";
const LEAN_FILE: &str = "codex-base.md";

fn is_image(mime: &str) -> bool {
    matches!(mime, "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp")
}

fn dir_of(p: &str) -> String {
    match p.rfind(['/', '\\']) {
        Some(0) => "/".to_owned(),
        Some(i) => p[..i].to_owned(),
        None => ".".to_owned(),
    }
}

fn mode(m: Mode) -> &'static str {
    match m {
        Mode::Plan => "plan",
        Mode::Edits => "edits",
        Mode::Auto => "auto",
    }
}

/// `developer`: this codex takes developer instructions (≥ 0.53, not Windows, none in config.toml).
pub fn launch(run: &HarnessRun, bin: &str, developer: bool) -> Launch {
    let resume = run.resume_id.as_deref().filter(|r| !r.is_empty());
    // resumed and forked threads have the protocol in their history already
    let developer = (developer && resume.is_none() && !run.system_prompt.trim().is_empty()).then_some(run.system_prompt.as_str());
    let with_path: Vec<_> = run.attachments.iter().filter(|a| a.path.is_some()).collect();
    let images: Vec<String> = with_path.iter().filter(|a| is_image(&a.mime)).filter_map(|a| a.path.clone()).collect();
    let attachment_dirs: Vec<String> = with_path.iter().filter_map(|a| a.path.as_deref().map(dir_of)).collect();
    let lean_base = format!("{{tmp}}/{LEAN_FILE}");
    // the long-thread hooks when the run names its offload folder (wisp.threads, ADR-0097)
    let wisp = run
        .env
        .get(wisp_runtime_api::points::OFFLOAD_DIR_ENV)
        .filter(|d| !d.is_empty())
        .and_then(|_| std::env::current_exe().ok())
        .map(|p| p.display().to_string());
    let a = CodexArgs {
        mode: mode(run.mode),
        model: &run.model,
        effort: &run.effort,
        dirs: &run.dirs,
        resume_id: resume,
        fork: run.fork,
        capped: run.capped,
        network: run.side,
        lean_base: run.lean.then_some(lean_base.as_str()),
        developer,
        mcp_servers: Some(&run.mcp_servers),
        images: &images,
        attachment_dirs: &attachment_dirs,
        hooks: wisp.as_deref(),
    };
    let mut input = if developer.is_some() || run.system_prompt.trim().is_empty() {
        run.prompt.clone()
    } else {
        with_protocol(&run.prompt, &run.system_prompt, resume)
    };
    let notes: Vec<FileNote> = with_path
        .iter()
        .filter(|a| !is_image(&a.mime))
        .map(|a| FileNote { path: a.path.clone().unwrap_or_default(), mime: a.mime.clone(), size: a.size })
        .collect();
    input.push_str(&attachment_note(&notes, Some("Read text files and PDFs from disk")));
    if !images.is_empty() {
        let n = images.len();
        let _ = write!(input, "\n\n({n} image{} attached to this message.)", if n > 1 { "s are" } else { " is" });
    }
    Launch {
        program: bin.to_owned(),
        args: args(&a),
        cwd: run.cwd.clone(),
        stdin: Stdin::Bytes(input.into_bytes()),
        // its MCP servers' secrets (wisp's token, a connector's env) in its environment, which only this user reads,
        // never on argv (ADR-0116, audit I IS3)
        env: mcp_env(Some(&run.mcp_servers)),
        files: if run.lean { vec![LaunchFile { path: LEAN_FILE.into(), contents: LEAN_BASE.into() }] } else { Vec::new() },
        ..Launch::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_runtime_api::model::Attachment;

    fn run() -> HarnessRun {
        HarnessRun {
            prompt: "fix the add bug".into(),
            cwd: "/w".into(),
            system_prompt: "# You are running inside wisp".into(),
            mode: Mode::Edits,
            ..HarnessRun::default()
        }
    }

    fn stdin(l: &Launch) -> String {
        match &l.stdin {
            Stdin::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
            _ => String::new(),
        }
    }

    /// Audit user-side-full-access: a side discussion's run is never read-only: workspace-write with the network on, or
    /// Full auto's bypass when its main task runs in Full auto; a channel's cap still keeps the network off.
    #[test]
    fn a_side_discussions_run_writes_and_reaches_the_network() {
        let side = HarnessRun { side: true, ..run() };
        let a = launch(&side, "codex", true).args;
        assert!(!a.iter().any(|x| x.contains("read-only")), "{a:?}");
        assert!(a.iter().any(|x| x == r#"sandbox_mode="workspace-write""#), "{a:?}");
        assert!(a.iter().any(|x| x == "sandbox_workspace_write.network_access=true"), "{a:?}");
        let full = launch(&HarnessRun { mode: Mode::Auto, ..side.clone() }, "codex", true).args;
        assert!(full.iter().any(|x| x == "--dangerously-bypass-approvals-and-sandbox"));
        assert!(!full.iter().any(|x| x.contains("read-only") || x.contains("network_access")));
        let capped = launch(&HarnessRun { capped: true, ..side }, "codex", true).args;
        assert!(capped.iter().any(|x| x == "sandbox_workspace_write.network_access=false"));
        let plain = launch(&run(), "codex", true).args;
        assert!(!plain.iter().any(|x| x.contains("network_access")), "a task's own run is as it was");
    }

    /// ADR-0097: a Full auto run that names its offload folder gets the long-thread hooks; without the folder, none.
    #[test]
    fn a_full_auto_run_with_an_offload_folder_gets_the_hooks() {
        let mut r = HarnessRun { mode: Mode::Auto, ..run() };
        assert!(!launch(&r, "codex", true).args.iter().any(|a| a.starts_with("hooks.")));
        r.env.insert(wisp_runtime_api::points::OFFLOAD_DIR_ENV.into(), "/h/threads/out/t1".into());
        let args = launch(&r, "codex", true).args;
        assert!(args.iter().any(|a| a.starts_with("hooks.PreToolUse=") && a.contains("hook offload --codex")), "{args:?}");
        assert!(args.iter().any(|a| a.starts_with("hooks.PostCompact=") && a.contains("hook compacted")));
        r.mode = Mode::Edits;
        assert!(!launch(&r, "codex", true).args.iter().any(|a| a.contains("hook-trust")), "not outside Full auto");
    }

    #[test]
    fn the_protocol_goes_as_developer_instructions_or_ahead_of_the_prompt() {
        let l = launch(&run(), "codex", true);
        assert!(l.args.iter().any(|a| a.starts_with("developer_instructions=\"# You are running inside wisp")));
        assert_eq!(stdin(&l), "fix the add bug");
        let old = launch(&run(), "codex", false);
        assert!(stdin(&old).contains("running inside wisp\n\n---\n\n# Task\n\nfix the add bug"));
        let mut r = run();
        r.resume_id = Some("prev-1".into());
        let resumed = launch(&r, "codex", true);
        assert_eq!(stdin(&resumed), "fix the add bug");
        assert!(resumed.args.ends_with(&["resume".to_owned(), "prev-1".to_owned()]));
    }

    #[test]
    fn attachments_images_and_lean() {
        let mut r = run();
        r.attachments = vec![
            Attachment { path: Some("/up/a.png".into()), mime: "image/png".into(), size: 10, ..Attachment::default() },
            Attachment { path: Some("/docs/b.pdf".into()), mime: "application/pdf".into(), size: 2048, ..Attachment::default() },
        ];
        r.lean = true;
        let l = launch(&r, "codex", false);
        assert!(l.args.windows(2).any(|w| w == ["-i", "/up/a.png"]));
        assert!(l.args.windows(2).any(|w| w == ["--add-dir", "/docs"]));
        assert!(l.args.iter().any(|a| a == "model_instructions_file=\"{tmp}/codex-base.md\""));
        assert_eq!(l.files.len(), 1);
        let s = stdin(&l);
        assert!(s.contains("/docs/b.pdf") && s.ends_with("(1 image is attached to this message.)"), "{s}");
    }
}
