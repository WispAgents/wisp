//! ADR-0111: every field that names another kind's resource (`KindInfo::names`) is checked by the plan: a name that
//! names nothing as the apply would leave it is refused, saying what there is (and what else the field takes: a
//! harness for an agent); a field that may not loop refuses a loop; a removed resource something still names is
//! refused too.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::needless_pass_by_value)]

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use wisp_kernel::{BoxFuture, Call, Caller, Manifest, Plugin, Result, Setup};
use wisp_resources_api::{KindInfo, Names, Op, OpKind, RESOURCE_KIND, Resource, ResourceKind};
use wisp_store::{Location, Store, SystemClock};
use wisp_testkit::TestApp;

struct Mem {
    info: KindInfo,
    also: Vec<String>,
    have: Mutex<Vec<Resource>>,
}

impl ResourceKind for Mem {
    fn info(&self) -> KindInfo {
        self.info.clone()
    }
    fn read<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Result<Vec<Resource>>> {
        Box::pin(async move { Ok(self.have.lock().unwrap().clone()) })
    }
    fn also_named<'a>(&'a self, _call: &'a Call) -> BoxFuture<'a, Vec<String>> {
        Box::pin(async move { self.also.clone() })
    }
    fn apply<'a>(&'a self, _call: &'a Call, op: &'a Op) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let mut have = self.have.lock().unwrap();
            have.retain(|r| r.name != op.name);
            if op.op != OpKind::Delete {
                have.push(Resource::new(op.kind.clone(), op.name.clone(), op.spec.clone()));
            }
            Ok(())
        })
    }
}

struct Kinds;
impl Plugin for Kinds {
    fn manifest(&self) -> Manifest {
        Manifest::from_value(json!({ "wispPlugin": 1, "id": "acme.kinds", "version": "1.0.0", "api": "^1", "activation": ["onStartup"] }))
            .unwrap()
    }
    fn setup(&self, s: &mut Setup) -> Result<()> {
        let mk = |info: KindInfo, also: &[&str]| -> Arc<dyn ResourceKind> {
            Arc::new(Mem { info, also: also.iter().map(|a| (*a).to_owned()).collect(), have: Mutex::new(Vec::new()) })
        };
        s.contribute(&RESOURCE_KIND, mk(KindInfo::new("agent", "agents", "acme.kinds", json!({})).order(1), &["codex", "auto"]));
        s.contribute(&RESOURCE_KIND, mk(KindInfo::new("connector", "connectors", "acme.kinds", json!({})).order(1), &[]));
        s.contribute(&RESOURCE_KIND, mk(KindInfo::new("goal", "goals", "acme.kinds", json!({})).order(1), &[]));
        s.contribute(&RESOURCE_KIND, mk(KindInfo::new("skill", "skills", "acme.kinds", json!({})).order(1), &[]));
        s.contribute(
            &RESOURCE_KIND,
            mk(KindInfo::new("task", "tasks", "acme.kinds", json!({})).order(2).names(Names::new("agent", "agent")), &[]),
        );
        s.contribute(
            &RESOURCE_KIND,
            mk(
                KindInfo::new("crew", "crew", "acme.kinds", json!({}))
                    .order(3)
                    .names(Names::new("agent", "agent"))
                    .names(Names::new("team[].crew", "crew").acyclic()),
                &[],
            ),
        );
        s.contribute(
            &RESOURCE_KIND,
            mk(
                KindInfo::new("trigger", "triggers", "acme.kinds", json!({}))
                    .order(4)
                    .names(Names::new("task", "task"))
                    .names(Names::new("crew", "crew")),
                &[],
            ),
        );
        Ok(())
    }
}

fn r(kind: &str, name: &str, spec: Value) -> Value {
    json!({ "kind": kind, "name": name, "spec": spec })
}

async fn problems(k: &wisp_kernel::Kernel, resources: Value, extra: Value) -> Vec<String> {
    let mut args = json!({ "resources": resources });
    if let (Some(a), Some(e)) = (args.as_object_mut(), extra.as_object()) {
        a.extend(e.clone());
    }
    let plan = k.call(Caller::user(), "resources.plan", args).await.unwrap();
    serde_json::from_value(plan.get("problems").cloned().unwrap_or_else(|| json!([]))).unwrap()
}

#[tokio::test]
async fn every_name_field_is_checked_unknown_names_say_what_exists_and_loops_are_refused() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().with(wisp_resources::ResourcesPlugin).with(Kinds).service(store).start().await;
    let none = json!({});

    // declared in the same formula, a harness name, `auto`: all fine
    let ok = json!([
        r("agent", "reviewer", json!({})),
        r("task", "fix", json!({ "agent": "reviewer" })),
        r("task", "quick", json!({ "agent": "codex" })),
        r("task", "any", json!({ "agent": "auto" })),
        r("crew", "Ada", json!({ "agent": "reviewer", "team": [{ "crew": "Bo" }] })),
        r("crew", "Bo", json!({ "agent": { "harness": "claude" } })),
        r("trigger", "nightly", json!({ "task": "fix", "crew": "Ada" })),
    ]);
    assert_eq!(problems(&k, ok.clone(), none.clone()).await, Vec::<String>::new());

    // an unknown name in each field: refused, naming what exists (and a harness, for an agent field)
    let bad = json!([
        r("agent", "reviewer", json!({})),
        r("task", "fix", json!({ "agent": "critic" })),
        r("crew", "Ada", json!({ "team": [{ "crew": "Zed" }] })),
        r("trigger", "nightly", json!({ "task": "nosuch" })),
    ]);
    let p = problems(&k, bad, none.clone()).await;
    assert!(p.contains(&"tasks.fix: no agent named critic: agents has reviewer (or codex, auto)".to_owned()), "{p:?}");
    assert!(p.contains(&"crew.Ada: no crew named Zed: crew has Ada".to_owned()), "{p:?}");
    assert!(p.contains(&"triggers.nightly: no task named nosuch: tasks has fix".to_owned()), "{p:?}");

    // a delegation loop: refused once, naming the loop
    let looped = json!([
        r("crew", "Ada", json!({ "team": [{ "crew": "Bo" }] })),
        r("crew", "Bo", json!({ "team": [{ "crew": "Cy" }] })),
        r("crew", "Cy", json!({ "team": [{ "crew": "Ada" }] })),
    ]);
    let p = problems(&k, looped, none.clone()).await;
    assert_eq!(p, ["crew.Ada: team[].crew: a loop: crew.Ada → crew.Bo → crew.Cy → crew.Ada"], "{p:?}");
    let own = json!([r("crew", "Solo", json!({ "team": [{ "crew": "Solo" }] }))]);
    assert_eq!(problems(&k, own, none.clone()).await, ["crew.Solo: team[].crew: a loop: crew.Solo → crew.Solo"]);

    // applied, then the task a trigger names destroyed: refused, as the trigger is left naming nothing
    k.call(Caller::user(), "resources.apply", json!({ "resources": ok })).await.unwrap();
    let p = problems(&k, json!([]), json!({ "destroy": ["tasks.fix"] })).await;
    assert!(p.contains(&"triggers.nightly: no task named fix: tasks has quick, any".to_owned()), "{p:?}");
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("wisp-overlay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join(".wisp/team")).unwrap();
    d
}

/// ADR-0111: a task file references its formula (a file beside it, a module of the folder's `.wisp/`) instead of an
/// inline copy; the names in its task are resolved against them (a declared goal inlined, one agent its own) and an
/// unknown one is refused naming what exists; a file that loops is refused; a declared task runs by its name.
#[tokio::test]
async fn a_task_files_formula_is_referenced_and_its_names_resolved_against_it() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().with(wisp_resources::ResourcesPlugin).with(Kinds).service(store).start().await;
    let d = tmp("task");
    std::fs::write(
        d.join("wisp.yml"),
        "wisp: 1\nvariables: { model: { default: opus } }\nagents:\n  critic: { model: \"${var.model}\" }\ngoals:\n  default: { maxRounds: 4 }\n  green: { text: the build is green }\ntasks:\n  release: { prompt: \"release {version}\", goal: green }\n",
    )
    .unwrap();
    std::fs::write(d.join(".wisp/main.wisp.yml"), "modules:\n  team: { source: ./team }\n").unwrap();
    std::fs::write(
        d.join(".wisp/team/team.wisp.yml"),
        "agents:\n  reviewer: { skills: [\"${skill.testing}\"] }\nskills:\n  testing: { body: x }\n",
    )
    .unwrap();
    let cwd = d.display().to_string();
    let ask = |args: Value| {
        let k = &k;
        async move { k.call(Caller::user(), "resources.overlay", args).await.unwrap() }
    };

    // a file beside it: its agent is the task's, its goal by name inlined, nothing applied
    let text = "wisp: 1\ntask: { prompt: fix the build, goal: green }\nformula: ./wisp.yml\n";
    let o = ask(json!({ "text": text, "name": "fix.task.yml", "cwd": cwd })).await;
    assert_eq!(o.get("problems"), None, "{o}");
    assert_eq!(o["task"]["agent"], "critic", "the one agent it declares");
    assert_eq!(o["task"]["goal"], json!({ "maxRounds": 4, "text": "the build is green" }));
    assert_eq!(o["overlay"]["agents"], json!([{ "name": "critic", "model": "opus" }]), "evaluated as a folder is");
    assert_eq!(o["tasks"], json!(["release"]));

    // a module of the folder's formula: its resources under the names they have inside it
    let text = "wisp: 1\ntask: { prompt: review it, agent: reviewer }\nformula: module.team\n";
    let o = ask(json!({ "text": text, "name": "review.task.yml", "cwd": cwd })).await;
    assert_eq!(o.get("problems"), None, "{o}");
    assert_eq!(o["overlay"]["agents"], json!([{ "name": "reviewer", "skills": ["testing"] }]));
    let o = ask(json!({ "text": "wisp: 1\ntask: { prompt: x }\nformula: module.nope\n", "name": "x.task.yml", "cwd": cwd })).await;
    assert!(
        o["problems"][0].as_str().unwrap().contains("no module named nope in")
            && o["problems"][0].as_str().unwrap().ends_with("it has team"),
        "{o}"
    );

    // a name that names nothing: refused, with what there is
    let text = "wisp: 1\ntask: { prompt: x, agent: nosuch, connectors: [jira], goal: blue }\nformula: [./wisp.yml]\n";
    let o = ask(json!({ "text": text, "name": "x.task.yml", "cwd": cwd })).await;
    let p: Vec<String> = serde_json::from_value(o["problems"].clone()).unwrap();
    assert!(p.contains(&"x.task.yml: no agent named nosuch: agents has critic (or codex, auto)".to_owned()), "{p:?}");
    assert!(p.contains(&"x.task.yml: no connector named jira: no connectors are declared".to_owned()), "{p:?}");
    assert!(p.contains(&"x.task.yml: no goal named blue: no goals are declared".to_owned()), "{p:?}");

    // a loop: the same file twice, or the task file itself
    let o =
        ask(json!({ "text": "wisp: 1\ntask: { prompt: x }\nformula: [./wisp.yml, ./wisp.yml]\n", "name": "x.task.yml", "cwd": cwd })).await;
    assert!(o["problems"][0].as_str().unwrap().ends_with("wisp.yml is named twice"), "{o}");
    let o = ask(json!({ "text": "wisp: 1\ntask: { prompt: x }\nformula: ./x.task.yml\n", "name": "x.task.yml", "cwd": cwd })).await;
    assert!(o["problems"][0].as_str().unwrap().ends_with("is this task file itself"), "{o}");

    // a declared task by its name, from the files: its params from words, its goal inlined
    let o = ask(json!({ "files": ["wisp.yml"], "cwd": cwd, "run": "release", "words": ["2.1"] })).await;
    assert_eq!(o.get("problems"), None, "{o}");
    assert_eq!((o["task"]["prompt"].as_str(), o["task"]["agent"].as_str()), (Some("release 2.1"), Some("critic")));
    assert_eq!(o["task"]["goal"]["text"], "the build is green");
    let o = ask(json!({ "files": ["wisp.yml"], "cwd": cwd, "run": "nope" })).await;
    assert_eq!(o["problems"], json!(["no task named nope in the formula: tasks has release"]));
    let _ = std::fs::remove_dir_all(&d);
}

/// Review r194 #5: a task file's `formula: ./x` is read from the task file's own folder when its path is known (`dir`);
/// a picked file with no path is read from the project folder, and the answer says so.
#[tokio::test]
async fn a_task_files_formula_is_read_from_its_own_folder_or_the_project_said() {
    let store = Store::open(Location::Memory, Arc::new(SystemClock)).unwrap();
    let k = TestApp::new().with(wisp_resources::ResourcesPlugin).with(Kinds).service(store).start().await;
    let d = tmp("dir");
    std::fs::create_dir_all(d.join("tasks")).unwrap();
    std::fs::write(d.join("wisp.yml"), "wisp: 1\nagents:\n  root-agent: {}\n").unwrap();
    std::fs::write(d.join("tasks/wisp.yml"), "wisp: 1\nagents:\n  beside: {}\n").unwrap();
    let text = "wisp: 1\ntask: { prompt: x }\nformula: ./wisp.yml\n";
    let (cwd, dir) = (d.display().to_string(), d.join("tasks").display().to_string());
    let o =
        k.call(Caller::user(), "resources.overlay", json!({ "text": text, "name": "x.task.yml", "cwd": cwd, "dir": dir })).await.unwrap();
    assert_eq!(o["overlay"]["agents"], json!([{ "name": "beside" }]), "{o}");
    assert_eq!(o.get("notes"), None, "{o}");
    let o = k.call(Caller::user(), "resources.overlay", json!({ "text": text, "name": "x.task.yml", "cwd": cwd })).await.unwrap();
    assert_eq!(o["overlay"]["agents"], json!([{ "name": "root-agent" }]), "{o}");
    assert_eq!(
        o["notes"],
        json!([format!("x.task.yml: ./wisp.yml was read from the project folder {cwd}: a picked file's own folder isn't known here")]),
        "{o}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
