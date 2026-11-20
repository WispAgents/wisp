//! Bring in (v1 server/services/bring-in.js): a project's worktrees that no wisp task knows, made by
//! another tool (Conductor) or by hand with `git worktree add`, offered as tasks once per project until
//! brought in or dismissed. Pure: the host lists the worktrees (git.service), the tasks' folders and the
//! dirt; this decides what the offer holds.

use std::collections::HashSet;

use wisp_worktrees_api::{Offer, OfferItem};

use wisp_runtime_api::paths::{folder_key, is_within, same_folder};

use crate::removal::in_conductor;

/// A worktree git lists for a project's main checkout (`git worktree list`), with its dirt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listed {
    pub path: String,
    /// "" when detached
    pub branch: String,
    pub bare: bool,
    pub prunable: bool,
    pub dirty: u32,
    pub chat: bool,
}

fn norm(p: &str) -> &str {
    p.trim_end_matches(['/', '\\'])
}

fn name_of(path: &str) -> String {
    norm(path).rsplit(['/', '\\']).next().unwrap_or_default().to_owned()
}

/// The offer for one project, or None when there's nothing to bring in. Left out: the main checkout,
/// wisp's own worktrees (`<home>/worktrees/…`), folders a task already has, bare and prunable entries;
/// a dismissed project offers nothing. Folders are compared by `folder_key` (`known` holds those keys): on Windows git
/// lists `C:/x/app` for the `C:\x\app` wisp keeps.
pub fn offer<S: std::hash::BuildHasher>(
    project: &str,
    root: &str,
    listed: &[Listed],
    known: &HashSet<String, S>,
    wisp_dir: &str,
    user_home: &str,
    dismissed: &[String],
) -> Option<Offer> {
    if dismissed.iter().any(|d| d == project) {
        return None;
    }
    let mut items = Vec::new();
    let mut detached = 0;
    let mut conductor = false;
    for l in listed {
        let p = norm(&l.path);
        if p.is_empty() || same_folder(p, root) || l.bare || l.prunable || is_within(p, wisp_dir) || known.contains(&folder_key(p)) {
            continue;
        }
        if l.branch.is_empty() {
            detached += 1;
            continue;
        }
        conductor |= in_conductor(p, user_home);
        items.push(OfferItem { path: p.to_owned(), branch: l.branch.clone(), name: name_of(p), dirty: l.dirty, chat: l.chat });
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Some(Offer {
        project: project.to_owned(),
        name: project.to_owned(),
        from: if conductor { "Conductor" } else { "git" }.to_owned(),
        items,
        detached,
    })
}

/// One offer per folder, under the project whose repository owns it (audit H HS4: `repo3` and `repo3-wt`, a linked
/// worktree of it added as a project, both offered `repo3-wt2`). Projects that are checkouts of one repository list the
/// same worktrees; the one whose root is that repository's main checkout (git lists it first) keeps them, else the one
/// listed first. `listed`: each project's root and the folders git lists for it, in git's order. Back: per project, the
/// folders (by `folder_key`) another project offers instead.
pub fn elsewhere(listed: &[(String, Vec<String>)]) -> Vec<HashSet<String>> {
    let main = |(root, paths): &(String, Vec<String>)| paths.first().is_some_and(|m| same_folder(m, root));
    let mut order: Vec<usize> = (0..listed.len()).collect();
    // stable: the main checkouts first, each group in the order given
    order.sort_by_key(|&i| !listed.get(i).is_some_and(main));
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = vec![HashSet::new(); listed.len()];
    for i in order {
        let Some((_, paths)) = listed.get(i) else { continue };
        for p in paths {
            let k = folder_key(p);
            if !seen.insert(k.clone())
                && let Some(drop) = out.get_mut(i)
            {
                drop.insert(k);
            }
        }
    }
    out
}

/// The paths a Bring in may take: the offer's own, in its order (a path that isn't one, or no longer is, is
/// skipped, never made a task).
/// Paths are compared by `folder_key`, as `offer` does: on Windows git lists `C:/x/stray` for the `C:\x\stray` a client
/// sends back (audit A, Windows CI: the stray worktree was skipped).
pub fn takes<'a>(offer: &'a Offer, paths: &[String]) -> (Vec<&'a OfferItem>, Vec<String>) {
    let want: HashSet<String> = paths.iter().map(|p| folder_key(p)).collect();
    let take: Vec<&OfferItem> = offer.items.iter().filter(|i| want.contains(&folder_key(&i.path))).collect();
    let skipped = paths.iter().filter(|p| !take.iter().any(|i| same_folder(&i.path, p))).cloned().collect();
    (take, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(path: &str, branch: &str) -> Listed {
        Listed { path: path.into(), branch: branch.into(), ..Listed::default() }
    }

    /// Audit H HS4: one worktree, two projects (the main checkout and a linked worktree of it): offered once, under the
    /// main checkout's project, whichever order the projects come in; a repository no project is the main checkout of
    /// offers under the first.
    #[test]
    fn one_worktree_of_two_projects_is_offered_once_under_the_repositorys_main_checkout_audit_h_hs4() {
        let git = vec!["/r/repo3".to_owned(), "/r/repo3-wt".to_owned(), "/r/repo3-wt2".to_owned()];
        for (wt_first, owner, other) in [(true, 1, 0), (false, 0, 1)] {
            let main = ("/r/repo3".to_owned(), git.clone());
            let linked = ("/r/repo3-wt".to_owned(), git.clone());
            let list = if wt_first { vec![linked, main] } else { vec![main, linked] };
            let drop = elsewhere(&list);
            assert!(drop[owner].is_empty(), "the main checkout's project keeps them: {drop:?}");
            assert!(drop[other].contains(&folder_key("/r/repo3-wt2")), "the other offers none of them: {drop:?}");
        }
        let two_linked = vec![
            ("/r/a".to_owned(), vec!["/r/main".into(), "/r/a".into(), "/r/b".into()]),
            ("/r/b".to_owned(), vec!["/r/main".into(), "/r/a".into(), "/r/b".into()]),
        ];
        let drop = elsewhere(&two_linked);
        assert!(drop[0].is_empty() && drop[1].contains(&folder_key("/r/main")));
        assert!(elsewhere(&[("/r/x".to_owned(), vec!["/r/x".into(), "/r/x2".into()])])[0].is_empty(), "one project: nothing moves");
    }

    #[test]
    fn a_projects_foreign_worktrees_are_offered_once_and_wisps_own_are_not() {
        let listed = vec![
            l("/src/app", "main"),
            l("/h/.wisp/worktrees/app-fix", "wisp/fix"),
            l("/src/app-known", "known"),
            l("/u/conductor/workspaces/app/oslo", "oslo"),
            l("/src/app-hand/", "hand"),
            l("/src/app-detached", ""),
            Listed { prunable: true, ..l("/gone", "g") },
        ];
        let known: HashSet<String> = ["/src/app-known".to_owned()].into();
        let o = offer("app", "/src/app", &listed, &known, "/h/.wisp/worktrees", "/u", &[]).unwrap();
        let paths: Vec<&str> = o.items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["/src/app-hand", "/u/conductor/workspaces/app/oslo"]);
        assert_eq!(o.from, "Conductor");
        assert_eq!(o.detached, 1);
        assert!(offer("app", "/src/app", &listed, &known, "/h/.wisp/worktrees", "/u", &["app".into()]).is_none(), "Not now stays");
        assert!(offer("app", "/src/app", &listed[..3], &known, "/h/.wisp/worktrees", "/u", &[]).is_none(), "nothing left: no offer");
    }

    /// Windows: git lists every worktree as `C:/…` while the project's root, wisp's worktrees folder and a task's
    /// folder are kept as `C:\…` (and in any case): only the foreign one is offered.
    #[test]
    fn on_windows_git_s_forward_slashes_still_leave_out_the_checkout_wisp_s_own_and_known_ones() {
        let listed = vec![
            l("C:/Users/runneradmin/Temp/t/src/app", "main"),
            l("C:/Users/runneradmin/Temp/t/h/worktrees/app-fix", "wisp/fix"),
            l("C:/Users/runneradmin/Temp/t/src/app-known", "known"),
            l("C:/Users/runneradmin/Temp/t/u/conductor/workspaces/app/oslo", "oslo"),
        ];
        let known: HashSet<String> = [folder_key(r"C:\Users\RunnerAdmin\Temp\t\src\app-known")].into();
        let (root, wisp, home) =
            (r"C:\Users\runneradmin\Temp\t\src\app", r"C:\Users\runneradmin\Temp\t\h\worktrees", r"C:\Users\runneradmin\Temp\t\u");
        let o = offer("app", root, &listed, &known, wisp, home, &[]).unwrap();
        let paths: Vec<&str> = o.items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["C:/Users/runneradmin/Temp/t/u/conductor/workspaces/app/oslo"]);
        assert_eq!((o.from.as_str(), o.items[0].name.as_str()), ("Conductor", "oslo"));
    }

    #[test]
    fn bring_in_takes_only_the_offers_paths() {
        let o = offer("app", "/r", &[l("/a", "a"), l("/b", "b")], &HashSet::new(), "/w", "/u", &[]).unwrap();
        assert_eq!(o.from, "git");
        let (take, skipped) = takes(&o, &["/b/".into(), "/elsewhere".into()]);
        assert_eq!(take.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(), ["/b"]);
        assert_eq!(skipped, ["/elsewhere"]);
    }

    /// Windows: git lists the worktree as `C:/…/stray`; the client sends wisp's `C:\…\stray` (any case, a trailing
    /// `\`). It's the same folder: taken, not skipped. A sibling with a longer name is still another folder.
    #[test]
    fn bring_in_takes_a_windows_path_in_either_spelling() {
        let git = "C:/Users/runneradmin/AppData/Local/Temp/t/stray";
        let o =
            offer("app", "C:/Users/runneradmin/AppData/Local/Temp/t/proj", &[l(git, "stray-branch")], &HashSet::new(), "C:/w", "C:/u", &[])
                .unwrap();
        for sent in [r"C:\Users\runneradmin\AppData\Local\Temp\t\stray", r"c:\users\RUNNERADMIN\appdata\local\temp\t\stray\", git] {
            let (take, skipped) = takes(&o, &[sent.into()]);
            assert_eq!(take.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(), [git], "{sent}");
            assert!(skipped.is_empty(), "{sent}: {skipped:?}");
        }
        let (take, skipped) = takes(&o, &[r"C:\Users\runneradmin\AppData\Local\Temp\t\stray2".into()]);
        assert!(take.is_empty() && skipped.len() == 1);
    }
}
