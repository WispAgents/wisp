//! wisp as code from the terminal (ADR-0026): `wisp apply -f`, `diff -f`, `validate -f`, `wisp schema`,
//! `wisp export [kinds…]`. The formula is read here (files, URLs, `gist:<id>`, includes, profiles,
//! `${env:…}`) with wisp-resources-core; the plan, the checks and the apply are wisp.resources' commands,
//! so the GUI, the SDK and a grid deploy get the same answers.
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wisp_cli_core::term::{self, BOLD, DIM, GREEN, RED, YELLOW, paint};
use wisp_resources_api::{KindInfo, Plan, Resource};
use wisp_resources_core::export;
use wisp_resources_core::formula::{self, Format, Loaded, Source};

use crate::client::{CliError, Client, Result};
use crate::scripted::say;

/// The options these commands read from the raw command line.
#[derive(Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)] // one per flag
pub struct Opts {
    pub files: Vec<String>,
    pub formulas: Vec<String>,
    pub profiles: Vec<String>,
    pub include: Vec<String>,
    pub format: Option<String>,
    pub prune: bool,
    /// `--prune-all`: prune whatever applied it, not only this formula's source (AAC-1)
    pub prune_all: bool,
    pub trust: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
    pub from: Option<String>,
    pub steps: Option<usize>,
    pub fill: bool,
    pub last: Option<usize>,
    pub project: Option<String>,
    pub wait: bool,
    pub crew: Option<String>,
    pub workflow: Option<String>,
    /// `--since 7d` (or 12h, 30m, a date)
    pub since: Option<String>,
    pub level: Option<String>,
    pub to: Option<String>,
    pub out: Option<String>,
    /// `wisp open`: what to do with it (run, theirs, import, compare)
    pub action: Option<String>,
    /// `--agent`, `--model`, `--effort`, `--mode`: over the spec
    pub over: serde_json::Map<String, Value>,
    /// the words that aren't options
    pub words: Vec<String>,
    /// `wisp init --here`: make `.wisp/` in a folder that's no git repo or project, without asking
    pub here: bool,
    /// `wisp computer on --grid <grid>`: sign in to that grid first if needed (what the Grid's Add a computer prints)
    pub grid: Option<String>,
    /// `-var k=v` (ADR-0034), in order
    pub vars: Vec<(String, String)>,
    pub var_files: Vec<String>,
    pub single_file: bool,
    pub with_deps: bool,
    pub upgrade: bool,
    pub check: bool,
    pub refresh_only: bool,
    /// `wisp open`: `--in <harness>`, `--print`, `--no-adopt`
    pub open_in: Option<String>,
    pub print: bool,
    pub no_adopt: bool,
    /// `wisp share --to cloud`: `--access anyone|people:a,b|teams:t|password`, `--expires 7d`, `--publish`
    pub access: Option<String>,
    pub expires: Option<String>,
    pub publish: bool,
    pub password: Option<String>,
    /// `wisp shares --revoke <id>`, `--updates`
    pub revoke: Option<String>,
    pub updates: bool,
}

/// Reads `argv` (after the command word) for the options above.
pub fn opts(argv: &[String]) -> Opts {
    let mut o = Opts::default();
    let mut it = argv.iter().peekable();
    while let Some(a) = it.next() {
        let (k, inline) = match a.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_owned(), Some(v.to_owned())),
            _ => (a.clone(), None),
        };
        let val =
            |it: &mut std::iter::Peekable<std::slice::Iter<'_, String>>| inline.clone().or_else(|| it.next().cloned()).unwrap_or_default();
        match k.as_str() {
            "-f" | "--file" => o.files.push(val(&mut it)),
            "--formula" => o.formulas.extend(val(&mut it).split(',').filter(|s| !s.is_empty()).map(str::to_owned)),
            "--profile" => o.profiles.extend(val(&mut it).split(',').map(str::to_owned)),
            "--include" => o.include.extend(val(&mut it).split(',').map(str::to_owned)),
            "--format" => o.format = Some(val(&mut it)),
            "--from" => o.from = Some(val(&mut it)),
            "--steps" => o.steps = val(&mut it).parse().ok(),
            "--last" => o.last = val(&mut it).parse().ok(),
            "--project" => o.project = Some(val(&mut it)),
            "--crew" => o.crew = Some(val(&mut it)),
            "--workflow" => o.workflow = Some(val(&mut it)),
            "--since" => o.since = Some(val(&mut it)),
            "--level" => o.level = Some(val(&mut it)),
            "--to" => o.to = Some(val(&mut it)),
            "--out" | "-o" => o.out = Some(val(&mut it)),
            "--run" => o.action = Some("run".into()),
            "--their-setup" => o.action = Some("theirs".into()),
            "--import" => o.action = Some("import".into()),
            "--compare" => o.action = Some("compare".into()),
            // `--factory` alone is the built-in `default` (ADR-0114); a word that can't be a name is the prompt's
            "--factory" => {
                let name = inline.clone().or_else(|| it.next_if(|v| crate::args::factory_name(v)).cloned()).unwrap_or_default();
                o.over.insert("factory".to_owned(), json!(name));
            }
            "--agent" | "-a" | "--model" | "--effort" | "--mode" | "-m" => {
                let key = match k.as_str() {
                    "--agent" | "-a" => "agent",
                    "--mode" | "-m" => "mode",
                    x => x.trim_start_matches("--"),
                };
                o.over.insert(key.to_owned(), json!(val(&mut it)));
            }
            "--port" | "-p" | "--cwd" | "-C" | "--on" => {
                let _ = val(&mut it);
            }
            "-var" | "--var" => {
                if let Some((k, v)) = val(&mut it).split_once('=') {
                    o.vars.push((k.to_owned(), v.to_owned()));
                }
            }
            "--var-file" | "-var-file" => o.var_files.push(val(&mut it)),
            "--single-file" => o.single_file = true,
            "--with-deps" => o.with_deps = true,
            "-upgrade" | "--upgrade" => o.upgrade = true,
            "--check" => o.check = true,
            "--refresh-only" | "-refresh-only" => o.refresh_only = true,
            "--in" => o.open_in = Some(val(&mut it)),
            "--grid" => o.grid = Some(val(&mut it)),
            "--here" => o.here = true,
            "--print" => o.print = true,
            "--no-adopt" => o.no_adopt = true,
            "--access" => o.access = Some(val(&mut it)),
            "--expires" => o.expires = Some(val(&mut it)),
            "--publish" => o.publish = true,
            "--password" => o.password = Some(val(&mut it)),
            "--revoke" => o.revoke = Some(val(&mut it)),
            "--updates" => o.updates = true,
            "--prune" => o.prune = true,
            "--prune-all" => {
                o.prune = true;
                o.prune_all = true;
            }
            "--trust" => o.trust = true,
            "--dry-run" => o.dry_run = true,
            "--yes" | "-y" => o.yes = true,
            "--json" => o.json = true,
            "--wait" | "-w" => o.wait = true,
            "--fill-from-snapshot" => o.fill = true,
            // the parser's (main.rs answers it before any command): never a word a command reads as its argument (audit
            // H1: `wisp workspace new --help` made a workspace named `--help`)
            "--help" | "-h" => {}
            _ => o.words.push(a.clone()),
        }
    }
    o
}

/// Files, URLs and `gist:<id>`, relative to the file that includes them.
pub struct Files {
    pub cwd: PathBuf,
}

fn fetch(url: &str) -> std::result::Result<String, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let http = reqwest::Client::new();
            let r = http.get(url).header("user-agent", "wisp").send().await.map_err(|e| format!("{url}: {e}"))?;
            if !r.status().is_success() {
                return Err(format!("{url}: HTTP {}", r.status().as_u16()));
            }
            r.text().await.map_err(|e| format!("{url}: {e}"))
        })
    })
}

impl Source for Files {
    fn load(&mut self, reference: &str, from: Option<&str>) -> std::result::Result<(String, String), String> {
        if let Some(id) = reference.strip_prefix("gist:") {
            let v: Value =
                serde_json::from_str(&fetch(&format!("https://api.github.com/gists/{id}"))?).map_err(|e| format!("gist {id}: {e}"))?;
            let file = v.get("files").and_then(Value::as_object).and_then(|f| f.values().next()).cloned().unwrap_or_default();
            let name = file.get("filename").and_then(Value::as_str).unwrap_or("wisp.yml");
            return Ok((file.get("content").and_then(Value::as_str).unwrap_or_default().to_owned(), format!("gist:{id}/{name}")));
        }
        if reference.starts_with("https://") || reference.starts_with("http://") {
            return Ok((fetch(reference)?, reference.to_owned()));
        }
        if let Some(f) = from.filter(|f| f.starts_with("http")) {
            let base = f.rsplit_once('/').map_or(f, |(b, _)| b);
            let url = format!("{base}/{reference}");
            return Ok((fetch(&url)?, url));
        }
        let dir = from.map_or_else(|| self.cwd.clone(), |f| Path::new(f).parent().map_or_else(|| self.cwd.clone(), Path::to_path_buf));
        let p = if Path::new(reference).is_absolute() { PathBuf::from(reference) } else { dir.join(reference) };
        let text = std::fs::read_to_string(&p).map_err(|e| crate::words::unreadable(&p, &e))?;
        Ok((text, p.display().to_string()))
    }
    fn env(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

pub async fn kinds(c: &Client) -> Result<Vec<KindInfo>> {
    let v = c.cmd("resources.kinds", json!({})).await?;
    serde_json::from_value(v).map_err(|e| CliError::said(format!("resources.kinds: {e}")))
}

/// The kinds a formula is read against: those there are and those put away (each with its reason), so a section that
/// names one is refused saying why (audit E6: "no plugin owns `router:`" while Auto was off).
pub async fn formula_kinds(c: &Client) -> Result<Vec<KindInfo>> {
    let v = c.cmd("resources.kinds", json!({ "away": true })).await?;
    serde_json::from_value(v).map_err(|e| CliError::said(format!("resources.kinds: {e}")))
}

/// Every `-f` (or `--formula`) file read and evaluated as `.wisp/` is (ADR-0111): `${kind.name}` references,
/// `variables` (`-var`, `--var-file`, and `WISP_VAR_<name>` for the ones it declares), `locals`, `count`, `modules`.
pub fn load_evaluated(files: &[String], o: &Opts, kinds: &[KindInfo], cwd: &Path) -> Result<Loaded> {
    let mut loaded = load(files, &o.profiles, kinds, cwd)?;
    let declared: Vec<String> = loaded
        .meta
        .iter()
        .flat_map(|(_, m)| m.get("variables").and_then(Value::as_object).into_iter().flat_map(|v| v.keys().cloned()))
        .collect();
    let mut vars = serde_json::Map::new();
    for (k, v) in std::env::vars() {
        if let Some(name) = k.strip_prefix("WISP_VAR_").filter(|n| declared.iter().any(|d| d == n)) {
            vars.insert(name.to_owned(), json!(v));
        }
    }
    for f in &o.var_files {
        let text = std::fs::read_to_string(cwd.join(f)).map_err(|e| CliError::said(crate::words::unreadable(&cwd.join(f), &e)))?;
        for doc in formula::documents(&text, formula::format_of(f)).map_err(CliError::said)? {
            vars.extend(doc.as_object().cloned().unwrap_or_default());
        }
    }
    for (k, v) in &o.vars {
        vars.insert(k.clone(), json!(v));
    }
    let root = files.first().map_or_else(|| cwd.to_owned(), |f| cwd.join(f).parent().map_or_else(|| cwd.to_owned(), Path::to_path_buf));
    let c = wisp_resources_core::module::evaluate(&loaded, kinds, vars, "default", &mut crate::folder::env_at(&root))
        .map_err(CliError::said)?;
    loaded.resources = c.resources;
    Ok(loaded)
}

/// Every `-f` file read, in order (a later file patches an earlier one's resources).
pub fn load(files: &[String], profiles: &[String], kinds: &[KindInfo], cwd: &Path) -> Result<Loaded> {
    let mut all = Loaded::default();
    for f in files {
        let got = formula::load(f, &mut Files { cwd: cwd.to_owned() }, kinds, profiles).map_err(CliError::said)?;
        for r in got.resources {
            if let Some(had) = all.resources.iter_mut().find(|x| x.kind == r.kind && x.name == r.name) {
                had.spec = wisp_resources_core::plan::merge(&had.spec, &r.spec);
            } else {
                all.resources.push(r);
            }
        }
        all.sources.extend(got.sources);
        all.secrets.extend(got.secrets);
        all.meta.extend(got.meta);
        for k in got.kinds {
            if !all.kinds.contains(&k) {
                all.kinds.push(k);
            }
        }
    }
    Ok(all)
}

fn need_files(o: &Opts, verb: &str) -> Result<()> {
    if o.files.is_empty() {
        return Err(CliError::said(format!("wisp {verb} -f wisp.yml (one -f per formula)")));
    }
    Ok(())
}

/// The plan, as the terminal shows it.
/// `wisp validate`'s answer: each problem, each secret that isn't set here with how to set it; 0 when there's none.
fn show_validate(plan: &Plan, loaded: &Loaded, st: &wisp_cli_api::Style) -> i32 {
    for p in &plan.problems {
        say(&format!("  {} {p}", paint(st, RED, "✗")));
    }
    for s in &plan.missing_secrets {
        say(&format!("  {} secret {s} isn't set here (wisp secret set {s})", paint(st, RED, "✗")));
    }
    if plan.problems.is_empty() && plan.missing_secrets.is_empty() {
        let n = loaded.resources.len();
        let what = if n == 1 { "1 resource".to_owned() } else { format!("{n} resources") }; // audit G17: "1 resources"
        say(&format!("  {} {what} in {} (wisp: 1)", paint(st, GREEN, "✓"), loaded.sources.join(", ")));
        return 0;
    }
    1
}

pub fn show_plan(p: &Plan, st: &wisp_cli_api::Style) {
    for (mark, line) in wisp_resources_core::plan::lines(&p.ops) {
        let painted = match mark {
            '+' => format!("  {} {}", paint(st, GREEN, "+"), line),
            '-' => format!("  {} {}", paint(st, RED, "-"), line),
            '~' if p.drift.contains(&line) => {
                format!("  {} {}  {}", paint(st, YELLOW, "~"), line, paint(st, DIM, "(drift: changed in wisp since the last apply)"))
            }
            '~' => format!("  {} {}", paint(st, YELLOW, "~"), line),
            _ => paint(st, DIM, &format!("  {line}")),
        };
        say(&term::truncate(&painted, st.width.max(40)));
    }
    let (mut add, mut change, mut del) = (0, 0, 0);
    for o in &p.ops {
        match o.op {
            wisp_resources_api::OpKind::Create => add += 1,
            wisp_resources_api::OpKind::Update => change += 1,
            wisp_resources_api::OpKind::Delete => del += 1,
        }
    }
    if p.problems.is_empty() {
        say(&format!("\n  {}", paint(st, BOLD, &format!("{add} to add, {change} to change, {del} to remove, {} unchanged", p.unchanged))));
    } else {
        // a formula with problems applies nothing: never "1 to add" above the reason it can't (audit A #14)
        let n = p.problems.len();
        say(&format!(
            "\n  {}",
            paint(
                st,
                BOLD,
                &format!(
                    "{n} problem{}: nothing can be applied until {} fixed",
                    if n == 1 { "" } else { "s" },
                    if n == 1 { "it's" } else { "they're" }
                )
            )
        ));
    }
    for k in &p.kept {
        say(&format!("  {}", paint(st, DIM, &format!("= {k}  (not from this formula: kept)"))));
    }
    if !p.stale.is_empty() {
        say(&format!(
            "  {} {} no longer declared here: --prune removes them ({})",
            paint(st, YELLOW, "!"),
            p.stale.len(),
            p.stale.join(", ")
        ));
    }
    for s in &p.sensitive {
        say(&format!("  {} {}", paint(st, YELLOW, "!"), s));
    }
    for s in &p.missing_secrets {
        say(&format!("  {} secret {s} isn't set here (wisp secret set {s})", paint(st, RED, "✗")));
    }
    for s in &p.problems {
        say(&format!("  {} {s}", paint(st, RED, "✗")));
    }
}

fn ask(question: &str) -> String {
    let mut o = std::io::stdout().lock();
    let _ = write!(o, "{question}");
    let _ = o.flush();
    read_answer()
}

/// [`ask`] on stderr: `--json`'s stdout is the JSON alone.
fn ask_err(question: &str) -> String {
    let mut e = std::io::stderr().lock();
    let _ = write!(e, "{question}");
    let _ = e.flush();
    read_answer()
}

fn read_answer() -> String {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
}

/// Paths in specs that name a file of the formula's own (a skill's `path`) made absolute from `base`: the server
/// reads them, from wherever it runs. URLs and absolute paths stay as they are.
pub fn anchor_paths(resources: &mut [wisp_resources_api::Resource], base: &Path) {
    for r in resources.iter_mut().filter(|r| r.kind == "skill") {
        let Some(p) = r.spec.get("path").and_then(Value::as_str).filter(|p| !p.is_empty() && !p.contains("://")) else { continue };
        let p = Path::new(p);
        // rooted is absolute enough (`/x` on Windows has no drive but isn't the formula's)
        if !p.has_root() && p.is_relative() {
            r.spec["path"] = Value::String(base.join(p).display().to_string());
        }
    }
}

/// Where a formula comes from, as each apply records it (AAC-1): the `.wisp/` folder (canonical) with its workspace, or
/// the sorted set of its `-f` files (canonical; a URL or gist as written). The same formula gives the same key from any
/// cwd, so `--prune` removes what it applied before and nothing another repo or file did.
pub fn source_key(dir: Option<&Path>, workspace: &str, files: &[String], cwd: &Path) -> String {
    let canon = |p: &Path| wisp_proc::canonical_blocking(p).unwrap_or_else(|_| p.components().collect()).display().to_string();
    if let Some(d) = dir {
        return format!("folder:{}#{workspace}", canon(d));
    }
    let mut all: Vec<String> =
        files.iter().map(|f| if f.contains("://") || f.starts_with("gist:") { f.clone() } else { canon(&cwd.join(f)) }).collect();
    all.sort();
    all.dedup();
    format!("files:{}", all.join(","))
}

/// What `--json` does once the plan is in (AAC-8: `wisp apply --json` printed the plan and applied nothing, exit 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonNext {
    /// print the plan and exit with this
    Plan(i32),
    /// ask (on stderr), then apply
    Ask,
    /// ask for `trust` (on stderr), then apply
    AskTrust,
    /// print `{"error": …}`, exit 1
    Refuse(&'static str),
    Apply,
}

/// [`JsonNext`] for `verb` and its plan: validate, plan, diff and `--dry-run` print it; apply and destroy apply it through
/// the same yes, trust and terminal gates as the text path. Every exit is 1 while there are problems or missing secrets.
pub fn json_next(verb: &str, o: &Opts, plan: &Plan, foreign: bool, tty: bool) -> JsonNext {
    let bad = !plan.problems.is_empty() || !plan.missing_secrets.is_empty();
    if matches!(verb, "validate" | "diff" | "plan") || o.dry_run || bad || plan.ops.is_empty() {
        return JsonNext::Plan(i32::from(bad));
    }
    if foreign && !plan.sensitive.is_empty() && !o.trust {
        return if o.yes || !tty {
            JsonNext::Refuse("this formula comes from elsewhere and changes who may act: read the plan, then apply it with --trust")
        } else {
            JsonNext::AskTrust
        };
    }
    match (o.yes, tty) {
        (true, _) => JsonNext::Apply,
        (false, true) => JsonNext::Ask,
        (false, false) => JsonNext::Refuse("not a terminal: apply with --yes"),
    }
}

/// The plan as `--json` prints it, with what the secret scanner hits masked (AAC-4).
fn plan_json(plan: &Plan) -> Value {
    let mut v = serde_json::to_value(plan).unwrap_or_default();
    wisp_resources_core::secrets::mask(&mut v);
    v
}

/// `--json` once the plan is in ([`json_next`]): the plan printed, or applied through the gates (asking on stderr) and
/// printed with how many changes went in.
async fn apply_json(c: &Client, verb: &str, o: &Opts, plan: &Plan, mut args: Value, foreign: bool) -> Result<i32> {
    let tty = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let trust = match json_next(verb, o, plan, foreign, tty) {
        JsonNext::Plan(code) => {
            say(&serde_json::to_string_pretty(&plan_json(plan)).unwrap_or_default());
            return Ok(code);
        }
        JsonNext::Refuse(why) => {
            say(&serde_json::to_string_pretty(&json!({ "error": why, "plan": plan_json(plan) })).unwrap_or_default());
            return Ok(1);
        }
        JsonNext::AskTrust => {
            if ask_err("It comes from elsewhere and changes who may act. Type trust to apply it: ") != "trust" {
                say(&json!({ "error": "nothing applied", "plan": plan_json(plan) }).to_string());
                return Ok(1);
            }
            true
        }
        JsonNext::Ask => {
            let n = plan.ops.len();
            if !matches!(ask_err(&format!("Apply {n} change{}? [y/N] ", if n == 1 { "" } else { "s" })).as_str(), "y" | "Y" | "yes") {
                say(&json!({ "error": "nothing applied", "plan": plan_json(plan) }).to_string());
                return Ok(1);
            }
            o.trust
        }
        JsonNext::Apply => o.trust,
    };
    args["trust"] = json!(trust);
    let done = c.cmd("resources.apply", args).await?;
    say(&serde_json::to_string_pretty(&json!({ "plan": plan_json(plan), "applied": done["applied"] })).unwrap_or_default());
    Ok(0)
}

/// `wisp apply -f`, `wisp diff -f`, `wisp validate -f`.
pub async fn apply(c: &Client, verb: &str, o: &Opts, cwd: &Path) -> Result<i32> {
    let st = crate::cli::style();
    let ks = formula_kinds(c).await?;
    let dir = crate::folder::folder(o, cwd);
    let source = source_key(dir.as_deref(), &dir.as_deref().map(crate::folder::workspace).unwrap_or_default(), &o.files, cwd);
    let loaded = if let Some(d) = &dir {
        let cfg = crate::folder::config(o, &ks, d)?;
        let ws = crate::folder::workspace(d);
        Loaded {
            resources: cfg.resources,
            sources: vec![if ws == "default" { d.display().to_string() } else { format!("{}, workspace {ws}", d.display()) }],
            ..Loaded::default()
        }
    } else {
        need_files(o, verb)?;
        load_evaluated(&o.files, o, &ks, cwd)?
    };
    let mut loaded = loaded;
    // a skill's `path: docs/testing.md` is the repo's (the folder's parent), or next to the -f file: never wherever
    // the server happens to run
    let base = dir
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .or_else(|| o.files.first().and_then(|f| cwd.join(f).parent().map(Path::to_path_buf)));
    anchor_paths(&mut loaded.resources, base.as_deref().unwrap_or(cwd));
    let foreign = formula::foreign(&loaded.sources);
    let destroy: Vec<String> =
        if verb == "destroy" { loaded.resources.iter().map(|r| format!("{}.{}", r.kind, r.name)).collect() } else { Vec::new() };
    let resources = if verb == "destroy" { Vec::new() } else { loaded.resources.clone() };
    let args = json!({
        "resources": resources, "prune": o.prune, "prune_all": o.prune_all, "source": source, "foreign": foreign, "trust": o.trust,
        "destroy": destroy, "kinds": loaded.kinds,
    });
    let plan: Plan = serde_json::from_value(c.cmd("resources.plan", args.clone()).await?).map_err(|e| CliError::said(e.to_string()))?;
    if o.json {
        return apply_json(c, verb, o, &plan, args, foreign).await;
    }
    if verb == "validate" {
        return Ok(show_validate(&plan, &loaded, &st));
    }
    say(&format!(
        "{} {}",
        paint(&st, BOLD, "Plan"),
        paint(&st, DIM, &format!("({}{})", loaded.sources.join(", "), if foreign { ", from elsewhere" } else { "" }))
    ));
    show_plan(&plan, &st);
    if o.refresh_only {
        let done = c.cmd("resources.refresh", json!({ "resources": plan.drift })).await?;
        let n = done.as_array().map_or(0, Vec::len);
        say(&format!(
            "  {} {n} drifted resource{} taken as wisp has {} (wisp export writes them into the files)",
            paint(&st, GREEN, "✓"),
            if n == 1 { "" } else { "s" },
            if n == 1 { "it" } else { "them" }
        ));
        return Ok(0);
    }
    if matches!(verb, "diff" | "plan") || o.dry_run || plan.ops.is_empty() {
        if plan.ops.is_empty() && !matches!(verb, "diff" | "plan") {
            say(&format!("  {} nothing to change", paint(&st, GREEN, "✓")));
        }
        prune_hint(o, &plan, &loaded, verb, &st);
        return Ok(i32::from(!plan.problems.is_empty()));
    }
    if !plan.problems.is_empty() || !plan.missing_secrets.is_empty() {
        return Ok(1);
    }
    let mut trust = o.trust;
    let sensitive = foreign && !plan.sensitive.is_empty();
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if sensitive && !trust {
        if o.yes || !tty {
            eprintln!("  ✗ this formula comes from elsewhere and changes who may act: read the plan, then apply it with --trust");
            return Ok(1);
        }
        trust = ask("\n  It comes from elsewhere and changes who may act. Type trust to apply it: ") == "trust";
        if !trust {
            say("  nothing applied");
            return Ok(1);
        }
    } else if !o.yes {
        if !tty {
            eprintln!("  ✗ not a terminal: apply with --yes");
            return Ok(1);
        }
        let a = ask(&format!("\n  Apply {} change{}? [y/N] ", plan.ops.len(), if plan.ops.len() == 1 { "" } else { "s" }));
        if !matches!(a.as_str(), "y" | "Y" | "yes") {
            say("  nothing applied");
            return Ok(1);
        }
    }
    let mut args = args;
    args["trust"] = json!(trust);
    let done = c.cmd("resources.apply", args).await?;
    say(&format!("  {} applied {} change{}", paint(&st, GREEN, "✓"), done["applied"], if done["applied"] == 1 { "" } else { "s" }));
    Ok(0)
}

/// --prune empties only the sections the formula names: one deleted whole names nothing, so say how (audit A #15).
fn prune_hint(o: &Opts, plan: &Plan, loaded: &Loaded, verb: &str, st: &wisp_cli_api::Style) {
    if o.prune_all && verb != "destroy" {
        say(&format!("  {}", paint(st, DIM, "--prune-all: what the formula doesn't declare goes, whichever formula, repo or app made it")));
    }
    if o.prune && plan.ops.is_empty() && loaded.kinds.is_empty() && verb != "destroy" {
        say(&format!(
            "  {}",
            paint(st, DIM, "--prune removes what a section no longer lists: keep the section (triggers: []) or wisp destroy <kind>.<name>")
        ));
    }
}

/// `wisp export [kinds…] [--include ui,account] [--format yaml|json|toml]`.
/// The format `--format` names (audit C5: `xml` wrote YAML and exited 0): yaml when unset, else one wisp writes.
pub fn export_format(asked: Option<&str>) -> std::result::Result<Format, String> {
    match asked.map(str::to_ascii_lowercase).as_deref() {
        None | Some("yaml" | "yml") => Ok(Format::Yaml),
        Some("json") => Ok(Format::Json),
        Some("toml") => Ok(Format::Toml),
        Some(other) => Err(format!("no format {other}: yaml, json or toml")),
    }
}

/// The words after `export` that name no kind, said with the kinds there are (audit C5); None: every one is a kind.
pub fn unknown_kinds(words: &[String], ks: &[KindInfo]) -> Option<String> {
    let bad: Vec<&str> = words.iter().map(String::as_str).filter(|w| !w.contains('.') && !ks.iter().any(|k| k.named(w))).collect();
    if bad.is_empty() {
        return None;
    }
    let mut known: Vec<&str> = ks.iter().map(|k| k.section.as_str()).collect();
    known.sort_unstable();
    known.dedup();
    Some(format!("no kind {}: {}", bad.join(", "), known.join(", ")))
}

pub async fn export_cmd(c: &Client, o: &Opts) -> Result<i32> {
    let format = match export_format(o.format.as_deref()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  ✗ {e}");
            return Ok(crate::cli::USAGE_EXIT);
        }
    };
    let ks = kinds(c).await?;
    let kinds_asked: Vec<&String> = o.words.iter().filter(|w| !w.contains('.')).collect();
    let list: Vec<Resource> =
        serde_json::from_value(c.cmd("resources.export", json!({ "kinds": kinds_asked, "include": o.include })).await?)
            .map_err(|e| CliError::said(e.to_string()))?;
    let mut list = if o.with_deps || o.words.iter().any(|w| w.contains('.')) {
        let all: Vec<Resource> = serde_json::from_value(c.cmd("resources.export", json!({ "include": o.include })).await?)
            .map_err(|e| CliError::said(e.to_string()))?;
        crate::folder::with_deps(&all, &o.words)
    } else {
        list
    };
    // a token pasted into a spec never leaves in an export (AAC-4): `wisp secret set` and `${secret:NAME}` are the way
    for r in &mut list {
        wisp_resources_core::secrets::mask(&mut r.spec);
    }
    if let Some(dir) = &o.out {
        let written = crate::folder::write_folder(&list, &ks, Path::new(dir), o.single_file)?;
        say(&format!("  {} {} resources → {dir}/ ({})", paint(&crate::cli::style(), GREEN, "✓"), list.len(), written.join(", ")));
        return Ok(0);
    }
    let text = export::write(&list, &ks, format);
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    Ok(0)
}

/// `wisp schema`: the JSON Schema of a formula for editors (`# yaml-language-server: $schema=…`).
pub async fn schema(c: &Client) -> Result<i32> {
    let ks = kinds(c).await?;
    say(&serde_json::to_string_pretty(&wisp_resources_core::schema::formula(&ks)).unwrap_or_default());
    Ok(0)
}

/// Whether `words` after `export` name kinds (`wisp export crew agents`), not a task ("export the csv").
pub fn kind_words(words: &[String], ks: &[KindInfo]) -> bool {
    words.iter().all(|w| ks.iter().any(|k| k.named(w)))
}

#[cfg(test)]
mod anchor_tests {
    use super::*;

    #[test]
    fn a_skills_relative_path_is_the_formulas_own() {
        let mut rs = vec![
            wisp_resources_api::Resource::new("skill", "a", serde_json::json!({ "path": "docs/testing.md" })),
            wisp_resources_api::Resource::new("skill", "b", serde_json::json!({ "path": "/abs/SKILL.md" })),
            wisp_resources_api::Resource::new("skill", "c", serde_json::json!({ "body": "inline" })),
            wisp_resources_api::Resource::new("agent", "d", serde_json::json!({ "path": "x" })),
        ];
        anchor_paths(&mut rs, Path::new("/repo"));
        assert_eq!(rs[0].spec["path"], Path::new("/repo").join("docs/testing.md").display().to_string());
        assert_eq!(rs[1].spec["path"], "/abs/SKILL.md");
        assert!(rs[2].spec.get("path").is_none());
        assert_eq!(rs[3].spec["path"], "x", "only a skill's path");
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;

    fn plan_with(missing: &[&str], ops: usize) -> Plan {
        let op = wisp_resources_api::Op {
            op: wisp_resources_api::OpKind::Create,
            kind: "agent".into(),
            name: "a".into(),
            spec: json!({}),
            id: None,
            changes: Vec::new(),
            before: None,
        };
        Plan { missing_secrets: missing.iter().map(|s| (*s).to_owned()).collect(), ops: vec![op; ops], ..Plan::default() }
    }

    /// AAC-8: `wisp apply --json` printed the plan and returned before applying; `validate --json` exited 0 with a
    /// secret missing. Apply and destroy go through the gates and apply; every exit counts missing secrets.
    #[test]
    fn json_applies_through_the_same_gates_and_counts_missing_secrets_aac_8() {
        let yes = Opts { yes: true, ..Opts::default() };
        let no = Opts::default();
        assert_eq!(json_next("validate", &no, &plan_with(&["K"], 0), false, false), JsonNext::Plan(1), "a missing secret is 1");
        assert_eq!(json_next("validate", &no, &plan_with(&[], 1), false, false), JsonNext::Plan(0));
        assert_eq!(json_next("plan", &yes, &plan_with(&[], 1), false, true), JsonNext::Plan(0), "plan never applies");
        assert_eq!(json_next("apply", &yes, &plan_with(&[], 1), false, false), JsonNext::Apply);
        assert_eq!(json_next("destroy", &yes, &plan_with(&[], 1), false, false), JsonNext::Apply);
        assert_eq!(json_next("apply", &yes, &plan_with(&["K"], 1), false, false), JsonNext::Plan(1), "missing secret: no apply");
        assert_eq!(json_next("apply", &Opts { dry_run: true, ..yes.clone() }, &plan_with(&[], 1), false, false), JsonNext::Plan(0));
        assert_eq!(json_next("apply", &no, &plan_with(&[], 1), false, false), JsonNext::Refuse("not a terminal: apply with --yes"));
        assert_eq!(json_next("apply", &no, &plan_with(&[], 1), false, true), JsonNext::Ask);
        let sensitive = Plan { sensitive: vec!["lets chat 1 start tasks".into()], ..plan_with(&[], 1) };
        assert!(matches!(json_next("apply", &yes, &sensitive, true, false), JsonNext::Refuse(_)), "foreign needs --trust");
        assert_eq!(json_next("apply", &Opts { trust: true, ..yes }, &sensitive, true, false), JsonNext::Apply);
    }

    /// AAC-1: a formula's source is the same from any cwd and in any -f order; a folder's names its workspace.
    #[test]
    fn a_formulas_source_is_stable_aac_1() {
        let tmp = std::env::temp_dir();
        let a = source_key(None, "", &["b.yml".into(), "a.yml".into()], &tmp);
        assert_eq!(a, source_key(None, "", &["a.yml".into(), "./b.yml".into(), "a.yml".into()], &tmp));
        assert!(a.starts_with("files:") && a.contains("a.yml,") && a.ends_with("b.yml"), "{a}");
        assert_eq!(source_key(None, "", &["https://x.dev/t.yml".into()], &tmp), "files:https://x.dev/t.yml");
        let f = source_key(Some(&tmp), "staging", &[], &tmp);
        assert!(f.starts_with("folder:") && f.ends_with("#staging"), "{f}");
        assert_ne!(f, source_key(Some(&tmp), "default", &[], &tmp));
    }
}

#[cfg(test)]
mod export_ask_tests {
    use super::*;

    /// Audit C5: `wisp export --format xml` wrote YAML and exited 0. A format wisp can't write is named, with the ones
    /// it can.
    #[test]
    fn an_unknown_export_format_is_named_with_the_ones_there_are() {
        assert!(matches!(export_format(None), Ok(Format::Yaml)));
        assert!(matches!(export_format(Some("json")), Ok(Format::Json)));
        assert!(matches!(export_format(Some("YAML")), Ok(Format::Yaml)));
        assert!(matches!(export_format(Some("yml")), Ok(Format::Yaml)));
        assert_eq!(export_format(Some("xml")).err().as_deref(), Some("no format xml: yaml, json or toml"));
    }

    /// Audit C5: `wisp export nosuchkind` printed only the usage line. A word that's no kind is named, with the kinds.
    #[test]
    fn an_unknown_kind_is_named_with_the_kinds_there_are() {
        let ks = vec![KindInfo::new("crew", "crew", "wisp.crew", json!({})), KindInfo::new("agent", "agents", "wisp.runtime", json!({}))];
        let w = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(unknown_kinds(&w(&["crew", "agents"]), &ks), None);
        assert_eq!(unknown_kinds(&w(&["nosuchkind"]), &ks).as_deref(), Some("no kind nosuchkind: agents, crew"));
        assert_eq!(unknown_kinds(&w(&["crew", "nope", "zap"]), &ks).as_deref(), Some("no kind nope, zap: agents, crew"));
    }
}
