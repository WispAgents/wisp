//! The model Claude Code's Default runs (UX 2026-11-18 #9: the chip says "Claude Code · Opus", not "Default"), as its own
//! config says, never a run: `ANTHROPIC_MODEL`, else the project's settings (`.claude/settings.local.json`, then
//! `.claude/settings.json`), else the user's (`~/.claude/settings.json` `model`, or `$CLAUDE_CONFIG_DIR/settings.json`).
//! An alias or a full model id, named as the model list names it. Unset (or `default`): unknown, the plan picks it, and
//! the chip says Default... unless the plan says (QA r6 desktop #4): with nothing configured, Claude Code's `default` is
//! its plan's, and `claude auth status` (local, no paid call) names the plan: Max runs Opus, Pro Sonnet. Asked once
//! per detect and remembered ([`Plan`]).

use std::sync::Mutex;

use serde_json::Value;
use wisp_runtime_api::harness::{ExecReq, HarnessSys};

/// A model as Claude Code takes it (`opus`, `sonnet[1m]`, `claude-opus-5-5`, `opusplan`), by its family's name.
pub fn label_of(model: &str) -> Option<String> {
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() || m == "default" {
        return None;
    }
    // opusplan: Opus to plan, Sonnet to work; the name says which runs the plan
    let family = ["fable", "opus", "sonnet", "haiku"].into_iter().find(|f| m.contains(f));
    Some(match family {
        Some(f) => {
            let mut c = f.chars();
            c.next().map_or_else(String::new, |h| h.to_ascii_uppercase().to_string() + c.as_str())
        }
        // a name we don't know, as written: a short one only (a repo's settings could put anything on the chip)
        None => Some(model.trim()).filter(|n| !n.is_empty() && n.chars().count() <= 24)?.to_owned(),
    })
}

/// What Default runs in `cwd` (a task's folder, the picked project), as Claude Code itself takes it: `ANTHROPIC_MODEL`,
/// else the project's `.claude/settings.local.json`, then its `.claude/settings.json`, then this computer's settings
/// (owner's delegate 2026-11-18: a project that pins a model is what runs there). None when nothing names it.
pub async fn resolved(sys: &dyn HarnessSys, cwd: Option<&str>) -> Option<String> {
    if let Some(m) = sys.env("ANTHROPIC_MODEL").filter(|m| !m.trim().is_empty()) {
        return label_of(&m);
    }
    let user = sys.env("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).unwrap_or_else(|| format!("{}/.claude", sys.home()));
    let project = cwd.map(|c| c.trim_end_matches(['/', '\\'])).filter(|c| !c.is_empty());
    let files = project
        .into_iter()
        .flat_map(|p| [format!("{p}/.claude/settings.local.json"), format!("{p}/.claude/settings.json")])
        .chain([format!("{user}/settings.json")]);
    for f in files {
        if let Some(m) = named(sys, &f).await {
            return label_of(&m);
        }
    }
    None
}

/// What Claude Code's `default` runs on the plan `claude auth status` prints (its model docs: Max runs Opus, Pro
/// Sonnet). Another plan (Team, Enterprise: set by the org), a cloud provider (Bedrock, Vertex: its own default) or no
/// sign-in: None, the chip says Default rather than guess.
pub fn of_plan(status: &str) -> Option<String> {
    let j: Value = serde_json::from_str(&status[status.find('{')?..]).ok()?;
    if j["loggedIn"].as_bool() != Some(true) || j["apiProvider"].as_str().is_some_and(|p| p != "firstParty") {
        return None;
    }
    match j["subscriptionType"].as_str()? {
        "max" => Some("Opus".into()),
        "pro" => Some("Sonnet".into()),
        _ => None,
    }
}

/// The plan's Default, asked once (`claude auth status --json`, a local read) and remembered until the next detect.
#[derive(Debug, Default)]
pub struct Plan(Mutex<Option<Asked>>);

/// The plan's answer once asked: the model's name, or None when it can't tell.
#[derive(Debug, Clone)]
struct Asked(Option<String>);

impl Plan {
    /// What Default runs in `cwd`: Claude Code's config ([`resolved`]), else its plan's.
    pub async fn or_config(&self, sys: &dyn HarnessSys, bin: &str, cwd: Option<&str>) -> Option<String> {
        if let Some(m) = resolved(sys, cwd).await {
            return Some(m);
        }
        if let Some(Asked(known)) = self.0.lock().ok().and_then(|p| p.clone()) {
            return known;
        }
        let req = ExecReq {
            program: bin.to_owned(),
            args: vec!["auth".into(), "status".into(), "--json".into()],
            timeout_ms: 10_000,
            ..ExecReq::default()
        };
        let named = match sys.exec(req).await {
            Ok(o) if o.code == Some(0) => of_plan(&o.stdout),
            _ => None,
        };
        if let Ok(mut p) = self.0.lock() {
            *p = Some(Asked(named.clone()));
        }
        named
    }

    /// Detect runs again (a sign-in, an update): ask the plan again on the next look.
    pub fn forget(&self) {
        if let Ok(mut p) = self.0.lock() {
            *p = None;
        }
    }
}

/// The `model` a settings file names, if it's there, parses and names one.
async fn named(sys: &dyn HarnessSys, file: &str) -> Option<String> {
    let settings: Value = serde_json::from_str(&sys.read_file(file).await?).ok()?;
    settings.get("model")?.as_str().map(str::to_owned)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use wisp_kernel::{BoxFuture, Result};
    use wisp_runtime_api::harness::{ExecOut, ExecReq, HarnessSys};

    use super::{Plan, label_of, of_plan, resolved};

    #[derive(Default)]
    struct Fake {
        files: BTreeMap<String, String>,
        env: BTreeMap<String, String>,
        /// what `claude auth status` prints
        status: Option<String>,
        execs: std::sync::Mutex<Vec<String>>,
    }
    impl HarnessSys for Fake {
        fn exec(&self, req: ExecReq) -> BoxFuture<'_, Result<ExecOut>> {
            self.execs.lock().unwrap().push(format!("{} {}", req.program, req.args.join(" ")));
            let out = self.status.clone().map_or_else(
                || ExecOut { code: Some(1), ..ExecOut::default() },
                |s| ExecOut { code: Some(0), stdout: s, ..ExecOut::default() },
            );
            Box::pin(async move { Ok(out) })
        }
        fn which(&self, _program: &str) -> BoxFuture<'_, Option<String>> {
            Box::pin(async { None })
        }
        fn read_file(&self, path: &str) -> BoxFuture<'_, Option<String>> {
            let v = self.files.get(path).cloned();
            Box::pin(async move { v })
        }
        fn write_file(&self, _path: &str, _contents: String) -> BoxFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn list_dir(&self, _path: &str) -> BoxFuture<'_, Vec<String>> {
            Box::pin(async { Vec::new() })
        }
        fn env(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
        }
        fn home(&self) -> String {
            "/Users/me".into()
        }
        fn platform(&self) -> String {
            "darwin".into()
        }
    }

    #[test]
    fn a_model_is_named_by_its_family() {
        assert_eq!(label_of("opus").as_deref(), Some("Opus"));
        assert_eq!(label_of("claude-sonnet-5-5").as_deref(), Some("Sonnet"));
        assert_eq!(label_of("sonnet[1m]").as_deref(), Some("Sonnet"));
        assert_eq!(label_of("Fable").as_deref(), Some("Fable"));
        assert_eq!(label_of("opusplan").as_deref(), Some("Opus"));
        // something else: its own words
        assert_eq!(label_of("my-proxy-model").as_deref(), Some("my-proxy-model"));
        // a repo's settings can't put a sentence on the chip
        assert_eq!(label_of(&"x".repeat(80)), None);
        // the plan's own pick: unknown
        assert_eq!(label_of("default"), None);
        assert_eq!(label_of(" "), None);
    }

    /// UX 2026-11-18 #9: the chip names what Default runs, from Claude Code's own config, never a paid call
    #[tokio::test]
    async fn default_reads_the_env_then_the_settings_file() {
        let files = BTreeMap::from([("/Users/me/.claude/settings.json".to_owned(), r#"{ "model": "opus", "theme": "dark" }"#.to_owned())]);
        let sys = Fake { files: files.clone(), ..Fake::default() };
        assert_eq!(resolved(&sys, None).await.as_deref(), Some("Opus"));
        // ANTHROPIC_MODEL wins, as it does for Claude Code
        let env = BTreeMap::from([("ANTHROPIC_MODEL".to_owned(), "claude-haiku-5-5".to_owned())]);
        assert_eq!(resolved(&Fake { files: files.clone(), env, ..Fake::default() }, None).await.as_deref(), Some("Haiku"));
        // its config dir moved
        let moved = BTreeMap::from([("/cfg/settings.json".to_owned(), r#"{"model":"sonnet"}"#.to_owned())]);
        let env = BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), "/cfg".to_owned())]);
        assert_eq!(resolved(&Fake { files: moved, env, ..Fake::default() }, None).await.as_deref(), Some("Sonnet"));
        // no settings, no model in them, or a broken file: unknown
        assert_eq!(resolved(&Fake::default(), None).await, None);
        let none = BTreeMap::from([("/Users/me/.claude/settings.json".to_owned(), r#"{"theme":"dark"}"#.to_owned())]);
        assert_eq!(resolved(&Fake { files: none, ..Fake::default() }, None).await, None);
        let broken = BTreeMap::from([("/Users/me/.claude/settings.json".to_owned(), "{ nope".to_owned())]);
        assert_eq!(resolved(&Fake { files: broken, ..Fake::default() }, None).await, None);
    }

    /// Owner's delegate 2026-11-18: a project that pins a model is what Claude Code runs there, so Default names it:
    /// its `.claude/settings.local.json`, then its `.claude/settings.json`, then this computer's; `ANTHROPIC_MODEL` first
    #[tokio::test]
    async fn a_project_that_pins_a_model_names_it_over_this_computers() {
        let user = ("/Users/me/.claude/settings.json".to_owned(), r#"{"model":"sonnet"}"#.to_owned());
        let shared = ("/code/app/.claude/settings.json".to_owned(), r#"{"model":"opus"}"#.to_owned());
        let local = ("/code/app/.claude/settings.local.json".to_owned(), r#"{"model":"haiku"}"#.to_owned());
        let at = |files: Vec<(String, String)>| Fake { files: files.into_iter().collect(), ..Fake::default() };
        // the shared project settings over this computer's
        assert_eq!(resolved(&at(vec![user.clone(), shared.clone()]), Some("/code/app")).await.as_deref(), Some("Opus"));
        // the local ones over the shared
        assert_eq!(resolved(&at(vec![user.clone(), shared.clone(), local.clone()]), Some("/code/app")).await.as_deref(), Some("Haiku"));
        // a project that names none, or another folder: this computer's
        let quiet = ("/code/app/.claude/settings.json".to_owned(), r#"{"permissions":{}}"#.to_owned());
        assert_eq!(resolved(&at(vec![user.clone(), quiet]), Some("/code/app")).await.as_deref(), Some("Sonnet"));
        assert_eq!(resolved(&at(vec![user.clone(), shared.clone()]), Some("/code/other")).await.as_deref(), Some("Sonnet"));
        // a trailing slash is the same folder
        assert_eq!(resolved(&at(vec![user.clone(), shared.clone()]), Some("/code/app/")).await.as_deref(), Some("Opus"));
        // ANTHROPIC_MODEL wins over the project too, as it does for Claude Code
        let mut env = at(vec![user, shared, local]);
        env.env.insert("ANTHROPIC_MODEL".into(), "fable".into());
        assert_eq!(resolved(&env, Some("/code/app")).await.as_deref(), Some("Fable"));
    }

    /// QA r6 desktop #4: nothing on the Mac pins a model, so Claude Code's Default is the plan's: `claude auth status`
    /// (local, free) says which, Max runs Opus and Pro Sonnet; another plan, a cloud provider or no sign-in: unknown
    #[test]
    fn the_plan_names_what_default_runs_when_no_config_does() {
        let max = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","subscriptionType":"max"}"#;
        assert_eq!(of_plan(max).as_deref(), Some("Opus"));
        assert_eq!(of_plan(&max.replace("max", "pro")).as_deref(), Some("Sonnet"));
        assert_eq!(of_plan(&max.replace("\"max\"", "\"enterprise\"")), None);
        assert_eq!(of_plan(&max.replace("firstParty", "bedrock")), None);
        assert_eq!(of_plan(r#"{"loggedIn":false}"#), None);
        assert_eq!(of_plan("not json"), None);
        // a warning line before the JSON
        assert_eq!(of_plan(&format!("note: update available\n{max}")).as_deref(), Some("Opus"));
    }

    /// asked once, then remembered; a config that names a model never asks; detect starts it over
    #[tokio::test]
    async fn the_plan_is_asked_once_and_only_when_no_config_names_a_model() {
        let plan = Plan::default();
        let sys =
            Fake { status: Some(r#"{"loggedIn":true,"apiProvider":"firstParty","subscriptionType":"max"}"#.into()), ..Fake::default() };
        assert_eq!(plan.or_config(&sys, "/bin/claude", None).await.as_deref(), Some("Opus"));
        assert_eq!(plan.or_config(&sys, "/bin/claude", Some("/code/app")).await.as_deref(), Some("Opus"));
        assert_eq!(sys.execs.lock().unwrap().as_slice(), ["/bin/claude auth status --json"]);
        // a project's pin wins and asks nothing
        let pinned = Fake {
            files: BTreeMap::from([("/code/app/.claude/settings.json".to_owned(), r#"{"model":"haiku"}"#.to_owned())]),
            ..Fake::default()
        };
        assert_eq!(Plan::default().or_config(&pinned, "/bin/claude", Some("/code/app")).await.as_deref(), Some("Haiku"));
        assert!(pinned.execs.lock().unwrap().is_empty());
        // signed out: unknown, remembered too, until detect forgets it
        let out = Fake::default();
        let p = Plan::default();
        assert_eq!(p.or_config(&out, "claude", None).await, None);
        assert_eq!(p.or_config(&out, "claude", None).await, None);
        assert_eq!(out.execs.lock().unwrap().len(), 1);
        p.forget();
        assert_eq!(p.or_config(&out, "claude", None).await, None);
        assert_eq!(out.execs.lock().unwrap().len(), 2);
    }
}
