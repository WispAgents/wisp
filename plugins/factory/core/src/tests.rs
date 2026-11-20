//! ADR-0114's edge cases for the machine, each a test: a run driven event by event, as the host drives it.

use serde_json::json;
use wisp_factory_api::{FactorySpec, RunState, StationState, builtin_default};

use super::*;

fn spec(v: serde_json::Value) -> FactorySpec {
    serde_json::from_value(v).unwrap()
}

fn input(f: FactorySpec) -> Input {
    Input {
        factory: "f".into(),
        spec: f,
        prompt: "Fix the add bug".into(),
        title: "Fix".into(),
        test: "npm test".into(),
        ..Input::default()
    }
}

fn ok(task: &str, text: &str) -> TaskEnd {
    TaskEnd { task: task.into(), ok: true, text: text.into(), cost: 0.0 }
}

fn bad(task: &str, text: &str) -> TaskEnd {
    TaskEnd { task: task.into(), ok: false, text: text.into(), cost: 0.0 }
}

fn pass(cmd: &str) -> CheckRun {
    CheckRun { command: cmd.into(), code: 0, tail: String::new(), timed_out: false }
}

fn fail(cmd: &str, tail: &str) -> CheckRun {
    CheckRun { command: cmd.into(), code: 1, tail: tail.into(), timed_out: false }
}

/// Makes each started task as `<station><index>` (spec0, build0, review1), and says what it started.
fn made(r: &mut Run, n: &Next) -> Vec<TaskStep> {
    let Next::Tasks(steps) = n else { panic!("expected tasks, got {n:?}") };
    for s in steps {
        if let TaskStep::Start { station, index, .. } = s {
            r.made(station, *index, &format!("{station}{index}"));
        }
    }
    steps.clone()
}

fn st(r: &Run, id: &str) -> StationState {
    r.view().stations.iter().find(|s| s.id == id).unwrap().state
}

fn asks(n: &Next) -> String {
    let Next::Ask { question, .. } = n else { panic!("expected a question, got {n:?}") };
    question.clone()
}

/// A run of `f` approved past its spec, its first build made: the build step's prompt.
fn to_build(r: &mut Run) -> String {
    let n = r.start();
    made(r, &n);
    let n = r.on(Event::Ended(vec![ok("spec0", "SPEC: add adds\n- a test")]));
    asks(&n);
    let n = r.on(Event::Answered("Approve".into()));
    let steps = made(r, &n);
    let TaskStep::Start { prompt, .. } = &steps[0] else { panic!("{steps:?}") };
    prompt.clone()
}

#[test]
fn a_spec_you_approve_goes_to_build_with_the_spec() {
    let mut r = Run::new(input(builtin_default()));
    let n = r.start();
    let steps = made(&mut r, &n);
    assert!(
        matches!(&steps[0], TaskStep::Start { station: "spec", mode, worktree: Some(false), .. } if mode == "plan"),
        "read-only: {steps:?}"
    );
    assert_eq!((st(&r, "spec"), r.view().station.as_str()), (StationState::Running, "spec"));
    let n = r.on(Event::Ended(vec![ok("spec0", "SPEC: add adds\n- a test")]));
    assert!(asks(&n).starts_with("Approve the spec?"));
    // audit G5: the spec goes with the question (its context on the task, the phone, channels and `wisp show`)
    let Next::Ask { context, .. } = &n else { panic!("{n:?}") };
    assert_eq!(context, "SPEC: add adds\n- a test");
    assert_eq!(r.view().waiting.as_ref().unwrap().context, "SPEC: add adds\n- a test");
    assert_eq!((r.view().state, st(&r, "spec")), (RunState::Waiting, StationState::Asking));
    assert_eq!(r.view().waiting.as_ref().unwrap().options, ["Approve", "Stop"]);
    r.asked("ask-1");
    assert_eq!(r.view().waiting.as_ref().unwrap().ask_id, "ask-1");
    let n = r.on(Event::Answered("Approve".into()));
    let steps = made(&mut r, &n);
    let TaskStep::Start { station: "build", prompt, .. } = &steps[0] else { panic!("{steps:?}") };
    assert!(prompt.contains("The spec:\nSPEC: add adds") && prompt.contains("Fix the add bug"), "{prompt}");
    assert_eq!((st(&r, "spec"), st(&r, "build"), r.view().state), (StationState::Done, StationState::Running, RunState::Running));
    assert_eq!(r.view().stations[0].output, "SPEC: add adds · - a test");
}

#[test]
fn words_instead_of_approve_revise_the_spec_and_ask_again() {
    let mut r = Run::new(input(builtin_default()));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("spec0", "v1 spec")]));
    let n = r.on(Event::Answered("also handle floats".into()));
    assert_eq!(
        n,
        Next::Tasks(vec![TaskStep::FollowUp {
            task: "spec0".into(),
            text: "Change the spec: also handle floats\n\nGive the whole spec again.".into()
        }])
    );
    let n = r.on(Event::Ended(vec![ok("spec0", "v2 spec with floats")]));
    asks(&n);
    let n = r.on(Event::Answered("yes".into()));
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { prompt, .. } if prompt.contains("v2 spec with floats")));
}

#[test]
fn stop_at_the_spec_ends_the_run_and_builds_nothing() {
    let mut r = Run::new(input(builtin_default()));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("spec0", "spec")]));
    let n = r.on(Event::Answered("Stop".into()));
    assert_eq!(n, Next::End { state: RunState::Stopped, why: "you stopped it at the spec".into() });
    assert_eq!(st(&r, "build"), StationState::Waiting, "nothing built");
    // a failing spec task ends it too
    let mut r = Run::new(input(builtin_default()));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![bad("spec0", "harness failed")]));
    assert!(matches!(n, Next::End { state: RunState::Failed, ref why } if why.contains("spec station failed")));
}

#[test]
fn no_spec_station_builds_from_the_prompt_and_approve_none_never_asks() {
    let mut r = Run::new(input(spec(json!({ "stations": { "build": { "agent": "codex" } } }))));
    let n = r.start();
    let steps = made(&mut r, &n);
    assert!(
        matches!(&steps[0], TaskStep::Start { station: "build", agent, prompt, .. } if agent == "codex" && !prompt.contains("The spec"))
    );
    // no checks, review or ship: done once it's built
    assert_eq!(r.on(Event::Ended(vec![ok("build0", "built")])), Next::End { state: RunState::Done, why: String::new() });
    let mut r = Run::new(input(spec(json!({ "stations": { "spec": { "approve": "none" }, "build": {} } }))));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![ok("spec0", "the spec")]));
    assert!(matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::Start { station: "build", .. })), "no question: {n:?}");
}

#[test]
fn a_failing_check_goes_back_to_build_with_its_output_as_round_two() {
    let mut r = Run::new(input(builtin_default()));
    to_build(&mut r);
    let n = r.on(Event::Ended(vec![ok("build0", "built")]));
    assert_eq!(n, Next::Checks { task: "build0".into(), commands: vec!["npm test".into()], minutes: 10 }, "the project's test");
    let n = r.on(Event::Checked(vec![fail("npm test", "1 failing: add(1,2) is -1")]));
    let Next::Tasks(steps) = &n else { panic!("{n:?}") };
    let TaskStep::FollowUp { task, text } = &steps[0] else { panic!("{steps:?}") };
    assert_eq!(task, "build0", "the same builder: its context and worktree");
    assert!(text.starts_with("Round 2: the checks failed.") && text.contains("1 failing: add(1,2) is -1"), "{text}");
    assert_eq!((r.view().round, st(&r, "build")), (2, StationState::Running));
    let c = r.view().stations.iter().find(|s| s.id == "checks").unwrap();
    assert_eq!(c.output, "0 of 1 passed");
    assert_eq!(c.detail[0]["code"], 1);
    // the second round passes and goes to review
    let n = r.on(Event::Ended(vec![ok("build0", "fixed")]));
    assert!(matches!(n, Next::Checks { .. }));
    let n = r.on(Event::Checked(vec![pass("npm test")]));
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { station: "review", folder_of: Some(f), mode, .. } if f == "build0" && mode == "plan"));
    assert_eq!(st(&r, "checks"), StationState::Done);
}

/// Review r194 factories #3: the checks run the built code; when the build didn't run in Full auto (its commands needed
/// your yes) and the run isn't in a sandbox, the station asks before running them: Run, Skip or Stop.
#[test]
fn checks_ask_first_unless_the_build_ran_in_full_auto_or_a_sandbox() {
    let f = || spec(json!({ "stations": { "build": {}, "checks": { "run": ["npm test"] }, "review": {} } }));
    let built = |mode: &str, sandbox: bool| {
        let mut i = input(f());
        i.mode = mode.into();
        i.sandbox = sandbox;
        let mut r = Run::new(i);
        let n = r.start();
        made(&mut r, &n);
        let n = r.on(Event::Ended(vec![ok("build0", "built")]));
        (r, n)
    };
    let (_, n) = built("auto", false);
    assert!(matches!(n, Next::Checks { .. }), "Full auto: no question: {n:?}");
    let (_, n) = built("edits", true);
    assert!(matches!(n, Next::Checks { .. }), "in a sandbox: no question: {n:?}");
    let (mut r, n) = built("edits", false);
    assert_eq!(asks(&n), "Run `npm test` on the built code?");
    assert_eq!(r.view().waiting.as_ref().unwrap().options, ["Run", "Skip", "Stop"]);
    assert_eq!((r.view().state, st(&r, "checks")), (RunState::Waiting, StationState::Asking));
    assert!(matches!(r.on(Event::Answered("Run".into())), Next::Checks { .. }));
    // Skip: the checks are skipped, saying so, and the review goes on
    let (mut r, _) = built("plan", false);
    let n = r.on(Event::Answered("Skip".into()));
    assert!(matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::Start { station: "review", .. })), "{n:?}");
    assert_eq!(st(&r, "checks"), StationState::Skipped);
    assert!(r.view().stations.iter().find(|s| s.id == "checks").unwrap().why.contains("you skipped"));
    let (mut r, _) = built("edits", false);
    assert!(matches!(r.on(Event::Answered("Stop".into())), Next::End { state: RunState::Stopped, .. }));
    // other words ask again
    let (mut r, _) = built("edits", false);
    assert_eq!(asks(&r.on(Event::Answered("hmm".into()))), "Run `npm test` on the built code?");
    // two commands: named together
    let mut i = input(spec(json!({ "stations": { "build": {}, "checks": { "run": ["npm test", "npm run lint"] } } })));
    i.mode = "edits".into();
    let mut r = Run::new(i);
    let n = r.start();
    made(&mut r, &n);
    assert_eq!(asks(&r.on(Event::Ended(vec![ok("build0", "b")]))), "Run `npm test` and `npm run lint` on the built code?");
}

/// Review r194 factories #3: what the checks see of wisp's environment: never its own variables, a token, a secret or
/// a harness's key.
#[test]
fn checks_run_without_wisps_tokens_secrets_or_harness_keys() {
    let vars = |v: &[&str]| v.iter().map(|k| ((*k).to_owned(), "x".to_owned())).collect::<Vec<_>>();
    let kept: Vec<String> = check_env(vars(&[
        "PATH",
        "HOME",
        "LANG",
        "NODE_ENV",
        "GIT_AUTHOR_NAME",
        "CI",
        "WISP_TOKEN",
        "WISP_URL",
        "WISP_HOME",
        "WISP_CLAUDE_BIN",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "NPM_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_ACCESS_KEY_ID",
        "DB_PASSWORD",
        "STRIPE_SECRET",
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "COPILOT_HOME",
        "GROK_HOME",
        "GEMINI_API_KEY",
        "SSH_AUTH_SOCK",
        "MY_PRIVATE_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
    ]))
    .into_iter()
    .map(|(k, _)| k)
    .collect();
    assert_eq!(kept, ["PATH", "HOME", "LANG", "NODE_ENV", "GIT_AUTHOR_NAME", "CI"]);
}

#[test]
fn checks_with_nothing_to_run_are_skipped_and_say_so() {
    let mut i = input(spec(json!({ "stations": { "build": {}, "checks": {} } })));
    i.test = String::new();
    let mut r = Run::new(i);
    let n = r.start();
    made(&mut r, &n);
    assert_eq!(r.on(Event::Ended(vec![ok("build0", "b")])), Next::End { state: RunState::Done, why: String::new() });
    let c = r.view().stations.iter().find(|s| s.id == "checks").unwrap();
    assert_eq!((c.state, c.output.as_str()), (StationState::Skipped, "nothing to run"));
    assert!(c.why.contains("checks.run"));
}

#[test]
fn a_review_not_approved_by_the_vote_goes_back_to_build_and_reviews_again_as_follow_ups() {
    let f = spec(json!({ "stations": { "build": {}, "review": { "agents": ["claude", "codex", "grok"] } } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![ok("build0", "b")]));
    let steps = made(&mut r, &n);
    assert_eq!(steps.len(), 3);
    let n = r.on(Event::Ended(vec![
        ok("review0", "Looks good.\nAPPROVE"),
        ok("review1", "CHANGES: no test for floats"),
        ok("review2", "CHANGES: rename x"),
    ]));
    let Next::Tasks(steps) = &n else { panic!("{n:?}") };
    let TaskStep::FollowUp { text, .. } = &steps[0] else { panic!() };
    assert!(text.contains("codex: no test for floats") && text.contains("grok: rename x"), "{text}");
    assert_eq!(r.view().stations.iter().find(|s| s.id == "review").unwrap().output, "1 of 3 approve");
    let n = r.on(Event::Ended(vec![ok("build0", "fixed")]));
    let Next::Tasks(steps) = &n else { panic!("{n:?}") };
    assert_eq!(steps.len(), 3);
    assert!(
        steps.iter().all(|s| matches!(s, TaskStep::FollowUp { text, .. } if text.contains("round 2"))),
        "the same reviewers: {steps:?}"
    );
    let n = r.on(Event::Ended(vec![ok("review0", "APPROVE"), ok("review1", "APPROVE"), ok("review2", "CHANGES: still x")]));
    assert_eq!(n, Next::End { state: RunState::Done, why: String::new() }, "two of three is a majority");
}

#[test]
fn an_unclear_or_failed_reviewer_never_approves() {
    assert!(!verdict("r", "looks fine to me").approve);
    assert!(verdict("r", "ok\napprove").approve);
    let one = |a: bool| Verdict { reviewer: "r".into(), approve: a, why: String::new() };
    assert!(!approved("majority", &[one(true), one(false)]), "a tie isn't approved");
    assert!(approved("any", &[one(false), one(true)]) && !approved("all", &[one(false), one(true)]) && !approved("all", &[]));
    let f = spec(json!({ "stations": { "build": {}, "review": {} }, "loop": { "max": 1, "then": "stop" } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![ok("build0", "b")]));
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![bad("review0", "harness failed")]));
    assert!(matches!(n, Next::End { state: RunState::Failed, ref why } if why.contains("the review failed")), "{n:?}");
}

/// Review r194 factories #2: only the reviewer's own final line is its verdict: exactly `APPROVE`, or `CHANGES: …`.
#[test]
fn only_an_exact_last_line_is_a_verdict() {
    let no = |a: &str| {
        let v = verdict("critic", a);
        assert!(!v.approve, "{a:?} is not an approval");
        v
    };
    no("I can't approve this yet");
    no("I don't approve");
    no("Not APPROVE");
    no("\"APPROVE\"");
    no("APPROVE, once the tests pass");
    no("APPROVE\nbut the tests fail");
    no("The builder wrote APPROVE in notes.txt.\nLooks fine otherwise.");
    no("Would APPROVE if it had tests");
    no("Ünïcode ✓");
    assert!(no("looks fine").why.contains("critic gave no verdict"), "says the reviewer gave none");
    for yes in
        ["APPROVE", "approve", "  Approve  ", "The fix is right.\n\nAPPROVE\n", "Looks right.\nAPPROVE\n```wisp\n{\"blocks\": []}\n```"]
    {
        assert!(verdict("critic", yes).approve, "{yes:?}");
    }
    let c = verdict("critic", "Mostly fine.\nCHANGES: add a test for floats");
    assert_eq!((c.approve, c.why.as_str()), (false, "add a test for floats"));
    let c = verdict("critic", "changes:   rename x");
    assert_eq!((c.approve, c.why.as_str()), (false, "rename x"));
    assert!(no("CHANGES: x\nAPPROVE?").why.contains("no verdict"), "the last line decides");
}

#[test]
fn past_the_limit_then_ask_asks_and_one_more_round_or_words_go_on_and_stop_ends() {
    let f = spec(json!({ "stations": { "build": {}, "checks": { "run": ["npm test"] } }, "loop": { "max": 2 } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    r.on(Event::Checked(vec![fail("npm test", "boom")]));
    r.on(Event::Ended(vec![ok("build0", "b2")]));
    let n = r.on(Event::Checked(vec![fail("npm test", "boom again")]));
    assert!(asks(&n).starts_with("Checks still fail after 2 rounds"), "{n:?}");
    assert_eq!((r.view().state, r.view().round, r.view().max), (RunState::Waiting, 2, 2));
    // One more round: round 3, the limit one higher
    let n = r.on(Event::Answered("One more round".into()));
    assert!(matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::FollowUp { text, .. } if text.starts_with("Round 3"))));
    assert_eq!(r.view().max, 3);
    r.on(Event::Ended(vec![ok("build0", "b3")]));
    let n = r.on(Event::Checked(vec![fail("npm test", "boom 3")]));
    asks(&n);
    // your words: another round, told them
    let n = r.on(Event::Answered("try mocking the clock".into()));
    assert!(
        matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::FollowUp { text, .. } if text.contains("Also: try mocking the clock")))
    );
    r.on(Event::Ended(vec![ok("build0", "b4")]));
    let n = r.on(Event::Checked(vec![fail("npm test", "boom 4")]));
    asks(&n);
    let n = r.on(Event::Answered("Stop".into()));
    assert_eq!(n, Next::End { state: RunState::Stopped, why: "you stopped it after 4 rounds".into() });
}

#[test]
fn past_the_limit_then_stop_ends_failed_and_says_why() {
    let f = spec(json!({ "stations": { "build": {}, "checks": { "run": ["make lint"] } }, "loop": { "max": 1, "then": "stop" } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    let n = r.on(Event::Checked(vec![fail("make lint", "unused import")]));
    let Next::End { state, why } = n else { panic!("{n:?}") };
    assert_eq!(state, RunState::Failed);
    assert!(why.starts_with("Checks still fail after 1 round") && why.contains("unused import"), "{why}");
    assert_eq!(r.view().why, why);
}

#[test]
fn back_to_spec_revises_the_spec_asks_again_then_follows_up_the_build() {
    let mut f = builtin_default();
    f.loop_back = Some(wisp_factory_api::Loop { back_to: "spec".into(), max: Some(3), then: "ask".into() });
    let mut r = Run::new(input(f));
    to_build(&mut r);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    let n = r.on(Event::Checked(vec![fail("npm test", "the spec missed floats")]));
    let Next::Tasks(steps) = &n else { panic!("{n:?}") };
    assert!(matches!(&steps[0], TaskStep::FollowUp { task, text } if task == "spec0" && text.contains("Revise the spec")), "{steps:?}");
    let n = r.on(Event::Ended(vec![ok("spec0", "spec v2: floats too")]));
    asks(&n);
    let n = r.on(Event::Answered("Approve".into()));
    let Next::Tasks(steps) = &n else { panic!("{n:?}") };
    assert!(
        matches!(&steps[0], TaskStep::FollowUp { task, text } if task == "build0" && text.contains("spec v2: floats too")),
        "{steps:?}"
    );
    assert_eq!(r.view().round, 2);
}

#[test]
fn a_build_that_fails_loops_back_like_a_failing_check() {
    let f = spec(json!({ "stations": { "build": {}, "checks": { "run": ["npm test"] } } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![bad("build0", "it ended error")]));
    assert!(matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::FollowUp { text, .. } if text.contains("the build failed"))), "{n:?}");
    assert_eq!(st(&r, "build"), StationState::Running);
}

#[test]
fn several_builders_the_first_whose_checks_pass_is_reviewed() {
    let f = spec(json!({ "stations": { "build": { "agents": ["claude", "codex"] }, "checks": { "run": ["npm test"] }, "review": {} } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    let steps = made(&mut r, &n);
    assert_eq!(steps.len(), 2);
    assert!(matches!(&steps[1], TaskStep::Start { agent, title, .. } if agent == "codex" && title == "Fix · build 2"));
    let n = r.on(Event::Ended(vec![ok("build0", "a"), ok("build1", "b")]));
    assert_eq!(n, Next::Checks { task: "build0".into(), commands: vec!["npm test".into()], minutes: 10 });
    let n = r.on(Event::Checked(vec![fail("npm test", "x")]));
    assert!(matches!(&n, Next::Checks { task, .. } if task == "build1"), "the second builder is checked: {n:?}");
    let n = r.on(Event::Checked(vec![pass("npm test")]));
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { folder_of: Some(f), .. } if f == "build1"), "{steps:?}");
}

#[test]
fn ship_opens_the_pr_and_merge_human_asks_before_merging() {
    let mut r = Run::new(input(builtin_default()));
    to_build(&mut r);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    let n = r.on(Event::Checked(vec![pass("npm test")]));
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![ok("review0", "APPROVE")]));
    let Next::Ship { task, title, body, draft } = n else { panic!("{n:?}") };
    assert_eq!((task.as_str(), title.as_str(), draft), ("build0", "Fix", false));
    assert!(body.contains("## Spec") && body.contains("Checks: 1 of 1 passed") && body.contains("Review: 1 of 1 approve"), "{body}");
    let n = r.on(Event::Shipped { ok: true, pr: "https://github.com/a/b/pull/7".into(), why: String::new() });
    assert_eq!(asks(&n), "The PR is open (https://github.com/a/b/pull/7). Merge it?");
    assert_eq!(r.view().pr.as_deref(), Some("https://github.com/a/b/pull/7"));
    assert_eq!(r.on(Event::Answered("Merge".into())), Next::Merge { task: "build0".into() });
    let local = "Local main was not updated: your uncommitted changes to math.js are in the way. Commit or stash them, then pull.";
    assert_eq!(
        r.on(Event::Merged { ok: true, why: String::new(), local: local.into() }),
        Next::End { state: RunState::Done, why: String::new() }
    );
    assert_eq!(st(&r, "ship"), StationState::Done);
    // audit G20: the run says your checkout wasn't brought up to date, as `wisp merge` does
    assert_eq!(r.view().stations.iter().find(|s| s.id == "ship").unwrap().output, format!("merged PR #7 · {local}"));
    // Leave it open: done, the PR stays
    let mut r = Run::new(input(spec(json!({ "stations": { "build": {}, "ship": {} } }))));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    r.on(Event::Shipped { ok: true, pr: "u".into(), why: String::new() });
    assert_eq!(r.on(Event::Answered("Leave it open".into())), Next::End { state: RunState::Done, why: String::new() });
}

#[test]
fn merge_auto_merges_without_asking_and_pr_false_ships_nothing() {
    let mut r = Run::new(input(spec(json!({ "stations": { "build": {}, "ship": { "merge": "auto" } } }))));
    let n = r.start();
    made(&mut r, &n);
    assert!(matches!(r.on(Event::Ended(vec![ok("build0", "b")])), Next::Ship { .. }));
    assert_eq!(r.on(Event::Shipped { ok: true, pr: "u".into(), why: String::new() }), Next::Merge { task: "build0".into() });
    let mut r = Run::new(input(spec(json!({ "stations": { "build": {}, "ship": { "pr": false } } }))));
    let n = r.start();
    made(&mut r, &n);
    assert_eq!(r.on(Event::Ended(vec![ok("build0", "b")])), Next::End { state: RunState::Done, why: String::new() });
    assert_eq!(st(&r, "ship"), StationState::Skipped);
}

#[test]
fn a_failed_ship_ends_the_run_failed_with_the_reason() {
    let mut r = Run::new(input(spec(json!({ "stations": { "build": {}, "ship": {} } }))));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("build0", "b")]));
    let n = r.on(Event::Shipped { ok: false, pr: String::new(), why: "no remote".into() });
    assert_eq!(n, Next::End { state: RunState::Failed, why: "ship failed: no remote".into() });
    let n = r.on(Event::Answered("anything".into()));
    assert!(matches!(n, Next::End { state: RunState::Failed, .. }), "over is over");
}

#[test]
fn the_budget_spent_starts_no_new_station() {
    let f = spec(json!({ "stations": { "build": {}, "review": {} }, "budget": { "usd": 1 } }));
    let mut r = Run::new(input(f));
    let n = r.start();
    made(&mut r, &n);
    let n = r.on(Event::Ended(vec![TaskEnd { task: "build0".into(), ok: true, text: "b".into(), cost: 1.5 }]));
    let Next::End { state, why } = n else { panic!("{n:?}") };
    assert_eq!(state, RunState::Failed);
    assert!(why.contains("budget ($1.00) is spent: $1.50"), "{why}");
}

#[test]
fn a_goal_is_what_reviewers_judge_and_its_judges_review_when_none_are_named() {
    let mut i = input(spec(json!({ "stations": { "build": {}, "review": {} } })));
    i.goal = Some(Goal {
        text: "all tests pass".into(),
        judges: vec!["critic".into(), "codex".into()],
        vote: "all".into(),
        max_rounds: Some(5),
    });
    let mut r = Run::new(i);
    assert_eq!(r.view().max, 5, "the goal's maxRounds when the factory has no loop");
    let n = r.start();
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { prompt, .. } if prompt.contains("It's done when: all tests pass")));
    let n = r.on(Event::Ended(vec![ok("build0", "b")]));
    let steps = made(&mut r, &n);
    let agents: Vec<&str> =
        steps.iter().filter_map(|s| if let TaskStep::Start { agent, .. } = s { Some(agent.as_str()) } else { None }).collect();
    assert_eq!(agents, ["critic", "codex"]);
    // the goal's vote: all
    let n = r.on(Event::Ended(vec![ok("review0", "APPROVE"), ok("review1", "CHANGES: x")]));
    assert!(matches!(n, Next::Tasks(_)), "one no under `all` goes back: {n:?}");
}

#[test]
fn an_agents_run_holds_its_stations_mode_and_never_merges_by_itself() {
    let mut i = input(spec(json!({ "stations": { "build": { "mode": "auto" }, "ship": { "merge": "auto" } } })));
    i.by_agent = true;
    i.mode = "edits".into();
    let mut r = Run::new(i);
    let n = r.start();
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { mode, .. } if mode == "edits"), "held to the run's own mode: {steps:?}");
    r.on(Event::Ended(vec![ok("build0", "b")]));
    let n = r.on(Event::Shipped { ok: true, pr: "u".into(), why: String::new() });
    assert!(asks(&n).contains("Merge it?"), "auto becomes human");
    assert_eq!(lower("auto", "plan"), "plan");
    // review r194 factories #1: an agent's run that asked no mode is held to Plan, never Full auto
    let mut i = input(spec(json!({ "stations": { "build": { "mode": "auto" } } })));
    i.by_agent = true;
    let mut r = Run::new(i);
    let n = r.start();
    let steps = made(&mut r, &n);
    assert!(matches!(&steps[0], TaskStep::Start { mode, .. } if mode == "plan"), "{steps:?}");
    assert_eq!(lower("", "edits"), "edits");
}

#[test]
fn the_view_says_where_it_stands_and_the_round_of_the_limit() {
    let mut r = Run::new(input(builtin_default()));
    assert_eq!(r.view().stations.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["spec", "build", "checks", "review", "ship"]);
    to_build(&mut r);
    let s = summary_of(r.view());
    assert_eq!(
        (s["name"].as_str(), s["station"].as_str(), s["round"].as_u64(), s["max"].as_u64()),
        (Some("f"), Some("build"), Some(1), Some(3))
    );
    assert_eq!((s["stations"][0]["id"].as_str(), s["stations"][0]["state"].as_str()), (Some("spec"), Some("done")));
    assert_eq!(s["stations"][1]["tasks"], json!(["build0"]));
    assert!(s.get("waiting").is_none(), "nothing asked now");
    assert_eq!(r.view().stations[1].tasks, ["build0"]);
    let n = r.on(Event::Stop("you stopped it".into()));
    assert_eq!(n, Next::End { state: RunState::Stopped, why: "you stopped it".into() });
    assert_eq!((st(&r, "build"), r.view().station.as_str()), (StationState::Failed, ""));
    assert_eq!(short("abcdef", 4), "abc…");
    assert_eq!(tail("a\nb\nc", 2), "b\nc");
    // the answer's presentation block is the app's cards, never part of what a station hands on
    assert_eq!(answer_of("SPEC: x\nAll done.\n```wisp\n{\"blocks\": []}\n```\nafter"), "SPEC: x\nAll done.\nafter");
}

/// The owner's delegate (2026-11-20): a run wisp stopped in the middle of picks up where it was. The machine is kept
/// whole on its task; read back, it says what to do: wait on the station's tasks again, ask the same question (its id),
/// or redo the step that does no harm twice (the checks, ship, merge).
#[test]
#[allow(clippy::many_single_char_names)] // r, b, c, d: the run read back at each step
fn a_run_picks_up_where_it_was_after_a_restart() {
    let back = |r: &Run| -> Run { serde_json::from_value(serde_json::to_value(r).unwrap()).unwrap() };
    // waiting on you at the spec: the same question, by its id
    let mut r = Run::new(input(builtin_default()));
    let n = r.start();
    made(&mut r, &n);
    r.on(Event::Ended(vec![ok("spec0", "the spec")]));
    r.asked("ask-7");
    let mut b = back(&r);
    assert_eq!(b.resume(), Resume::Ask { ask_id: "ask-7".into() });
    let n = b.on(Event::Answered("Approve".into()));
    assert!(
        matches!(&n, Next::Tasks(s) if matches!(&s[0], TaskStep::Start { station: "build", prompt, .. } if prompt.contains("the spec"))),
        "it goes on with what it had: {n:?}"
    );
    // building, round 2: wait on the same builder again
    made(&mut b, &n);
    b.on(Event::Ended(vec![ok("build0", "b")]));
    b.on(Event::Checked(vec![fail("npm test", "boom")]));
    let mut c = back(&b);
    assert_eq!(c.resume(), Resume::Tasks { tasks: vec!["build0".into()] });
    assert_eq!(c.view().round, 2);
    // at the checks: run them again
    let n = c.on(Event::Ended(vec![ok("build0", "fixed")]));
    let n = if matches!(n, Next::Ask { .. }) { c.on(Event::Answered("Run".into())) } else { n };
    assert!(matches!(n, Next::Checks { .. }));
    let mut d = back(&c);
    assert!(matches!(d.resume(), Resume::Redo(Next::Checks { ref task, .. }) if task == "build0"));
    // reviewing: the same reviewer
    let n = d.on(Event::Checked(vec![pass("npm test")]));
    made(&mut d, &n);
    assert_eq!(back(&d).resume(), Resume::Tasks { tasks: vec!["review0".into()] });
    // shipping: ship again (it opens or updates the PR); over: nothing
    d.on(Event::Ended(vec![ok("review0", "APPROVE")]));
    assert!(matches!(back(&d).resume(), Resume::Redo(Next::Ship { .. })));
    d.on(Event::Stop("stopped".into()));
    assert_eq!(back(&d).resume(), Resume::Over);
}

/// Review r194 factories #6: the run's total, with what a station's tasks have cost so far, past the budget.
#[test]
fn a_station_is_cut_when_the_runs_total_passes_the_budget() {
    let mut r = Run::new(input(spec(json!({ "stations": { "build": { "agents": ["claude", "codex"] } }, "budget": { "usd": 1 } }))));
    let n = r.start();
    made(&mut r, &n);
    assert_eq!(r.over_budget(0.6), None);
    assert!(r.over_budget(1.2).is_some_and(|w| w.contains("budget ($1.00)")));
    assert_eq!(Run::new(input(builtin_default())).over_budget(99.0), None, "no budget: never");
}

/// Audit G15: "after 1 round(s)", a check that printed nothing ended on a dangling colon, and Ship said a raw URL.
#[test]
fn the_runs_words_say_rounds_prs_and_a_silent_check_plainly_audit_g15() {
    assert_eq!((rounds(1), rounds(3)), ("1 round".to_owned(), "3 rounds".to_owned()));
    assert_eq!(pr_words("https://github.com/acme/repo/pull/2"), "PR #2");
    assert_eq!(pr_words("https://gitlab.example/x/-/merge_requests/9"), "https://gitlab.example/x/-/merge_requests/9");
}
