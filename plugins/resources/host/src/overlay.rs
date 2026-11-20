//! `resources.overlay` (ADR-0111): a formula as one task's overlay, nothing applied, and a task resolved under it with
//! every name it gives checked against the formula and this wisp. The CLI sends the resources it read (files, URLs,
//! gists); New task sends a picked file's text; a task's `wisp_launch` names files on this computer. Files are read off
//! the runtime (`spawn_blocking`): a formula, its includes and modules, a task file's `formula:` and `module.x` (the
//! `.wisp/` folder of `cwd`).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value, json};
use wisp_automations_api::tasks::{DEFAULT, TaskDecl, fill, goal_arg, params_of, with_defaults};
use wisp_kernel::{Call, Ctx, Result, WireError};
use wisp_resources_api::{KindInfo, OverlayArgs, OverlayOut, Resource, ResourceKind};
use wisp_resources_core::formula::{self, Loaded, Source, documents, format_of};
use wisp_resources_core::module::{self, Env};
use wisp_resources_core::overlay::{self as ov, FormulaRef};

/// Who reads the formula (review r194 #1). The user reads any file on this computer, as their CLI does. Anyone else (an
/// agent's `wisp_launch`, a channel) reads only inside its own task's folders: a relative path, no `..`, resolved after
/// links, never wisp's home or a secrets file (`secrets.json`, `.env*`), and never `file()` or the environment.
#[derive(Debug, Clone, Default)]
pub(crate) struct Access {
    /// None: the user. Some: the folders it may read, canonical (empty: none at all)
    inside: Option<Vec<PathBuf>>,
    /// wisp's home, canonical: never read by a confined reader
    home: Option<PathBuf>,
    /// a free reader that still never reads wisp's home or a secrets file (a grid box's owner, review r194 round 2 #7)
    guarded: bool,
}

/// The reader a caller's set is: the user (and the kernel) read freely; a grid box's owner (`box`) too, but never
/// wisp's home or a secrets file; anything else only inside `dirs`, its task's folders (none: no files).
pub(crate) fn access_for(set: &str, dirs: Vec<PathBuf>, home: Option<PathBuf>) -> Access {
    match set {
        "user:full" | "kernel" => Access { inside: None, home, guarded: false },
        "box" => Access { inside: None, home, guarded: true },
        _ => Access { inside: Some(dirs), home, guarded: false },
    }
}

/// A file a confined reader never reads: wisp's secrets, an env file.
fn secret_name(n: &str) -> bool {
    n == "secrets.json" || n.starts_with(".env")
}

const NO_FILE: &str = "an agent's formula can't read files";
const NO_ENV: &str = "an agent's formula can't read the environment";

impl Access {
    fn confined(&self) -> bool {
        self.inside.is_some()
    }

    /// The folder relative names start from: the task's own folder for a confined reader, else `dir`.
    fn base(&self, dir: &Path) -> PathBuf {
        match &self.inside {
            Some(inside) => inside.first().cloned().unwrap_or_else(|| dir.to_owned()),
            None => dir.to_owned(),
        }
    }

    /// `reference` as written, from `dir`, if this reader may read it.
    fn open(&self, dir: &Path, reference: &str) -> std::result::Result<PathBuf, String> {
        let p = Path::new(reference);
        let Some(inside) = &self.inside else {
            let full = if p.is_absolute() { p.to_owned() } else { dir.join(p) };
            if self.guarded {
                let real = wisp_proc::canonical_blocking(&full).map_err(|e| format!("{reference}: {e}"))?;
                if self.home.as_ref().is_some_and(|h| real.starts_with(h))
                    || real.file_name().is_some_and(|n| secret_name(&n.to_string_lossy()))
                {
                    return Err(format!("{reference}: a secrets file isn't read for a formula from here"));
                }
                return Ok(real);
            }
            return Ok(full);
        };
        if p.has_root() || p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
            return Err(format!("{reference}: an agent's formula is read from inside its task's folder (no absolute paths, no ..)"));
        }
        if p.file_name().is_some_and(|n| secret_name(&n.to_string_lossy())) {
            return Err(format!("{reference}: a secrets file isn't read for an agent's formula"));
        }
        let real = wisp_proc::canonical_blocking(dir.join(p)).map_err(|e| format!("{reference}: {e}"))?;
        self.inside_ok(&real, reference, inside)?;
        Ok(real)
    }

    /// A folder a module is read from, if this reader may.
    fn folder(&self, at: &Path, shown: &str) -> std::result::Result<PathBuf, String> {
        let Some(inside) = &self.inside else { return Ok(at.to_owned()) };
        let real = wisp_proc::canonical_blocking(at).map_err(|e| format!("{shown}: {e}"))?;
        self.inside_ok(&real, shown, inside)?;
        Ok(real)
    }

    fn inside_ok(&self, real: &Path, shown: &str, inside: &[PathBuf]) -> std::result::Result<(), String> {
        // wisp's home is never read, but for a task folder of its own inside it (its worktree, `~/.wisp/worktrees/…`:
        // audit G1); a task folder that is the home or holds it doesn't open the home
        let in_home = self
            .home
            .as_ref()
            .is_some_and(|h| real.starts_with(h) && !inside.iter().any(|r| r.starts_with(h) && r != h && real.starts_with(r)));
        if in_home || real.file_name().is_some_and(|n| secret_name(&n.to_string_lossy())) {
            return Err(format!("{shown}: a secrets file isn't read for an agent's formula"));
        }
        if !inside.iter().any(|r| real.starts_with(r)) {
            return Err(format!("{shown}: it leads out of the task's folder (a link out?)"));
        }
        Ok(())
    }

    /// A confined reader's text that reads the environment: refused, saying which.
    fn env_in(&self, text: &str) -> std::result::Result<(), String> {
        if !self.confined() {
            return Ok(());
        }
        match text.find("${env:") {
            Some(i) => {
                let end = text[i..].find('}').map_or(text.len(), |e| i + e + 1);
                Err(format!("{NO_ENV}: {}", &text[i..end]))
            }
            None => Ok(()),
        }
    }
}

/// Files on this computer, relative to the file that names them (else `cwd`), as `access` allows. A URL is the CLI's to
/// fetch.
struct Disk {
    cwd: PathBuf,
    /// the root's text when it was given (New task's picked file), under its name
    given: Option<(String, String)>,
    access: Access,
}

impl Source for Disk {
    fn load(&mut self, reference: &str, from: Option<&str>) -> std::result::Result<(String, String), String> {
        if let Some((name, text)) = self.given.as_ref().filter(|(n, _)| from.is_none() && n == reference) {
            self.access.env_in(text)?;
            return Ok((text.clone(), name.clone()));
        }
        if reference.contains("://") || reference.starts_with("gist:") {
            return Err(format!("{reference}: a formula from a URL is read by the CLI (wisp run --formula {reference}): give a file here"));
        }
        let dir = from.and_then(|f| Path::new(f).parent().map(Path::to_path_buf)).unwrap_or_else(|| self.cwd.clone());
        let p = self.access.open(&dir, reference)?;
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        self.access.env_in(&text)?;
        Ok((text, p.display().to_string()))
    }
    fn env(&self, name: &str) -> Option<String> {
        if self.access.confined() { None } else { std::env::var(name).ok() }
    }
}

/// A module's files from a folder on this computer (`source` relative to the file declaring it; a fetched git module
/// under `.wisp/modules/<name>`), as `access` allows.
struct Folders {
    root: PathBuf,
    access: Access,
}

/// The formula files of a folder (`*.wisp.yml`, `.json`, `.toml`; in a module folder any `.yml`), by name; never a
/// secrets file. For a confined reader each is resolved first and kept inside its folders (review r194 round 2 #5: a link
/// in a module folder led out).
fn files_in(dir: &Path, access: &Access) -> std::result::Result<Vec<(String, String)>, String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(Vec::new()) };
    let mut out: Vec<(String, String)> = Vec::new();
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let ok = [".yml", ".yaml", ".json", ".toml"].iter().any(|x| n.ends_with(x))
            && n != "lock.yml"
            && !n.ends_with(".task.yml")
            && !secret_name(&n);
        if !ok || !e.path().is_file() {
            continue;
        }
        let path = match &access.inside {
            Some(inside) => {
                let real = wisp_proc::canonical_blocking(e.path()).map_err(|err| format!("{n}: {err}"))?;
                access.inside_ok(&real, &n, inside)?;
                real
            }
            None => e.path(),
        };
        if let Ok(t) = std::fs::read_to_string(path) {
            out.push((n, t));
        }
    }
    out.sort();
    Ok(out)
}

impl Env for Folders {
    fn outside(&self, f: &str, arg: &str) -> std::result::Result<Value, String> {
        if self.access.confined() {
            return Err(if f == "file" { format!("{NO_FILE}: file(\"{arg}\")") } else { format!("{NO_ENV}: {f}(\"{arg}\")") });
        }
        match f {
            "file" => std::fs::read_to_string(self.root.join(arg)).map(Value::String).map_err(|e| format!("file(\"{arg}\"): {e}")),
            _ => Ok(std::env::var(arg).map_or(Value::Null, Value::String)),
        }
    }
    fn module(&mut self, name: &str, source: &str, from: &str) -> std::result::Result<Vec<(String, String)>, String> {
        let at = if source.contains("://") || source.starts_with("git::") {
            self.root.join(".wisp").join("modules").join(name)
        } else {
            if self.access.confined() && (Path::new(source).has_root() || Path::new(source).components().any(|c| c == Component::ParentDir))
            {
                return Err(format!(
                    "module.{name}: {source}: an agent's formula is read from inside its task's folder (no absolute paths, no ..)"
                ));
            }
            let base = Path::new(from).parent().map_or_else(|| self.root.clone(), Path::to_path_buf);
            let base = if base.is_absolute() { base } else { self.root.join(base) };
            base.join(source)
        };
        if !at.is_dir() {
            return Err(format!("module.{name}: {} isn't a folder here (wisp init fetches a git module)", at.display()));
        }
        let at = self.access.folder(&at, &format!("module.{name}"))?;
        let files = files_in(&at, &self.access)?;
        for (_, t) in &files {
            self.access.env_in(t)?;
        }
        Ok(files)
    }
}

/// A path with its `.` and `..` resolved by its text (`/w/./x.yml` is `/w/x.yml`): two spellings of one file compare equal.
fn lexical(p: &Path) -> String {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out.display().to_string()
}

/// Everything read, before the overlay is made.
#[derive(Default)]
struct Read {
    resources: Vec<Resource>,
    task: Option<Value>,
    sources: Vec<String>,
    problems: Vec<String>,
    notes: Vec<String>,
}

/// `resources` patched by `more` (a later file's resource of the same name patches the earlier one).
fn add(into: &mut Vec<Resource>, more: Vec<Resource>) {
    for r in more {
        if let Some(had) = into.iter_mut().find(|x| x.kind == r.kind && x.name == r.name) {
            had.spec = wisp_resources_core::plan::merge(&had.spec, &r.spec);
        } else {
            into.push(r);
        }
    }
}

/// One formula file (or the given text) read and evaluated as a folder is.
fn formula_at(
    reference: &str,
    cwd: &Path,
    given: Option<(String, String)>,
    kinds: &[KindInfo],
    access: &Access,
) -> std::result::Result<Loaded, String> {
    let mut src = Disk { cwd: cwd.to_owned(), given, access: access.clone() };
    let loaded = formula::load(reference, &mut src, kinds, &[])?;
    let c = module::evaluate(&loaded, kinds, Map::new(), "default", &mut Folders { root: cwd.to_owned(), access: access.clone() })?;
    Ok(Loaded { resources: c.resources, ..loaded })
}

/// The `.wisp/` folder of `cwd` as one configuration (for `module.x`).
fn folder_config(cwd: &Path, kinds: &[KindInfo], access: &Access) -> std::result::Result<module::Config, String> {
    let dir = cwd.join(".wisp");
    let files = if dir.is_dir() { files_in(&access.folder(&dir, ".wisp")?, access)? } else { Vec::new() };
    if files.is_empty() {
        return Err(format!("no .wisp/ formula in {}: module.x names a module it declares", cwd.display()));
    }
    for (_, t) in &files {
        access.env_in(t)?;
    }
    let input = module::Input { files, workspace: "default".into(), ..module::Input::default() };
    module::configure(&input, kinds, &mut Folders { root: dir, access: access.clone() })
}

/// Module `m` of `cwd`'s `.wisp/` formula, under the names it has inside it; one it doesn't declare says which it does.
fn module_of(cwd: &Path, kinds: &[KindInfo], m: &str, access: &Access) -> std::result::Result<Vec<Resource>, String> {
    let c = folder_config(cwd, kinds, access).map_err(|e| format!("module.{m}: {e}"))?;
    ov::module_resources(&c, m).ok_or_else(|| {
        let has: Vec<&str> = c.modules.iter().map(|(n, _)| n.as_str()).filter(|n| !n.contains('.')).collect();
        let has = if has.is_empty() { "it declares none".to_owned() } else { format!("it has {}", has.join(", ")) };
        format!("no module named {m} in {}/.wisp: {has}", cwd.display())
    })
}

/// Where the picked file `name` with this `text` is in the project `root`: the one folder (at most 5 deep, past no
/// `.git`, `node_modules`, `target` or `dist`, at most 5,000 entries) that holds a file of that name and text. None when
/// there's none, or two.
fn beside(root: &Path, name: &str, text: &str) -> Option<PathBuf> {
    const SKIP: [&str; 4] = [".git", "node_modules", "target", "dist"];
    let (mut queue, mut seen, mut hits) = (std::collections::VecDeque::from([(root.to_owned(), 0)]), 0usize, Vec::new());
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > 5_000 {
                return None;
            }
            let (p, n) = (e.path(), e.file_name().to_string_lossy().into_owned());
            match e.file_type() {
                Ok(t) if t.is_dir() && depth < 5 && !SKIP.contains(&n.as_str()) => queue.push_back((p, depth + 1)),
                Ok(t) if t.is_file() && n == name && std::fs::read_to_string(&p).is_ok_and(|x| x == text) => hits.push(dir.clone()),
                _ => {}
            }
        }
    }
    match hits.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

/// Reads what `a` gives: its resources, its text (a formula, or a task file and what its `formula:` names), its files,
/// as `access` allows. A task file's relative names are read from its own folder (`dir`) when that's known; a picked
/// file's isn't, so they're read from the project (`cwd`), and a note says so (review r194 #5).
fn read_all(a: &OverlayArgs, kinds: &[KindInfo], access: &Access) -> Read {
    let mut r = Read { resources: a.resources.clone(), task: a.task.clone(), ..Read::default() };
    let cwd = access.base(&PathBuf::from(if a.cwd.is_empty() { "." } else { a.cwd.as_str() }));
    let files: Vec<String> = a.files.clone();
    if !a.text.trim().is_empty() {
        let name = if a.name.is_empty() { "pasted.yml".to_owned() } else { a.name.clone() };
        let doc = documents(&a.text, format_of(&name)).map(|d| d.into_iter().next().unwrap_or_default());
        match doc {
            Err(e) => r.problems.push(format!("{name}: {e}")),
            Ok(_) if access.env_in(&a.text).is_err() => r.problems.push(access.env_in(&a.text).err().unwrap_or_default()),
            // a task file: its task, and the formula it names
            // review r194 round 2 #3: an agent's formula never replaces the task it starts
            Ok(doc) if doc.get("task").is_some() && access.confined() => {
                r.problems.push("an agent's formula can't carry a task: give its prompt and fields to wisp_launch".to_owned());
            }
            Ok(doc) if doc.get("task").is_some() => {
                r.task = doc.get("task").cloned();
                // a picked file's folder isn't sent (a browser's file has no path): the same file (its name, its text)
                // found in the project is where it is (audit G10: `.qa/a.task.yml`'s `./team.yml` was read at the root)
                let found = if a.dir.is_empty() && !access.confined() { beside(&cwd, &name, &a.text) } else { None };
                let own = (!a.dir.is_empty() || found.is_some()) && !access.confined();
                let base = match (&found, own) {
                    (Some(d), _) => d.clone(),
                    (None, true) => PathBuf::from(&a.dir),
                    (None, false) => cwd.clone(),
                };
                match ov::formula_refs(doc.get("formula")) {
                    Err(e) => r.problems.push(format!("{name}: {e}")),
                    Ok(refs) => {
                        let mut paths = Vec::new();
                        for f in refs {
                            match f {
                                FormulaRef::Inline(v) => add(&mut r.resources, ov::inline_resources(&v)),
                                FormulaRef::File(p) => paths.push(p),
                                FormulaRef::Module(m) => {
                                    if !own && !access.confined() {
                                        r.notes.push(format!(
                                            "{name}: module.{m} was read from the project folder {}: a picked file's own folder isn't known here",
                                            base.display()
                                        ));
                                    }
                                    match module_of(&base, kinds, &m, access) {
                                        Ok(rs) => add(&mut r.resources, rs),
                                        Err(e) => r.problems.push(format!("{name}: formula: {e}")),
                                    }
                                }
                            }
                        }
                        let full: Vec<String> = paths.iter().map(|p| lexical(&base.join(p))).collect();
                        if let Some(e) = ov::looping_files(&lexical(&base.join(&name)), &full) {
                            r.problems.push(format!("{name}: {e}"));
                        } else {
                            if !own && !access.confined() {
                                for p in &paths {
                                    r.notes.push(format!(
                                        "{name}: {p} was read from the project folder {}: a picked file's own folder isn't known here",
                                        base.display()
                                    ));
                                }
                            }
                            for p in paths {
                                match formula_at(&p, &base, None, kinds, access) {
                                    Ok(l) => {
                                        r.sources.extend(l.sources);
                                        add(&mut r.resources, l.resources);
                                    }
                                    // plain words, not the OS's: where it was looked for, and what reads it beside the file
                                    Err(_) if !own && !base.join(&p).exists() => r.problems.push(format!(
                                        "{name}: formula: {p} isn't in the project folder {}, where a picked task file's names are read (its own folder isn't known here): run it with wisp run -f <its path>",
                                        base.display()
                                    )),
                                    Err(e) => r.problems.push(format!("formula: {e}")),
                                }
                            }
                        }
                    }
                }
            }
            Ok(_) => match formula_at(&name, &cwd, Some((name.clone(), a.text.clone())), kinds, access) {
                Ok(l) => {
                    r.sources.extend(l.sources);
                    add(&mut r.resources, l.resources);
                }
                Err(e) => r.problems.push(format!("formula: {e}")),
            },
        }
    }
    for f in &files {
        match formula_at(f, &cwd, None, kinds, access) {
            Ok(l) => {
                r.sources.extend(l.sources);
                add(&mut r.resources, l.resources);
            }
            Err(e) => r.problems.push(format!("formula: {e}")),
        }
    }
    r
}

/// The declared task `name` in `resources`, `tasks.default` under it, its `{params}` filled: `runtime.create`'s args.
///
/// # Errors
/// No such task (saying which there are), `tasks.default`, or a param left open.
pub fn declared_task(
    resources: &[Resource],
    name: &str,
    params: &BTreeMap<String, String>,
    words: &[String],
) -> std::result::Result<Value, String> {
    let decl =
        |n: &str| resources.iter().find(|r| r.kind == "task" && r.name == n).map(|r| serde_json::from_value::<TaskDecl>(r.spec.clone()));
    if name == DEFAULT {
        return Err("tasks.default gives every task its defaults: it isn't a task to run".into());
    }
    let Some(t) = decl(name) else {
        let mut has: Vec<&str> = resources.iter().filter(|r| r.kind == "task" && r.name != DEFAULT).map(|r| r.name.as_str()).collect();
        has.sort_unstable();
        let has = if has.is_empty() { "it declares no tasks".to_owned() } else { format!("tasks has {}", has.join(", ")) };
        return Err(format!("no task named {name} in the formula: {has}"));
    };
    let t = t.map_err(|e| format!("tasks.{name}: {e}"))?;
    let d = decl(DEFAULT).transpose().map_err(|e| format!("tasks.default: {e}"))?.unwrap_or_default();
    let t = with_defaults(&t, &d);
    let mut values = params.clone();
    let mut it = words.iter();
    let names = params_of(&format!("{} {}", t.prompt, t.steps.join(" ")));
    for p in &names {
        if !values.contains_key(p)
            && let Some(w) = it.next()
        {
            values.insert(p.clone(), w.clone());
        }
    }
    // words past its params were dropped (review r194 #6): said, with how to pass them
    let left: Vec<&str> = it.map(String::as_str).collect();
    if !left.is_empty() {
        let takes = if names.is_empty() {
            "takes no values".to_owned()
        } else {
            format!("takes {}", names.iter().map(|p| format!("{{{p}}}")).collect::<Vec<_>>().join(" "))
        };
        let how = names.first().map_or_else(String::new, |p| format!("give a value as {p}=…, or "));
        return Err(format!("{name} {takes}: \"{}\" is left over ({how}run a prompt: wisp \"…\" --formula …)", left.join(" ")));
    }
    let prompt = fill(&t.prompt, &values)?;
    let steps: Vec<String> = t.steps.iter().map(|s| fill(s, &values)).collect::<std::result::Result<_, _>>()?;
    let mut out = Map::new();
    out.insert("prompt".into(), json!(prompt));
    out.insert("title".into(), json!(if t.title.is_empty() { name } else { t.title.as_str() }));
    for (k, v) in
        [("agent", &t.agent), ("model", &t.model), ("effort", &t.effort), ("mode", &t.mode), ("cwd", &t.project), ("crew", &t.crew)]
    {
        if !v.is_empty() {
            out.insert(k.into(), json!(v));
        }
    }
    if !steps.is_empty() {
        out.insert("steps".into(), json!(steps));
    }
    if !t.connectors.is_empty() {
        out.insert("connectors".into(), json!(t.connectors));
    }
    if let Some(w) = t.worktree {
        out.insert("worktree".into(), json!(w));
    }
    if let Some(g) = goal_arg(t.goal.as_ref()) {
        // a declared name stays a name here ({ name }); inline_goal writes in the formula's own
        out.insert("goal".into(), g);
    }
    if !t.notes.is_empty()
        && let Ok(n) = serde_json::to_value(&t.notes)
    {
        out.insert("notes".into(), n);
    }
    out.insert("ext".into(), json!({ "wisp.automations": { "task": name } }));
    Ok(Value::Object(out))
}

/// The names each kind has here (stored), and what else a field of it takes (an agent's harnesses).
async fn known_here(
    ks: &[Arc<dyn ResourceKind>],
    call: &Call,
) -> (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>, BTreeMap<String, String>) {
    let (mut known, mut also, mut sections) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    for n in ov::task_name_fields() {
        if known.contains_key(&n.kind) {
            continue;
        }
        let Some(k) = ks.iter().find(|k| k.info().kind == n.kind) else { continue };
        let names: Vec<String> = k.read(call).await.unwrap_or_default().into_iter().map(|r| r.name).filter(|n| n != DEFAULT).collect();
        known.insert(n.kind.clone(), names);
        also.insert(n.kind.clone(), k.also_named(call).await);
        sections.insert(n.kind.clone(), k.info().section);
    }
    (known, also, sections)
}

/// The reader `call` is (review r194 #1): the user, or anything else confined to its task's folders (its folder, its
/// worktree and that worktree's checkout), read as the kernel holds them; a caller with no task reads no files.
async fn access_of(ctx: &Ctx, call: &Call) -> Access {
    let home = match wisp_kernel::services::paths(ctx) {
        Ok(p) => wisp_proc::canonical(p.home()).await.ok(),
        Err(_) => None,
    };
    let c = call.caller();
    let mut inside = Vec::new();
    if let Some(id) = c.session.as_deref().filter(|_| !matches!(c.set.as_str(), "user:full" | "kernel" | "box")) {
        let args = wisp_runtime_api::commands::IdArgs { id: id.into() };
        let t = ctx
            .run::<wisp_runtime_api::commands::Get>(&ctx.root_call(), args)
            .await
            .ok()
            .and_then(|t| serde_json::to_value(t).ok())
            .unwrap_or_default();
        for at in ["/cwd", "/ext/wisp.worktrees/path", "/ext/wisp.worktrees/root"] {
            if let Some(dir) = t.pointer(at).and_then(Value::as_str).filter(|d| !d.is_empty())
                && let Ok(real) = wisp_proc::canonical(dir).await
                && !inside.contains(&real)
            {
                inside.push(real);
            }
        }
    }
    access_for(&c.set, inside, home)
}

/// Every name the formula's own resources give (an agent's connectors and skills, a team's agents), checked as the plan
/// checks them: against the formula's and this wisp's (review r194 round 2 #1: an agent's formula names only connectors
/// the user set up).
async fn names_in_formula(ks: &[Arc<dyn ResourceKind>], call: &Call, rs: &[Resource]) -> Vec<String> {
    let mut known: BTreeMap<String, (String, Vec<String>, Vec<String>)> = BTreeMap::new();
    let mut out = Vec::new();
    for r in rs {
        let Some(info) = ks.iter().map(|k| k.info()).find(|i| i.kind == r.kind) else { continue };
        for n in &info.names {
            for name in wisp_resources_api::names_in(&r.spec, n) {
                if !known.contains_key(&n.kind) {
                    let (section, mut have, also) = match ks.iter().find(|k| k.info().kind == n.kind) {
                        Some(k) => (
                            k.info().section,
                            k.read(call).await.unwrap_or_default().into_iter().map(|x| x.name).filter(|x| x != DEFAULT).collect::<Vec<_>>(),
                            k.also_named(call).await,
                        ),
                        None => (n.kind.clone(), Vec::new(), Vec::new()),
                    };
                    have.extend(rs.iter().filter(|x| x.kind == n.kind && x.name != DEFAULT).map(|x| x.name.clone()));
                    known.insert(n.kind.clone(), (section, have, also));
                }
                let (section, have, also) = &known[&n.kind];
                if have.contains(&name) || also.contains(&name) {
                    continue;
                }
                let mut has =
                    if have.is_empty() { format!("no {section} are declared") } else { format!("{section} has {}", ov::some_of(have)) };
                if !also.is_empty() {
                    has = format!("{has} (or {})", ov::some_of(also));
                }
                let line = format!("{}.{}: no {} named {name}: {has}", info.section, r.name, n.kind);
                if !out.contains(&line) {
                    out.push(line);
                }
            }
        }
    }
    out
}

/// `resources.overlay`.
pub async fn overlay(ctx: &Ctx, call: &Call, ks: Vec<Arc<dyn ResourceKind>>, a: OverlayArgs) -> Result<OverlayOut> {
    let kinds: Vec<KindInfo> = ks.iter().map(|k| k.info()).collect();
    let access = access_of(ctx, call).await;
    let args = a.clone();
    let infos = kinds.clone();
    let confined = access.confined();
    let read =
        tokio::task::spawn_blocking(move || read_all(&args, &infos, &access)).await.map_err(|e| WireError::internal(e.to_string()))?;
    let (overlay, left) = ov::overlay_of(&read.resources);
    let mut problems = read.problems;
    // the schema, as `wisp validate` checks it: a formula a task runs under is refused for what validate names (audit
    // G8: `agents.critic.brief`, an unknown field, was dropped, and the task launched with "no harness critic")
    for r in &read.resources {
        if let Some(info) = kinds.iter().find(|k| k.kind == r.kind).filter(|k| !k.map_form) {
            problems.extend(wisp_resources_core::schema::validate(&info.schema, &r.spec, &format!("{}.{}", info.section, r.name)));
        }
    }
    problems.extend(names_in_formula(&ks, call, &read.resources).await);
    // an agent's formula declares only agents, skills (as text), teams and goals, with no secret (review r194 round 2 #1)
    if confined {
        problems.extend(wisp_resources_api::agent_formula_problems(&read.resources));
    }
    let mut task = read.task;
    if !a.run.is_empty() {
        match declared_task(&read.resources, &a.run, &a.params, &a.words) {
            Ok(t) => task = Some(merge_task(t, task)),
            Err(e) => problems.push(e),
        }
    }
    let agents = ov::declared_in(&overlay, "agents");
    if let Some(t) = task.as_mut() {
        // one agent declared and the task names none: that one (as `wisp run --formula` always did)
        if t.get("agent").and_then(Value::as_str).is_none_or(str::is_empty)
            && let [one] = agents.as_slice()
        {
            t["agent"] = json!(one);
        }
        ov::inline_goal(t, &read.resources);
        let (mut known, also, sections) = known_here(&ks, call).await;
        // the formula's own, for one task: its overlay's sections (agents, teams, connectors, projects)
        for (kind, section) in ov::SECTIONS {
            known.entry(kind.to_owned()).or_default().extend(ov::declared_in(&overlay, section));
        }
        let at = if a.name.is_empty() { "the task".to_owned() } else { a.name.clone() };
        let mut unknown = ov::unknown_names(&at, t, &known, &also, &sections);
        // a crew member the formula declares but this wisp hasn't got: say how to have them
        for line in &mut unknown {
            if let Some(c) =
                read.resources.iter().filter(|r| r.kind == "crew").find(|r| line.starts_with(&format!("{at}: no crew named {}: ", r.name)))
            {
                *line = format!(
                    "{at}: crew.{} is declared in the formula but not set up here: wisp apply it first (a crew member works from this wisp)",
                    c.name
                );
            }
        }
        problems.extend(unknown);
    }
    let mut tasks: Vec<String> = read.resources.iter().filter(|r| r.kind == "task" && r.name != DEFAULT).map(|r| r.name.clone()).collect();
    tasks.sort();
    Ok(OverlayOut { overlay, left, task, tasks, agents, sources: read.sources, problems, notes: read.notes })
}

/// A declared task with the caller's fields over it (`wisp run release --formula f --agent codex`).
fn merge_task(declared: Value, over: Option<Value>) -> Value {
    let mut t = declared;
    if let (Some(o), Some(Value::Object(over))) = (t.as_object_mut(), over) {
        for (k, v) in over {
            if !v.is_null() {
                o.insert(k, v);
            }
        }
    }
    t
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn r(kind: &str, name: &str, spec: Value) -> Resource {
        Resource::new(kind, name, spec)
    }

    /// A task the formula declares runs by its name: `tasks.default` under it, its `{params}` from words; `default` and
    /// a name it doesn't declare are refused, saying which there are.
    #[test]
    fn a_declared_task_by_name_takes_the_defaults_and_its_params() {
        let f = [
            r("task", "default", json!({ "agent": "builder", "mode": "auto" })),
            r("task", "release", json!({ "prompt": "release {version}", "goal": "green", "worktree": false })),
            r("task", "docs", json!({ "prompt": "docs", "agent": "writer" })),
        ];
        let t = declared_task(&f, "release", &BTreeMap::new(), &["2.1".to_owned()]).unwrap();
        assert_eq!(t["prompt"], "release 2.1");
        assert_eq!((t["agent"].as_str(), t["mode"].as_str()), (Some("builder"), Some("auto")), "tasks.default under it");
        assert_eq!(t["goal"], json!({ "name": "green" }));
        assert_eq!(t["worktree"], false);
        assert_eq!(t["title"], "release");
        assert_eq!(declared_task(&f, "docs", &BTreeMap::new(), &[]).unwrap()["agent"], "writer", "its own field wins");
        assert!(declared_task(&f, "release", &BTreeMap::new(), &[]).unwrap_err().contains("needs {version}"));
        assert_eq!(
            declared_task(&f, "nosuch", &BTreeMap::new(), &[]).unwrap_err(),
            "no task named nosuch in the formula: tasks has docs, release"
        );
        assert!(declared_task(&f, "default", &BTreeMap::new(), &[]).unwrap_err().contains("isn't a task to run"));
        // review r194 #6: words past its params aren't dropped: said, with how to pass them
        let e = declared_task(&f, "release", &BTreeMap::new(), &["2.1".to_owned(), "and".to_owned(), "more".to_owned()]).unwrap_err();
        assert_eq!(
            e,
            "release takes {version}: \"and more\" is left over (give a value as version=…, or run a prompt: wisp \"…\" --formula …)"
        );
        let e = declared_task(&f, "docs", &BTreeMap::new(), &["extra".to_owned()]).unwrap_err();
        assert!(e.starts_with("docs takes no values: \"extra\" is left over"), "{e}");
    }

    /// Audit G1: a task's worktree lives under wisp's home (`~/.wisp/worktrees/…`) and is its folder (ADR-0111): its
    /// agent reads a formula there; the rest of wisp's home, `secrets.json` and `.env*` stay refused, even in it.
    #[test]
    fn an_agent_reads_a_formula_in_its_own_worktree_under_wisps_home() {
        let d = std::env::temp_dir().join(format!("wisp-access-wt-{}", std::process::id()));
        let wt = d.join("home/worktrees/proj-1");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join("ok.yml"), "wisp: 1").unwrap();
        std::fs::write(wt.join(".env.local"), "A=1").unwrap();
        std::fs::write(d.join("home/secrets.json"), "{}").unwrap();
        std::fs::write(d.join("home/other.yml"), "wisp: 1").unwrap();
        let real = wisp_proc::canonical_blocking(&d).unwrap();
        let (home, wt) = (real.join("home"), real.join("home/worktrees/proj-1"));
        let agent = access_for("agent:session", vec![wt.clone()], Some(home.clone()));
        assert_eq!(agent.open(&wt, "ok.yml").unwrap(), wt.join("ok.yml"), "its own worktree is its folder");
        assert!(agent.open(&wt, ".env.local").unwrap_err().contains("a secrets file isn't read"));
        // a link out of the worktree into the rest of the home is still refused
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(home.join("other.yml"), wt.join("link.yml")).unwrap();
            assert!(agent.open(&wt, "link.yml").is_err(), "the rest of wisp's home");
        }
        // a task whose folder is the home itself (or holds it) reads nothing of it
        let wide = access_for("agent:session", vec![real.clone()], Some(home.clone()));
        assert!(wide.open(&real, "home/other.yml").unwrap_err().contains("isn't read for an agent's formula"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Review r194 round 2 #7: a grid box's owner reads their own files as the user does, never wisp's home or a secrets
    /// file; an agent reads only inside its folders.
    #[test]
    fn a_boxs_owner_reads_as_the_user_but_never_secrets() {
        let d = std::env::temp_dir().join(format!("wisp-access-box-{}", std::process::id()));
        std::fs::create_dir_all(d.join("home")).unwrap();
        std::fs::write(d.join("f.yml"), "wisp: 1").unwrap();
        std::fs::write(d.join("secrets.json"), "{}").unwrap();
        std::fs::write(d.join("home/x.yml"), "wisp: 1").unwrap();
        let real = wisp_proc::canonical_blocking(&d).unwrap();
        let home = Some(real.join("home"));
        let b = access_for("box", Vec::new(), home.clone());
        let f = d.join("f.yml").display().to_string();
        assert!(b.open(Path::new("/"), &f).is_ok(), "an absolute path of its own");
        let sec = d.join("secrets.json").display().to_string();
        assert!(b.open(Path::new("/"), &sec).unwrap_err().contains("a secrets file isn't read"));
        let inhome = d.join("home/x.yml").display().to_string();
        assert!(b.open(Path::new("/"), &inhome).unwrap_err().contains("a secrets file isn't read"));
        assert!(access_for("user:full", Vec::new(), home.clone()).open(Path::new("/"), &f).is_ok());
        let agent = access_for("agent:session", vec![real.clone()], home);
        assert!(agent.open(Path::new("/"), &f).unwrap_err().contains("no absolute paths"));
        assert!(agent.open(&real, "f.yml").is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }
}
