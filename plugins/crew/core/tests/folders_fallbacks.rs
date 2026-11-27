//! A member works in several folders (their own, where each job starts, and more beside it) and carries on with
//! fallbacks you set when a job's run stops at a usage limit (owner, 2026-11-27).
#![allow(clippy::unwrap_used)]

use wisp_crew_api::commands::MemberBody;
use wisp_crew_api::{Fallback, Member};
use wisp_crew_core::cron::CronOnly;
use wisp_crew_core::kit::Ids;
use wisp_crew_core::member::{Checks, normalize_member};

struct Seq(u32);
impl Ids for Seq {
    fn next_id(&mut self) -> String {
        self.0 += 1;
        format!("{:016x}", self.0)
    }
}

fn made(body: &MemberBody, prev: Option<&Member>) -> Result<Member, String> {
    let exists = |p: &str| p.starts_with("/repo");
    let no = |_: &str| false;
    let c = Checks { taken: &[], exists: &exists, machine_ok: &no, words: &CronOnly, agent_ok: &no, default_agent: "claude" };
    normalize_member(body, prev, &c, &mut Seq(0), 1).map_err(|e| e.to_string())
}

fn body(cwd: &str, dirs: &[&str]) -> MemberBody {
    MemberBody {
        name: Some("Ada".into()),
        cwd: Some(cwd.into()),
        dirs: Some(dirs.iter().map(|d| (*d).to_owned()).collect()),
        ..MemberBody::default()
    }
}

#[test]
fn a_member_keeps_more_folders_beside_their_own_once_each_and_never_their_own_twice() {
    let m = made(&body("/repo/app", &["/repo/api", "/repo/api/", "/repo/app", " /repo/docs "]), None).unwrap();
    assert_eq!(m.cwd, "/repo/app");
    assert_eq!(m.dirs, ["/repo/api", "/repo/docs"]);
    // an edit that leaves them out keeps them; moved onto one of them, that one leaves the list
    let moved = made(&MemberBody { cwd: Some("/repo/api".into()), ..MemberBody::default() }, Some(&m)).unwrap();
    assert_eq!((moved.cwd.as_str(), moved.dirs.clone()), ("/repo/api", vec!["/repo/docs".to_owned()]));
    // their own folder taken away takes the others with it; `[]` clears them
    let none = made(&MemberBody { cwd: Some(String::new()), ..MemberBody::default() }, Some(&m)).unwrap();
    assert_eq!(none.dirs, Vec::<String>::new());
    assert_eq!(made(&MemberBody { dirs: Some(vec![]), ..MemberBody::default() }, Some(&m)).unwrap().dirs, Vec::<String>::new());
}

#[test]
fn a_folder_that_isnt_one_here_a_relative_one_or_more_with_no_own_folder_is_refused() {
    assert!(made(&body("/repo/app", &["/elsewhere"]), None).unwrap_err().contains("Not a folder on this computer: /elsewhere"));
    assert!(made(&body("/repo/app", &["repo/api"]), None).unwrap_err().contains("full path"));
    assert!(made(&body("", &["/repo/api"]), None).unwrap_err().contains("their own folder first"));
    let many: Vec<String> = (0..11).map(|i| format!("/repo/{i}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    assert!(made(&body("/repo/app", &many), None).unwrap_err().contains("at most 10"));
}

#[test]
fn fallbacks_are_harnesses_wisp_runs_in_the_order_given_once_each() {
    let fb = |agent: &str, model: &str| Fallback { agent: agent.into(), model: model.into(), effort: String::new() };
    let b = |list: Vec<Fallback>| MemberBody { name: Some("Ada".into()), fallbacks: Some(list), ..MemberBody::default() };
    let m = made(&b(vec![fb("codex", "gpt-5.5"), fb(" claude ", "sonnet"), fb("codex", "gpt-5.5"), fb("", "")]), None).unwrap();
    assert_eq!(m.fallbacks, [fb("codex", "gpt-5.5"), fb("claude", "sonnet")]);
    // kept by an edit that leaves them out
    assert_eq!(made(&MemberBody { role: Some("x".into()), ..MemberBody::default() }, Some(&m)).unwrap().fallbacks.len(), 2);
    assert!(made(&b(vec![fb("auto", "")]), None).unwrap_err().contains("Auto moves on by itself"));
    assert!(made(&b(vec![fb("gpt", "")]), None).unwrap_err().contains("isn't a harness"));
    let bad = Fallback { agent: "codex".into(), model: String::new(), effort: "very hard!".into() };
    assert!(made(&b(vec![bad]), None).unwrap_err().contains("thinking level"));
    let six = ["claude", "codex", "opencode", "cursor", "grok", "copilot"].map(|a| fb(a, "")).to_vec();
    assert!(made(&b(six), None).unwrap_err().contains("at most 5"));
}

#[test]
fn a_jobs_note_names_the_members_other_folders_as_not_every_harness_is_handed_them() {
    let m = Member { name: "Ada".into(), cwd: "/repo/app".into(), dirs: vec!["/repo/api".into()], ..Member::default() };
    let job = wisp_crew_api::CrewExt::default();
    let note = wisp_crew_core::note::crew_note(&m, &job, None, &wisp_crew_core::cron::Utc);
    assert!(note.contains("### Your folders") && note.contains("`/repo/app`") && note.contains("- `/repo/api`"), "{note}");
    let alone = Member { dirs: Vec::new(), ..m };
    assert!(!wisp_crew_core::note::crew_note(&alone, &job, None, &wisp_crew_core::cron::Utc).contains("Your folders"));
}
