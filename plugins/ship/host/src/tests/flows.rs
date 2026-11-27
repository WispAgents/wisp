//! Ship, merge and attach (v1 test/ship.test.js), on the fake git.service and the fake gh.

use wisp_kernel::{Call, Caller};
use wisp_ship_api::PrSummary;

use super::h;

#[tokio::test]
async fn ship_commits_pushes_and_opens_a_pr_with_each_step_seen() {
    let h = h();
    let wts = h.track("t1", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    let out = h.ship.ship_all(&h.call, "t1", "fix: the login", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out.pr.number, 1);
    assert_eq!(out.pr.state, "OPEN");
    assert_eq!(h.git.logged("commit").len(), 1);
    assert!(
        h.git.logged("commit")[0].contains("fix: the login|Wisp-Task: t1|all=true"),
        "the commit names its task: {:?}",
        h.git.logged("commit")
    );
    assert_eq!(h.git.logged("push"), [format!("push origin HEAD:refs/heads/{}", wts[0].branch)]);
    assert_eq!(h.steps("t1"), [Some("start".into()), Some("commit".into()), Some("push".into()), Some("pr".into()), None]);
    let create = h.gh.calls("create");
    assert_eq!(create.len(), 1);
    assert!(create[0].windows(2).any(|w| w[0] == "--base" && w[1] == "main"));
    assert_eq!(h.task("t1").repos[0].pr.as_ref().map(|p| p.number), Some(1));
}

/// final4 F1: before the push, the branch's remote tip is caught up with (wisp's history commits merged in); someone
/// else's commits there are refused in words, with no push and never a force.
#[tokio::test]
async fn ship_catches_up_with_the_history_and_refuses_anything_else_on_the_remote() {
    let h = h();
    let wts = h.track("t1", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.git.lock().catch_up = Some(wisp_git_api::CaughtUp::Merged);
    h.ship.ship_all(&h.call, "t1", "fix: one", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.git.logged("catch-up"), [format!("catch-up origin {} .wisp tasks,history", wts[0].branch)]);
    assert_eq!(h.git.logged("push"), [format!("push origin HEAD:refs/heads/{}", wts[0].branch)]);
    h.dirty(&wts[0], 1);
    h.git.lock().catch_up = Some(wisp_git_api::CaughtUp::Diverged { files: vec!["src/app.rs".into()] });
    let e = h.ship.ship_all(&h.call, "t1", "fix: two", "", false).await.map(|_| ()).unwrap_err().to_string();
    assert!(e.contains("has commits this branch doesn't (they change src/app.rs)") && e.contains("never force-pushes"), "{e}");
    assert_eq!(h.git.logged("push").len(), 1, "no second push");
}

#[tokio::test]
async fn ship_refuses_with_v1_words() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    let nothing = h.ship.ship_all(&h.call, "t", "x", "", false).await.map(|_| ()).unwrap_err().to_string();
    assert!(nothing.contains("Nothing to ship"), "{nothing}");
    h.dirty(&wts[0], 2);
    let untitled = h.ship.ship_all(&h.call, "t", "  ", "", false).await.map(|_| ()).unwrap_err().to_string();
    assert!(untitled.contains("A commit message is required"), "{untitled}");
    h.ship.tasks.set_live("t", true);
    let busy = h.ship.ship_all(&h.call, "t", "x", "", false).await.map(|_| ()).unwrap_err().to_string();
    assert!(busy.contains("Still working"), "{busy}");
    assert!(h.ship.ship_all(&h.call, "nope", "x", "", false).await.is_err());
}

#[tokio::test]
async fn ship_without_origin_says_so() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    let mut task = h.task("t");
    task.repos[0].worktree.remote = None;
    h.ship.tasks.put(task);
    h.dirty(&wts[0], 1);
    let e = h.ship.ship_all(&h.call, "t", "x", "", false).await.map(|_| ()).unwrap_err().to_string();
    assert!(e.contains("origin"), "{e}");
}

#[tokio::test]
async fn a_second_ship_updates_the_open_pr() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.ship.ship_all(&h.call, "t", "feat: one", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    h.dirty(&wts[0], 1);
    let again = h.ship.ship_all(&h.call, "t", "feat: two", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(again.pr.number, 1, "the same PR");
    assert_eq!(h.gh.calls("create").len(), 1);
    assert_eq!(h.git.logged("push").len(), 2);
}

/// Edge case Git 8: squash with --match-head-commit, ready a draft first, then only fast-forward the
/// user's checkout; the task's worktree and branch go, here and on the remote.
#[tokio::test]
async fn merge_readies_a_draft_squashes_the_reviewed_head_and_tidies() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.ship.ship_all(&h.call, "t", "feat: x", "", true).await.unwrap_or_else(|e| panic!("{e}"));
    let merged = h.ship.merge_all(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(merged.pr.state, "MERGED");
    assert!(merged.sync.ok);
    assert_eq!(h.gh.calls("ready").len(), 1, "the draft was readied first");
    let m = h.gh.calls("merge");
    assert!(m[0].iter().any(|a| a == "--squash") && m[0].iter().any(|a| a == "--match-head-commit"));
    assert_eq!(h.git.logged("sync").len(), 1);
    assert_eq!(h.git.logged("branch -D"), [format!("branch -D {}", wts[0].branch)]);
    assert!(h.git.logged("push").iter().any(|l| l.contains("--delete")));
    assert!(h.ship.merge_all(&h.call, "t").await.is_err(), "nothing left to merge");
}

/// After a merge the folder and its branch stay (here and on the remote), with a note why, when git can't read the
/// folder, the task has Keep worktree on, it is working, a commit sits on its branch the PR lacks (HEAD elsewhere), or
/// the worktrees plugin couldn't remove it. The branch goes only once the folder did.
#[tokio::test]
async fn a_merge_keeps_the_folder_and_its_branch_unless_its_removal_went() {
    use wisp_ship_core::decide::Tidied;
    async fn merged(h: &crate::tests::H) -> (wisp_ship_api::Worktree, wisp_ship_api::Merged) {
        let wts = h.track("t", &[("repo", None)]);
        h.dirty(&wts[0], 1);
        h.ship.ship_all(&h.call, "t", "feat: x", "", false).await.unwrap_or_else(|e| panic!("{e}"));
        (wts[0].clone(), h.ship.merge_all(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}")))
    }
    let untouched = |h: &crate::tests::H, wt: &wisp_ship_api::Worktree| {
        assert!(std::path::Path::new(&wt.path).is_dir(), "the folder stays");
        assert_eq!(h.git.logged("branch -D"), [] as [String; 0], "its branch stays");
        assert!(!h.git.logged("push").iter().any(|l| l.contains("--delete")), "and on the remote");
    };

    // git status fails: never read as clean, and the worktrees plugin isn't even asked
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.ship.ship_all(&h.call, "t", "feat: x", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    h.git.lock().status_fails = Some("index file corrupt".into());
    let m = h.ship.merge_all(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert!(m.kept.iter().any(|n| n.contains("git status failed") && n.contains("index file corrupt")), "{:?}", m.kept);
    assert_eq!(h.git.logged("tidy"), [] as [String; 0]);
    untouched(&h, &wts[0]);

    for (answer, says) in [
        (Tidied::Kept, "Keep worktree is on"),
        (Tidied::Busy, "still working"),
        (Tidied::Failed("it is locked".into()), "couldn't be removed (it is locked)"),
    ] {
        let h = crate::tests::h();
        *h.tidy.lock().unwrap() = Some(answer);
        let (wt, m) = merged(&h).await;
        assert!(m.kept.iter().any(|n| n.contains(says)), "{says}: {:?}", m.kept);
        assert_eq!(h.git.logged("tidy").len(), 1, "removed through the worktrees plugin");
        untouched(&h, &wt);
    }

    // a commit on the branch the PR doesn't have, while the folder's HEAD is elsewhere
    let h = crate::tests::h();
    h.git.lock().counts.insert("refs/remotes/origin/wisp/repo..refs/heads/wisp/repo".into(), Some(1));
    let (wt, m) = merged(&h).await;
    assert!(m.kept.iter().any(|n| n.contains("1 commit the pull request doesn't have")), "{:?}", m.kept);
    untouched(&h, &wt);

    // and when it went: the branch too, here then on the remote
    let h = crate::tests::h();
    let (wt, m) = merged(&h).await;
    assert!(m.kept.is_empty() && !std::path::Path::new(&wt.path).exists());
    assert_eq!(h.git.logged("branch -D"), [format!("branch -D {}", wt.branch)]);
    assert!(h.git.logged("push").iter().any(|l| l.contains("--delete")));
}

#[tokio::test]
async fn a_foreign_worktree_is_merged_but_never_tidied() {
    let h = h();
    h.track("t", &[("repo", None)]);
    let mut task = h.task("t");
    task.repos[0].worktree.foreign = true;
    h.ship.tasks.put(task);
    h.dirty(&h.task("t").repos[0].worktree, 1);
    h.ship.ship_all(&h.call, "t", "feat: x", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    h.ship.merge_all(&h.call, "t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.git.logged("branch -D"), [] as [std::string::String; 0]);
    assert_eq!(h.git.logged("worktree remove"), [] as [std::string::String; 0]);
}

#[tokio::test]
async fn attach_and_detach_a_pr_by_url() {
    let h = h();
    h.track("t", &[("repo", None)]);
    h.gh.add_pr(serde_json::json!({"number": 9, "url": "https://github.com/acme/docs/pull/9", "title": "Docs", "state": "OPEN", "isDraft": false, "headRefName": "docs-x", "baseRefName": "main", "headRefOid": "abc", "statusCheckRollup": []}));
    let a = h.ship.attach(&h.call, "t", "https://github.com/acme/docs/pull/9", false).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(a.pr.number, 9);
    assert!(a.text.starts_with("Attached PR #9 “Docs” (open)"));
    assert_eq!(h.task("t").prs.len(), 1);
    assert_eq!(h.task("t").prs[0].repo, "docs");
    let d = h.ship.attach(&h.call, "t", "https://github.com/acme/docs/pull/9", true).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(d.text.starts_with("Detached PR #9"));
    assert_eq!(h.task("t").prs, [] as [wisp_ship_api::AttachedPr; 0]);
    assert!(h.ship.attach(&h.call, "t", "https://github.com/acme/docs/pull/404", false).await.is_err());
}

#[tokio::test]
async fn the_panel_reads_each_pr_in_full_and_says_why_when_gh_cannot() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    h.ship.ship_all(&h.call, "t", "feat: x", "", false).await.unwrap_or_else(|e| panic!("{e}"));
    let mut task = h.task("t");
    task.prs.push(wisp_ship_api::AttachedPr {
        repo: "gone".into(),
        pr: PrSummary { number: 77, url: "https://github.com/acme/gone/pull/77".into(), state: "OPEN".into(), ..PrSummary::default() },
    });
    h.ship.tasks.put(task);
    let panel = h.ship.details("t").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(panel.len(), 2);
    assert!(panel[0].more.is_some() && panel[0].error.is_none());
    assert!(panel[1].more.is_none() && panel[1].error.is_some(), "the PR gh can't read keeps what the card knew");
    assert_eq!(panel[1].pr.number, 77);
}

#[tokio::test]
async fn a_caller_without_git_commit_is_refused() {
    let h = h();
    let wts = h.track("t", &[("repo", None)]);
    h.dirty(&wts[0], 1);
    let preview = Call::new_root(Caller { set: "preview".into(), session: None, plugin: None });
    assert!(h.ship.ship_all(&preview, "t", "feat: x", "", false).await.is_err());
    assert!(h.git.logged("commit").is_empty(), "nothing written");
}
