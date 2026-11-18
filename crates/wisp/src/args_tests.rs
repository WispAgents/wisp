use super::*;

fn p(args: &[&str]) -> Parsed {
    let v: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    parse(&v, None, Path::new("/home/ada"), Path::new("/work")).unwrap()
}

#[test]
fn tasks_and_commands() {
    assert_eq!(p(&["fix", "the", "tests"]).command, None);
    assert_eq!(p(&["ls"]).command, Some("ls"));
    assert_eq!(p(&["serve"]).command, Some("serve"));
    // a command name that doesn't fit is the first word of a task
    assert_eq!(p(&["ship", "the", "docs"]).command, None);
    assert_eq!(p(&["ship", "0123456789ab"]).command, Some("ship"));
    assert_eq!(p(&["ship", "0190f3c2-5b1e-7c4a-9d2e-0123456789ab"]).command, Some("ship"));
    assert_eq!(p(&["usage", "of", "memory"]).command, None);
    assert_eq!(p(&["run"]).command, None);
    assert_eq!(p(&["run", "deploy"]).command, Some("run"));
    assert_eq!(p(&["grid", "serve"]).command, Some("grid"));
    assert_eq!(p(&["grid", "the", "page"]).command, None);
    assert_eq!(p(&["crew", "export", "ada"]).command, None, "a crew is shared as a formula now");
    assert_eq!(p(&["crew"]).command, Some("crew"));
    assert_eq!(p(&["plugin", "new", "x"]).command, Some("plugin"));
    // `--` first: always a task
    assert_eq!(p(&["--", "ls", "the", "repo"]).command, None);
    assert_eq!(p(&["--", "ls", "the", "repo"]).words, ["ls", "the", "repo"]);
}

#[test]
fn every_grid_command_routes_to_the_grid() {
    for sub in GRID_SUB {
        assert_eq!(p(&["grid", sub]).command, Some("grid"), "{sub}");
    }
}

#[test]
fn flags() {
    let x = p(&["--state=review", "ls", "--port", "5000", "-C", "~/code", "--also", "lib", "--compare", "claude,codex:gpt-5", "--no-kit"]);
    assert_eq!(x.flags.state.as_deref(), Some("review"));
    assert_eq!((x.flags.port, x.flags.port_given), (5000, true));
    assert_eq!(x.flags.cwd, Some(PathBuf::from("/home/ada/code")));
    assert_eq!(x.flags.also, vec![PathBuf::from("/work/lib")]);
    assert_eq!(x.flags.compare.as_ref().map(Vec::len), Some(2));
    assert_eq!(x.flags.compare.as_ref().unwrap()[1], CompareRun { agent: "codex".into(), model: Some("gpt-5".into()) });
    assert!(!x.flags.kit);
    assert_eq!(x.command, Some("ls"));
    assert_eq!(p(&["--version"]).action, Action::Version);
    assert_eq!(p(&["-h"]).action, Action::Help);
}

#[test]
fn a_flag_without_its_value_is_a_usage_error() {
    let v = vec!["--port".to_owned()];
    assert_eq!(parse(&v, None, Path::new("/"), Path::new("/")).unwrap_err(), UsageError("--port".into()));
    let v = vec!["--port".to_owned(), "nope".to_owned()];
    assert!(parse(&v, None, Path::new("/"), Path::new("/")).is_err());
}

#[test]
fn env_port_and_literal_after_words() {
    let v: Vec<String> = ["launch", "--", "--weird"].iter().map(|s| (*s).to_owned()).collect();
    let x = parse(&v, Some("4500"), Path::new("/"), Path::new("/")).unwrap();
    assert_eq!(x.flags.port, 4500);
    assert!(!x.flags.port_given);
    assert_eq!(x.words, ["launch", "--weird"]);
    assert_eq!(x.literal, Literal::Yes);
}

#[test]
fn session_ids() {
    assert!(session_id("0123456789ab"));
    assert!(session_id("0190f3c25b1e7c4a9d2e0123456789ab"));
    assert!(!session_id("0123456789AB"));
    assert!(!session_id("the"));
}

/// QA round 13 #1: `wisp add a README` registered a project "a" for a folder that doesn't exist and lost the prompt.
/// `add` is the command only alone or with one argument that is a folder here or reads as a path or a link; else the
/// words are a task.
#[test]
fn add_is_a_command_only_for_a_folder_a_path_or_a_link() {
    let here = std::env::temp_dir().join(format!("wisp-add-{}", std::process::id()));
    std::fs::create_dir_all(here.join("api")).unwrap();
    let at = |args: &[&str]| {
        let v: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        parse(&v, None, Path::new("/home/ada"), &here).unwrap().command
    };
    for args in [
        &["add"][..],
        &["add", "."],
        &["add", "api"],
        &["add", "./web"],
        &["add", "../other"],
        &["add", "~/code/site"],
        &["add", "/srv/app"],
        &["add", "code/site"],
        &["add", "https://github.com/acme/api"],
        &["add", "git@github.com:acme/api.git"],
    ] {
        assert_eq!(at(args), Some("add"), "{args:?}");
    }
    for args in [
        &["add", "a", "README"][..],
        &["add", "tests", "for", "math.js"],
        &["add", "dark", "mode"],
        &["add", "tests"],
        &["add", "api", "docs"],
    ] {
        assert_eq!(at(args), None, "{args:?} is a task");
    }
    // `-C dir`: the folder is looked for there
    assert_eq!(at(&["-C", here.to_str().unwrap(), "add", "api"]), Some("add"));
    let _ = std::fs::remove_dir_all(&here);
}
